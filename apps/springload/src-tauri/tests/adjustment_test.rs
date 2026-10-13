//! 复权维护命令的集成测试：命令注册、DTO 形状，以及过期检查与重算的完整流程。
//!
//! 过期检查与重算需要联网，命令层固定走真实网络。这里用两条路径覆盖：
//! 命令本身用空输入通过 IPC 调用（不发起请求），完整流程用录制样本回放的抓取实现
//! 直接调用 `springload-core` 的检查与重算，存储用临时 SQLite 文件。

use chrono::NaiveDate;
use serde_json::{json, Value};
use springload_core::adjustment::{self, AdjustmentFetch};
use springload_core::error::{CoreError, Result as CoreResult};
use springload_core::market::{eastmoney, Bar};
use springload_core::storage::bar::BarRow;
use springload_core::storage::bar_sync_state::BarSyncState;
use springload_core::storage::corporate_action;
use springload_core::storage::stock::Stock;
use springload_core::storage::{bar, bar_sync_state, stock, Database};
use springload_lib::commands;
use springload_lib::state::AppState;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{App, WebviewWindow, WebviewWindowBuilder};

const FIXTURES: &str = "../../../crates/springload-core/tests/fixtures";
const RAW: &str = "eastmoney_kline_001309_raw.json";
const QFQ: &str = "eastmoney_kline_001309_qfq.json";
const TAIL: &str = "eastmoney_kline_001309_tail_qfq.json";
const BONUS: &str = "eastmoney_sharebonus_001309.json";

fn fixture(name: &str) -> String {
    let path = format!("{FIXTURES}/{name}");
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("读取样本 {path}: {error}"))
}

fn bars(name: &str) -> Vec<Bar> {
    eastmoney::parse_kline(&fixture(name)).expect("解析样本").bars
}

fn rows(bars: &[Bar]) -> Vec<BarRow> {
    bars.iter()
        .map(|bar| BarRow {
            code: bar.code.clone(),
            date: bar.date.clone(),
            open: bar.open,
            high: bar.high,
            low: bar.low,
            close: bar.close,
            volume: bar.volume,
            amount: bar.amount,
        })
        .collect()
}

/// 用录制样本回放的抓取实现。
struct FakeFetch {
    tail: String,
    raw: String,
    dividends: String,
    dividends_fail: bool,
}

impl FakeFetch {
    fn new() -> Self {
        Self {
            tail: fixture(TAIL),
            raw: fixture(RAW),
            dividends: fixture(BONUS),
            dividends_fail: false,
        }
    }
}

impl AdjustmentFetch for FakeFetch {
    fn qfq_tail(&self, _code: &str, _lmt: u32) -> CoreResult<String> {
        Ok(self.tail.clone())
    }

    fn raw_history(&self, _code: &str, _beg: &str) -> CoreResult<String> {
        Ok(self.raw.clone())
    }

    fn dividends(&self, _code: &str) -> CoreResult<String> {
        if self.dividends_fail {
            return Err(CoreError::InvalidData("分红送配接口不可用".to_string()));
        }
        Ok(self.dividends.clone())
    }
}

struct Harness {
    webview: WebviewWindow<MockRuntime>,
    #[allow(dead_code)]
    app: App<MockRuntime>,
}

impl Harness {
    fn new(dir: &std::path::Path) -> Self {
        let state = AppState::from_data_dir(dir.to_path_buf()).expect("装配应用状态");
        let app = mock_builder()
            .manage(state)
            .invoke_handler(tauri::generate_handler![
                commands::adjustment::corporate_actions_stale,
                commands::adjustment::resync_adjusted,
            ])
            .build(mock_context(noop_assets()))
            .expect("构建测试应用");
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("建测试窗口");
        Self { webview, app }
    }

    fn invoke(&self, cmd: &str, args: Value) -> Result<Value, Value> {
        let body = if args.is_null() {
            InvokeBody::default()
        } else {
            InvokeBody::Json(args)
        };
        let response = get_ipc_response(
            &self.webview,
            InvokeRequest {
                cmd: cmd.to_string(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "tauri://localhost".parse().unwrap(),
                body,
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )?;
        Ok(response.deserialize::<Value>().expect("响应可反序列化"))
    }
}

fn seed_stock(db: &Database) {
    let conn = db.conn().unwrap();
    stock::upsert(
        &conn,
        &Stock {
            code: "001309".to_string(),
            name: "德明利".to_string(),
            market: "SZ".to_string(),
            board: "main".to_string(),
            sector: None,
            listed_at: Some("2020-01-01".to_string()),
            is_st: false,
            updated_at: "2026-10-13T00:00:00Z".to_string(),
        },
    )
    .unwrap();
}

fn seed_state(db: &Database, last_bar_date: &str, tail_qfq: &str) {
    let conn = db.conn().unwrap();
    bar_sync_state::upsert(
        &conn,
        &BarSyncState {
            code: "001309".to_string(),
            last_bar_date: Some(last_bar_date.to_string()),
            fetched_at: Some("2026-10-12T08:00:00Z".to_string()),
            dktotal: Some(1037),
            tail_qfq: Some(tail_qfq.to_string()),
            updated_at: "2026-10-12T08:00:00Z".to_string(),
        },
    )
    .unwrap();
}

fn tail_json(bars: &[Bar]) -> String {
    let start = bars.len().saturating_sub(adjustment::TAIL_LEN as usize);
    let tail: Vec<Value> = bars[start..]
        .iter()
        .map(|bar| json!([bar.date, bar.close]))
        .collect();
    Value::Array(tail).to_string()
}

#[test]
fn corporate_actions_stale_is_registered_and_empty_without_stocks() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());

    let stale = harness
        .invoke("corporate_actions_stale", Value::Null)
        .expect("命令成功");
    assert_eq!(stale, json!([]));
}

#[test]
fn resync_adjusted_without_codes_reports_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());

    let report = harness
        .invoke("resync_adjusted", json!({ "codes": [] }))
        .expect("命令成功");
    assert_eq!(report["stocks"], 0);
    assert_eq!(report["bars"], 0);
    assert_eq!(report["errors"], json!([]));
    assert!(report["elapsedMs"].is_number());
}

#[test]
fn stale_check_flags_history_before_the_ex_date_and_resync_rewrites_it() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::from_data_dir(dir.path().to_path_buf()).expect("装配应用状态");
    let db = state.db().clone();
    seed_stock(&db);
    {
        // 本地存的是除权前抓的前复权历史，等于当时的不复权序列。
        let mut conn = db.conn().unwrap();
        bar::upsert_many(&mut conn, &rows(&bars(RAW))).unwrap();
    }
    seed_state(&db, "2026-10-12", &tail_json(&bars(RAW)));

    let fetch = FakeFetch::new();
    let stale = adjustment::detect_stale(&db, &fetch, 2).expect("检查过期");
    assert_eq!(stale.len(), 1, "除权日之后应有股票判为过期");
    assert_eq!(stale[0].code, "001309");
    assert_eq!(stale[0].ex_date.as_deref(), Some("2026-10-13"));

    let report = adjustment::resync(
        &db,
        &fetch,
        &["001309".to_string()],
        NaiveDate::from_ymd_opt(2026, 10, 13).unwrap(),
    )
    .expect("重算");
    assert!(report.errors.is_empty(), "{:?}", report.errors);

    // 同一段历史被整体改写，与行情源的前复权序列一致。
    let conn = db.conn().unwrap();
    let stored = bar::list_for_code(&conn, "001309").unwrap();
    let expected = bars(QFQ);
    assert_eq!(stored.len(), expected.len());
    for (row, want) in stored.iter().zip(expected.iter()) {
        assert!((row.close - want.close).abs() < 0.005, "{}", row.date);
    }
    // 除权除息事件与参考价都写进了表。
    let actions = corporate_action::list_for_code(&conn, "001309").unwrap();
    let latest = actions
        .iter()
        .find(|action| action.ex_date == "2026-10-13")
        .expect("应记录 2026-10-13 事件");
    assert_eq!(latest.dividend_per_share, 1.5);
    assert_eq!(latest.split_ratio, 0.4);
    assert!((latest.pre_close.unwrap() - 228.521428571).abs() < 1e-6);

    // 重算后不再判过期。
    assert!(adjustment::detect_stale(&db, &fetch, 2).unwrap().is_empty());
}

#[test]
fn stale_check_keeps_fresh_history() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::from_data_dir(dir.path().to_path_buf()).expect("装配应用状态");
    let db = state.db().clone();
    seed_stock(&db);
    // 本地尾部与服务端一致，且最后一条已到除权日。
    seed_state(&db, "2026-10-13", &tail_json(&bars(TAIL)));

    let fetch = FakeFetch::new();
    assert!(adjustment::detect_stale(&db, &fetch, 2).unwrap().is_empty());
}

#[test]
fn stale_check_falls_back_to_tail_comparison() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::from_data_dir(dir.path().to_path_buf()).expect("装配应用状态");
    let db = state.db().clone();
    seed_stock(&db);
    seed_state(&db, "2026-10-12", &tail_json(&bars(RAW)));

    let fetch = FakeFetch {
        dividends_fail: true,
        ..FakeFetch::new()
    };
    let stale = adjustment::detect_stale(&db, &fetch, 2).unwrap();
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].ex_date, None);
}

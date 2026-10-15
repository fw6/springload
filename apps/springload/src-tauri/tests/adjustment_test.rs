//! 复权维护命令的集成测试：命令注册、DTO 形状，以及过期检查与重算的完整流程。
//!
//! 命令本身用空输入通过 IPC 调用（不发起请求），完整流程用样本数据源直接调用
//! `springload-core` 的检查与重算，存储用临时 SQLite 文件。
//!
//! 场景取 001309 德明利 2026-10-13 的除权除息，方案 `10转4股派15.00元`。

use chrono::NaiveDate;
use serde_json::{json, Value};
use springload_core::adjustment;
use springload_core::error::Result as CoreResult;
use springload_core::market::{Bar, MinuteBar, QuoteSource};
use springload_core::storage::bar::BarRow;
use springload_core::storage::bar_sync_state::BarSyncState;
use springload_core::storage::corporate_action::{self, CorporateAction};
use springload_core::storage::stock::Stock;
use springload_core::storage::{bar, bar_sync_state, stock, Database};
use springload_lib::commands;
use springload_lib::state::AppState;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{App, WebviewWindow, WebviewWindowBuilder};

/// 除权日。
const EX_DATE: &str = "2026-10-13";
/// 除权日前的最后一根不复权日线。
const PRE_EX_DATE: &str = "2026-10-12";
const PRE_EX_CLOSE: f64 = 321.43;
const EX_CLOSE: f64 = 233.78;
const DIVIDEND: f64 = 1.5;
const SPLIT: f64 = 0.4;

fn raw_bar(date: &str, close: f64) -> Bar {
    Bar {
        code: "001309".to_string(),
        date: date.to_string(),
        open: close,
        high: close,
        low: close,
        close,
        volume: 1_000_000.0,
        amount: close * 1_000_000.0,
    }
}

fn raw_history() -> Vec<Bar> {
    vec![raw_bar(PRE_EX_DATE, PRE_EX_CLOSE), raw_bar(EX_DATE, EX_CLOSE)]
}

fn ex_event() -> CorporateAction {
    CorporateAction {
        code: "001309".to_string(),
        ex_date: EX_DATE.to_string(),
        dividend_per_share: DIVIDEND,
        split_ratio: SPLIT,
        pre_close: None,
        recorded_at: "2026-10-13T00:00:00Z".to_string(),
    }
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

/// 用样本回放的行情数据源。
struct FakeSource {
    raw: Vec<Bar>,
    events: Vec<CorporateAction>,
}

impl QuoteSource for FakeSource {
    fn security_list(&self) -> CoreResult<Vec<springload_core::market::Security>> {
        Ok(Vec::new())
    }

    fn daily_raw(&self, _code: &str, _beg: &str) -> CoreResult<Vec<Bar>> {
        Ok(self.raw.clone())
    }

    fn corporate_actions(&self, _code: &str) -> CoreResult<Vec<CorporateAction>> {
        Ok(self.events.clone())
    }

    fn minute_bars(&self, _code: &str, _period: u32) -> CoreResult<Vec<MinuteBar>> {
        Ok(Vec::new())
    }
}

fn source() -> FakeSource {
    FakeSource {
        raw: raw_history(),
        events: vec![ex_event()],
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

fn seed_state(db: &Database, last_bar_date: &str) {
    let conn = db.conn().unwrap();
    bar_sync_state::upsert(
        &conn,
        &BarSyncState {
            code: "001309".to_string(),
            last_bar_date: Some(last_bar_date.to_string()),
            fetched_at: Some("2026-10-12T08:00:00Z".to_string()),
            updated_at: "2026-10-12T08:00:00Z".to_string(),
        },
    )
    .unwrap();
}

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 13).unwrap()
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
        bar::upsert_many(&mut conn, &rows(&raw_history())).unwrap();
    }
    seed_state(&db, PRE_EX_DATE);

    let source = source();
    let stale = adjustment::detect_stale(&db, &source, 2).expect("检查过期");
    assert_eq!(stale.len(), 1, "除权日之后应有股票判为过期");
    assert_eq!(stale[0].code, "001309");
    assert_eq!(stale[0].ex_date.as_deref(), Some(EX_DATE));

    let report = adjustment::resync(&db, &source, &["001309".to_string()], today()).expect("重算");
    assert!(report.errors.is_empty(), "{:?}", report.errors);

    // 同一段历史被整体改写：除权日前换算，除权日当天不变。
    let conn = db.conn().unwrap();
    let stored = bar::list_for_code(&conn, "001309").unwrap();
    assert_eq!(stored.len(), 2);
    assert_eq!(stored[0].close, 228.52);
    assert_eq!(stored[1].close, EX_CLOSE);

    // 除权除息事件与参考价都写进了表。
    let actions = corporate_action::list_for_code(&conn, "001309").unwrap();
    let latest = actions
        .iter()
        .find(|action| action.ex_date == EX_DATE)
        .expect("应记录除权日事件");
    assert_eq!(latest.dividend_per_share, DIVIDEND);
    assert_eq!(latest.split_ratio, SPLIT);
    assert!((latest.pre_close.unwrap() - 228.521428571).abs() < 1e-6);

    // 重算后不再判过期。
    assert!(adjustment::detect_stale(&db, &source, 2).unwrap().is_empty());
}

#[test]
fn stale_check_keeps_history_whose_events_are_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::from_data_dir(dir.path().to_path_buf()).expect("装配应用状态");
    let db = state.db().clone();
    seed_stock(&db);
    seed_state(&db, EX_DATE);
    {
        let conn = db.conn().unwrap();
        adjustment::record_events(&conn, "001309", &[ex_event()], &raw_history()).unwrap();
    }

    let source = source();
    assert!(adjustment::detect_stale(&db, &source, 2).unwrap().is_empty());
}

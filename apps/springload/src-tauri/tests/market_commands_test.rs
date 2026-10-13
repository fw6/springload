//! `commands::market` 的四条命令：股票列表、K 线读取与同步状态。
//!
//! 存储用临时 SQLite；数据由测试直接写入各表，命令只负责读取。

use serde_json::{json, Value};
use springload_core::storage::bar::BarRow;
use springload_core::storage::bar_sync_state::BarSyncState;
use springload_core::storage::stock::Stock;
use springload_core::storage::{bar, bar_sync_state, stock, Database};
use springload_lib::commands;
use springload_lib::state::AppState;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{App, WebviewWindow, WebviewWindowBuilder};

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
                commands::market::sync_market,
                commands::market::sync_status,
                commands::market::stock_list,
                commands::market::bars_get,
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

/// 往临时库里写一只股票、两根日线、一行回补状态和一根 5 分钟线。
fn seed(dir: &std::path::Path) {
    let db = Database::open(&dir.join("springload.db")).expect("打开数据库");
    {
        let conn = db.conn().unwrap();
        stock::upsert(
            &conn,
            &Stock {
                code: "600000".to_string(),
                name: "浦发银行".to_string(),
                market: "SH".to_string(),
                board: "main".to_string(),
                sector: None,
                listed_at: Some("1999-11-10".to_string()),
                is_st: false,
                updated_at: "2026-10-13T07:00:00Z".to_string(),
            },
        )
        .unwrap();
        bar_sync_state::upsert(
            &conn,
            &BarSyncState {
                code: "600000".to_string(),
                last_bar_date: Some("2026-10-13".to_string()),
                fetched_at: Some("2026-10-13T07:00:00Z".to_string()),
                dktotal: Some(6404),
                tail_qfq: Some("[]".to_string()),
                updated_at: "2026-10-13T07:00:00Z".to_string(),
            },
        )
        .unwrap();
        conn.execute(
            "INSERT INTO bar_minute (code, ts, period, open, high, low, close, volume, amount)
             VALUES ('600000', '2026-10-13 09:35', 5, 9.60, 9.70, 9.55, 9.65, 120000, 1158000)",
            [],
        )
        .unwrap();
    }
    let mut conn = db.conn().unwrap();
    bar::upsert_many(
        &mut conn,
        &[
            BarRow {
                code: "600000".to_string(),
                date: "2026-10-10".to_string(),
                open: 9.60,
                high: 9.72,
                low: 9.58,
                close: 9.67,
                volume: 34_435_200.0,
                amount: 331_521_401.0,
            },
            BarRow {
                code: "600000".to_string(),
                date: "2026-10-13".to_string(),
                open: 9.67,
                high: 9.70,
                low: 9.58,
                close: 9.65,
                volume: 34_435_200.0,
                amount: 331_521_401.0,
            },
        ],
    )
    .unwrap();
}

#[test]
fn stock_list_returns_seeded_rows() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path());
    let harness = Harness::new(dir.path());

    let rows = harness.invoke("stock_list", Value::Null).expect("stock_list 成功");
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["code"], "600000");
    assert_eq!(rows[0]["name"], "浦发银行");
    assert_eq!(rows[0]["market"], "SH");
    assert_eq!(rows[0]["board"], "main");
    assert_eq!(rows[0]["isSt"], false);
    assert_eq!(rows[0]["listedAt"], "1999-11-10");
}

#[test]
fn bars_get_returns_daily_bars_in_order() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path());
    let harness = Harness::new(dir.path());

    let bars = harness
        .invoke("bars_get", json!({ "code": "600000", "period": "day" }))
        .expect("bars_get 成功");
    let bars = bars.as_array().unwrap();
    assert_eq!(bars.len(), 2);
    assert_eq!(bars[0]["date"], "2026-10-10");
    assert_eq!(bars[1]["date"], "2026-10-13");
    assert_eq!(bars[1]["close"], 9.65);
    assert_eq!(bars[1]["volume"], 34_435_200.0);
}

#[test]
fn bars_get_reads_minute_periods() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path());
    let harness = Harness::new(dir.path());

    let bars = harness
        .invoke("bars_get", json!({ "code": "600000", "period": "5" }))
        .expect("bars_get 成功");
    let bars = bars.as_array().unwrap();
    assert_eq!(bars.len(), 1);
    assert_eq!(bars[0]["date"], "2026-10-13 09:35");
    assert_eq!(bars[0]["close"], 9.65);
}

#[test]
fn bars_get_rejects_unknown_period() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path());
    let harness = Harness::new(dir.path());

    let error = harness
        .invoke("bars_get", json!({ "code": "600000", "period": "week" }))
        .expect_err("未知周期应报错");
    assert!(error.as_str().unwrap().contains("week"), "{error}");
}

#[test]
fn sync_status_reports_last_sync_time() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path());
    let harness = Harness::new(dir.path());

    let status = harness.invoke("sync_status", Value::Null).expect("sync_status 成功");
    assert_eq!(status["running"], false);
    assert_eq!(status["lastSyncAt"], "2026-10-13T07:00:00Z");
    assert_eq!(status["done"], 0);
    assert_eq!(status["total"], 0);
}

#[test]
fn sync_status_without_history_has_null_last_sync() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());

    let status = harness.invoke("sync_status", Value::Null).expect("sync_status 成功");
    assert_eq!(status["running"], false);
    assert!(status["lastSyncAt"].is_null());
}

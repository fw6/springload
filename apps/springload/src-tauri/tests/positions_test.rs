//! `commands::positions` 的三条命令：持仓列表、三类提示与 T+1、填成交价离场。
//!
//! 存储用临时 SQLite；日线由测试写进 `bar` 表，实时快照由测试直接传给
//! `alerts::refresh_positions`（命令层的抓取走网络，不在集成测试里跑）。

use serde_json::{json, Value};
use springload_core::market::Market;
use springload_core::alerts::{self, AlertKind, STOP_LOSS_MESSAGE};
use springload_core::storage::bar::BarRow;
use springload_core::storage::position::{self, Position};
use springload_core::storage::stock::Stock;
use springload_core::storage::trade_log;
use springload_core::storage::trade_plan::{self, TradePlan};
use springload_core::storage::{bar, stock, Database};
use springload_lib::commands;
use springload_lib::state::AppState;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{App, WebviewWindow, WebviewWindowBuilder};

/// 买入日的次一交易日。
const NEXT_TRADING_DAY: &str = "2026-10-14";

/// 可执行日：不早于买入日的次一交易日，也不早于今天。
fn executable_day() -> String {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    if today.as_str() > NEXT_TRADING_DAY {
        today
    } else {
        NEXT_TRADING_DAY.to_string()
    }
}

struct Harness {
    webview: WebviewWindow<MockRuntime>,
    #[allow(dead_code)]
    app: App<MockRuntime>,
    dir: std::path::PathBuf,
}

impl Harness {
    fn new(dir: &std::path::Path) -> Self {
        let state = AppState::from_data_dir(dir.to_path_buf()).expect("装配应用状态");
        let app = mock_builder()
            .manage(state)
            .invoke_handler(tauri::generate_handler![
                commands::positions::positions_get,
                commands::positions::position_exit,
            ])
            .build(mock_context(noop_assets()))
            .expect("构建测试应用");
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("建测试窗口");
        Self {
            webview,
            app,
            dir: dir.to_path_buf(),
        }
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

    fn db(&self) -> Database {
        Database::open(&self.dir.join("springload.db")).expect("打开数据库")
    }
}

/// 交易日历：从 2026-10-09 起的连续交易日，跳过周末。
fn calendar() -> Vec<&'static str> {
    vec![
        "2026-10-09",
        "2026-10-12",
        "2026-10-13",
        "2026-10-14",
        "2026-10-15",
        "2026-10-16",
        "2026-10-19",
        "2026-10-20",
        "2026-10-21",
        "2026-10-22",
        "2026-10-23",
        "2026-10-26",
    ]
}

/// 一只股票、一条计划与一条持仓。`stop` 与 `per_r` 决定 3R / 5R 的位置。
fn seed(dir: &std::path::Path, entry: f64, stop: f64, entry_date: &str) {
    let db = Database::open(&dir.join("springload.db")).expect("打开数据库");
    let mut conn = db.conn().unwrap();
    let per_r = entry - stop;
    stock::upsert(
        &conn,
        &Stock {
            code: "600036".to_string(),
            name: "招商银行".to_string(),
            market: "SH".to_string(),
            board: "main".to_string(),
            sector: Some("银行".to_string()),
            listed_at: None,
            is_st: false,
            updated_at: springload_core::storage::now(),
        },
    )
    .unwrap();
    trade_plan::upsert(
        &conn,
        &TradePlan {
            id: "plan-1".to_string(),
            code: "600036".to_string(),
            entry,
            stop,
            account: 100_000.0,
            risk_pct: 0.005,
            per_r,
            shares: 1000,
            capital: entry * 1000.0,
            pos_pct: 0.2,
            stop_distance: per_r / entry,
            t3: entry + 3.0 * per_r,
            t5: entry + 5.0 * per_r,
            status: "executed".to_string(),
            zone_id: None,
            signal_id: None,
            created_at: springload_core::storage::now(),
            updated_at: springload_core::storage::now(),
        },
    )
    .unwrap();
    position::upsert(
        &conn,
        &Position {
            id: "position-1".to_string(),
            plan_id: "plan-1".to_string(),
            code: "600036".to_string(),
            entry_date: entry_date.to_string(),
            entry_price: entry,
            quantity: 1000,
            current_stop: stop,
            follow_ema: true,
            exit_price: None,
            exit_date: None,
            created_at: springload_core::storage::now(),
            updated_at: springload_core::storage::now(),
        },
    )
    .unwrap();
    // 交易日历：写一只别的股票的平价日线，把日期铺出来。
    let bars: Vec<BarRow> = calendar()
        .iter()
        .map(|date| BarRow {
            code: "600000".to_string(),
            date: date.to_string(),
            open: 10.0,
            high: 10.0,
            low: 10.0,
            close: 10.0,
            volume: 1000.0,
            amount: 10_000.0,
        })
        .collect();
    bar::upsert_many(&mut conn, &bars).unwrap();
}

/// 持仓列表里唯一的那个 view。
fn only(db: &Database, quote: f64) -> alerts::PositionView {
    alerts::refresh_positions(db, &[("600036".to_string(), quote)], Market::Cn)
        .expect("刷新成功")
        .into_iter()
        .next()
        .expect("有一条持仓")
}

#[test]
fn positions_get_lists_the_open_position_with_its_plan_numbers() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path(), 20.0, 19.5, "2026-10-13");
    let harness = Harness::new(dir.path());

    let rows = harness.invoke("positions_get", Value::Null).expect("读持仓成功");
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row["code"], "600036");
    assert_eq!(row["name"], "招商银行");
    assert_eq!(row["entryDate"], "2026-10-13");
    assert_eq!(row["entryPrice"], 20.0);
    assert_eq!(row["stop"], 19.5);
    assert_eq!(row["quantity"], 1000);
    assert_eq!(row["perR"], 0.5);
    assert_eq!(row["t3"], 21.5);
    assert_eq!(row["t5"], 22.5);
    assert!(row["alerts"].as_array().unwrap().is_empty(), "没有行情时没有提示");
}

#[test]
fn a_price_at_or_below_the_stop_gives_the_stop_loss_hint() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path(), 20.0, 19.5, "2026-10-13");
    let db = Database::open(&dir.path().join("springload.db")).unwrap();

    let view = only(&db, 19.5);
    let stop = view
        .alerts
        .iter()
        .find(|alert| alert.kind == AlertKind::StopLoss)
        .expect("应给出止损提示");
    assert_eq!(stop.message, STOP_LOSS_MESSAGE);
    assert_eq!(stop.executable_date, executable_day());
}

#[test]
fn a_price_below_ema9_gives_the_trend_hint() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path(), 20.0, 19.0, "2026-10-13");
    let db = Database::open(&dir.path().join("springload.db")).unwrap();
    {
        // 这只股票自己的 9 日均线：一路 20.50 之后掉到 20.10。
        let mut conn = db.conn().unwrap();
        let days = calendar();
        let bars: Vec<BarRow> = days
            .iter()
            .enumerate()
            .map(|(index, date)| BarRow {
                code: "600036".to_string(),
                date: date.to_string(),
                open: 20.5,
                high: 20.5,
                low: 20.5,
                close: if index + 1 == days.len() { 20.1 } else { 20.5 },
                volume: 1000.0,
                amount: 20_500.0,
            })
            .collect();
        bar::upsert_many(&mut conn, &bars).unwrap();
    }

    let view = only(&db, 20.1);
    let trend = view
        .alerts
        .iter()
        .find(|alert| alert.kind == AlertKind::BelowEma9)
        .expect("应给出跌破 9 日均线的提示");
    assert_eq!(trend.message, "趋势可能走坏，建议评估离场");
    assert_eq!(trend.executable_date, executable_day());
    // 20.10 既没到 3R 也没跌破止损。
    assert_eq!(view.alerts.len(), 1);
}

#[test]
fn reaching_3r_and_5r_gives_the_reduce_hints() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path(), 20.0, 19.5, "2026-10-13");
    let db = Database::open(&dir.path().join("springload.db")).unwrap();

    let at_3r = only(&db, 21.5);
    assert!(at_3r.alerts.iter().any(|a| a.kind == AlertKind::Reduce3R));
    assert!(!at_3r.alerts.iter().any(|a| a.kind == AlertKind::Reduce5R));

    let at_5r = only(&db, 22.5);
    assert!(at_5r.alerts.iter().any(|a| a.kind == AlertKind::Reduce5R));
    assert!(!at_5r.alerts.iter().any(|a| a.kind == AlertKind::Reduce3R));
}

#[test]
fn the_executable_date_is_never_the_entry_day() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path(), 20.0, 19.5, "2026-10-13");
    let db = Database::open(&dir.path().join("springload.db")).unwrap();

    let view = only(&db, 19.0);
    let stop = view
        .alerts
        .iter()
        .find(|alert| alert.kind == AlertKind::StopLoss)
        .unwrap();
    assert_ne!(stop.executable_date, "2026-10-13", "当天买入当天不能卖");
    assert_eq!(stop.executable_date, executable_day());
}

#[test]
fn entering_the_exit_price_writes_the_exit_row() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path(), 20.0, 19.5, "2026-10-13");
    let harness = Harness::new(dir.path());

    let entry = harness
        .invoke(
            "position_exit",
            json!({ "id": "position-1", "exitPrice": 21.5, "exitDate": "2026-10-14" }),
        )
        .expect("离场成功");
    assert_eq!(entry["exitPrice"], 21.5);
    assert_eq!(entry["exitDate"], "2026-10-14");
    assert_eq!(entry["quantity"], 1000);
    // (21.50 − 20.00) ÷ 0.50 = 3R；(21.50 − 20.00) × 1000 = 1500 元。
    assert_eq!(entry["rMultiple"], 3.0);
    assert_eq!(entry["pnl"], 1500.0);

    let db = harness.db();
    let conn = db.conn().unwrap();
    assert!(position::list_open(&conn).unwrap().is_empty(), "持仓已平");
    let rows = trade_log::list_closed(&conn).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].exit_price, Some(21.5));
    assert_eq!(rows[0].r_multiple, Some(3.0));
    assert_eq!(rows[0].pnl, Some(1500.0));
}

#[test]
fn a_non_positive_exit_price_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path(), 20.0, 19.5, "2026-10-13");
    let harness = Harness::new(dir.path());

    let error = harness
        .invoke(
            "position_exit",
            json!({ "id": "position-1", "exitPrice": 0.0, "exitDate": "2026-10-14" }),
        )
        .expect_err("0 不是合法成交价");
    assert!(error.as_str().unwrap().contains("必须是正数"));
}

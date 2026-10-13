//! `commands::signal` 的三条命令：两级触发的三条路径。
//!
//! 存储用临时 SQLite；日线与分钟线由测试直接写进 `bar` / `bar_minute` 表，
//! 命令按已冻结的签名读取。`signal_status` 只读已入库的数据，不发网络请求。

use serde_json::{json, Value};
use springload_core::market::MinuteBar;
use springload_core::storage::bar::BarRow;
use springload_core::storage::support_zone::{self, SupportZone};
use springload_core::storage::{bar, bar_minute, signal, Database};
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
    dir: std::path::PathBuf,
}

impl Harness {
    fn new(dir: &std::path::Path) -> Self {
        let state = AppState::from_data_dir(dir.to_path_buf()).expect("装配应用状态");
        let app = mock_builder()
            .manage(state)
            .invoke_handler(tauri::generate_handler![
                commands::signal::signal_status,
                commands::signal::poll_tick,
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

/// 第 `index` 根日线的日期。
fn date_of(index: usize) -> String {
    format!("2026-{:02}-{:02}", 1 + index / 28, 1 + index % 28)
}

/// 80 根平价日线，收盘价 10.00：EMA21 与 AVWAP 都落在 10.00。
fn seed_daily(dir: &std::path::Path, close: f64) {
    let bars: Vec<BarRow> = (0..80)
        .map(|i| BarRow {
            code: "600000".to_string(),
            date: date_of(i),
            open: close,
            high: close,
            low: close,
            close,
            volume: 1000.0,
            amount: close * 1000.0,
        })
        .collect();
    let db = Database::open(&dir.join("springload.db")).expect("打开数据库");
    let mut conn = db.conn().unwrap();
    bar::upsert_many(&mut conn, &bars).expect("写日线");
}

/// 一条高置信支撑区，下沿 9.90、上沿 10.10。
fn seed_high_confidence_zone(dir: &std::path::Path) {
    let db = Database::open(&dir.join("springload.db")).expect("打开数据库");
    let conn = db.conn().unwrap();
    support_zone::upsert(
        &conn,
        &SupportZone {
            id: "zone-1".to_string(),
            code: "600000".to_string(),
            kind: "support".to_string(),
            low: 9.90,
            high: 10.10,
            anchor_date: None,
            ema9: Some(10.0),
            ema21: Some(10.0),
            ema50: Some(10.0),
            avwap_start_date: None,
            avwap_price: Some(10.0),
            prev_high: Some(10.0),
            prev_low: Some(10.0),
            gap_top: None,
            gap_bottom: None,
            confidence: "高".to_string(),
            spread: Some(0.0),
            created_at: springload_core::storage::now(),
        },
    )
    .expect("写支撑区");
}

/// 写一段 1 分钟线。`(ts 后缀, open, high, low, close)`。
fn seed_minutes(dir: &std::path::Path, bars: &[(&str, f64, f64, f64, f64)]) {
    let rows: Vec<MinuteBar> = bars
        .iter()
        .map(|(hhmm, open, high, low, close)| MinuteBar {
            code: "600000".to_string(),
            ts: format!("2026-10-13 {hhmm}"),
            period: 1,
            open: *open,
            high: *high,
            low: *low,
            close: *close,
            volume: 1000.0,
            amount: close * 1000.0,
        })
        .collect();
    let db = Database::open(&dir.join("springload.db")).expect("打开数据库");
    let mut conn = db.conn().unwrap();
    bar_minute::upsert_many(&mut conn, &rows).expect("写分钟线");
}

/// 下探回升后突破前一根高点的一段分钟线：09:31 下探到 9.95，09:33 收盘回到 10.02，
/// 09:34 最高 10.06 超过前一根的 10.03。
fn triggered_minutes(dir: &std::path::Path) {
    seed_minutes(
        dir,
        &[
            ("09:31", 10.00, 10.03, 9.95, 9.97),
            ("09:32", 9.97, 10.00, 9.96, 9.99),
            ("09:33", 9.99, 10.04, 9.99, 10.02),
            ("09:34", 10.02, 10.06, 10.01, 10.05),
        ],
    );
}

fn status(harness: &Harness) -> Value {
    harness
        .invoke("signal_status", json!({ "code": "600000" }))
        .expect("signal_status 成功")
}

#[test]
fn without_a_high_confidence_zone_the_first_condition_is_not_met() {
    let dir = tempfile::tempdir().unwrap();
    seed_daily(dir.path(), 10.0);
    let harness = Harness::new(dir.path());

    let value = status(&harness);
    assert_eq!(value["stage"], "waiting");
    assert!(value["zoneId"].is_null());
    assert!(
        value["detail"].as_str().unwrap().contains("前置条件未满足"),
        "detail 应说明前置条件未满足，实际是 {}",
        value["detail"]
    );
}

#[test]
fn a_low_confidence_zone_does_not_unlock_the_trigger() {
    let dir = tempfile::tempdir().unwrap();
    seed_daily(dir.path(), 10.0);
    let db = Database::open(&dir.path().join("springload.db")).unwrap();
    let conn = db.conn().unwrap();
    support_zone::upsert(
        &conn,
        &SupportZone {
            id: "zone-low".to_string(),
            code: "600000".to_string(),
            kind: "support".to_string(),
            low: 9.90,
            high: 10.10,
            anchor_date: None,
            ema9: None,
            ema21: None,
            ema50: None,
            avwap_start_date: None,
            avwap_price: None,
            prev_high: None,
            prev_low: None,
            gap_top: None,
            gap_bottom: None,
            confidence: "低".to_string(),
            spread: Some(0.4),
            created_at: springload_core::storage::now(),
        },
    )
    .unwrap();
    drop(conn);
    triggered_minutes(dir.path());
    let harness = Harness::new(dir.path());

    assert_eq!(status(&harness)["stage"], "waiting");
}

#[test]
fn price_below_the_zone_keeps_waiting_without_any_signal() {
    let dir = tempfile::tempdir().unwrap();
    // 日线收盘 9.50，低于支撑区下沿 9.90：直接跌穿且没有回升。
    seed_daily(dir.path(), 9.50);
    seed_high_confidence_zone(dir.path());
    seed_minutes(dir.path(), &[("09:31", 9.50, 9.52, 9.45, 9.48)]);
    let harness = Harness::new(dir.path());

    let value = status(&harness);
    assert_eq!(value["stage"], "failed");
    assert!(
        value["detail"].as_str().unwrap().contains("跌穿"),
        "detail 应说明跌穿，实际是 {}",
        value["detail"]
    );

    // 跌穿不落 signal 行。
    let db = harness.db();
    let conn = db.conn().unwrap();
    assert!(signal::list_for_code(&conn, "600000").unwrap().is_empty());
}

#[test]
fn dip_recovery_then_breaking_the_previous_high_triggers() {
    let dir = tempfile::tempdir().unwrap();
    seed_daily(dir.path(), 10.05);
    seed_high_confidence_zone(dir.path());
    triggered_minutes(dir.path());
    let harness = Harness::new(dir.path());

    let value = status(&harness);
    assert_eq!(value["stage"], "triggered");
    assert_eq!(value["zoneId"], "zone-1");
    let detail = value["detail"].as_str().unwrap();
    assert!(detail.contains("已触发"), "detail 应是已触发，实际是 {detail}");
    assert!(detail.contains("超过前一根最高"), "detail 应写明突破，实际是 {detail}");
    // 触发点带上时间与前后高点，供分钟图标出位置。
    assert_eq!(value["trigger"]["ts"], "2026-10-13 09:34");
    assert_eq!(value["trigger"]["brokenHigh"], 10.06);
    assert_eq!(value["trigger"]["preHigh"], 10.04);
}

#[test]
fn the_trigger_point_stays_visible_after_it_is_recorded() {
    let dir = tempfile::tempdir().unwrap();
    seed_daily(dir.path(), 10.05);
    seed_high_confidence_zone(dir.path());
    triggered_minutes(dir.path());
    let harness = Harness::new(dir.path());

    // 第一次判定把信号落库，之后再读状态时触发点仍然在。
    let first = status(&harness);
    assert_eq!(first["trigger"]["ts"], "2026-10-13 09:34");
    let again = status(&harness);
    assert_eq!(again["trigger"]["ts"], "2026-10-13 09:34");
    assert_eq!(again["trigger"]["brokenHigh"], 10.06);
}

#[test]
fn no_trigger_point_before_the_breakout() {
    let dir = tempfile::tempdir().unwrap();
    seed_daily(dir.path(), 10.05);
    seed_high_confidence_zone(dir.path());
    let harness = Harness::new(dir.path());

    assert_eq!(status(&harness)["trigger"], Value::Null);
}

#[test]
fn only_a_recovery_without_a_breakout_stays_waiting() {
    let dir = tempfile::tempdir().unwrap();
    seed_daily(dir.path(), 10.05);
    seed_high_confidence_zone(dir.path());
    // 下探后回升，但每一根的最高价都没有超过前一根。
    seed_minutes(
        dir.path(),
        &[
            ("09:31", 10.00, 10.03, 9.95, 9.97),
            ("09:32", 9.97, 10.02, 9.96, 10.01),
            ("09:33", 10.01, 10.00, 9.99, 10.00),
            ("09:34", 10.00, 10.01, 9.99, 10.00),
        ],
    );
    let harness = Harness::new(dir.path());

    let value = status(&harness);
    assert_eq!(value["stage"], "waiting");
    assert!(value["detail"].as_str().unwrap().contains("等待分钟图"));
}

#[test]
fn poll_tick_only_refreshes_waiting_codes_and_positions() {
    let dir = tempfile::tempdir().unwrap();
    seed_daily(dir.path(), 10.05);
    seed_high_confidence_zone(dir.path());
    let harness = Harness::new(dir.path());

    // 本轮没有网络，`poll_tick` 会因抓取失败返回错误；这里只断言轮询对象的选取。
    let db = harness.db();
    let conn = db.conn().unwrap();
    let (waiting, positions) = springload_core::minute::poll_targets(&conn).unwrap();
    assert_eq!(waiting, vec!["600000".to_string()]);
    assert!(positions.is_empty());
}

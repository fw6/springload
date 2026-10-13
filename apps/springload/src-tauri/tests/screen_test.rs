//! `commands::screen` 的六条命令：筛选、上次结果、观察名单与四步解锁。
//!
//! 存储用临时 SQLite；日线由测试造出来直接写进 `bar` 表，再调用命令断言结果。

use chrono::{Duration, NaiveDate};
use serde_json::{json, Value};
use springload_core::storage::bar::BarRow;
use springload_core::storage::signal::Signal;
use springload_core::storage::stock::Stock;
use springload_core::storage::support_zone::SupportZone;
use springload_core::storage::{bar, signal, stock, support_zone, Database};
use springload_lib::commands;
use springload_lib::state::AppState;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{App, WebviewWindow, WebviewWindowBuilder};

/// 每只股票造多少根日线。
const BARS: usize = 200;
/// 四条判定全中、排名落进区间的那只。
const PASS: &str = "600005";
/// 四条判定全中但成交额不足的那只。
const ILLIQUID: &str = "600006";
/// 回调低点下移、C2 不成立的那只。
const C2_MISS: &str = "600010";
/// 高点低点均下降、C4 命中的那只。
const C4_HIT: &str = "600022";

struct Harness {
    webview: WebviewWindow<MockRuntime>,
    #[allow(dead_code)]
    app: App<MockRuntime>,
    db: Database,
}

impl Harness {
    fn new(dir: &std::path::Path) -> Self {
        let state = AppState::from_data_dir(dir.to_path_buf()).expect("装配应用状态");
        let db = state.db().clone();
        let app = mock_builder()
            .manage(state)
            .invoke_handler(tauri::generate_handler![
                commands::screen::screen_stocks,
                commands::screen::screen_last,
                commands::screen::watchlist_get,
                commands::screen::watchlist_add,
                commands::screen::watchlist_remove,
                commands::screen::step_unlock,
            ])
            .build(mock_context(noop_assets()))
            .expect("构建测试应用");
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("建测试窗口");
        Self { webview, app, db }
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

    /// 跑一遍筛选并返回报告。
    fn screen(&self) -> Value {
        self.invoke("screen_stocks", Value::Null).expect("筛选成功")
    }
}

/// 平稳上行、两段回踩且第二段低点更高的路径；末端接到 `end`。
fn rising_path(end: f64) -> Vec<f64> {
    interpolate(&[
        (0, 5.0),
        (60, 10.5),
        (89, 9.0),
        (119, 12.5),
        (139, 11.0),
        (178, 11.8),
        (BARS - 1, end),
    ])
}

/// 回调低点下移的路径：第二次回踩低于第一次。
fn lower_pullback_path(end: f64) -> Vec<f64> {
    interpolate(&[
        (0, 5.0),
        (60, 10.5),
        (89, 8.0),
        (119, 12.5),
        (139, 7.5),
        (178, 11.8),
        (BARS - 1, end),
    ])
}

/// 高点与低点逐级下移的路径。
fn declining_path() -> Vec<f64> {
    interpolate(&[
        (0, 31.0),
        (20, 29.0),
        (40, 34.0),
        (60, 28.0),
        (80, 31.0),
        (100, 25.0),
        (120, 27.0),
        (140, 22.0),
        (160, 24.0),
        (BARS - 1, 20.0),
    ])
}

fn interpolate(anchors: &[(usize, f64)]) -> Vec<f64> {
    (0..BARS)
        .map(|index| {
            for window in anchors.windows(2) {
                let (start, start_value) = window[0];
                let (end, end_value) = window[1];
                if index >= start && index <= end {
                    let ratio = (index - start) as f64 / (end - start) as f64;
                    return start_value + (end_value - start_value) * ratio;
                }
            }
            anchors.last().expect("至少一个锚点").1
        })
        .collect()
}

/// 一只股票的造数说明。
struct Seed {
    code: String,
    name: String,
    closes: Vec<f64>,
    /// 每根日线的成交额（元）。
    amount: f64,
}

/// 造 23 只股票：排名第 5 的那只四条全中，其余各差在不同的地方。
fn fixtures() -> Vec<Seed> {
    let mut seeds = Vec::new();
    let mut push = |code: String, name: String, closes: Vec<f64>, amount: f64| {
        seeds.push(Seed {
            code,
            name,
            closes,
            amount,
        });
    };
    // 排名 1~4：涨幅最高，分位落在 20% 以内，被 C1 排除。
    for (code, name, end) in [
        ("600001", "控制一", 14.4),
        ("600002", "控制二", 14.3),
        ("600003", "控制三", 14.2),
        ("600004", "控制四", 14.1),
    ] {
        push(code.to_string(), name.to_string(), rising_path(end), 2e8);
    }
    // 排名 5：四条全中。
    push(PASS.to_string(), "入选股".to_string(), rising_path(14.0), 2e8);
    // 排名 6：四条全中但成交额不足。
    push(
        ILLIQUID.to_string(),
        "低成交".to_string(),
        rising_path(13.9),
        5e7,
    );
    // 排名 7~21：普通股，末端涨幅递减，分位超过 30%。
    for index in 0..15 {
        push(
            format!("6001{:02}", index + 1),
            format!("普通{}", index + 1),
            rising_path(13.8 - index as f64 * 0.12),
            2e8,
        );
    }
    // 回调低点下移，C2 不成立。
    push(
        C2_MISS.to_string(),
        "低点下移".to_string(),
        lower_pullback_path(13.0),
        2e8,
    );
    // 高点低点均下降，C4 命中。
    push(C4_HIT.to_string(), "逐级下移".to_string(), declining_path(), 2e8);
    seeds
}

/// 把造好的股票与日线写进临时库。
fn seed(db: &Database) {
    let start = NaiveDate::from_ymd_opt(2025, 1, 1).expect("起始日");
    for item in fixtures() {
        let rows: Vec<BarRow> = item
            .closes
            .iter()
            .enumerate()
            .map(|(index, close)| BarRow {
                code: item.code.clone(),
                date: (start + Duration::days(index as i64))
                    .format("%Y-%m-%d")
                    .to_string(),
                open: *close,
                high: close * 1.005,
                low: close * 0.995,
                close: *close,
                volume: 1_000_000.0,
                amount: item.amount,
            })
            .collect();
        let mut conn = db.conn().expect("连接数据库");
        stock::upsert(
            &conn,
            &Stock {
                code: item.code.clone(),
                name: item.name.clone(),
                market: "SH".to_string(),
                board: "main".to_string(),
                sector: Some("测试".to_string()),
                listed_at: Some("2000-01-01".to_string()),
                is_st: false,
                updated_at: "2026-10-13T07:00:00Z".to_string(),
            },
        )
        .expect("写股票");
        bar::upsert_many(&mut conn, &rows).expect("写日线");
    }
}

fn row<'a>(rows: &'a Value, code: &str) -> &'a Value {
    rows.as_array()
        .expect("数组")
        .iter()
        .find(|row| row["code"] == code)
        .unwrap_or_else(|| panic!("结果里应有 {code}"))
}

#[test]
fn screen_picks_only_the_stock_that_meets_all_four_conditions() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());
    seed(&harness.db);

    let report = harness.screen();

    assert_eq!(report["universe"], 23, "筛选跑在 23 只股票上");
    let picked = report["picked"].as_array().unwrap();
    assert_eq!(picked.len(), 1, "只有四条全中且成交额达标的那只入选");
    let pass = &picked[0];
    assert_eq!(pass["code"], PASS);
    assert_eq!(pass["name"], "入选股");
    assert_eq!(pass["c1"], true);
    assert_eq!(pass["c2"], true);
    assert_eq!(pass["c3"], true);
    assert_eq!(pass["c4"], false);
    assert_eq!(pass["liquid"], true);
    // 23 只里排第 5，分位 5/23 ≈ 0.2174，落在前 20%~30% 这一段。
    let rank = pass["rankPct"].as_f64().unwrap();
    assert!((rank - 5.0 / 23.0).abs() < 1e-12, "实际分位 {rank}");
    // 近 6 个月涨幅过 30%。
    assert!(pass["chg6m"].as_f64().unwrap() > 0.30);
    assert!(pass["chg1m"].as_f64().is_some());
}

#[test]
fn rejected_rows_record_which_condition_they_missed() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());
    seed(&harness.db);

    let report = harness.screen();
    let rejected = &report["rejected"];

    // 涨幅最高的那只被 C1 的排名区间排除在外。
    let top = row(rejected, "600001");
    assert_eq!(top["c1"], false);
    assert!(top["chg6m"].as_f64().unwrap() > 0.30, "涨幅本身够，差在排名");
    assert!((top["rankPct"].as_f64().unwrap() - 1.0 / 23.0).abs() < 1e-12);

    // 成交额不足的那只四条判定都成立，只差在剔除条件上。
    let thin = row(rejected, ILLIQUID);
    assert_eq!(thin["c1"], true);
    assert_eq!(thin["c2"], true);
    assert_eq!(thin["c3"], true);
    assert_eq!(thin["c4"], false);
    assert_eq!(thin["liquid"], false);
    assert!((thin["rankPct"].as_f64().unwrap() - 6.0 / 23.0).abs() < 1e-12);

    // 回调低点下移的那只差在 C2。
    let c2 = row(rejected, C2_MISS);
    assert_eq!(c2["c2"], false);
    assert_eq!(c2["c3"], true);

    // 高点低点均下降的那只命中 C4 排除。
    let c4 = row(rejected, C4_HIT);
    assert_eq!(c4["c4"], true);
    assert_eq!(c4["c3"], false);
}

#[test]
fn screening_fills_the_watchlist_and_screen_last_returns_the_report() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());
    seed(&harness.db);

    assert!(
        harness.invoke("screen_last", Value::Null).unwrap().is_null(),
        "还没跑过筛选时没有上次结果"
    );
    let report = harness.screen();

    let last = harness.invoke("screen_last", Value::Null).expect("screen_last 成功");
    assert_eq!(last["ranAt"], report["ranAt"]);
    assert_eq!(last["universe"], 23);
    assert_eq!(last["picked"].as_array().unwrap().len(), 1);
    assert_eq!(last["rejected"].as_array().unwrap().len(), 22);
    assert!(last["elapsedMs"].as_u64().is_some());

    // 入选的四条判定与观察名单里记的一致。
    let watchlist = harness.invoke("watchlist_get", Value::Null).expect("watchlist_get 成功");
    let rows = watchlist.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["code"], PASS);
    assert_eq!(rows[0]["name"], "入选股");
    assert_eq!(rows[0]["c1"], true);
    assert_eq!(rows[0]["c2"], true);
    assert_eq!(rows[0]["c3"], true);
    assert_eq!(rows[0]["c4"], false);
    assert_eq!(rows[0]["rankPct"], report["picked"][0]["rankPct"]);
    assert_eq!(rows[0]["chg6m"], report["picked"][0]["chg6m"]);
    assert_eq!(rows[0]["addedAt"], report["ranAt"]);
}

#[test]
fn watchlist_add_and_remove_track_the_screen_judgment() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());
    seed(&harness.db);
    harness.screen();

    // 手工补进一只未入选的股票，判定取值取最近一次筛选结果。
    harness
        .invoke("watchlist_add", json!({ "code": C2_MISS }))
        .expect("加进观察名单");
    let rows = harness
        .invoke("watchlist_get", Value::Null)
        .expect("watchlist_get 成功");
    assert_eq!(rows.as_array().unwrap().len(), 2);
    let added = row(&rows, C2_MISS);
    assert_eq!(added["c2"], false);
    assert_eq!(added["c3"], true);
    assert_eq!(added["name"], "低点下移");

    // 重复添加不产生第二行。
    harness
        .invoke("watchlist_add", json!({ "code": C2_MISS }))
        .expect("重复添加");
    assert_eq!(
        harness
            .invoke("watchlist_get", Value::Null)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );

    harness
        .invoke("watchlist_remove", json!({ "code": C2_MISS }))
        .expect("移除");
    let rows = harness
        .invoke("watchlist_get", Value::Null)
        .expect("watchlist_get 成功");
    assert_eq!(rows.as_array().unwrap().len(), 1);
    assert_eq!(rows[0]["code"], PASS);
}

#[test]
fn watchlist_add_rejects_a_code_outside_the_stock_pool() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());
    seed(&harness.db);

    let error = harness
        .invoke("watchlist_add", json!({ "code": "999999" }))
        .expect_err("股票池外的代码应报错");
    assert!(error.as_str().unwrap().contains("999999"), "{error}");
}

#[test]
fn step_unlock_follows_the_watchlist_zone_and_signal() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());
    seed(&harness.db);

    // 还没有筛选、观察名单为空：第 2 步不可进。
    let before = harness
        .invoke("step_unlock", json!({ "code": PASS }))
        .expect("step_unlock 成功");
    assert_eq!(before["unlocked"], json!([true, false, false, false]));
    assert!(before["reasons"][1].as_str().unwrap().contains("第 1 步"));

    // 筛选之后观察名单非空，第 2 步可进；还没有支撑区，第 3 步仍不可进。
    harness.screen();
    let after = harness
        .invoke("step_unlock", json!({ "code": PASS }))
        .expect("step_unlock 成功");
    assert_eq!(after["unlocked"], json!([true, true, false, false]));
    assert!(after["reasons"][2].as_str().unwrap().contains("高置信"));

    // 中置信支撑区不解锁第 3 步。
    {
        let conn = harness.db.conn().unwrap();
        support_zone::upsert(
            &conn,
            &SupportZone {
                id: "zone-mid".to_string(),
                code: PASS.to_string(),
                kind: "support".to_string(),
                low: 13.0,
                high: 13.5,
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
                confidence: "中".to_string(),
                spread: Some(0.03),
                created_at: "2026-10-13T07:00:00Z".to_string(),
            },
        )
        .unwrap();
    }
    let medium = harness
        .invoke("step_unlock", json!({ "code": PASS }))
        .expect("step_unlock 成功");
    assert_eq!(medium["unlocked"], json!([true, true, false, false]));

    // 高置信支撑区解锁第 3 步；还没有触发信号，第 4 步不可进。
    {
        let conn = harness.db.conn().unwrap();
        support_zone::upsert(
            &conn,
            &SupportZone {
                id: "zone-high".to_string(),
                code: PASS.to_string(),
                kind: "support".to_string(),
                low: 13.0,
                high: 13.5,
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
                confidence: "高".to_string(),
                spread: Some(0.01),
                created_at: "2026-10-13T07:00:00Z".to_string(),
            },
        )
        .unwrap();
        signal::upsert(
            &conn,
            &Signal {
                id: "sig-waiting".to_string(),
                code: PASS.to_string(),
                triggered_at: "2026-10-13 10:00".to_string(),
                timeframe: "1m".to_string(),
                candle_high_broken: 13.6,
                pre_candle_high: 13.5,
                zone_id: None,
                status: "waiting".to_string(),
                created_at: "2026-10-13T07:00:00Z".to_string(),
            },
        )
        .unwrap();
    }
    let high = harness
        .invoke("step_unlock", json!({ "code": PASS }))
        .expect("step_unlock 成功");
    assert_eq!(high["unlocked"], json!([true, true, true, false]));

    // 触发信号解锁第 4 步。
    {
        let conn = harness.db.conn().unwrap();
        signal::upsert(
            &conn,
            &Signal {
                id: "sig-triggered".to_string(),
                code: PASS.to_string(),
                triggered_at: "2026-10-13 14:32".to_string(),
                timeframe: "1m".to_string(),
                candle_high_broken: 13.8,
                pre_candle_high: 13.6,
                zone_id: None,
                status: "triggered".to_string(),
                created_at: "2026-10-13T07:00:00Z".to_string(),
            },
        )
        .unwrap();
    }
    let triggered = harness
        .invoke("step_unlock", json!({ "code": PASS }))
        .expect("step_unlock 成功");
    assert_eq!(triggered["unlocked"], json!([true, true, true, true]));
    assert_eq!(triggered["reasons"], json!(["", "", "", ""]));

    // 别的股票没有高置信区，第 3、4 步仍不可进。
    let other = harness
        .invoke("step_unlock", json!({ "code": "600001" }))
        .expect("step_unlock 成功");
    assert_eq!(other["unlocked"], json!([true, true, false, false]));
}

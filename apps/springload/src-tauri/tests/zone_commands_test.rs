//! `commands::zone` 的四条命令：标注读写与三个参考价。
//!
//! 存储用临时 SQLite；日线由测试直接写入 `bar` 表，命令按已冻结的签名读取。
//! 断言写在 `support_zone` 表里的行与命令返回的置信度。

use serde_json::{json, Value};
use springload_core::storage::bar::BarRow;
use springload_core::storage::support_zone;
use springload_core::storage::{bar, Database};
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
                commands::zone::support_zones_get,
                commands::zone::support_zone_save,
                commands::zone::support_zone_delete,
                commands::zone::support_refs,
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

    fn rows(&self) -> Vec<springload_core::storage::support_zone::SupportZone> {
        let db = Database::open(&self.dir.join("springload.db")).expect("打开数据库");
        let conn = db.conn().unwrap();
        support_zone::list_for_code(&conn, "600000").unwrap()
    }
}

/// 第 `index` 根日线的日期：按 28 天一段拼出递增且唯一的日期串。
fn date_of(index: usize) -> String {
    format!("2026-{:02}-{:02}", 1 + index / 28, 1 + index % 28)
}

fn write_bars(dir: &std::path::Path, bars: &[BarRow]) {
    let db = Database::open(&dir.join("springload.db")).expect("打开数据库");
    let mut conn = db.conn().unwrap();
    bar::upsert_many(&mut conn, bars).expect("写日线");
}

/// 80 根平价日线：三条均线、AVWAP 与前期高低点都落在 10.00。
fn seed_flat(dir: &std::path::Path) {
    let bars: Vec<BarRow> = (0..80)
        .map(|i| BarRow {
            code: "600000".to_string(),
            date: date_of(i),
            open: 10.0,
            high: 10.0,
            low: 10.0,
            close: 10.0,
            volume: 1000.0,
            amount: 10_000.0,
        })
        .collect();
    write_bars(dir, &bars);
}

/// 参考价分散在 10.00 与 10.20：收盘价全为 10.00（EMA21 = AVWAP = 10.00），
/// 前 60 根的最高价为 10.20（前期高点 = 10.20），离散度约 1.99%，仍是高置信。
fn seed_distinct_refs(dir: &std::path::Path) {
    let bars: Vec<BarRow> = (0..80)
        .map(|i| {
            let prev = i < 60;
            BarRow {
                code: "600000".to_string(),
                date: date_of(i),
                open: 10.0,
                high: if prev { 10.2 } else { 10.0 },
                low: 10.0,
                close: 10.0,
                volume: 1000.0,
                amount: 10_000.0,
            }
        })
        .collect();
    write_bars(dir, &bars);
}

fn save(harness: &Harness, zone: Value) -> Value {
    harness
        .invoke("support_zone_save", json!({ "zone": zone }))
        .expect("support_zone_save 成功")
}

#[test]
fn support_refs_returns_three_reference_prices() {
    let dir = tempfile::tempdir().unwrap();
    seed_flat(dir.path());
    let harness = Harness::new(dir.path());

    let refs = harness
        .invoke("support_refs", json!({ "code": "600000", "avwapStartDate": null }))
        .expect("support_refs 成功");
    assert!((refs["ema21"].as_f64().unwrap() - 10.0).abs() < 1e-9);
    assert!((refs["avwap"].as_f64().unwrap() - 10.0).abs() < 1e-9);
    assert!((refs["prevHigh"].as_f64().unwrap() - 10.0).abs() < 1e-9);
}

#[test]
fn support_refs_without_bars_returns_nulls() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());

    let refs = harness
        .invoke("support_refs", json!({ "code": "600000", "avwapStartDate": null }))
        .expect("support_refs 成功");
    assert!(refs["ema21"].is_null());
    assert!(refs["avwap"].is_null());
    assert!(refs["prevHigh"].is_null());
}

#[test]
fn save_flat_zone_is_high_confidence_and_persists_refs() {
    let dir = tempfile::tempdir().unwrap();
    seed_flat(dir.path());
    let harness = Harness::new(dir.path());

    let view = save(
        &harness,
        json!({
            "id": "box-1",
            "code": "600000",
            "kind": "support",
            "low": 9.5,
            "high": 10.5,
            "avwapStartDate": null
        }),
    );
    assert_eq!(view["id"], "box-1");
    assert_eq!(view["kind"], "support");
    assert_eq!(view["low"], 9.5);
    assert_eq!(view["high"], 10.5);
    assert_eq!(view["confidence"], "高");
    assert!((view["spread"].as_f64().unwrap() - 0.0).abs() < 1e-9);

    let rows = harness.rows();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.id, "box-1");
    assert_eq!(row.code, "600000");
    assert_eq!(row.kind, "support");
    assert_eq!(row.confidence, "高");
    assert!((row.ema21.unwrap() - 10.0).abs() < 1e-9);
    assert!((row.avwap_price.unwrap() - 10.0).abs() < 1e-9);
    assert!((row.prev_high.unwrap() - 10.0).abs() < 1e-9);
}

#[test]
fn zones_get_returns_saved_rows() {
    let dir = tempfile::tempdir().unwrap();
    seed_flat(dir.path());
    let harness = Harness::new(dir.path());

    save(
        &harness,
        json!({ "id": "box-1", "code": "600000", "kind": "support", "low": 9.5, "high": 10.5 }),
    );
    save(
        &harness,
        json!({ "id": "line-1", "code": "600000", "kind": "hline", "low": 9.8, "high": 9.8 }),
    );

    let zones = harness
        .invoke("support_zones_get", json!({ "code": "600000" }))
        .expect("support_zones_get 成功");
    let zones = zones.as_array().unwrap();
    assert_eq!(zones.len(), 2);
    let kinds: Vec<&str> = zones.iter().map(|z| z["kind"].as_str().unwrap()).collect();
    assert!(kinds.contains(&"support"));
    assert!(kinds.contains(&"hline"));
}

#[test]
fn dragging_a_zone_recomputes_confidence_and_keeps_created_at() {
    let dir = tempfile::tempdir().unwrap();
    seed_flat(dir.path());
    let harness = Harness::new(dir.path());

    // 一开始框裹住参考价，高置信。
    save(
        &harness,
        json!({ "id": "box-1", "code": "600000", "kind": "support", "low": 9.5, "high": 10.5 }),
    );
    let created_at = harness.rows()[0].created_at.clone();

    // 拖到远离参考价的位置，重新计算后降为低置信。
    let moved = save(
        &harness,
        json!({ "id": "box-1", "code": "600000", "kind": "support", "low": 11.5, "high": 12.5 }),
    );
    assert_eq!(moved["confidence"], "低");
    assert_eq!(moved["low"], 11.5);

    let rows = harness.rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].created_at, created_at);
    assert_eq!(rows[0].low, 11.5);
    assert_eq!(rows[0].high, 12.5);
}

#[test]
fn box_wrapping_one_reference_is_downgraded() {
    let dir = tempfile::tempdir().unwrap();
    seed_distinct_refs(dir.path());
    let harness = Harness::new(dir.path());
    let start = date_of(70);

    // 参考价是 10.00 / 10.00 / 10.20，离散度约 1.99%，本身是高置信。
    let refs = harness
        .invoke("support_refs", json!({ "code": "600000", "avwapStartDate": start }))
        .expect("support_refs 成功");
    assert!((refs["prevHigh"].as_f64().unwrap() - 10.2).abs() < 1e-9);
    assert!((refs["avwap"].as_f64().unwrap() - 10.0).abs() < 1e-9);

    // 只裹住前期高点，落框数 1，降为低置信。
    let one = save(
        &harness,
        json!({
            "id": "box-one", "code": "600000", "kind": "support",
            "low": 10.1, "high": 10.3, "avwapStartDate": start
        }),
    );
    assert_eq!(one["confidence"], "低");

    // 裹住 EMA21 与 AVWAP 两个参考价，沿用离散度等级（高）。
    let two = save(
        &harness,
        json!({
            "id": "box-two", "code": "600000", "kind": "support",
            "low": 9.9, "high": 10.05, "avwapStartDate": start
        }),
    );
    assert_eq!(two["confidence"], "高");
}

#[test]
fn gap_zone_records_gap_bounds() {
    let dir = tempfile::tempdir().unwrap();
    seed_flat(dir.path());
    let harness = Harness::new(dir.path());

    save(
        &harness,
        json!({ "id": "gap-1", "code": "600000", "kind": "gap", "low": 10.2, "high": 10.8 }),
    );
    let row = &harness.rows()[0];
    assert_eq!(row.kind, "gap");
    assert_eq!(row.gap_top, Some(10.8));
    assert_eq!(row.gap_bottom, Some(10.2));
}

#[test]
fn save_without_id_generates_one() {
    let dir = tempfile::tempdir().unwrap();
    seed_flat(dir.path());
    let harness = Harness::new(dir.path());

    let view = save(
        &harness,
        json!({ "code": "600000", "kind": "support", "low": 9.5, "high": 10.5 }),
    );
    let id = view["id"].as_str().unwrap();
    assert!(id.starts_with("zone-"), "{id}");
    assert_eq!(harness.rows()[0].id, id);
}

#[test]
fn save_rejects_unknown_kind() {
    let dir = tempfile::tempdir().unwrap();
    seed_flat(dir.path());
    let harness = Harness::new(dir.path());

    let error = harness
        .invoke(
            "support_zone_save",
            json!({ "zone": { "code": "600000", "kind": "circle", "low": 9.5, "high": 10.5 } }),
        )
        .expect_err("未知类型应报错");
    assert!(error.as_str().unwrap().contains("circle"), "{error}");
}

#[test]
fn delete_removes_the_row() {
    let dir = tempfile::tempdir().unwrap();
    seed_flat(dir.path());
    let harness = Harness::new(dir.path());

    save(
        &harness,
        json!({ "id": "box-1", "code": "600000", "kind": "support", "low": 9.5, "high": 10.5 }),
    );
    harness
        .invoke("support_zone_delete", json!({ "id": "box-1" }))
        .expect("support_zone_delete 成功");

    let zones = harness
        .invoke("support_zones_get", json!({ "code": "600000" }))
        .expect("support_zones_get 成功");
    assert!(zones.as_array().unwrap().is_empty());
    assert!(harness.rows().is_empty());
}

#[test]
fn avwap_start_date_shifts_the_reference_price() {
    let dir = tempfile::tempdir().unwrap();
    seed_distinct_refs(dir.path());
    let harness = Harness::new(dir.path());

    // 默认起点取末尾往前 60 根，覆盖到前段的高价 bar。
    let default_refs = harness
        .invoke("support_refs", json!({ "code": "600000", "avwapStartDate": null }))
        .expect("support_refs 成功");
    // 起点设在末尾 10 根，AVWAP 只覆盖平价段。
    let late_refs = harness
        .invoke("support_refs", json!({ "code": "600000", "avwapStartDate": date_of(70) }))
        .expect("support_refs 成功");

    let default_avwap = default_refs["avwap"].as_f64().unwrap();
    let late_avwap = late_refs["avwap"].as_f64().unwrap();
    assert!(default_avwap > late_avwap, "{default_avwap} vs {late_avwap}");
    assert!((late_avwap - 10.0).abs() < 1e-9);
}

#[test]
fn support_refs_rejects_start_date_after_all_bars() {
    let dir = tempfile::tempdir().unwrap();
    seed_flat(dir.path());
    let harness = Harness::new(dir.path());

    let error = harness
        .invoke(
            "support_refs",
            json!({ "code": "600000", "avwapStartDate": "2030-01-01" }),
        )
        .expect_err("起点晚于全部日线应报错");
    assert!(error.as_str().unwrap().contains("2030-01-01"), "{error}");
}

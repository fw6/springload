//! `commands::journal` 的四条命令：统计、费用模型、手动补录与 CSV 导出。
//!
//! 存储用临时 SQLite；断言写在 `trade_log` 表里的行与命令返回的统计数字。

use serde_json::{json, Value};
use springload_core::journal::STATS_NOTE;
use springload_core::storage::stock::Stock;
use springload_core::storage::trade_log::{self, TradeLogEntry};
use springload_core::storage::{stock, Database};
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
                commands::journal::journal_get,
                commands::journal::journal_add_manual,
                commands::journal::journal_export_csv,
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

/// 一只股票。
fn seed_stock(dir: &std::path::Path) {
    let db = Database::open(&dir.join("springload.db")).expect("打开数据库");
    let conn = db.conn().unwrap();
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
}

/// 一笔已出场的记录。
fn seed_closed(dir: &std::path::Path, id: &str, r_multiple: f64, pnl: f64) {
    let db = Database::open(&dir.join("springload.db")).expect("打开数据库");
    let conn = db.conn().unwrap();
    trade_log::upsert(
        &conn,
        &TradeLogEntry {
            id: id.to_string(),
            position_id: None,
            code: "600036".to_string(),
            name: Some("招商银行".to_string()),
            entry_date: "2026-10-01".to_string(),
            entry_price: 20.0,
            stop_price: 19.5,
            exit_date: Some("2026-10-10".to_string()),
            exit_price: Some(20.0 + r_multiple * 0.5),
            quantity: 1000,
            per_r: Some(0.5),
            r_multiple: Some(r_multiple),
            pnl: Some(pnl),
            fees: 0.0,
            note: None,
            source: "manual".to_string(),
            created_at: "2026-10-10T00:00:00Z".to_string(),
        },
    )
    .unwrap();
}

/// 把费用模型打开。
fn enable_fees(dir: &std::path::Path) {
    let path = dir.join("springload.config.json");
    let mut settings = springload_core::config::load_settings(&path).unwrap().settings;
    settings.fee_model_enabled = true;
    settings.commission_rate = 0.00025;
    springload_core::config::save_settings(&path, &settings).unwrap();
}

#[test]
fn statistics_count_only_closed_trades() {
    let dir = tempfile::tempdir().unwrap();
    seed_stock(dir.path());
    seed_closed(dir.path(), "log-1", 3.7, 8200.0);
    seed_closed(dir.path(), "log-2", -1.0, -2400.0);
    let harness = Harness::new(dir.path());

    // 一笔还没出场的。
    harness
        .invoke(
            "journal_add_manual",
            json!({ "entry": {
                "code": "600036",
                "entryDate": "2026-10-12",
                "entryPrice": 20.0,
                "stopPrice": 19.5,
                "quantity": 1000
            }}),
        )
        .expect("补录成功");

    let view = harness.invoke("journal_get", Value::Null).expect("读日志成功");
    assert_eq!(view["stats"]["trades"], 2);
    assert_eq!(view["stats"]["wins"], 1);
    assert_eq!(view["stats"]["winRate"], 0.5);
    assert!((view["stats"]["avgR"].as_f64().unwrap() - 1.35).abs() < 1e-9);
    assert!((view["stats"]["totalPnl"].as_f64().unwrap() - 5800.0).abs() < 1e-9);
    assert_eq!(view["stats"]["note"], STATS_NOTE);
    assert_eq!(view["entries"].as_array().unwrap().len(), 3);
    assert_eq!(view["feeModelEnabled"], false);
}

#[test]
fn the_fee_model_is_off_by_default_and_deducts_per_trade_once_enabled() {
    let dir = tempfile::tempdir().unwrap();
    seed_stock(dir.path());
    seed_closed(dir.path(), "log-1", 3.0, 1500.0);
    let harness = Harness::new(dir.path());

    let off = harness.invoke("journal_get", Value::Null).unwrap();
    assert_eq!(off["feeModelEnabled"], false);
    assert_eq!(off["entries"][0]["fees"], 0.0);
    assert_eq!(off["entries"][0]["pnl"], 1500.0);
    assert_eq!(off["stats"]["totalFees"], 0.0);

    enable_fees(dir.path());
    let on = harness.invoke("journal_get", Value::Null).unwrap();
    assert_eq!(on["feeModelEnabled"], true);
    // 20.00 买入、21.50 卖出、1000 股：买入侧 6.282 元，卖出侧 17.50315 元。
    let fees = on["entries"][0]["fees"].as_f64().unwrap();
    assert!((fees - 23.78515).abs() < 1e-9, "实际费用 {fees}");
    let pnl = on["entries"][0]["pnl"].as_f64().unwrap();
    assert!((pnl - (1500.0 - 23.78515)).abs() < 1e-9, "实际盈亏 {pnl}");
    assert!((on["stats"]["totalFees"].as_f64().unwrap() - 23.78515).abs() < 1e-9);
    assert!((on["stats"]["totalPnl"].as_f64().unwrap() - (1500.0 - 23.78515)).abs() < 1e-9);
    // R 倍数不因费用改变，胜率与笔数也不变。
    assert_eq!(on["entries"][0]["rMultiple"], 3.0);
    assert_eq!(on["stats"]["trades"], 1);
    assert_eq!(on["stats"]["wins"], 1);

    // 再读一次不重复扣减。
    let again = harness.invoke("journal_get", Value::Null).unwrap();
    let fees_again = again["entries"][0]["fees"].as_f64().unwrap();
    assert!((fees_again - 23.78515).abs() < 1e-9, "重复读取不重复扣减");
}

#[test]
fn manual_entries_are_validated() {
    let dir = tempfile::tempdir().unwrap();
    seed_stock(dir.path());
    let harness = Harness::new(dir.path());

    let base = json!({
        "code": "600036",
        "entryDate": "2026-10-12",
        "entryPrice": 20.0,
        "stopPrice": 19.5,
        "quantity": 1000
    });

    let mut missing_code = base.clone();
    missing_code["code"] = json!("");
    assert!(harness
        .invoke("journal_add_manual", json!({ "entry": missing_code }))
        .expect_err("标的不能为空")
        .as_str()
        .unwrap()
        .contains("标的不为空"));

    let mut zero_quantity = base.clone();
    zero_quantity["quantity"] = json!(0);
    assert!(harness
        .invoke("journal_add_manual", json!({ "entry": zero_quantity }))
        .expect_err("股数必须是正整数")
        .as_str()
        .unwrap()
        .contains("股数必须是正整数"));

    let mut half_exit = base.clone();
    half_exit["exitPrice"] = json!(21.5);
    assert!(harness
        .invoke("journal_add_manual", json!({ "entry": half_exit }))
        .expect_err("出场价与出场日要一起给")
        .as_str()
        .unwrap()
        .contains("要一起给"));
}

#[test]
fn a_manual_entry_with_an_exit_price_gets_its_r_multiple() {
    let dir = tempfile::tempdir().unwrap();
    seed_stock(dir.path());
    let harness = Harness::new(dir.path());

    let entry = harness
        .invoke(
            "journal_add_manual",
            json!({ "entry": {
                "code": "600036",
                "entryDate": "2026-10-01",
                "entryPrice": 20.0,
                "stopPrice": 19.5,
                "exitPrice": 21.5,
                "exitDate": "2026-10-10",
                "quantity": 1000,
                "note": "到 3R 减三分之一"
            }}),
        )
        .expect("补录成功");
    assert_eq!(entry["perR"], 0.5);
    assert_eq!(entry["rMultiple"], 3.0);
    assert_eq!(entry["pnl"], 1500.0);
    assert_eq!(entry["source"], "manual");
    assert_eq!(entry["name"], "招商银行");

    let view = harness.invoke("journal_get", Value::Null).unwrap();
    assert_eq!(view["stats"]["trades"], 1);
    assert_eq!(view["stats"]["wins"], 1);
    assert_eq!(view["stats"]["winRate"], 1.0);
}

#[test]
fn csv_export_writes_a_header_and_one_line_per_entry() {
    let dir = tempfile::tempdir().unwrap();
    seed_stock(dir.path());
    seed_closed(dir.path(), "log-1", 3.0, 1500.0);
    let harness = Harness::new(dir.path());

    let path = dir.path().join("journal.csv");
    harness
        .invoke("journal_export_csv", json!({ "path": path.to_str().unwrap() }))
        .expect("导出成功");

    let csv = std::fs::read_to_string(&path).expect("读到导出的文件");
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("代码,名称,入场日,买入价,止损价,出场日,出场价,股数,每股风险,R倍数,盈亏,费用,备注"));
    assert!(lines[1].starts_with("600036,招商银行,2026-10-01,20.00,19.50,2026-10-10,21.50,1000,0.5000,3.0000,1500.00,0.00,"));
}

#[test]
fn exporting_to_an_unwritable_path_reports_the_path() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());

    let error = harness
        .invoke(
            "journal_export_csv",
            json!({ "path": "/definitely/not/writable/journal.csv" }),
        )
        .expect_err("写不进去要报错");
    assert!(error
        .as_str()
        .unwrap()
        .contains("/definitely/not/writable/journal.csv"));
}

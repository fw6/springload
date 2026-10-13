//! `commands::plan` 的五条命令：计划卡的存取、状态与「确认已执行」写入场记录。
//!
//! 存储用临时 SQLite；断言写在 `trade_plan` / `position` / `trade_log` 三张表里的行。

use serde_json::{json, Value};
use springload_core::storage::stock::Stock;
use springload_core::storage::watchlist::WatchlistItem;
use springload_core::storage::{position, stock, trade_log, trade_plan, watchlist, Database};
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
                commands::plan::plan_card,
                commands::plan::plan_save,
                commands::plan::plan_get,
                commands::plan::plan_list,
                commands::plan::plan_confirm_executed,
                commands::plan::plan_set_status,
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

/// 一只在观察名单里的股票。
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
            listed_at: Some("2002-04-09".to_string()),
            is_st: false,
            updated_at: springload_core::storage::now(),
        },
    )
    .unwrap();
    watchlist::upsert(
        &conn,
        &WatchlistItem {
            code: "600036".to_string(),
            added_at: springload_core::storage::now(),
            c1: true,
            c2: true,
            c3: true,
            c4: false,
            chg_1m: Some(0.1),
            chg_3m: Some(0.2),
            chg_6m: Some(0.4),
            rank_pct: Some(0.25),
            avg_amount: Some(2e8),
            convergence: Some(0.1),
            screen_run_at: springload_core::storage::now(),
            manual: false,
        },
    )
    .unwrap();
}

/// 规格里的示例输入。
fn example_plan() -> Value {
    json!({
        "plan": {
            "code": "600036",
            "entry": 20.00,
            "stop": 19.50,
            "account": 100000.0,
            "riskPct": 0.005,
            "zoneId": "zone-1",
            "signalId": null
        }
    })
}

#[test]
fn saving_a_plan_computes_the_sizing_and_starts_as_a_draft() {
    let dir = tempfile::tempdir().unwrap();
    seed_stock(dir.path());
    let harness = Harness::new(dir.path());

    let plan = harness.invoke("plan_save", example_plan()).expect("保存成功");
    assert_eq!(plan["status"], "draft");
    assert_eq!(plan["shares"], 1000);
    assert_eq!(plan["capital"], 20000.0);
    assert_eq!(plan["t3"], 21.50);
    assert_eq!(plan["t5"], 22.50);
    assert_eq!(plan["perR"], 0.50);
    assert_eq!(plan["stopDistance"], 0.025);

    let db = harness.db();
    let conn = db.conn().unwrap();
    let rows = trade_plan::list(&conn, None).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "draft");
    assert_eq!(rows[0].shares, 1000);
}

#[test]
fn the_card_carries_the_screening_verdict_the_board_and_the_reduce_plan() {
    let dir = tempfile::tempdir().unwrap();
    seed_stock(dir.path());
    let harness = Harness::new(dir.path());
    let plan = harness.invoke("plan_save", example_plan()).unwrap();

    let card = harness
        .invoke("plan_card", json!({ "id": plan["id"] }))
        .expect("读计划卡成功");
    assert_eq!(card["name"], "招商银行");
    assert_eq!(card["board"], "main");
    assert_eq!(card["sector"], "银行");
    assert_eq!(card["c1"], true);
    assert_eq!(card["c2"], true);
    assert_eq!(card["c3"], true);
    assert_eq!(card["c4"], false);
    assert_eq!(card["reduce"]["shares3r"], 333);
    assert_eq!(card["reduce"]["shares5r"], 333);
    assert_eq!(card["reduce"]["remaining"], 334);
    // 取整后的实际风险占账户 1000 × 0.50 ÷ 100000 = 0.5%。
    assert_eq!(card["actualRiskPct"], 0.005);
}

#[test]
fn entry_not_above_stop_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());

    let mut input = example_plan();
    input["plan"]["entry"] = json!(19.50);
    let error = harness.invoke("plan_save", input).expect_err("应报错");
    assert!(
        error.as_str().unwrap().contains("买入价必须高于止损价"),
        "错误文案应说明买入价与止损价的关系，实际是 {error}"
    );
}

#[test]
fn confirming_execution_writes_the_entry_row_into_the_trade_log() {
    let dir = tempfile::tempdir().unwrap();
    seed_stock(dir.path());
    let harness = Harness::new(dir.path());
    let plan = harness.invoke("plan_save", example_plan()).unwrap();
    let id = plan["id"].as_str().unwrap().to_string();

    let before = {
        let db = harness.db();
        let conn = db.conn().unwrap();
        trade_log::list(&conn).unwrap().len()
    };
    assert_eq!(before, 0, "确认之前交易日志是空的");

    let entry = harness
        .invoke(
            "plan_confirm_executed",
            json!({ "id": id, "entryDate": "2026-10-14" }),
        )
        .expect("确认执行成功");
    assert_eq!(entry["code"], "600036");
    assert_eq!(entry["entryDate"], "2026-10-14");
    assert_eq!(entry["entryPrice"], 20.00);
    assert_eq!(entry["stopPrice"], 19.50);
    assert_eq!(entry["quantity"], 1000);
    assert_eq!(entry["source"], "plan");
    assert!(entry["exitDate"].is_null(), "入场记录还没有出场");

    let db = harness.db();
    let conn = db.conn().unwrap();
    let rows = trade_log::list(&conn).unwrap();
    assert_eq!(rows.len(), before + 1, "交易日志多出一条入场行");
    assert_eq!(rows[0].per_r, Some(0.50));
    assert!(rows[0].r_multiple.is_none());

    // 计划状态变成已执行，并开出一条持仓。
    assert_eq!(trade_plan::get(&conn, &id).unwrap().unwrap().status, "executed");
    let open = position::list_open(&conn).unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].code, "600036");
    assert_eq!(open[0].quantity, 1000);
    assert_eq!(open[0].entry_date, "2026-10-14");
    assert!(open[0].follow_ema);
}

#[test]
fn confirming_twice_does_not_write_a_second_entry_row() {
    let dir = tempfile::tempdir().unwrap();
    seed_stock(dir.path());
    let harness = Harness::new(dir.path());
    let plan = harness.invoke("plan_save", example_plan()).unwrap();
    let id = plan["id"].as_str().unwrap().to_string();

    harness
        .invoke("plan_confirm_executed", json!({ "id": id, "entryDate": "2026-10-14" }))
        .unwrap();
    harness
        .invoke("plan_confirm_executed", json!({ "id": id, "entryDate": "2026-10-14" }))
        .unwrap();

    let db = harness.db();
    let conn = db.conn().unwrap();
    assert_eq!(trade_log::list(&conn).unwrap().len(), 1);
    assert_eq!(position::list_open(&conn).unwrap().len(), 1);
}

#[test]
fn a_plan_with_zero_shares_cannot_be_confirmed() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());
    let mut input = example_plan();
    input["plan"]["account"] = json!(1000.0);
    let plan = harness.invoke("plan_save", input).unwrap();
    assert_eq!(plan["shares"], 0);

    let error = harness
        .invoke(
            "plan_confirm_executed",
            json!({ "id": plan["id"], "entryDate": "2026-10-14" }),
        )
        .expect_err("股数为 0 不能确认");
    assert!(error.as_str().unwrap().contains("股数为 0"));
}

#[test]
fn status_moves_through_the_four_values_and_rejects_unknown_ones() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());
    let plan = harness.invoke("plan_save", example_plan()).unwrap();
    let id = plan["id"].as_str().unwrap().to_string();

    for status in ["triggered", "draft", "abandoned"] {
        let updated = harness
            .invoke("plan_set_status", json!({ "id": id, "status": status }))
            .expect("改状态成功");
        assert_eq!(updated["status"], status);
    }

    let error = harness
        .invoke("plan_set_status", json!({ "id": id, "status": "unknown" }))
        .expect_err("未知状态应报错");
    assert!(error.as_str().unwrap().contains("未知的计划状态"));
}

#[test]
fn list_filters_by_status() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());
    let first = harness.invoke("plan_save", example_plan()).unwrap();
    let mut second = example_plan();
    second["plan"]["entry"] = json!(30.0);
    second["plan"]["stop"] = json!(29.0);
    let second = harness.invoke("plan_save", second).unwrap();
    // 同一毫秒内连存两条也不能撞上同一个 id，否则第二条会覆盖第一条。
    assert_ne!(first["id"], second["id"]);
    harness
        .invoke(
            "plan_set_status",
            json!({ "id": first["id"], "status": "abandoned" }),
        )
        .unwrap();

    let drafts = harness
        .invoke("plan_list", json!({ "status": "draft" }))
        .expect("列出草稿成功");
    assert_eq!(drafts.as_array().unwrap().len(), 1);
    let all = harness.invoke("plan_list", json!({ "status": null })).unwrap();
    assert_eq!(all.as_array().unwrap().len(), 2);
}

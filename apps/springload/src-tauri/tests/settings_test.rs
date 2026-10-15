//! #22 设置页与首次启动确认的集成测试：直接调用命令，断言配置读写往返、
//! 首次启动确认、以及 CSV 导入的字段校验与写库结果。存储用临时 SQLite 文件。

use serde_json::{json, Value};
use springload_core::storage::{bar, Database};
use springload_lib::commands;
use springload_lib::state::AppState;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{App, WebviewWindow, WebviewWindowBuilder};

/// 测试用应用：状态指向临时数据目录，注册 #11 与 #22 的命令。
struct Harness {
    // 先声明 webview：释放顺序是先 webview 后 app。
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
                commands::app::app_info,
                commands::app::settings_get,
                commands::app::settings_update,
                commands::settings::import_bars_csv,
                commands::settings::confirm_first_run,
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

    /// 通过 IPC 层调用一个命令，返回命令的 JSON 结果或错误值。
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

    /// 另开一条连接读库，用来核对导入结果。
    fn read_bars(&self, code: &str) -> Vec<bar::BarRow> {
        let db = Database::open(&self.dir.join("springload.db")).expect("打开库");
        let conn = db.conn().expect("取连接");
        bar::list_for_code(&conn, code).expect("读日线")
    }

    /// 往临时目录写一个 CSV 文件，返回它的路径。
    fn write_csv(&self, name: &str, content: &str) -> String {
        let path = self.dir.join(name);
        std::fs::write(&path, content).expect("写 CSV");
        path.to_string_lossy().into_owned()
    }
}

const HEADER: &str = "code,date,open,high,low,close,volume,amount\n";

#[test]
fn settings_round_trip_covers_page_fields() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());

    let updated = harness
        .invoke(
            "settings_update",
            json!({ "patch": {
                "riskPct": 0.008,
                "stopDistanceWarn": 0.04,
                "cn": { "liquidityThreshold": 5e7, "commissionRate": 0.0003 },
                "chg6mThreshold": 0.35,
                "rankBand": [0.15, 0.25],
                "convergenceThreshold": 0.20,
                "beginnerMode": false,
            }}),
        )
        .expect("settings_update 成功");
    assert_eq!(updated["riskPct"], 0.008);
    assert_eq!(updated["cn"]["liquidityThreshold"], 5e7);
    assert_eq!(updated["rankBand"], json!([0.15, 0.25]));
    assert_eq!(updated["beginnerMode"], false);
    // 未给出的字段保持原值。
    assert_eq!(updated["cn"]["commissionRate"], 0.0003);
    assert_eq!(updated["us"]["commissionRate"], 0.0);

    // 重新读到的就是写下去的值。
    let again = harness.invoke("settings_get", Value::Null).expect("settings_get 成功");
    assert_eq!(again["riskPct"], 0.008);
    assert_eq!(again["stopDistanceWarn"], 0.04);
    assert_eq!(again["cn"]["liquidityThreshold"], 5e7);
    assert_eq!(again["chg6mThreshold"], 0.35);
    assert_eq!(again["rankBand"], json!([0.15, 0.25]));
    assert_eq!(again["convergenceThreshold"], 0.20);
    assert_eq!(again["beginnerMode"], false);

    // 落盘内容能被 core 直接读出来。
    let path = dir.path().join("springload.config.json");
    let load = springload_core::config::load_settings(&path).expect("读配置文件");
    assert_eq!(load.settings.risk_pct, 0.008);
    assert_eq!(load.settings.rank_band, (0.15, 0.25));
    assert!(!load.settings.beginner_mode);
}

#[test]
fn confirm_first_run_sets_flag_and_persists() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());

    let before = harness.invoke("settings_get", Value::Null).expect("settings_get 成功");
    assert_eq!(before["firstRunConfirmed"], false);

    let after = harness
        .invoke("confirm_first_run", Value::Null)
        .expect("confirm_first_run 成功");
    assert_eq!(after["firstRunConfirmed"], true);

    let again = harness.invoke("settings_get", Value::Null).expect("settings_get 成功");
    assert_eq!(again["firstRunConfirmed"], true);

    let path = dir.path().join("springload.config.json");
    let load = springload_core::config::load_settings(&path).expect("读配置文件");
    assert!(load.settings.first_run_confirmed);
}

#[test]
fn import_csv_writes_valid_rows_and_replaces_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());

    let csv = format!(
        "{HEADER}\
         600000,2024-01-02,10.0,10.5,9.8,10.2,100000,1020000\n\
         600000,2024-01-03,10.2,10.6,10.1,10.4,120000,1248000\n"
    );
    let path = harness.write_csv("bars.csv", &csv);

    let report = harness
        .invoke("import_bars_csv", json!({ "path": path }))
        .expect("import_bars_csv 成功");
    assert_eq!(report["rows"], 2);
    assert_eq!(report["skipped"], 0);
    assert_eq!(report["errors"], json!([]));

    let bars = harness.read_bars("600000");
    assert_eq!(bars.len(), 2);
    assert_eq!(bars[0].date, "2024-01-02");
    assert_eq!(bars[0].close, 10.2);
    assert_eq!(bars[1].amount, 1248000.0);

    // 再导一次同样的内容，按主键覆盖而不是新增。
    let report = harness
        .invoke("import_bars_csv", json!({ "path": path }))
        .expect("重复导入成功");
    assert_eq!(report["rows"], 2);
    assert_eq!(harness.read_bars("600000").len(), 2);
}

#[test]
fn import_csv_reports_missing_header_column() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());

    let csv = "code,date,open,high,low,close,volume\n\
               600000,2024-01-02,10.0,10.5,9.8,10.2,100000\n";
    let path = harness.write_csv("missing_column.csv", csv);

    let error = harness
        .invoke("import_bars_csv", json!({ "path": path }))
        .expect_err("缺列应返回错误");
    let message = error.as_str().expect("错误是可展示文案");
    assert!(message.contains("amount"), "错误要指出缺的是哪一列：{message}");
    assert!(message.contains("表头"), "错误要说明是表头缺列：{message}");
}

#[test]
fn import_csv_reports_type_error_with_row_and_column() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());

    let csv = format!(
        "{HEADER}\
         600000,2024-01-02,10.0,10.5,9.8,10.2,100000,1020000\n\
         600001,2024-01-02,不是数字,10.5,9.8,10.2,100000,1020000\n"
    );
    let path = harness.write_csv("type_error.csv", &csv);

    let report = harness
        .invoke("import_bars_csv", json!({ "path": path }))
        .expect("类型错按行跳过，整体仍成功");
    assert_eq!(report["rows"], 1);
    assert_eq!(report["skipped"], 1);
    let errors = report["errors"].as_array().unwrap();
    assert_eq!(errors.len(), 1);
    let message = errors[0].as_str().unwrap();
    assert!(message.contains("第 3 行"), "错误要指出行号：{message}");
    assert!(message.contains("open"), "错误要指出列名：{message}");

    // 合法的那一行照常入库，出错的那一行不入库。
    assert_eq!(harness.read_bars("600000").len(), 1);
    assert_eq!(harness.read_bars("600001").len(), 0);
}

#[test]
fn import_csv_reports_bad_date_with_row_and_column() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());

    let csv = format!(
        "{HEADER}\
         600000,2024/01/02,10.0,10.5,9.8,10.2,100000,1020000\n"
    );
    let path = harness.write_csv("bad_date.csv", &csv);

    let report = harness
        .invoke("import_bars_csv", json!({ "path": path }))
        .expect("日期格式错按行跳过，整体仍成功");
    assert_eq!(report["rows"], 0);
    assert_eq!(report["skipped"], 1);
    let errors = report["errors"].as_array().unwrap();
    let message = errors[0].as_str().unwrap();
    assert!(message.contains("第 2 行"), "错误要指出行号：{message}");
    assert!(message.contains("date"), "错误要指出列名：{message}");
    assert!(message.contains("2024/01/02"), "错误要回显原始值：{message}");
    assert_eq!(harness.read_bars("600000").len(), 0);
}

#[test]
fn import_csv_reports_unreadable_path() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());

    let error = harness
        .invoke("import_bars_csv", json!({ "path": dir.path().join("nope.csv").to_string_lossy() }))
        .expect_err("读不到文件应返回错误");
    assert!(error.as_str().unwrap().contains("nope.csv"));
}

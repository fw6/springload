//! 集成测试直接调用应用信息、术语、设置与市场切换这几个命令，断言输入与输出，
//! 以及首次启动建好的目录与文件。

use serde_json::{json, Value};
use springload_lib::commands;
use springload_lib::state::AppState;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{App, WebviewWindow, WebviewWindowBuilder};

/// 测试用应用：状态指向临时数据目录，只注册本 ticket 的四个命令。
struct Harness {
    // 先声明 webview：释放顺序是先 webview 后 app。
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
                commands::app::app_info,
                commands::app::glossary,
                commands::app::settings_get,
                commands::app::settings_update,
                commands::app::market_set,
            ])
            .build(mock_context(noop_assets()))
            .expect("构建测试应用");
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("建测试窗口");
        Self { webview, app }
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
}

#[test]
fn first_launch_creates_data_dir_database_and_config() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().join(".springload");

    AppState::from_data_dir(data_dir.clone()).expect("首次装配");

    assert!(data_dir.is_dir(), "首次启动应建好数据目录");
    assert!(data_dir.join("springload.db").is_file(), "应建好 A 股 SQLite 库");
    assert!(
        data_dir.join("springload.config.json").is_file(),
        "缺失的配置文件应生成默认值"
    );

    // 重复装配不报错。
    AppState::from_data_dir(data_dir.clone()).expect("重复装配");
}

#[test]
fn app_info_reports_version_source_and_sync_time() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());

    let info = harness.invoke("app_info", Value::Null).expect("app_info 成功");
    assert_eq!(info["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(info["market"], "cn");
    assert_eq!(info["marketLabel"], "A 股");
    assert_eq!(info["currency"], "元");
    assert_eq!(info["tPlusOne"], true);
    assert_eq!(info["dataSource"], "通达信 · 前复权");
    assert!(info["lastSyncAt"].is_null(), "还没有同步记录时最近同步时间为 null");
}

/// 切换市场后整个应用换一套：库文件、数据源、货币与四步文案。
#[test]
fn market_set_switches_the_whole_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());

    let switched = harness
        .invoke("market_set", json!({ "market": "us" }))
        .expect("切到美股");
    assert_eq!(switched["market"], "us");
    assert_eq!(switched["us"]["liquidityThreshold"], 2e7);
    assert_eq!(switched["us"]["commissionRate"], 0.0);
    // A 股那一份参数留着不动。
    assert_eq!(switched["cn"]["liquidityThreshold"], 1e8);

    let info = harness.invoke("app_info", Value::Null).expect("app_info 成功");
    assert_eq!(info["market"], "us");
    assert_eq!(info["marketLabel"], "美股");
    assert_eq!(info["currency"], "美元");
    assert_eq!(info["tPlusOne"], false);
    assert_eq!(info["dataSource"], "Yahoo Finance · 前复权");

    // 美股用另一个库文件，切换时按需建好。
    assert!(
        dir.path().join("springload.us.db").is_file(),
        "切到美股应建好美股库"
    );

    // 四步文案跟着换：股票池范围与买入申报单位都是美股的说法。
    let g = harness.invoke("glossary", Value::Null).expect("glossary 成功");
    assert!(g["steps"][0]["oneLine"]
        .as_str()
        .unwrap()
        .contains("纳斯达克"));
    assert!(g["steps"][3]["detail"]
        .as_str()
        .unwrap()
        .contains("美股按整股交易"));

    // 切回 A 股：配置里记的是 cn，文案与数据源也回到 A 股那一套。
    let back = harness
        .invoke("market_set", json!({ "market": "cn" }))
        .expect("切回 A 股");
    assert_eq!(back["market"], "cn");
    let info = harness.invoke("app_info", Value::Null).expect("app_info 成功");
    assert_eq!(info["dataSource"], "通达信 · 前复权");
    let g = harness.invoke("glossary", Value::Null).expect("glossary 成功");
    assert!(g["steps"][0]["oneLine"].as_str().unwrap().contains("沪深主板"));

    // 未知市场被拒绝。
    assert!(harness
        .invoke("market_set", json!({ "market": "hk" }))
        .is_err());
}

#[test]
fn glossary_returns_ten_terms_and_four_steps() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());

    let g = harness.invoke("glossary", Value::Null).expect("glossary 成功");
    assert_eq!(g["terms"].as_array().unwrap().len(), 10);
    assert_eq!(g["steps"].as_array().unwrap().len(), 4);
    assert_eq!(g["terms"][0]["key"], "pullback");
    assert_eq!(g["steps"][0]["title"], "强势股筛选");
    assert!(g["steps"][3]["detail"]
        .as_str()
        .unwrap()
        .contains("3R 卖三分之一"));
}

#[test]
fn settings_get_returns_contract_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());

    let s = harness.invoke("settings_get", Value::Null).expect("settings_get 成功");
    assert_eq!(s["market"], "cn");
    assert_eq!(s["riskPct"], 0.005);
    assert_eq!(s["stopDistanceWarn"], 0.05);
    assert_eq!(s["chg6mThreshold"], 0.30);
    assert_eq!(s["rankBand"], json!([0.20, 0.30]));
    assert_eq!(s["convergenceThreshold"], 0.25);
    assert_eq!(s["beginnerMode"], true);
    assert_eq!(s["autostart"], false);
    assert_eq!(s["feeModelEnabled"], false);
    assert_eq!(s["firstRunConfirmed"], false);
    // 随市场变化的两项按市场各存一份。
    assert_eq!(s["cn"]["liquidityThreshold"], 1e8);
    assert_eq!(s["cn"]["commissionRate"], 0.00025);
    assert_eq!(s["us"]["liquidityThreshold"], 2e7);
    assert_eq!(s["us"]["commissionRate"], 0.0);
}

#[test]
fn settings_update_patches_and_persists() {
    let dir = tempfile::tempdir().unwrap();
    let harness = Harness::new(dir.path());

    let updated = harness
        .invoke(
            "settings_update",
            json!({ "patch": { "autostart": true, "riskPct": 0.01 } }),
        )
        .expect("settings_update 成功");
    assert_eq!(updated["autostart"], true);
    assert_eq!(updated["riskPct"], 0.01);
    // 未给出的字段保持原值。
    assert_eq!(updated["stopDistanceWarn"], 0.05);

    // 写回了配置文件，重新读到的就是新值。
    let again = harness.invoke("settings_get", Value::Null).expect("settings_get 成功");
    assert_eq!(again["autostart"], true);
    assert_eq!(again["riskPct"], 0.01);

    // 落盘的内容也能被 core 直接读出来。
    let path = dir.path().join("springload.config.json");
    let load = springload_core::config::load_settings(&path).expect("读配置文件");
    assert!(load.settings.autostart);
    assert_eq!(load.settings.risk_pct, 0.01);
}

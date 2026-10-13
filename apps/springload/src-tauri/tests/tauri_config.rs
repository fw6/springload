//! 应用的真实配置要能被 tauri-build 接受，且命名与版本逐项符合契约。

use serde_json::Value;

fn app_config() -> Value {
    let raw =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tauri.conf.json")).unwrap();
    serde_json::from_str(&raw).unwrap()
}

#[test]
fn version_matches_the_cargo_package() {
    let conf = app_config();
    assert_eq!(
        conf["version"].as_str().unwrap(),
        env!("CARGO_PKG_VERSION"),
        "tauri.conf.json 的 version 与 Cargo 的包版本不一致"
    );
}

#[test]
fn bundle_identifier_and_product_name_match_the_naming_table() {
    let conf = app_config();
    assert_eq!(conf["identifier"].as_str().unwrap(), "com.springload.app");
    assert_eq!(conf["productName"].as_str().unwrap(), "springload");
}

#[test]
fn main_window_title_is_the_chinese_brand() {
    let conf = app_config();
    assert_eq!(conf["app"]["windows"][0]["label"].as_str().unwrap(), "main");
    assert_eq!(
        conf["app"]["windows"][0]["title"].as_str().unwrap(),
        "蓄势 Springload"
    );
}

#[test]
fn frontend_dist_points_at_the_vite_output() {
    let conf = app_config();
    assert_eq!(conf["build"]["frontendDist"].as_str().unwrap(), "../dist");
    assert_eq!(
        conf["build"]["devUrl"].as_str().unwrap(),
        "http://localhost:1420"
    );
}

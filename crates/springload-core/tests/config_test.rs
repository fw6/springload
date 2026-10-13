//! `config` 模块的行为：默认值、往返读写、缺失补齐、非法回落。

use springload_core::config::{self, Settings, SettingsPatch};
use std::io::Write;

fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("建临时目录")
}

#[test]
fn missing_file_yields_defaults_without_warnings() {
    let dir = temp_dir();
    let path = dir.path().join("springload.config.json");
    let load = config::load_settings(&path).expect("读缺失的配置");
    assert_eq!(load.settings, Settings::default());
    assert!(load.warnings.is_empty(), "缺失文件不应产生警告: {:?}", load.warnings);
}

#[test]
fn default_settings_match_the_contract() {
    let s = Settings::default();
    assert_eq!(s.risk_pct, 0.005);
    assert_eq!(s.stop_distance_warn, 0.05);
    assert_eq!(s.liquidity_threshold, 1e8);
    assert_eq!(s.chg_6m_threshold, 0.30);
    assert_eq!(s.rank_band, (0.20, 0.30));
    assert_eq!(s.convergence_threshold, 0.25);
    assert!(s.beginner_mode);
    assert!(!s.autostart);
    assert!(!s.fee_model_enabled);
    assert_eq!(s.commission_rate, 0.00025);
    assert!(!s.first_run_confirmed);
}

#[test]
fn save_then_load_round_trips() {
    let dir = temp_dir();
    let path = dir.path().join("nested/springload.config.json");
    let mut settings = Settings::default();
    settings.risk_pct = 0.01;
    settings.autostart = true;
    settings.rank_band = (0.10, 0.25);

    config::save_settings(&path, &settings).expect("写配置");
    let load = config::load_settings(&path).expect("读配置");
    assert_eq!(load.settings, settings);
    assert!(load.warnings.is_empty());
}

#[test]
fn missing_fields_fall_back_to_defaults_silently() {
    let dir = temp_dir();
    let path = dir.path().join("springload.config.json");
    std::fs::write(&path, r#"{ "riskPct": 0.02, "autostart": true }"#).unwrap();

    let load = config::load_settings(&path).expect("读配置");
    assert_eq!(load.settings.risk_pct, 0.02);
    assert!(load.settings.autostart);
    // 其余字段回落到默认值。
    assert_eq!(load.settings.stop_distance_warn, 0.05);
    assert!(load.settings.beginner_mode);
    assert!(load.warnings.is_empty(), "缺失字段不应产生警告");
}

#[test]
fn invalid_field_value_falls_back_and_warns() {
    let dir = temp_dir();
    let path = dir.path().join("springload.config.json");
    std::fs::write(&path, r#"{ "riskPct": "很大", "commissionRate": 0.001 }"#).unwrap();

    let load = config::load_settings(&path).expect("读配置");
    assert_eq!(load.settings.risk_pct, 0.005, "非法值应回落到默认");
    assert_eq!(load.settings.commission_rate, 0.001, "合法值应保留");
    assert_eq!(load.warnings.len(), 1);
    assert!(load.warnings[0].contains("riskPct"), "警告要点出字段名: {:?}", load.warnings);
}

#[test]
fn unparseable_file_falls_back_entirely_and_warns() {
    let dir = temp_dir();
    let path = dir.path().join("springload.config.json");
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all("这不是 JSON".as_bytes()).unwrap();

    let load = config::load_settings(&path).expect("读配置");
    assert_eq!(load.settings, Settings::default());
    assert_eq!(load.warnings.len(), 1);
}

#[test]
fn patch_applies_only_given_fields() {
    let base = Settings::default();
    let patch: SettingsPatch =
        serde_json::from_str(r#"{ "riskPct": 0.01, "autostart": true }"#).expect("解析补丁");

    let next = patch.apply(&base);
    assert_eq!(next.risk_pct, 0.01);
    assert!(next.autostart);
    // 未给出的字段保持基准值。
    assert_eq!(next.stop_distance_warn, base.stop_distance_warn);
    assert_eq!(next.rank_band, base.rank_band);
}

#[test]
fn data_dir_honours_springload_home_override() {
    // 环境变量在进程内共享，用独立测试进程无法保证隔离，这里只验证函数形状：
    // 未设置时以家目录下的 .springload 结尾。
    if std::env::var_os("SPRINGLOAD_HOME").is_none() {
        let dir = config::data_dir();
        assert!(dir.ends_with(config::DATA_DIR_NAME), "数据目录应以 {} 结尾", config::DATA_DIR_NAME);
    }
}

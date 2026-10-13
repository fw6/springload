//! `~/.springload/` 数据目录、配置文件读写与 [`Settings`] 结构。
//!
//! 配置文件缺失时用默认值，读到非法值时该项回落到默认值并在返回值里带一条警告。

use crate::error::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// 数据源名称，`app_info` 命令返回给前端展示。
pub const DATA_SOURCE: &str = "东方财富 · 前复权";
/// 用户数据目录名，位于家目录之下。
pub const DATA_DIR_NAME: &str = ".springload";
/// 配置文件名。
pub const CONFIG_FILE_NAME: &str = "springload.config.json";
/// 数据库文件名。
pub const DB_FILE_NAME: &str = "springload.db";

/// 全局配置。字段与默认值见架构契约第 6 节。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// 单笔风险比例。
    pub risk_pct: f64,
    /// 止损距离警告阈值。
    pub stop_distance_warn: f64,
    /// 日均成交额下限（元）。
    pub liquidity_threshold: f64,
    /// 近 6 个月涨幅阈值。
    pub chg_6m_threshold: f64,
    /// 近 1 个月涨幅排名分位区间。
    pub rank_band: (f64, f64),
    /// 近 20 日极差阈值。
    pub convergence_threshold: f64,
    /// 小白模式。
    pub beginner_mode: bool,
    /// 登录时自启。
    pub autostart: bool,
    /// 费用模型。
    pub fee_model_enabled: bool,
    /// 佣金费率。
    pub commission_rate: f64,
    /// 首次启动风险提示已确认。
    pub first_run_confirmed: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            risk_pct: 0.005,
            stop_distance_warn: 0.05,
            liquidity_threshold: 1e8,
            chg_6m_threshold: 0.30,
            rank_band: (0.20, 0.30),
            convergence_threshold: 0.25,
            beginner_mode: true,
            autostart: false,
            fee_model_enabled: false,
            commission_rate: 0.00025,
            first_run_confirmed: false,
        }
    }
}

/// `settings_update` 的入参：只带要改的字段，其余保持原值。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SettingsPatch {
    pub risk_pct: Option<f64>,
    pub stop_distance_warn: Option<f64>,
    pub liquidity_threshold: Option<f64>,
    pub chg_6m_threshold: Option<f64>,
    pub rank_band: Option<(f64, f64)>,
    pub convergence_threshold: Option<f64>,
    pub beginner_mode: Option<bool>,
    pub autostart: Option<bool>,
    pub fee_model_enabled: Option<bool>,
    pub commission_rate: Option<f64>,
    pub first_run_confirmed: Option<bool>,
}

impl SettingsPatch {
    /// 把补丁叠加到基准配置上，未给出的字段保持基准值。
    pub fn apply(&self, base: &Settings) -> Settings {
        let mut next = base.clone();
        if let Some(v) = self.risk_pct {
            next.risk_pct = v;
        }
        if let Some(v) = self.stop_distance_warn {
            next.stop_distance_warn = v;
        }
        if let Some(v) = self.liquidity_threshold {
            next.liquidity_threshold = v;
        }
        if let Some(v) = self.chg_6m_threshold {
            next.chg_6m_threshold = v;
        }
        if let Some(v) = self.rank_band {
            next.rank_band = v;
        }
        if let Some(v) = self.convergence_threshold {
            next.convergence_threshold = v;
        }
        if let Some(v) = self.beginner_mode {
            next.beginner_mode = v;
        }
        if let Some(v) = self.autostart {
            next.autostart = v;
        }
        if let Some(v) = self.fee_model_enabled {
            next.fee_model_enabled = v;
        }
        if let Some(v) = self.commission_rate {
            next.commission_rate = v;
        }
        if let Some(v) = self.first_run_confirmed {
            next.first_run_confirmed = v;
        }
        next
    }
}

/// 读取配置的结果：解析出的配置，以及被回落成默认值的非法项说明。
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsLoad {
    pub settings: Settings,
    pub warnings: Vec<String>,
}

/// 用户数据目录：优先取 `SPRINGLOAD_HOME`（测试用），否则是家目录下的 `.springload`。
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("SPRINGLOAD_HOME") {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(DATA_DIR_NAME)
}

/// 建好数据目录并返回它。
pub fn ensure_data_dir() -> Result<PathBuf> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 配置文件路径。
pub fn config_path() -> PathBuf {
    data_dir().join(CONFIG_FILE_NAME)
}

/// 数据库文件路径。
pub fn db_path() -> PathBuf {
    data_dir().join(DB_FILE_NAME)
}

/// 从指定路径读取配置。文件不存在时返回默认值；文件无法解析或某项非法时
/// 该项回落到默认值并记一条警告。
pub fn load_settings(path: &Path) -> Result<SettingsLoad> {
    if !path.exists() {
        return Ok(SettingsLoad {
            settings: Settings::default(),
            warnings: Vec::new(),
        });
    }
    let text = std::fs::read_to_string(path)?;
    let value: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => {
            return Ok(SettingsLoad {
                settings: Settings::default(),
                warnings: vec![format!(
                    "配置文件 {} 无法解析，已改用全部默认值",
                    path.display()
                )],
            });
        }
    };
    let object = match value.as_object() {
        Some(map) => map.clone(),
        None => {
            return Ok(SettingsLoad {
                settings: Settings::default(),
                warnings: vec![format!(
                    "配置文件 {} 的顶层不是对象，已改用全部默认值",
                    path.display()
                )],
            });
        }
    };

    let defaults = Settings::default();
    let mut warnings = Vec::new();

    macro_rules! take {
        ($key:literal, $field:ident) => {
            take_field(&object, $key, &mut warnings).unwrap_or(defaults.$field)
        };
    }

    let settings = Settings {
        risk_pct: take!("riskPct", risk_pct),
        stop_distance_warn: take!("stopDistanceWarn", stop_distance_warn),
        liquidity_threshold: take!("liquidityThreshold", liquidity_threshold),
        chg_6m_threshold: take!("chg6mThreshold", chg_6m_threshold),
        rank_band: take!("rankBand", rank_band),
        convergence_threshold: take!("convergenceThreshold", convergence_threshold),
        beginner_mode: take!("beginnerMode", beginner_mode),
        autostart: take!("autostart", autostart),
        fee_model_enabled: take!("feeModelEnabled", fee_model_enabled),
        commission_rate: take!("commissionRate", commission_rate),
        first_run_confirmed: take!("firstRunConfirmed", first_run_confirmed),
    };

    Ok(SettingsLoad { settings, warnings })
}

/// 从对象里取一项；缺失返回 `None`，类型不符时记警告并返回 `None`。
fn take_field<T: serde::de::DeserializeOwned>(
    object: &serde_json::Map<String, Value>,
    key: &str,
    warnings: &mut Vec<String>,
) -> Option<T> {
    let value = object.get(key)?;
    match serde_json::from_value::<T>(value.clone()) {
        Ok(parsed) => Some(parsed),
        Err(_) => {
            warnings.push(format!("配置项 {key} 的值非法，已改用默认值"));
            None
        }
    }
}

/// 把配置写到指定路径，父目录不存在时先建好。
pub fn save_settings(path: &Path, settings: &Settings) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(settings)?;
    std::fs::write(path, format!("{text}\n"))?;
    Ok(())
}

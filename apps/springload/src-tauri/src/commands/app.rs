//! 应用信息、术语、设置与市场切换六个命令。领域规则在 `springload-core`，这里只做
//! 接线与错误文案。

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use springload_core::config::{Settings, SettingsPatch};
use springload_core::glossary::Glossary;
use springload_core::market::Market;
use tauri::State;

/// `app_info` 的返回：版本号、当前市场、数据源名称与最近同步时间。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    /// `cn` | `us`。
    pub market: String,
    /// 界面上的市场名。
    pub market_label: String,
    /// 计价货币名。
    pub currency: String,
    /// 是否当日买入次日才能卖出。
    pub t_plus_one: bool,
    pub data_source: String,
    pub last_sync_at: Option<String>,
}

/// 版本号、数据源名称与最近同步时间。
#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> Result<AppInfo, String> {
    state.app_info().map_err(|e| e.to_string())
}

/// 十个术语与四步文案。
#[tauri::command]
pub fn glossary(state: State<'_, AppState>) -> Result<Glossary, String> {
    Ok(state.glossary())
}

/// 读当前配置。
#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> Result<Settings, String> {
    state.settings_get().map_err(|e| e.to_string())
}

/// 改配置里的一部分字段，返回改完的完整配置。
#[tauri::command]
pub fn settings_update(
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> Result<Settings, String> {
    state.settings_update(&patch).map_err(|e| e.to_string())
}

/// 切换市场。切完整个应用的数据、规则与文案都换成新市场那一套。
#[tauri::command]
pub fn market_set(state: State<'_, AppState>, market: String) -> Result<Settings, String> {
    let target =
        Market::parse(&market).ok_or_else(|| format!("未知的市场 {market}，只接受 cn 或 us"))?;
    state.set_market(target).map_err(|e| e.to_string())
}

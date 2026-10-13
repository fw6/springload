//! 应用信息、术语与设置四个命令。领域规则在 `springload-core`，这里只做接线与错误文案。

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use springload_core::config::{Settings, SettingsPatch};
use springload_core::glossary::Glossary;
use tauri::State;

/// `app_info` 的返回：版本号、数据源名称与最近同步时间。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
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

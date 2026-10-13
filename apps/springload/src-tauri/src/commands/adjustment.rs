//! 复权维护命令：检查前复权历史是否过期、重算过期的股票。
//!
//! 领域规则在 `springload-core` 的 `adjustment`，这里只做数据库连接与错误文案。

use crate::state::AppState;
use chrono::Local;
use springload_core::adjustment::{self, HttpAdjustmentFetch, ResyncReport, StaleStock};
use tauri::State;

/// 检查已回补股票的前复权历史是否过期。
///
/// 逐只取服务端尾部与分红送配记录：判据 A 比较最近一次除权除息日与本地最后一条日线，
/// 分红送配接口不可用时退回判据 B 的尾部数值比对。
#[tauri::command]
pub async fn corporate_actions_stale(
    state: State<'_, AppState>,
) -> Result<Vec<StaleStock>, String> {
    let db = state.db().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let fetch = HttpAdjustmentFetch::new().map_err(|error| error.to_string())?;
        adjustment::detect_stale(&db, &fetch, adjustment::DEFAULT_CONCURRENCY)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

/// 重算给定股票的前复权历史，并记录除权除息事件。
#[tauri::command]
pub async fn resync_adjusted(
    state: State<'_, AppState>,
    codes: Vec<String>,
) -> Result<ResyncReport, String> {
    let db = state.db().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let fetch = HttpAdjustmentFetch::new().map_err(|error| error.to_string())?;
        adjustment::resync(&db, &fetch, &codes, Local::now().date_naive())
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

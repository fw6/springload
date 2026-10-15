//! 复权维护命令：检查前复权历史是否过期、重算过期的股票。
//!
//! 领域规则在 `springload-core` 的 `adjustment`，这里只做数据库连接与错误文案。

use crate::state::AppState;
use springload_core::adjustment::{self, ResyncReport, StaleStock};
use tauri::State;

/// 检查已回补股票的前复权历史是否过期。
///
/// 逐只取除权除息记录，比较最近一次已生效的除权除息日与本地最后一条日线。
#[tauri::command]
pub async fn corporate_actions_stale(
    state: State<'_, AppState>,
) -> Result<Vec<StaleStock>, String> {
    let db = state.db();
    let source = state.source();
    tauri::async_runtime::spawn_blocking(move || {
        adjustment::detect_stale(&db, source.as_ref(), adjustment::DEFAULT_CONCURRENCY)
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
    let db = state.db();
    let market = state.market();
    let source = state.source();
    tauri::async_runtime::spawn_blocking(move || {
        adjustment::resync(&db, source.as_ref(), &codes, market.local_now().date())
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

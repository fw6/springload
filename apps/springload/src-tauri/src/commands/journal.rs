//! 交易日志的四条命令：记录与统计、手动补录、导出 CSV、AI 复盘。
//!
//! 领域规则在 `springload_core::journal`，这里只做接线、文件写入与 AI 调用。

use crate::state::AppState;
use springload_core::ai::{self, AiResult, ChatClient, Explanation, Scope};
use springload_core::journal::{self, JournalView, ManualEntry};
use springload_core::storage::trade_log::{self, TradeLogEntry};
use springload_core::market::Market;
use springload_core::storage::Database;
use std::sync::Arc;
use tauri::State;

/// 复盘总结不针对某一只股票，缓存键里的股票代码留空。
const JOURNAL_CODE: &str = "";

/// 读记录表与统计摘要。费用模型开启时逐笔扣减后再统计。
#[tauri::command]
pub fn journal_get(state: State<'_, AppState>) -> Result<JournalView, String> {
    let settings = state.settings_get().map_err(|error| error.to_string())?;
    let market = state.market();
    let current = settings.current();
    journal::view(&state.db(), market, settings.fee_model_enabled, current.commission_rate)
        .map_err(|error| error.to_string())
}

/// 手动补录一笔历史交易。
#[tauri::command]
pub fn journal_add_manual(
    state: State<'_, AppState>,
    entry: ManualEntry,
) -> Result<TradeLogEntry, String> {
    let conn = state.db().conn().map_err(|error| error.to_string())?;
    journal::add_manual(&conn, &entry).map_err(|error| error.to_string())
}

/// 把全部记录导出成 CSV，写到给定路径。
#[tauri::command]
pub fn journal_export_csv(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let conn = state.db().conn().map_err(|error| error.to_string())?;
    let entries = trade_log::list(&conn).map_err(|error| error.to_string())?;
    let csv = journal::export_csv(&entries);
    std::fs::write(&path, csv).map_err(|error| format!("写入 {path} 失败: {error}"))
}

/// AI 复盘总结：描述已经发生的结果，不给判断。
///
/// 要等模型返回，可能十几秒，挪到线程池里跑，别占住主线程。
#[tauri::command]
pub async fn journal_review(state: State<'_, AppState>) -> Result<Explanation, String> {
    let db = state.db();
    let market = state.market();
    tauri::async_runtime::spawn_blocking(move || {
        journal_review_with(&db, market, ai::default_client)
    })
    .await
    .map_err(|error| error.to_string())?
}

/// `journal_review` 的接线主体：模型客户端由调用方给出，便于测试注入。
pub fn journal_review_with<F>(db: &Database, market: Market, client: F) -> Result<Explanation, String>
where
    F: FnOnce() -> AiResult<Arc<dyn ChatClient>>,
{
    ai::explain(db, client, JOURNAL_CODE, Scope::Journal, market).map_err(|error| error.to_string())
}

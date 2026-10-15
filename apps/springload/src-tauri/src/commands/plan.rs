//! 交易计划卡的五条命令。领域规则在 `springload_core::plan`，这里只做接线。

use crate::state::AppState;
use springload_core::plan::{self, PlanCard, PlanInput};
use springload_core::storage::trade_log::TradeLogEntry;
use springload_core::storage::trade_plan::{self, TradePlan};
use tauri::State;

/// 读一条计划的计划卡（含筛选结论、板块与减仓安排）。
#[tauri::command]
pub fn plan_card(state: State<'_, AppState>, id: String) -> Result<PlanCard, String> {
    let conn = state.db().conn().map_err(|error| error.to_string())?;
    plan::card(&conn, &id).map_err(|error| error.to_string())
}

/// 算仓位并存一条计划。新建时状态是草稿。
#[tauri::command]
pub fn plan_save(state: State<'_, AppState>, plan: PlanInput) -> Result<TradePlan, String> {
    let conn = state.db().conn().map_err(|error| error.to_string())?;
    plan::save(&conn, &plan, state.market()).map_err(|error| error.to_string())
}

/// 读一条计划。
#[tauri::command]
pub fn plan_get(state: State<'_, AppState>, id: String) -> Result<TradePlan, String> {
    let conn = state.db().conn().map_err(|error| error.to_string())?;
    trade_plan::get(&conn, &id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("找不到计划 {id}"))
}

/// 按状态列出计划；`status` 为空时列全部。
#[tauri::command]
pub fn plan_list(state: State<'_, AppState>, status: Option<String>) -> Result<Vec<TradePlan>, String> {
    let conn = state.db().conn().map_err(|error| error.to_string())?;
    trade_plan::list(&conn, status.as_deref()).map_err(|error| error.to_string())
}

/// 确认计划已执行：状态改成已执行，开一条持仓，并写入交易日志的入场记录。
#[tauri::command]
pub fn plan_confirm_executed(
    state: State<'_, AppState>,
    id: String,
    entry_date: String,
) -> Result<TradeLogEntry, String> {
    let mut conn = state.db().conn().map_err(|error| error.to_string())?;
    plan::confirm_executed(&mut conn, &id, &entry_date).map_err(|error| error.to_string())
}

/// 改一条计划的状态。
#[tauri::command]
pub fn plan_set_status(
    state: State<'_, AppState>,
    id: String,
    status: String,
) -> Result<TradePlan, String> {
    let conn = state.db().conn().map_err(|error| error.to_string())?;
    plan::set_status(&conn, &id, &status).map_err(|error| error.to_string())
}

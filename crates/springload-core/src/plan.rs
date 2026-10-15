//! 交易计划卡：把这一步的结果打包成一张可存、可执行、可导出的计划。
//!
//! 领域规则都在这里：算仓位、落 `trade_plan` 行、确认执行时写 `position` 与
//! `trade_log` 的入场记录。命令层只做参数转换与错误文案。

use crate::error::{CoreError, Result};
use crate::market::Market;
use crate::sizing::{self, ReducePlan, SizingInput};
use crate::storage::trade_log::TradeLogEntry;
use crate::storage::trade_plan::TradePlan;
use crate::storage::{now, position, stock, trade_log, trade_plan};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// 计划卡的状态取值。
pub const DRAFT: &str = "draft";
/// 计划卡的状态取值。
pub const TRIGGERED: &str = "triggered";
/// 计划卡的状态取值。
pub const EXECUTED: &str = "executed";
/// 计划卡的状态取值。
pub const ABANDONED: &str = "abandoned";

/// 全部状态取值，按界面上的显示顺序。
pub const STATUSES: [&str; 4] = [DRAFT, TRIGGERED, EXECUTED, ABANDONED];

/// `plan_save` 的入参。股数、投入金额与目标位由服务端按 [`SizingInput`] 算出。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanInput {
    /// 已有计划的 id；为空时新建。
    pub id: Option<String>,
    pub code: String,
    pub entry: f64,
    pub stop: f64,
    pub account: f64,
    pub risk_pct: f64,
    pub zone_id: Option<String>,
    pub signal_id: Option<String>,
}

/// 计划卡上要展示的全部字段。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanCard {
    pub plan: TradePlan,
    pub name: Option<String>,
    pub board: Option<String>,
    pub sector: Option<String>,
    /// 四条判定是否全中；这只股票不在观察名单里时为 `None`。
    pub c1: Option<bool>,
    pub c2: Option<bool>,
    pub c3: Option<bool>,
    pub c4: Option<bool>,
    /// 减仓安排：3R 与 5R 各卖多少、剩余多少。
    pub reduce: ReducePlan,
    /// 按取整后的股数算出的实际风险占账户比例。
    pub actual_risk_pct: f64,
}

/// 按入参算仓位并写一条计划。新建时状态是草稿，改已有计划时保留原状态。
pub fn save(conn: &Connection, input: &PlanInput, market: Market) -> Result<TradePlan> {
    let sizing_input = SizingInput {
        entry: input.entry,
        stop: input.stop,
        account: input.account,
        risk_pct: input.risk_pct,
    };
    let result = sizing::size(&sizing_input, market)?;

    let existing = match &input.id {
        Some(id) if !id.is_empty() => trade_plan::get(conn, id)?,
        _ => None,
    };
    let timestamp = now();
    let plan = TradePlan {
        id: existing
            .as_ref()
            .map(|plan| plan.id.clone())
            .unwrap_or_else(|| new_id(&input.code)),
        code: input.code.clone(),
        entry: input.entry,
        stop: input.stop,
        account: input.account,
        risk_pct: input.risk_pct,
        per_r: result.per_r,
        shares: result.shares,
        capital: result.capital,
        pos_pct: result.pos_pct,
        stop_distance: result.stop_distance,
        t3: result.t3,
        t5: result.t5,
        status: existing
            .as_ref()
            .map(|plan| plan.status.clone())
            .unwrap_or_else(|| DRAFT.to_string()),
        zone_id: input.zone_id.clone(),
        signal_id: input.signal_id.clone(),
        created_at: existing
            .as_ref()
            .map(|plan| plan.created_at.clone())
            .unwrap_or_else(|| timestamp.clone()),
        updated_at: timestamp,
    };
    trade_plan::upsert(conn, &plan)?;
    Ok(plan)
}

/// 读一条计划，连带把计划卡需要的其余字段拼出来。
pub fn card(conn: &Connection, id: &str) -> Result<PlanCard> {
    let plan = trade_plan::get(conn, id)?
        .ok_or_else(|| CoreError::InvalidData(format!("找不到计划 {id}")))?;
    let stock_row = stock::get(conn, &plan.code)?;
    let watchlist_row = crate::storage::watchlist::get(conn, &plan.code)?;
    let actual_risk_pct = if plan.account == 0.0 {
        0.0
    } else {
        plan.shares as f64 * plan.per_r / plan.account
    };
    Ok(PlanCard {
        name: stock_row.as_ref().map(|row| row.name.clone()),
        board: stock_row.as_ref().map(|row| row.board.clone()),
        sector: stock_row.as_ref().and_then(|row| row.sector.clone()),
        c1: watchlist_row.as_ref().map(|row| row.c1),
        c2: watchlist_row.as_ref().map(|row| row.c2),
        c3: watchlist_row.as_ref().map(|row| row.c3),
        c4: watchlist_row.as_ref().map(|row| row.c4),
        reduce: sizing::reduce_shares(plan.shares),
        actual_risk_pct,
        plan,
    })
}

/// 改一条计划的状态。
pub fn set_status(conn: &Connection, id: &str, status: &str) -> Result<TradePlan> {
    if !STATUSES.contains(&status) {
        return Err(CoreError::InvalidData(format!("未知的计划状态 {status}")));
    }
    let changed = trade_plan::set_status(conn, id, status, &now())?;
    if changed == 0 {
        return Err(CoreError::InvalidData(format!("找不到计划 {id}")));
    }
    trade_plan::get(conn, id)?
        .ok_or_else(|| CoreError::InvalidData(format!("找不到计划 {id}")))
}

/// 确认计划已执行：状态改成已执行，开一条持仓，并写入交易日志的入场记录。
///
/// id 由计划 id 推出，重复确认只覆盖同一行，不会写出两条入场记录。
pub fn confirm_executed(conn: &mut Connection, id: &str, entry_date: &str) -> Result<TradeLogEntry> {
    let plan = trade_plan::get(conn, id)?
        .ok_or_else(|| CoreError::InvalidData(format!("找不到计划 {id}")))?;
    if plan.shares <= 0 {
        return Err(CoreError::InvalidData(
            "股数为 0 的计划不能确认执行，请先调整止损位或风险比例".to_string(),
        ));
    }
    let timestamp = now();
    let position_id = format!("position-{}", plan.id);

    let existing_position = position::get(conn, &position_id)?;
    let position_row = position::Position {
        id: position_id.clone(),
        plan_id: plan.id.clone(),
        code: plan.code.clone(),
        entry_date: entry_date.to_string(),
        entry_price: plan.entry,
        quantity: plan.shares,
        current_stop: plan.stop,
        follow_ema: true,
        exit_price: existing_position.as_ref().and_then(|row| row.exit_price),
        exit_date: existing_position.as_ref().and_then(|row| row.exit_date.clone()),
        created_at: existing_position
            .as_ref()
            .map(|row| row.created_at.clone())
            .unwrap_or_else(|| timestamp.clone()),
        updated_at: timestamp.clone(),
    };
    position::upsert(conn, &position_row)?;

    let entry = TradeLogEntry {
        id: format!("log-{}", plan.id),
        position_id: Some(position_id),
        code: plan.code.clone(),
        name: stock::get(conn, &plan.code)?.map(|row| row.name),
        entry_date: entry_date.to_string(),
        entry_price: plan.entry,
        stop_price: plan.stop,
        exit_date: existing_position.as_ref().and_then(|row| row.exit_date.clone()),
        exit_price: existing_position.as_ref().and_then(|row| row.exit_price),
        quantity: plan.shares,
        per_r: Some(plan.per_r),
        r_multiple: None,
        pnl: None,
        fees: 0.0,
        note: None,
        source: "plan".to_string(),
        created_at: timestamp.clone(),
    };
    trade_log::upsert(conn, &entry)?;
    trade_plan::set_status(conn, &plan.id, EXECUTED, &timestamp)?;
    Ok(entry)
}

/// 新计划的 id：代码 + 毫秒时间戳 + 进程内自增序号。
///
/// 序号让同一毫秒内连存两条计划也不会撞上同一个 id。
fn new_id(code: &str) -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    format!("plan-{code}-{}-{seq}", chrono::Utc::now().timestamp_millis())
}

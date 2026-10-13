//! `trade_plan` 表的读写：交易计划。`status` 取 `draft` | `triggered` | `executed` | `abandoned`。

use crate::error::Result;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

/// 一条交易计划。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradePlan {
    pub id: String,
    pub code: String,
    pub entry: f64,
    pub stop: f64,
    pub account: f64,
    pub risk_pct: f64,
    pub per_r: f64,
    pub shares: i64,
    pub capital: f64,
    pub pos_pct: f64,
    pub stop_distance: f64,
    pub t3: f64,
    pub t5: f64,
    pub status: String,
    pub zone_id: Option<String>,
    pub signal_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl TradePlan {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            code: row.get("code")?,
            entry: row.get("entry")?,
            stop: row.get("stop")?,
            account: row.get("account")?,
            risk_pct: row.get("risk_pct")?,
            per_r: row.get("per_r")?,
            shares: row.get("shares")?,
            capital: row.get("capital")?,
            pos_pct: row.get("pos_pct")?,
            stop_distance: row.get("stop_distance")?,
            t3: row.get("t3")?,
            t5: row.get("t5")?,
            status: row.get("status")?,
            zone_id: row.get("zone_id")?,
            signal_id: row.get("signal_id")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }
}

/// 写入或更新一条交易计划。
pub fn upsert(conn: &Connection, plan: &TradePlan) -> Result<()> {
    conn.execute(
        "INSERT INTO trade_plan
           (id, code, entry, stop, account, risk_pct, per_r, shares, capital, pos_pct,
            stop_distance, t3, t5, status, zone_id, signal_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
         ON CONFLICT(id) DO UPDATE SET
           code = excluded.code,
           entry = excluded.entry,
           stop = excluded.stop,
           account = excluded.account,
           risk_pct = excluded.risk_pct,
           per_r = excluded.per_r,
           shares = excluded.shares,
           capital = excluded.capital,
           pos_pct = excluded.pos_pct,
           stop_distance = excluded.stop_distance,
           t3 = excluded.t3,
           t5 = excluded.t5,
           status = excluded.status,
           zone_id = excluded.zone_id,
           signal_id = excluded.signal_id,
           created_at = excluded.created_at,
           updated_at = excluded.updated_at",
        params![
            plan.id,
            plan.code,
            plan.entry,
            plan.stop,
            plan.account,
            plan.risk_pct,
            plan.per_r,
            plan.shares,
            plan.capital,
            plan.pos_pct,
            plan.stop_distance,
            plan.t3,
            plan.t5,
            plan.status,
            plan.zone_id,
            plan.signal_id,
            plan.created_at,
            plan.updated_at,
        ],
    )?;
    Ok(())
}

/// 读一条交易计划。
pub fn get(conn: &Connection, id: &str) -> Result<Option<TradePlan>> {
    let plan = conn
        .query_row("SELECT * FROM trade_plan WHERE id = ?1", params![id], TradePlan::from_row)
        .optional()?;
    Ok(plan)
}

/// 列出交易计划，可按状态过滤；按创建时间倒序。
pub fn list(conn: &Connection, status: Option<&str>) -> Result<Vec<TradePlan>> {
    let mut out = Vec::new();
    match status {
        Some(status) => {
            let mut stmt = conn.prepare(
                "SELECT * FROM trade_plan WHERE status = ?1 ORDER BY created_at DESC, id DESC",
            )?;
            let rows = stmt.query_map(params![status], TradePlan::from_row)?;
            for row in rows {
                out.push(row?);
            }
        }
        None => {
            let mut stmt =
                conn.prepare("SELECT * FROM trade_plan ORDER BY created_at DESC, id DESC")?;
            let rows = stmt.query_map([], TradePlan::from_row)?;
            for row in rows {
                out.push(row?);
            }
        }
    }
    Ok(out)
}

/// 改一条交易计划的状态。
pub fn set_status(conn: &Connection, id: &str, status: &str, updated_at: &str) -> Result<usize> {
    let n = conn.execute(
        "UPDATE trade_plan SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![status, updated_at, id],
    )?;
    Ok(n)
}

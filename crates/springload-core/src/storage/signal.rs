//! `signal` 表的读写：买入信号记录。

use crate::error::Result;
use rusqlite::{params, Connection, OptionalExtension, Row};

/// 一条买入信号。`status` 取 `waiting` | `triggered` | `failed`。
#[derive(Debug, Clone, PartialEq)]
pub struct Signal {
    pub id: String,
    pub code: String,
    pub triggered_at: String,
    /// `1m` | `5m` | `15m` | `30m` | `60m`。
    pub timeframe: String,
    pub candle_high_broken: f64,
    pub pre_candle_high: f64,
    pub zone_id: Option<String>,
    pub status: String,
    pub created_at: String,
}

impl Signal {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            code: row.get("code")?,
            triggered_at: row.get("triggered_at")?,
            timeframe: row.get("timeframe")?,
            candle_high_broken: row.get("candle_high_broken")?,
            pre_candle_high: row.get("pre_candle_high")?,
            zone_id: row.get("zone_id")?,
            status: row.get("status")?,
            created_at: row.get("created_at")?,
        })
    }
}

/// 写入或更新一条信号。
pub fn upsert(conn: &Connection, signal: &Signal) -> Result<()> {
    conn.execute(
        "INSERT INTO signal
           (id, code, triggered_at, timeframe, candle_high_broken, pre_candle_high, zone_id, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET
           code = excluded.code,
           triggered_at = excluded.triggered_at,
           timeframe = excluded.timeframe,
           candle_high_broken = excluded.candle_high_broken,
           pre_candle_high = excluded.pre_candle_high,
           zone_id = excluded.zone_id,
           status = excluded.status,
           created_at = excluded.created_at",
        params![
            signal.id,
            signal.code,
            signal.triggered_at,
            signal.timeframe,
            signal.candle_high_broken,
            signal.pre_candle_high,
            signal.zone_id,
            signal.status,
            signal.created_at,
        ],
    )?;
    Ok(())
}

/// 读一条信号。
pub fn get(conn: &Connection, id: &str) -> Result<Option<Signal>> {
    let signal = conn
        .query_row("SELECT * FROM signal WHERE id = ?1", params![id], Signal::from_row)
        .optional()?;
    Ok(signal)
}

/// 按触发时间列出某只股票的信号。
pub fn list_for_code(conn: &Connection, code: &str) -> Result<Vec<Signal>> {
    let mut stmt =
        conn.prepare("SELECT * FROM signal WHERE code = ?1 ORDER BY triggered_at, id")?;
    let rows = stmt.query_map(params![code], Signal::from_row)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// 某只股票最近一条信号。
pub fn latest_for_code(conn: &Connection, code: &str) -> Result<Option<Signal>> {
    let signal = conn
        .query_row(
            "SELECT * FROM signal WHERE code = ?1 ORDER BY triggered_at DESC, id DESC LIMIT 1",
            params![code],
            Signal::from_row,
        )
        .optional()?;
    Ok(signal)
}

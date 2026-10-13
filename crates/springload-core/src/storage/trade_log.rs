//! `trade_log` 表的读写。`exit_date` 为空表示入场记录已写入、尚未出场。

use crate::error::Result;
use rusqlite::{params, Connection, OptionalExtension, Row};

/// 一条交易记录。`source` 取 `plan` | `manual`。
#[derive(Debug, Clone, PartialEq)]
pub struct TradeLogEntry {
    pub id: String,
    pub position_id: Option<String>,
    pub code: String,
    pub name: Option<String>,
    pub entry_date: String,
    pub entry_price: f64,
    pub stop_price: f64,
    pub exit_date: Option<String>,
    pub exit_price: Option<f64>,
    pub quantity: i64,
    pub per_r: Option<f64>,
    pub r_multiple: Option<f64>,
    pub pnl: Option<f64>,
    pub fees: f64,
    pub note: Option<String>,
    pub source: String,
    pub created_at: String,
}

impl TradeLogEntry {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            position_id: row.get("position_id")?,
            code: row.get("code")?,
            name: row.get("name")?,
            entry_date: row.get("entry_date")?,
            entry_price: row.get("entry_price")?,
            stop_price: row.get("stop_price")?,
            exit_date: row.get("exit_date")?,
            exit_price: row.get("exit_price")?,
            quantity: row.get("quantity")?,
            per_r: row.get("per_r")?,
            r_multiple: row.get("r_multiple")?,
            pnl: row.get("pnl")?,
            fees: row.get("fees")?,
            note: row.get("note")?,
            source: row.get("source")?,
            created_at: row.get("created_at")?,
        })
    }
}

/// 写入或更新一条交易记录。
pub fn upsert(conn: &Connection, entry: &TradeLogEntry) -> Result<()> {
    conn.execute(
        "INSERT INTO trade_log
           (id, position_id, code, name, entry_date, entry_price, stop_price,
            exit_date, exit_price, quantity, per_r, r_multiple, pnl, fees, note, source, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
         ON CONFLICT(id) DO UPDATE SET
           position_id = excluded.position_id,
           code = excluded.code,
           name = excluded.name,
           entry_date = excluded.entry_date,
           entry_price = excluded.entry_price,
           stop_price = excluded.stop_price,
           exit_date = excluded.exit_date,
           exit_price = excluded.exit_price,
           quantity = excluded.quantity,
           per_r = excluded.per_r,
           r_multiple = excluded.r_multiple,
           pnl = excluded.pnl,
           fees = excluded.fees,
           note = excluded.note,
           source = excluded.source,
           created_at = excluded.created_at",
        params![
            entry.id,
            entry.position_id,
            entry.code,
            entry.name,
            entry.entry_date,
            entry.entry_price,
            entry.stop_price,
            entry.exit_date,
            entry.exit_price,
            entry.quantity,
            entry.per_r,
            entry.r_multiple,
            entry.pnl,
            entry.fees,
            entry.note,
            entry.source,
            entry.created_at,
        ],
    )?;
    Ok(())
}

/// 读一条交易记录。
pub fn get(conn: &Connection, id: &str) -> Result<Option<TradeLogEntry>> {
    let entry = conn
        .query_row("SELECT * FROM trade_log WHERE id = ?1", params![id], TradeLogEntry::from_row)
        .optional()?;
    Ok(entry)
}

/// 列出全部交易记录，按入场日倒序。
pub fn list(conn: &Connection) -> Result<Vec<TradeLogEntry>> {
    let mut stmt = conn.prepare("SELECT * FROM trade_log ORDER BY entry_date DESC, id DESC")?;
    let rows = stmt.query_map([], TradeLogEntry::from_row)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// 列出已出场的交易记录（`exit_date` 非空），统计只计这些行。
pub fn list_closed(conn: &Connection) -> Result<Vec<TradeLogEntry>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM trade_log WHERE exit_date IS NOT NULL ORDER BY entry_date DESC, id DESC",
    )?;
    let rows = stmt.query_map([], TradeLogEntry::from_row)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

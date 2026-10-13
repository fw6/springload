//! `position` 表的读写：持仓与跟踪。

use crate::error::Result;
use rusqlite::{params, Connection, OptionalExtension, Row};

/// 一条持仓。
#[derive(Debug, Clone, PartialEq)]
pub struct Position {
    pub id: String,
    pub plan_id: String,
    pub code: String,
    pub entry_date: String,
    pub entry_price: f64,
    pub quantity: i64,
    pub current_stop: f64,
    /// 剩余仓位是否跟随 9 日均线。
    pub follow_ema: bool,
    pub exit_price: Option<f64>,
    pub exit_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Position {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            plan_id: row.get("plan_id")?,
            code: row.get("code")?,
            entry_date: row.get("entry_date")?,
            entry_price: row.get("entry_price")?,
            quantity: row.get("quantity")?,
            current_stop: row.get("current_stop")?,
            follow_ema: row.get::<_, i64>("follow_ema")? != 0,
            exit_price: row.get("exit_price")?,
            exit_date: row.get("exit_date")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }
}

/// 写入或更新一条持仓。
pub fn upsert(conn: &Connection, position: &Position) -> Result<()> {
    conn.execute(
        "INSERT INTO position
           (id, plan_id, code, entry_date, entry_price, quantity, current_stop, follow_ema,
            exit_price, exit_date, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(id) DO UPDATE SET
           plan_id = excluded.plan_id,
           code = excluded.code,
           entry_date = excluded.entry_date,
           entry_price = excluded.entry_price,
           quantity = excluded.quantity,
           current_stop = excluded.current_stop,
           follow_ema = excluded.follow_ema,
           exit_price = excluded.exit_price,
           exit_date = excluded.exit_date,
           created_at = excluded.created_at,
           updated_at = excluded.updated_at",
        params![
            position.id,
            position.plan_id,
            position.code,
            position.entry_date,
            position.entry_price,
            position.quantity,
            position.current_stop,
            position.follow_ema as i64,
            position.exit_price,
            position.exit_date,
            position.created_at,
            position.updated_at,
        ],
    )?;
    Ok(())
}

/// 读一条持仓。
pub fn get(conn: &Connection, id: &str) -> Result<Option<Position>> {
    let position = conn
        .query_row("SELECT * FROM position WHERE id = ?1", params![id], Position::from_row)
        .optional()?;
    Ok(position)
}

/// 列出全部持仓，按入场日倒序。
pub fn list(conn: &Connection) -> Result<Vec<Position>> {
    let mut stmt = conn.prepare("SELECT * FROM position ORDER BY entry_date DESC, id DESC")?;
    let rows = stmt.query_map([], Position::from_row)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// 列出尚未出场的持仓。
pub fn list_open(conn: &Connection) -> Result<Vec<Position>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM position WHERE exit_date IS NULL ORDER BY entry_date DESC, id DESC",
    )?;
    let rows = stmt.query_map([], Position::from_row)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// 记录一条持仓的出场价与出场日。
pub fn close(conn: &Connection, id: &str, exit_price: f64, exit_date: &str, updated_at: &str) -> Result<usize> {
    let n = conn.execute(
        "UPDATE position SET exit_price = ?1, exit_date = ?2, updated_at = ?3 WHERE id = ?4",
        params![exit_price, exit_date, updated_at, id],
    )?;
    Ok(n)
}

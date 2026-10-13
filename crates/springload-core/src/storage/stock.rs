//! `stock` 表的读写。

use crate::error::Result;
use rusqlite::{params, Connection, OptionalExtension, Row};

/// 标的主表的一行。
#[derive(Debug, Clone, PartialEq)]
pub struct Stock {
    /// 六位代码。
    pub code: String,
    pub name: String,
    /// `SH` | `SZ`。
    pub market: String,
    /// `main` | `star` | `chinext` | `bse`。
    pub board: String,
    pub sector: Option<String>,
    /// 上市日 YYYY-MM-DD。
    pub listed_at: Option<String>,
    pub is_st: bool,
    pub updated_at: String,
}

impl Stock {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            code: row.get("code")?,
            name: row.get("name")?,
            market: row.get("market")?,
            board: row.get("board")?,
            sector: row.get("sector")?,
            listed_at: row.get("listed_at")?,
            is_st: row.get::<_, i64>("is_st")? != 0,
            updated_at: row.get("updated_at")?,
        })
    }
}

/// 写入或更新一只股票。
pub fn upsert(conn: &Connection, stock: &Stock) -> Result<()> {
    conn.execute(
        "INSERT INTO stock (code, name, market, board, sector, listed_at, is_st, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(code) DO UPDATE SET
           name = excluded.name,
           market = excluded.market,
           board = excluded.board,
           sector = excluded.sector,
           listed_at = excluded.listed_at,
           is_st = excluded.is_st,
           updated_at = excluded.updated_at",
        params![
            stock.code,
            stock.name,
            stock.market,
            stock.board,
            stock.sector,
            stock.listed_at,
            stock.is_st as i64,
            stock.updated_at,
        ],
    )?;
    Ok(())
}

/// 读一只股票。
pub fn get(conn: &Connection, code: &str) -> Result<Option<Stock>> {
    let stock = conn
        .query_row("SELECT * FROM stock WHERE code = ?1", params![code], Stock::from_row)
        .optional()?;
    Ok(stock)
}

/// 按代码升序列出全部股票。
pub fn list(conn: &Connection) -> Result<Vec<Stock>> {
    let mut stmt = conn.prepare("SELECT * FROM stock ORDER BY code")?;
    let rows = stmt.query_map([], Stock::from_row)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// 股票总数。
pub fn count(conn: &Connection) -> Result<i64> {
    let n = conn.query_row("SELECT COUNT(*) FROM stock", [], |row| row.get(0))?;
    Ok(n)
}

/// 清空整张表，返回删除的行数。
pub fn delete_all(conn: &Connection) -> Result<usize> {
    let n = conn.execute("DELETE FROM stock", [])?;
    Ok(n)
}

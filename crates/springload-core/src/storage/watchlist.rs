//! `watchlist_item` 表的读写：观察名单与筛选判定结果。

use crate::error::Result;
use rusqlite::{params, Connection, OptionalExtension, Row};

/// 观察名单的一行。
#[derive(Debug, Clone, PartialEq)]
pub struct WatchlistItem {
    pub code: String,
    pub added_at: String,
    /// C1~C4 四条判定。
    pub c1: bool,
    pub c2: bool,
    pub c3: bool,
    pub c4: bool,
    pub chg_1m: Option<f64>,
    pub chg_3m: Option<f64>,
    pub chg_6m: Option<f64>,
    /// 近 1 个月涨幅的全市场排名分位。
    pub rank_pct: Option<f64>,
    /// 日均成交额（元）。
    pub avg_amount: Option<f64>,
    /// 近 20 日极差。
    pub convergence: Option<f64>,
    /// 这次筛选的运行时间。
    pub screen_run_at: String,
}

impl WatchlistItem {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            code: row.get("code")?,
            added_at: row.get("added_at")?,
            c1: row.get::<_, i64>("c1")? != 0,
            c2: row.get::<_, i64>("c2")? != 0,
            c3: row.get::<_, i64>("c3")? != 0,
            c4: row.get::<_, i64>("c4")? != 0,
            chg_1m: row.get("chg_1m")?,
            chg_3m: row.get("chg_3m")?,
            chg_6m: row.get("chg_6m")?,
            rank_pct: row.get("rank_pct")?,
            avg_amount: row.get("avg_amount")?,
            convergence: row.get("convergence")?,
            screen_run_at: row.get("screen_run_at")?,
        })
    }
}

/// 写入或更新一条观察名单记录。
pub fn upsert(conn: &Connection, item: &WatchlistItem) -> Result<()> {
    conn.execute(
        "INSERT INTO watchlist_item
           (code, added_at, c1, c2, c3, c4, chg_1m, chg_3m, chg_6m, rank_pct, avg_amount, convergence, screen_run_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(code) DO UPDATE SET
           added_at = excluded.added_at,
           c1 = excluded.c1,
           c2 = excluded.c2,
           c3 = excluded.c3,
           c4 = excluded.c4,
           chg_1m = excluded.chg_1m,
           chg_3m = excluded.chg_3m,
           chg_6m = excluded.chg_6m,
           rank_pct = excluded.rank_pct,
           avg_amount = excluded.avg_amount,
           convergence = excluded.convergence,
           screen_run_at = excluded.screen_run_at",
        params![
            item.code,
            item.added_at,
            item.c1 as i64,
            item.c2 as i64,
            item.c3 as i64,
            item.c4 as i64,
            item.chg_1m,
            item.chg_3m,
            item.chg_6m,
            item.rank_pct,
            item.avg_amount,
            item.convergence,
            item.screen_run_at,
        ],
    )?;
    Ok(())
}

/// 读一条观察名单记录。
pub fn get(conn: &Connection, code: &str) -> Result<Option<WatchlistItem>> {
    let item = conn
        .query_row(
            "SELECT * FROM watchlist_item WHERE code = ?1",
            params![code],
            WatchlistItem::from_row,
        )
        .optional()?;
    Ok(item)
}

/// 按加入时间、代码升序列出观察名单。
pub fn list(conn: &Connection) -> Result<Vec<WatchlistItem>> {
    let mut stmt = conn.prepare("SELECT * FROM watchlist_item ORDER BY added_at, code")?;
    let rows = stmt.query_map([], WatchlistItem::from_row)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// 移除一条观察名单记录。
pub fn remove(conn: &Connection, code: &str) -> Result<usize> {
    let n = conn.execute("DELETE FROM watchlist_item WHERE code = ?1", params![code])?;
    Ok(n)
}

/// 清空整张表。
pub fn delete_all(conn: &Connection) -> Result<usize> {
    let n = conn.execute("DELETE FROM watchlist_item", [])?;
    Ok(n)
}

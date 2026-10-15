//! `bar` 表（前复权日线）的读写。

use crate::error::Result;
use rusqlite::{params, Connection, OptionalExtension, Row};

/// 一根前复权日线。
///
/// `volume` 为 0 表示当天没有交易。存储层保留原始数值，指标计算负责跳过。
#[derive(Debug, Clone, PartialEq)]
pub struct BarRow {
    pub code: String,
    /// YYYY-MM-DD。
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    /// 股。
    pub volume: f64,
    /// 元。
    pub amount: f64,
}

impl BarRow {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            code: row.get("code")?,
            date: row.get("date")?,
            open: row.get("open")?,
            high: row.get("high")?,
            low: row.get("low")?,
            close: row.get("close")?,
            volume: row.get("volume")?,
            amount: row.get("amount")?,
        })
    }
}

/// 批量写入或更新日线，返回写入的行数。
pub fn upsert_many(conn: &mut Connection, bars: &[BarRow]) -> Result<usize> {
    let tx = conn.transaction()?;
    let mut written = 0usize;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO bar (code, date, open, high, low, close, volume, amount)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(code, date) DO UPDATE SET
               open = excluded.open,
               high = excluded.high,
               low = excluded.low,
               close = excluded.close,
               volume = excluded.volume,
               amount = excluded.amount",
        )?;
        for bar in bars {
            stmt.execute(params![
                bar.code, bar.date, bar.open, bar.high, bar.low, bar.close, bar.volume, bar.amount,
            ])?;
            written += 1;
        }
    }
    tx.commit()?;
    Ok(written)
}

/// 读一只股票在 `[from, to]` 闭区间内的日线，按日期升序。
pub fn range(conn: &Connection, code: &str, from: &str, to: &str) -> Result<Vec<BarRow>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM bar WHERE code = ?1 AND date >= ?2 AND date <= ?3 ORDER BY date",
    )?;
    let rows = stmt.query_map(params![code, from, to], BarRow::from_row)?;
    collect(rows)
}

/// 读一只股票的全部日线，按日期升序。
pub fn list_for_code(conn: &Connection, code: &str) -> Result<Vec<BarRow>> {
    let mut stmt = conn.prepare("SELECT * FROM bar WHERE code = ?1 ORDER BY date")?;
    let rows = stmt.query_map(params![code], BarRow::from_row)?;
    collect(rows)
}

/// 一只股票最近的交易日。
pub fn latest_date(conn: &Connection, code: &str) -> Result<Option<String>> {
    let date = conn
        .query_row(
            "SELECT MAX(date) FROM bar WHERE code = ?1",
            params![code],
            |row| row.get::<_, Option<String>>(0),
        )
        .unwrap_or(None);
    Ok(date)
}

/// 某一天的前复权收盘价；那天没有记录时返回 `None`。
pub fn close_at(conn: &Connection, code: &str, date: &str) -> Result<Option<f64>> {
    let close = conn
        .query_row(
            "SELECT close FROM bar WHERE code = ?1 AND date = ?2",
            params![code, date],
            |row| row.get::<_, f64>(0),
        )
        .optional()?;
    Ok(close)
}

/// 全部日线行数。
pub fn count(conn: &Connection) -> Result<i64> {
    let n = conn.query_row("SELECT COUNT(*) FROM bar", [], |row| row.get(0))?;
    Ok(n)
}

/// 删除一只股票的全部日线，返回删除的行数。
pub fn delete_for_code(conn: &Connection, code: &str) -> Result<usize> {
    let n = conn.execute("DELETE FROM bar WHERE code = ?1", params![code])?;
    Ok(n)
}

/// 交易日历：库里所有股票出现过的交易日，按日期升序。
///
/// 停牌日的记录本来就是缺的，所以这份日历是「市场开过市的日子」的近似。
pub fn trading_days(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT DISTINCT date FROM bar ORDER BY date")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn collect(rows: rusqlite::MappedRows<'_, impl FnMut(&Row<'_>) -> rusqlite::Result<BarRow>>) -> Result<Vec<BarRow>> {
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

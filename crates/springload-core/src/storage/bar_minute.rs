//! `bar_minute` 表的读写：分钟线。
//!
//! 分钟线由 #17 写入：抓取时按 `(code, period, ts)` 覆盖同一根，重复拉取不会产生重复行。
//! `bars_get`（#12）按 `period` 从这里读取。

use crate::error::Result;
use crate::market::MinuteBar;
use rusqlite::{params, Connection};

/// 批量写入或更新分钟线，返回写入的行数。
pub fn upsert_many(conn: &mut Connection, bars: &[MinuteBar]) -> Result<usize> {
    let tx = conn.transaction()?;
    let mut written = 0usize;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO bar_minute (code, ts, period, open, high, low, close, volume, amount)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(code, period, ts) DO UPDATE SET
               open = excluded.open,
               high = excluded.high,
               low = excluded.low,
               close = excluded.close,
               volume = excluded.volume,
               amount = excluded.amount",
        )?;
        for bar in bars {
            stmt.execute(params![
                bar.code,
                bar.ts,
                bar.period,
                bar.open,
                bar.high,
                bar.low,
                bar.close,
                bar.volume,
                bar.amount,
            ])?;
            written += 1;
        }
    }
    tx.commit()?;
    Ok(written)
}

/// 读一只股票某个周期的全部分钟线，按时间升序。
pub fn list(conn: &Connection, code: &str, period: u32) -> Result<Vec<MinuteBar>> {
    let mut stmt = conn.prepare(
        "SELECT code, ts, period, open, high, low, close, volume, amount
         FROM bar_minute WHERE code = ?1 AND period = ?2 ORDER BY ts",
    )?;
    let rows = stmt.query_map(params![code, period], |row| {
        Ok(MinuteBar {
            code: row.get(0)?,
            ts: row.get(1)?,
            period: row.get(2)?,
            open: row.get(3)?,
            high: row.get(4)?,
            low: row.get(5)?,
            close: row.get(6)?,
            volume: row.get(7)?,
            amount: row.get(8)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// 一只股票某个周期最近一根分钟线的时间，`YYYY-MM-DD HH:MM`。
pub fn latest_ts(conn: &Connection, code: &str, period: u32) -> Result<Option<String>> {
    let ts = conn.query_row(
        "SELECT MAX(ts) FROM bar_minute WHERE code = ?1 AND period = ?2",
        params![code, period],
        |row| row.get::<_, Option<String>>(0),
    )?;
    Ok(ts)
}

/// 删除一只股票某个周期的全部分钟线，返回删除的行数。
pub fn delete_for_code(conn: &Connection, code: &str, period: u32) -> Result<usize> {
    let n = conn.execute(
        "DELETE FROM bar_minute WHERE code = ?1 AND period = ?2",
        params![code, period],
    )?;
    Ok(n)
}

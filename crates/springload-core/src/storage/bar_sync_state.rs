//! `bar_sync_state` 表的读写：每只股票的日线回补进度。

use crate::error::Result;
use rusqlite::{params, Connection, OptionalExtension, Row};

/// 一只股票的回补状态。
#[derive(Debug, Clone, PartialEq)]
pub struct BarSyncState {
    pub code: String,
    /// 已回补到的最后交易日。
    pub last_bar_date: Option<String>,
    /// 上次抓取时间。
    pub fetched_at: Option<String>,
    pub updated_at: String,
}

impl BarSyncState {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            code: row.get("code")?,
            last_bar_date: row.get("last_bar_date")?,
            fetched_at: row.get("fetched_at")?,
            updated_at: row.get("updated_at")?,
        })
    }
}

/// 写入或更新一只股票的回补状态。
pub fn upsert(conn: &Connection, state: &BarSyncState) -> Result<()> {
    conn.execute(
        "INSERT INTO bar_sync_state (code, last_bar_date, fetched_at, updated_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(code) DO UPDATE SET
           last_bar_date = excluded.last_bar_date,
           fetched_at = excluded.fetched_at,
           updated_at = excluded.updated_at",
        params![
            state.code,
            state.last_bar_date,
            state.fetched_at,
            state.updated_at,
        ],
    )?;
    Ok(())
}

/// 读一只股票的回补状态。
pub fn get(conn: &Connection, code: &str) -> Result<Option<BarSyncState>> {
    let state = conn
        .query_row(
            "SELECT * FROM bar_sync_state WHERE code = ?1",
            params![code],
            BarSyncState::from_row,
        )
        .optional()?;
    Ok(state)
}

/// 最近一次抓取时间：全部回补状态里最大的 `fetched_at`。
pub fn latest_fetched_at(conn: &Connection) -> Result<Option<String>> {
    let at = conn
        .query_row("SELECT MAX(fetched_at) FROM bar_sync_state", [], |row| {
            row.get::<_, Option<String>>(0)
        })
        .unwrap_or(None);
    Ok(at)
}

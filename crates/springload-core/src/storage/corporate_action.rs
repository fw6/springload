//! `corporate_action` 表的读写：每只股票的除权除息事件。
//!
//! 一行表示一次除权除息。`dividend_per_share` 是每股现金红利 D，
//! `split_ratio` 是每股送转比例 r，`pre_close` 是除权日的前收盘价，也就是
//! 除权（息）参考价 `(前收盘价 - D) / (1 + r)`。

use crate::error::Result;
use rusqlite::{params, Connection, OptionalExtension, Row};

/// 一次除权除息事件。
#[derive(Debug, Clone, PartialEq)]
pub struct CorporateAction {
    pub code: String,
    /// 除权除息日 YYYY-MM-DD。
    pub ex_date: String,
    /// 每股现金分红 D。
    pub dividend_per_share: f64,
    /// 每股送转比例 r。
    pub split_ratio: f64,
    /// 除权日的前收盘价，即除权（息）参考价。
    pub pre_close: Option<f64>,
    pub recorded_at: String,
}

impl CorporateAction {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            code: row.get("code")?,
            ex_date: row.get("ex_date")?,
            dividend_per_share: row.get("dividend_per_share")?,
            split_ratio: row.get("split_ratio")?,
            pre_close: row.get("pre_close")?,
            recorded_at: row.get("recorded_at")?,
        })
    }
}

/// 写入或更新一次除权除息事件。
pub fn upsert(conn: &Connection, action: &CorporateAction) -> Result<()> {
    conn.execute(
        "INSERT INTO corporate_action
           (code, ex_date, dividend_per_share, split_ratio, pre_close, recorded_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(code, ex_date) DO UPDATE SET
           dividend_per_share = excluded.dividend_per_share,
           split_ratio = excluded.split_ratio,
           pre_close = excluded.pre_close,
           recorded_at = excluded.recorded_at",
        params![
            action.code,
            action.ex_date,
            action.dividend_per_share,
            action.split_ratio,
            action.pre_close,
            action.recorded_at,
        ],
    )?;
    Ok(())
}

/// 批量写入或更新除权除息事件，返回写入的行数。
pub fn upsert_many(conn: &mut Connection, actions: &[CorporateAction]) -> Result<usize> {
    let tx = conn.transaction()?;
    let mut written = 0usize;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO corporate_action
               (code, ex_date, dividend_per_share, split_ratio, pre_close, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(code, ex_date) DO UPDATE SET
               dividend_per_share = excluded.dividend_per_share,
               split_ratio = excluded.split_ratio,
               pre_close = excluded.pre_close,
               recorded_at = excluded.recorded_at",
        )?;
        for action in actions {
            stmt.execute(params![
                action.code,
                action.ex_date,
                action.dividend_per_share,
                action.split_ratio,
                action.pre_close,
                action.recorded_at,
            ])?;
            written += 1;
        }
    }
    tx.commit()?;
    Ok(written)
}

/// 读一只股票的全部除权除息事件，按除权日升序。
pub fn list_for_code(conn: &Connection, code: &str) -> Result<Vec<CorporateAction>> {
    let mut stmt =
        conn.prepare("SELECT * FROM corporate_action WHERE code = ?1 ORDER BY ex_date")?;
    let rows = stmt.query_map(params![code], CorporateAction::from_row)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// 一只股票最近一次除权除息日。
pub fn latest_ex_date(conn: &Connection, code: &str) -> Result<Option<String>> {
    let date = conn
        .query_row(
            "SELECT MAX(ex_date) FROM corporate_action WHERE code = ?1",
            params![code],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten();
    Ok(date)
}

/// 删除一只股票的全部除权除息事件，返回删除的行数。
pub fn delete_for_code(conn: &Connection, code: &str) -> Result<usize> {
    let n = conn.execute("DELETE FROM corporate_action WHERE code = ?1", params![code])?;
    Ok(n)
}

/// 全部除权除息事件行数。
pub fn count(conn: &Connection) -> Result<i64> {
    let n = conn.query_row("SELECT COUNT(*) FROM corporate_action", [], |row| row.get(0))?;
    Ok(n)
}

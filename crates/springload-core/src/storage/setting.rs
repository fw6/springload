//! `setting` 表的读写：通用键值设置。

use crate::error::Result;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::BTreeMap;

/// 写入或更新一项设置。
pub fn set(conn: &Connection, key: &str, value: &str, updated_at: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO setting (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value, updated_at],
    )?;
    Ok(())
}

/// 读一项设置。
pub fn get(conn: &Connection, key: &str) -> Result<Option<String>> {
    let value = conn
        .query_row("SELECT value FROM setting WHERE key = ?1", params![key], |row| row.get(0))
        .optional()?;
    Ok(value)
}

/// 读出全部设置，按键升序。
pub fn all(conn: &Connection) -> Result<BTreeMap<String, String>> {
    let mut stmt = conn.prepare("SELECT key, value FROM setting ORDER BY key")?;
    let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
    let mut out = BTreeMap::new();
    for row in rows {
        let (key, value) = row?;
        out.insert(key, value);
    }
    Ok(out)
}

/// 删除一项设置。
pub fn remove(conn: &Connection, key: &str) -> Result<usize> {
    let n = conn.execute("DELETE FROM setting WHERE key = ?1", params![key])?;
    Ok(n)
}

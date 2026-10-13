//! 解释缓存的读写。
//!
//! 缓存键是（股票代码、步骤、数据截止日）。缓存行存在 `setting` 表的键值里，
//! 键前缀 `ai_explain:`，值与全局配置互不干扰。

use super::Scope;
use crate::error::{CoreError, Result};
use crate::storage::{now, setting};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// 缓存行在 `setting` 表里的键前缀。
pub const KEY_PREFIX: &str = "ai_explain:";

/// 一条缓存下来的解释。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheEntry {
    pub text: String,
    pub generated_at: String,
    pub model: String,
}

/// 缓存键：`ai_explain:{code}:{scope}:{data_as_of}`。
pub fn key(code: &str, scope: Scope, data_as_of: &str) -> String {
    format!("{KEY_PREFIX}{code}:{}:{data_as_of}", scope.as_str())
}

/// 读一条缓存。内容损坏时返回错误，不当作缓存命中。
pub fn get(
    conn: &Connection,
    code: &str,
    scope: Scope,
    data_as_of: &str,
) -> Result<Option<CacheEntry>> {
    let storage_key = key(code, scope, data_as_of);
    let Some(raw) = setting::get(conn, &storage_key)? else {
        return Ok(None);
    };
    let entry = serde_json::from_str::<CacheEntry>(&raw)
        .map_err(|error| CoreError::InvalidData(format!("解释缓存内容损坏（{storage_key}）: {error}")))?;
    Ok(Some(entry))
}

/// 写入一条缓存。
pub fn put(
    conn: &Connection,
    code: &str,
    scope: Scope,
    data_as_of: &str,
    entry: &CacheEntry,
) -> Result<()> {
    let value = serde_json::to_string(entry)?;
    setting::set(conn, &key(code, scope, data_as_of), &value, &now())
}

/// 让某只股票某一步的全部缓存失效，覆盖所有数据截止日。返回删除的行数。
pub fn invalidate(conn: &Connection, code: &str, scope: Scope) -> Result<usize> {
    let prefix = format!("{KEY_PREFIX}{code}:{}:", scope.as_str());
    let keys: Vec<String> = setting::all(conn)?
        .into_keys()
        .filter(|key| key.starts_with(&prefix))
        .collect();
    let mut removed = 0;
    for key in keys {
        removed += setting::remove(conn, &key)?;
    }
    Ok(removed)
}

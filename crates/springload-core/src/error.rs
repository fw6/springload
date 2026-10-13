//! `springload-core` 的错误类型与统一结果别名。

use std::fmt;

/// 领域核心的统一错误类型。
///
/// IO、JSON、SQLite 与连接池的错误直接携带原始错误；语义错误（配置非法、
/// 表里读不到期望的行）用 [`CoreError::InvalidData`] 描述。
#[derive(Debug)]
pub enum CoreError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Sqlite(rusqlite::Error),
    Pool(r2d2::Error),
    InvalidData(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::Io(e) => write!(f, "文件读写失败: {e}"),
            CoreError::Json(e) => write!(f, "JSON 解析失败: {e}"),
            CoreError::Sqlite(e) => write!(f, "数据库操作失败: {e}"),
            CoreError::Pool(e) => write!(f, "数据库连接池失败: {e}"),
            CoreError::InvalidData(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for CoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CoreError::Io(e) => Some(e),
            CoreError::Json(e) => Some(e),
            CoreError::Sqlite(e) => Some(e),
            CoreError::Pool(e) => Some(e),
            CoreError::InvalidData(_) => None,
        }
    }
}

impl From<std::io::Error> for CoreError {
    fn from(value: std::io::Error) -> Self {
        CoreError::Io(value)
    }
}

impl From<serde_json::Error> for CoreError {
    fn from(value: serde_json::Error) -> Self {
        CoreError::Json(value)
    }
}

impl From<rusqlite::Error> for CoreError {
    fn from(value: rusqlite::Error) -> Self {
        CoreError::Sqlite(value)
    }
}

impl From<r2d2::Error> for CoreError {
    fn from(value: r2d2::Error) -> Self {
        CoreError::Pool(value)
    }
}

/// 领域核心的统一结果别名。
pub type Result<T> = std::result::Result<T, CoreError>;

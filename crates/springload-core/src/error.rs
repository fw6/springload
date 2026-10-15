//! `springload-core` 的错误类型与统一结果别名。

use std::fmt;

/// 领域核心的统一错误类型。
///
/// IO、JSON、SQLite 与连接池的错误直接携带原始错误；网络请求失败与语义错误
/// （配置非法、表里读不到期望的行）用字符串描述。
#[derive(Debug)]
pub enum CoreError {
    Io(std::io::Error),
    Network(String),
    Json(serde_json::Error),
    Sqlite(rusqlite::Error),
    Pool(r2d2::Error),
    InvalidData(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::Io(e) => write!(f, "文件读写失败: {e}"),
            CoreError::Network(msg) => write!(f, "网络请求失败: {msg}"),
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
            CoreError::Network(_) | CoreError::InvalidData(_) => None,
        }
    }
}

/// 把 reqwest 的错误展开成一句可读的说明。
///
/// reqwest 的 `Display` 只给一句 `error sending request for url (...)`，请求地址
/// 由调用方自己拼，真正的原因（连接被断开、超时、TLS 失败）在 source 链里，
/// 逐层拼上才看得出是哪种故障。
pub fn describe_request_error(error: reqwest::Error) -> String {
    let mut causes = Vec::new();
    let mut cause = std::error::Error::source(&error);
    while let Some(current) = cause {
        causes.push(current.to_string());
        cause = current.source();
    }
    let mut text = error.without_url().to_string();
    for cause in causes {
        text.push_str(": ");
        text.push_str(&cause);
    }
    text
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 网络失败不能报成文件读写失败，否则排查时会被引到错误的方向。
    #[test]
    fn network_error_is_not_reported_as_file_io() {
        let error = CoreError::Network("请求 http://example.com/ 失败".to_string());
        let text = error.to_string();
        assert!(text.starts_with("网络请求失败: "), "实际文案: {text}");
        assert!(!text.contains("文件读写失败"), "实际文案: {text}");
    }

    /// reqwest 的 `Display` 只有一句 `error sending request for url (...)`，
    /// 底层原因在 source 链里；请求地址由调用方拼，不该在消息里重复出现。
    #[test]
    fn request_error_carries_cause_without_duplicating_url() {
        // 连本机一个没有服务监听的端口，拿到真实的连接失败错误。
        let error = reqwest::blocking::Client::new()
            .get("http://127.0.0.1:1/")
            .send()
            .expect_err("端口 1 不应有服务在听");
        let text = describe_request_error(error);
        assert!(
            text.to_lowercase().contains("connect"),
            "应带上底层原因，实际文案: {text}"
        );
        assert!(!text.contains("127.0.0.1"), "不该重复请求地址，实际文案: {text}");
    }
}

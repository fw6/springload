//! 行情数据源：前复权日线、分钟线与实时快照，以及东方财富 / 腾讯 / 新浪三家适配器。
//!
//! `Bar`、`MinuteBar`、`Snapshot` 的字段按架构契约第 4 节冻结。`volume` 为 0 或
//! 日期缺口都表示当天没有交易，指标计算必须跳过这些 bar。

pub mod eastmoney;
pub mod sina;
pub mod tencent;
pub mod url_budget;

use serde::{Deserialize, Serialize};

/// 前复权日线的一根。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bar {
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

impl Bar {
    /// 当天是否有成交。`volume` 为 0 表示无交易。
    pub fn traded(&self) -> bool {
        self.volume > 0.0
    }
}

/// 分钟线的一根。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinuteBar {
    pub code: String,
    /// YYYY-MM-DD HH:MM。
    pub ts: String,
    /// 1 / 5 / 15 / 30 / 60。
    pub period: u32,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    /// 股。
    pub volume: f64,
    /// 元。
    pub amount: f64,
}

/// 一只证券的交易状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TradeStatus {
    Trading,
    Suspended,
    Delisted,
    Unknown,
}

/// 实时快照。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub code: String,
    pub name: String,
    pub last: f64,
    pub prev_close: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    /// 股。
    pub volume: f64,
    /// 元。
    pub amount: f64,
    pub status: TradeStatus,
    pub captured_at: String,
}

/// 日线数据源。前复权日线用东方财富；新浪作为备用来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuoteSource {
    Eastmoney,
    Sina,
}

impl QuoteSource {
    /// 默认的数据源。
    pub fn default_source() -> Self {
        QuoteSource::Eastmoney
    }
}

impl From<crate::storage::bar::BarRow> for Bar {
    fn from(row: crate::storage::bar::BarRow) -> Self {
        Bar {
            code: row.code,
            date: row.date,
            open: row.open,
            high: row.high,
            low: row.low,
            close: row.close,
            volume: row.volume,
            amount: row.amount,
        }
    }
}

/// 解析失败或字段不是数值时统一的错误。
pub(crate) fn parse_f64(field: &str, value: &str) -> crate::error::Result<f64> {
    value
        .trim()
        .parse::<f64>()
        .map_err(|_| crate::error::CoreError::InvalidData(format!("{field} 不是数值: {value}")))
}

/// `YYYYMMDD` 转 `YYYY-MM-DD`，输入不是八位数字时返回 `None`。
pub(crate) fn yyyymmdd_to_date(value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() != 8 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(format!(
        "{}-{}-{}",
        &value[0..4],
        &value[4..6],
        &value[6..8]
    ))
}

/// `YYYYMMDDHHMMSS` 转 `YYYY-MM-DD HH:MM:SS`，长度不符时原样返回。
pub(crate) fn yyyymmddhhmmss_to_datetime(value: &str) -> String {
    let value = value.trim();
    if value.len() != 14 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return value.to_string();
    }
    format!(
        "{}-{}-{} {}:{}:{}",
        &value[0..4],
        &value[4..6],
        &value[6..8],
        &value[8..10],
        &value[10..12],
        &value[12..14]
    )
}

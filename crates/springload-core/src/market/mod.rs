//! 行情数据源与交易市场：前复权日线、分钟线与实时快照。
//!
//! 日线与分钟线按市场走不同来源：A 股走通达信协议（[`tdx`]），美股走 Yahoo Finance
//! 的公开接口（[`us`]）。实时快照两家共用腾讯与新浪的公开接口（[`tencent`]、[`sina`]）。
//!
//! `Bar`、`MinuteBar`、`Snapshot` 的字段按架构契约第 4 节冻结。`volume` 为 0 或
//! 日期缺口都表示当天没有交易，指标计算必须跳过这些 bar。

pub mod sina;
pub mod tdx;
pub mod tencent;
pub mod url_budget;
pub mod us;

use crate::error::Result;
use crate::storage::corporate_action::CorporateAction;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

/// 交易市场。整个应用的数据、交易规则与文案都按它分流。
///
/// 每个市场有各自独立的 SQLite 库文件、行情源与参数，切换市场等于换一套工作台。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Market {
    /// 沪深京 A 股。
    #[default]
    Cn,
    /// 美股。
    Us,
}

/// A 股收盘时刻（市场当地时间）。
const CN_CLOSE: (u32, u32) = (15, 0);
/// 美股常规时段收盘时刻（市场当地时间）。
const US_CLOSE: (u32, u32) = (16, 0);

impl Market {
    /// 配置与命令入参里的取值。
    pub const fn as_str(self) -> &'static str {
        match self {
            Market::Cn => "cn",
            Market::Us => "us",
        }
    }

    /// 界面上的市场名。
    pub const fn label(self) -> &'static str {
        match self {
            Market::Cn => "A 股",
            Market::Us => "美股",
        }
    }

    /// 计价货币名。
    pub const fn currency(self) -> &'static str {
        match self {
            Market::Cn => "元",
            Market::Us => "美元",
        }
    }

    /// 买入申报的最小股数。A 股一手 100 股，美股按整股买卖。
    pub const fn lot_size(self) -> i64 {
        match self {
            Market::Cn => 100,
            Market::Us => 1,
        }
    }

    /// 是否当日买入次日才能卖出。
    pub const fn t_plus_one(self) -> bool {
        match self {
            Market::Cn => true,
            Market::Us => false,
        }
    }

    /// 收盘时刻（市场当地时间）。
    pub fn close_time(self) -> chrono::NaiveTime {
        let (hour, minute) = match self {
            Market::Cn => CN_CLOSE,
            Market::Us => US_CLOSE,
        };
        chrono::NaiveTime::from_hms_opt(hour, minute, 0).expect("收盘时刻总是合法时刻")
    }

    /// 市场所在时区。
    pub const fn tz(self) -> chrono_tz::Tz {
        match self {
            Market::Cn => chrono_tz::Asia::Shanghai,
            Market::Us => chrono_tz::America::New_York,
        }
    }

    /// 当前时刻的市场当地时间。收盘时间与「今天跑过没有」都按它算。
    pub fn local_now(self) -> chrono::NaiveDateTime {
        chrono::Utc::now().with_timezone(&self.tz()).naive_local()
    }

    /// 库文件名。两个市场各用一个库，互不干扰。
    pub const fn db_file_name(self) -> &'static str {
        match self {
            Market::Cn => "springload.db",
            Market::Us => "springload.us.db",
        }
    }

    /// 行情来源名称，`app_info` 展示用。
    pub const fn data_source(self) -> &'static str {
        match self {
            Market::Cn => "通达信 · 前复权",
            Market::Us => "Yahoo Finance · 前复权",
        }
    }

    /// 全部市场，界面上的切换顺序。
    pub const ALL: [Market; 2] = [Market::Cn, Market::Us];

    /// 解析配置或命令里的取值。
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "cn" => Some(Market::Cn),
            "us" => Some(Market::Us),
            _ => None,
        }
    }
}

/// 证券列表里的一行。
#[derive(Debug, Clone, PartialEq)]
pub struct Security {
    /// A 股是六位数字代码，美股是交易代码。
    pub code: String,
    pub name: String,
}

/// A 股六位代码所属的板块。只认沪深主板、科创板、创业板与北交所。
pub fn board_of(code: &str) -> Option<&'static str> {
    let prefix = code.get(0..3)?;
    match prefix {
        "600" | "601" | "603" | "605" | "000" | "001" | "002" | "003" => Some("main"),
        "688" | "689" => Some("star"),
        "300" | "301" => Some("chinext"),
        _ if prefix.starts_with('4') || prefix.starts_with('8') || prefix == "920" => Some("bse"),
        _ => None,
    }
}

/// A 股六位代码所属的交易所。
pub fn market_of(code: &str) -> &'static str {
    if code.starts_with("920") || code.starts_with('4') || code.starts_with('8') {
        "BJ"
    } else if code.starts_with('6') || code.starts_with('9') {
        "SH"
    } else {
        "SZ"
    }
}

/// 行情数据源的契约。
///
/// 回补、复权维护与分钟线轮询都通过它取数，测试用录制样本回放。每个市场一个实现：
/// A 股是 [`tdx::TdxSource`]，美股是 [`us::UsSource`]。
pub trait QuoteSource: Send + Sync {
    /// 取本市场的全部证券。
    fn security_list(&self) -> Result<Vec<Security>>;
    /// 取一只股票自 `beg`（`YYYYMMDD`）以来的日线，按日期升序。
    ///
    /// A 股返回不复权原始价，由 [`crate::adjustment`] 按除权除息事件换算；美股返回
    /// 已经过前复权的价，除权除息事件为空。
    fn daily_raw(&self, code: &str, beg: &str) -> Result<Vec<Bar>>;
    /// 取一只股票的除权除息事件，按除权日升序。
    fn corporate_actions(&self, code: &str) -> Result<Vec<CorporateAction>>;
    /// 取一只股票某个周期（`1/5/15/30/60`）的分钟线，按时间升序。
    fn minute_bars(&self, code: &str, period: u32) -> Result<Vec<MinuteBar>>;
}

/// 取某个市场的行情数据源。
pub fn source(market: Market) -> Arc<dyn QuoteSource> {
    match market {
        Market::Cn => Arc::new(tdx::TdxSource::new()),
        Market::Us => Arc::new(us::UsSource::new()),
    }
}

/// 分钟线的时间点 `ts` 落到 `Bar` 的 `date` 字段。
impl From<MinuteBar> for Bar {
    fn from(bar: MinuteBar) -> Self {
        Bar {
            code: bar.code,
            date: bar.ts,
            open: bar.open,
            high: bar.high,
            low: bar.low,
            close: bar.close,
            volume: bar.volume,
            amount: bar.amount,
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

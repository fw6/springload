//! 美股行情适配器：Yahoo Finance 的 chart 接口（走 `yahoo_finance_api` crate）与
//! Nasdaq Trader 的证券目录。
//!
//! 事实来源见 `docs/research/us-quote-source.md`。三处与 A 股不同的口径：
//!
//! - **价格已经复权。** Yahoo 的 `quote` 序列本身是拆股调整后的价，`adjclose` 再叠加
//!   分红调整。所以日线取 `adjclose ÷ close` 这个系数缩放开高低收，`corporate_actions`
//!   返回空——库里存的仍然是前复权日线，[`crate::adjustment`] 对空事件表是恒等换算。
//! - **成交量单位是股。** 与 A 股的「手」不同，这里不再乘 100。
//! - **成交额要自己算。** 接口不返回成交额，用当日收盘价乘成交量近似。
//!
//! 证券代码保留 Nasdaq 目录里的写法（`BRK.B`），调 Yahoo 时把点换成连字符（`BRK-B`）。

use super::{Bar, MinuteBar, QuoteSource, Security};
use crate::error::{describe_request_error, CoreError, Result};
use crate::storage::corporate_action::CorporateAction;
use chrono::{NaiveDate, TimeZone, Utc};
use chrono_tz::America::New_York;
use std::sync::OnceLock;
use std::time::Duration;
use yahoo_finance_api::time::{Duration as TimeDuration, OffsetDateTime};
use yahoo_finance_api::{YahooConnector, YResponse};

/// Nasdaq Trader 的证券目录：纳斯达克挂牌一份，其余交易所挂牌一份。
pub const DIRECTORY_URLS: [&str; 2] = [
    "https://www.nasdaqtrader.com/dynamic/SymDir/nasdaqlisted.txt",
    "https://www.nasdaqtrader.com/dynamic/SymDir/otherlisted.txt",
];
/// 取证券目录时带的 User-Agent。Yahoo 那侧由 crate 自己设。
pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)";
/// 单次请求的超时。一只股票一次请求，失败就跳过这一只，不拖住整次同步。
const TIMEOUT: Duration = Duration::from_secs(20);
/// 前复权价格保留的小数位。
///
/// 两位小数会把低价股的相对精度毁掉（0.5 美元的股票只到 0.5%），四位足够覆盖
/// 分股以下的价格又不会留下浮点噪声。
const PRICE_DECIMALS: f64 = 10_000.0;
/// 日线周期。
pub const INTERVAL_DAY: &str = "1d";
/// 每个分钟周期对应的 Yahoo `interval` 与 `range`。
///
/// Yahoo 对短周期限制了可回溯的长度：1 分钟最多 7 天，5/15/30/60 分钟最多 60 天。
pub const MINUTE_WINDOWS: [(u32, &str, &str); 5] = [
    (1, "1m", "1d"),
    (5, "5m", "5d"),
    (15, "15m", "5d"),
    (30, "30m", "1mo"),
    (60, "60m", "1mo"),
];

/// 走真实网络的美股行情数据源。
pub struct UsSource;

impl UsSource {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UsSource {
    fn default() -> Self {
        Self::new()
    }
}

/// 进程共用的 Yahoo 连接器。连接复用对逐个股票取数的场景差别很大。
fn connector() -> Result<&'static YahooConnector> {
    static CONNECTOR: OnceLock<YahooConnector> = OnceLock::new();
    if let Some(existing) = CONNECTOR.get() {
        return Ok(existing);
    }
    let built = YahooConnector::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|error| CoreError::Network(format!("构建 Yahoo 客户端失败: {error}")))?;
    Ok(CONNECTOR.get_or_init(|| built))
}

/// 取证券目录用的 HTTP 客户端。
fn directory_client() -> Result<&'static reqwest::blocking::Client> {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    if let Some(existing) = CLIENT.get() {
        return Ok(existing);
    }
    let built = reqwest::blocking::Client::builder()
        .connect_timeout(TIMEOUT)
        .timeout(TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| CoreError::Network(format!("构建 HTTP 客户端失败: {error}")))?;
    Ok(CLIENT.get_or_init(|| built))
}

impl QuoteSource for UsSource {
    /// 沪深之外的三个美股交易所（纳斯达克、纽交所、美交所）的普通股与存托凭证。
    fn security_list(&self) -> Result<Vec<Security>> {
        let mut out = Vec::new();
        for (index, url) in DIRECTORY_URLS.iter().enumerate() {
            let body = directory_client()?
                .get(*url)
                .send()
                .map_err(|error| CoreError::Network(describe_request_error(error)))?
                .text()
                .map_err(|error| CoreError::Network(format!("读取证券目录失败: {error}")))?;
            out.extend(parse_directory(&body, index == 0)?);
        }
        Ok(out)
    }

    fn daily_raw(&self, code: &str, beg: &str) -> Result<Vec<Bar>> {
        // 多取一天，免得起点落在美股开盘之前被过滤掉。
        let start = OffsetDateTime::from_unix_timestamp(epoch_of(beg)? - 86_400)
            .map_err(|error| CoreError::InvalidData(format!("起始日期不是合法时刻: {error}")))?;
        let end = OffsetDateTime::now_utc() + TimeDuration::days(1);
        let response = connector()?
            .get_quote_history_interval(&yahoo_symbol(code), start, end, INTERVAL_DAY)
            .map_err(|error| CoreError::Network(format!("取 {code} 日线失败: {error}")))?;
        bars_from(&response, code)
    }

    /// Yahoo 的日线已经是复权价，没有需要本地施加的除权除息事件。
    fn corporate_actions(&self, _code: &str) -> Result<Vec<CorporateAction>> {
        Ok(Vec::new())
    }

    fn minute_bars(&self, code: &str, period: u32) -> Result<Vec<MinuteBar>> {
        let (_, interval, range) = MINUTE_WINDOWS
            .iter()
            .find(|(value, _, _)| *value == period)
            .ok_or_else(|| CoreError::InvalidData(format!("Yahoo 接口不支持 {period} 分钟周期")))?;
        let response = connector()?
            .get_quote_range(&yahoo_symbol(code), interval, range)
            .map_err(|error| CoreError::Network(format!("取 {code} {period} 分钟线失败: {error}")))?;
        let bars = minutes_from(&response, code, period)?;
        // 1 分钟线跨到了前一个交易日，裁到最新那一天，与触发判定「看当前这个交易日」一致。
        if period == 1 {
            if let Some(last_date) = bars.last().map(|bar| bar.ts[..10].to_string()) {
                return Ok(bars
                    .into_iter()
                    .filter(|bar| bar.ts.starts_with(&last_date))
                    .collect());
            }
        }
        Ok(bars)
    }
}

/// 调 Yahoo 用的代码：类别股在目录里写作 `BRK.B`，Yahoo 写作 `BRK-B`。
pub fn yahoo_symbol(code: &str) -> String {
    code.trim().to_uppercase().replace('.', "-")
}

/// `YYYYMMDD` 转成当天 UTC 零点的秒数。
fn epoch_of(beg: &str) -> Result<i64> {
    let date = NaiveDate::parse_from_str(beg, "%Y%m%d")
        .map_err(|_| CoreError::InvalidData(format!("起始日期不是 YYYYMMDD: {beg}")))?;
    let midnight = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| CoreError::InvalidData(format!("起始日期不是合法时刻: {beg}")))?;
    Ok(Utc.from_utc_datetime(&midnight).timestamp())
}

/// 时间戳对应的美股当地日期。
fn session_date(seconds: i64) -> String {
    let moment = Utc
        .timestamp_opt(seconds, 0)
        .single()
        .unwrap_or_else(|| Utc.timestamp_opt(0, 0).single().expect("纪元时刻总是合法"));
    moment
        .with_timezone(&New_York)
        .format("%Y-%m-%d")
        .to_string()
}

/// 时间戳对应的美股当地时刻，精确到分钟。
fn session_minute(seconds: i64) -> String {
    let moment = Utc
        .timestamp_opt(seconds, 0)
        .single()
        .unwrap_or_else(|| Utc.timestamp_opt(0, 0).single().expect("纪元时刻总是合法"));
    moment
        .with_timezone(&New_York)
        .format("%Y-%m-%d %H:%M")
        .to_string()
}

/// 复权系数：`adjclose ÷ close`。
///
/// 两个价有一个不为正就按 1 处理——那种日子没有成交，前一步已经把它跳过了。
fn adjust_factor(close: f64, adjclose: f64) -> f64 {
    if close > 0.0 && adjclose > 0.0 {
        adjclose / close
    } else {
        1.0
    }
}

/// 把日线响应转成 `Bar`。价格按复权系数缩放，成交额按收盘价乘成交量算。
pub fn bars_from(response: &YResponse, code: &str) -> Result<Vec<Bar>> {
    let quotes = response
        .quotes()
        .map_err(|error| CoreError::InvalidData(format!("Yahoo 日线没有可用的报价: {error}")))?;
    let round = |value: f64| (value * PRICE_DECIMALS).round() / PRICE_DECIMALS;
    Ok(quotes
        .iter()
        .map(|quote| {
            let factor = adjust_factor(quote.close, quote.adjclose);
            Bar {
                code: code.to_string(),
                date: session_date(quote.timestamp),
                open: round(quote.open * factor),
                high: round(quote.high * factor),
                low: round(quote.low * factor),
                close: round(quote.close * factor),
                volume: quote.volume as f64,
                amount: quote.close * quote.volume as f64,
            }
        })
        .collect())
}

/// 把分钟线响应转成 `MinuteBar`。时间戳换算成美股当地时刻，成交量为 0 的那一根不写库。
pub fn minutes_from(response: &YResponse, code: &str, period: u32) -> Result<Vec<MinuteBar>> {
    let quotes = response
        .quotes()
        .map_err(|error| CoreError::InvalidData(format!("Yahoo 分钟线没有可用的报价: {error}")))?;
    let mut bars = Vec::new();
    for quote in &quotes {
        // 收盘后 Yahoo 还会补一根成交量为 0 的收尾线，它不代表任何成交。
        if quote.volume == 0 {
            continue;
        }
        let factor = adjust_factor(quote.close, quote.adjclose);
        bars.push(MinuteBar {
            code: code.to_string(),
            ts: session_minute(quote.timestamp),
            period,
            open: quote.open * factor,
            high: quote.high * factor,
            low: quote.low * factor,
            close: quote.close * factor,
            volume: quote.volume as f64,
            amount: quote.close * quote.volume as f64,
        });
    }
    Ok(bars)
}

/// 解析 Nasdaq Trader 的证券目录。
///
/// `nasdaq` 为真时读 `nasdaqlisted.txt` 的列序，否则读 `otherlisted.txt` 的列序。
/// 只保留普通股与存托凭证：剔除测试代码、ETF、权证、单位、权利与优先股。
pub fn parse_directory(body: &str, nasdaq: bool) -> Result<Vec<Security>> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'|')
        .has_headers(true)
        .flexible(true)
        .from_reader(body.as_bytes());
    let mut out = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|error| {
            CoreError::InvalidData(format!("证券目录不是竖线分隔的文本: {error}"))
        })?;
        // 文件末尾有一行 `File Creation Time: ...`，列数对不上，直接跳过。
        if record.len() < 8 {
            continue;
        }
        let (symbol, name, exchange, test_issue, etf) = if nasdaq {
            (
                record.get(0).unwrap_or("").trim().to_string(),
                record.get(1).unwrap_or("").trim().to_string(),
                "Q".to_string(),
                record.get(3).unwrap_or("").trim() == "N",
                record.get(6).unwrap_or("").trim() == "N",
            )
        } else {
            (
                record.get(0).unwrap_or("").trim().to_string(),
                record.get(1).unwrap_or("").trim().to_string(),
                record.get(2).unwrap_or("").trim().to_string(),
                record.get(6).unwrap_or("").trim() == "N",
                record.get(4).unwrap_or("").trim() == "N",
            )
        };
        if !test_issue || !etf || !common_stock(symbol.as_str(), name.as_str(), &exchange, nasdaq) {
            continue;
        }
        out.push(Security { code: symbol, name });
    }
    Ok(out)
}

/// 是不是可交易的普通股或存托凭证。
///
/// 交易所只认纳斯达克、纽交所与美交所；证券名里的「 - 」之后是证券种类说明，
/// 优先股、权证、单位、权利与票据都在说明里写明，据此剔除。
pub fn common_stock(symbol: &str, name: &str, exchange: &str, nasdaq: bool) -> bool {
    if symbol.is_empty() || symbol.contains('$') || symbol.starts_with(|c: char| c.is_ascii_digit())
    {
        return false;
    }
    if !nasdaq && !matches!(exchange, "N" | "A") {
        return false;
    }
    let lower = name.to_lowercase();
    let description = match lower.rfind(" - ") {
        Some(index) => &lower[index + 3..],
        None => lower.as_str(),
    };
    let depositary = description.contains("depositary share");
    // 存托凭证（ADR）本身是普通股的凭证，与优先股的存托份额不是一回事。
    if depositary && !description.contains("american depositary") {
        return false;
    }
    let junk = [
        "preferred",
        "warrant",
        "rights",
        "right",
        "units",
        "unit",
        "notes",
        "note",
        "when issued",
        "when-issued",
        "trust preferred",
        "closed-end",
        "etf",
        "fund",
        "%",
    ];
    if junk.iter().any(|word| description.contains(word)) {
        return false;
    }
    ["common", "ordinary share", "american depositary", "depositary shares"]
        .iter()
        .any(|word| description.contains(word))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yahoo_symbols_use_hyphens_for_class_shares() {
        assert_eq!(yahoo_symbol("aapl"), "AAPL");
        assert_eq!(yahoo_symbol("BRK.B"), "BRK-B");
    }

    #[test]
    fn epoch_comes_from_yyyymmdd() {
        assert_eq!(epoch_of("19700101").unwrap(), 0);
        assert_eq!(epoch_of("20241028").unwrap(), 1_730_073_600);
        assert!(epoch_of("2024-10-28").is_err());
    }

    #[test]
    fn local_timestamps_are_new_york_time() {
        // 2026-10-14T13:30:00Z 是美东夏令时的 09:30。
        assert_eq!(session_date(1_791_984_600), "2026-10-14");
        assert_eq!(session_minute(1_791_984_600), "2026-10-14 09:30");
        // 2026-11-16T14:30:00Z 是美东冬令时的 09:30。
        assert_eq!(session_minute(1_794_839_400), "2026-11-16 09:30");
    }

    /// 复权系数取 `adjclose ÷ close`；两个价有一个不为正就按 1 处理。
    #[test]
    fn adjust_factor_divides_by_the_raw_close() {
        assert!((adjust_factor(225.0, 223.3789) - 0.99279511).abs() < 1e-6);
        assert_eq!(adjust_factor(0.0, 100.0), 1.0);
        assert_eq!(adjust_factor(100.0, 0.0), 1.0);
    }

    /// 每个分钟周期都要有一条 Yahoo 的 interval 与 range。
    #[test]
    fn every_minute_period_has_a_window() {
        for period in crate::minute::PERIODS {
            assert!(
                MINUTE_WINDOWS.iter().any(|(value, _, _)| *value == period),
                "{period} 分钟没有配窗口"
            );
        }
        assert_eq!(MINUTE_WINDOWS.len(), crate::minute::PERIODS.len());
    }

    #[test]
    fn directory_keeps_common_stock_and_adr_only() {
        let nasdaq = "\
Symbol|Security Name|Market Category|Test Issue|Financial Status|Round Lot Size|ETF|NextShares
AAPL|Apple Inc. - Common Stock|Q|N|N|40|N|N
AACIW|Armada Acquisition Corp. III - Warrant|G|N|N|100|N|N
ACGLN|Arch Capital Group Ltd. - Depositary Shares, each Representing a 1/1,000th Interest in a 4.550% Non-Cumulative Preferred Share, Series G|Q|N|N|100|N|N
AACG|ATA Creativity Global - American Depositary Shares, each representing two common shares|S|N|D|100|N|N
AAAP|Pacer Barings CLO Market Flex ETF|G|N|N|100|Y|N
ZTEST|Test Issue Corp - Common Stock|Q|Y|N|100|N|N
File Creation Time: 1015202603:03|||||||
";
        let listed = parse_directory(nasdaq, true).unwrap();
        let codes: Vec<&str> = listed.iter().map(|row| row.code.as_str()).collect();
        assert_eq!(codes, vec!["AAPL", "AACG"]);
        assert_eq!(listed[0].name, "Apple Inc. - Common Stock");
    }

    #[test]
    fn directory_keeps_only_the_three_exchanges() {
        let others = "\
ACT Symbol|Security Name|Exchange|CQS Symbol|ETF|Round Lot Size|Test Issue|NASDAQ Symbol
A|Agilent Technologies, Inc. Common Stock|N|A|N|100|N|A
AA|Alcoa Corporation Common Stock |N|AA|N|100|N|AA
BRK.B|Berkshire Hathaway Inc. Class B Common Stock|N|BRK.B|N|100|N|BRK.B
ZFOO|Bats Only Corp - Common Stock|Z|ZFOO|N|100|N|ZFOO
File Creation Time: 1015202603:03|||||||
";
        let listed = parse_directory(others, false).unwrap();
        let codes: Vec<&str> = listed.iter().map(|row| row.code.as_str()).collect();
        assert_eq!(codes, vec!["A", "AA", "BRK.B"]);
    }
}

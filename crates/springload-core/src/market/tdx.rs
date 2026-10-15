//! 通达信协议适配器：全市场证券列表、不复权日线、分钟线与除权除息事件。
//!
//! 协议的二进制解析由 `rustdx-complete` 承担。本模块负责三件事：挑一台真的能
//! 出数据的服务器、把协议字段换算成 `Bar` 与 `MinuteBar` 的单位、把除权除息
//! 记录换算成 `CorporateAction` 的每股口径。
//!
//! 服务器要按「能出数据」来挑，不能按 TCP 连得上挑。实测 104 台服务器里 33 台
//! 能建立连接，但只有 8 台能返回 K 线与快照；其余能返回除权除息与分时，K 线一律
//! 为空。这 8 台都在国泰君安同一网段，下面的 [`SERVERS`] 是实测结果。
//!
//! 成交量单位随周期变化：日线是手，分钟线是股。成交额两家都是元。

use crate::error::{CoreError, Result};
use crate::market::{Bar, MinuteBar, Security};
use crate::storage::corporate_action::CorporateAction;
use rustdx_complete::tcp::ip::STOCK_IP;
use rustdx_complete::tcp::stock::Client;
use rustdx_complete::tcp::TcpConfig;
use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::Duration;

/// 日线的 K 线种类。
pub const CATEGORY_DAY: u16 = 9;

/// 协议里日线成交量的单位是手，换算成股要乘这个数。
pub const LOTS: f64 = 100.0;

/// 建连与读写的超时时间。
const TIMEOUT: Duration = Duration::from_secs(10);
/// 探测服务器时的超时时间，比正常使用短。
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// 实测能返回 K 线与快照的行情服务器。
pub const SERVERS: &[&str] = &[
    "117.34.114.13:7709",
    "117.34.114.14:7709",
    "117.34.114.15:7709",
    "117.34.114.16:7709",
    "117.34.114.17:7709",
    "117.34.114.18:7709",
    "117.34.114.20:7709",
    "117.34.114.27:7709",
];

/// 分钟周期对应的 K 线种类。
pub fn category_of(period: u32) -> Option<u16> {
    match period {
        1 => Some(7),
        5 => Some(0),
        15 => Some(1),
        30 => Some(2),
        60 => Some(3),
        _ => None,
    }
}

/// 1 分钟线一次取多少根。A 股一天 240 根，够覆盖当日。
const MINUTE_1M_BARS: u16 = 240;
/// 除 1 分钟以外的周期往前取多少天。
///
/// 盘中轮询只要当日 1 分钟线，其余周期按最近一个多月取就够；不设上限会去翻全部历史
/// （1 分钟有 2 万多根），每分钟轮询一次扛不住。
const MINUTE_WINDOW_DAYS: u32 = 60;

/// 六位代码所属的市场：0 深圳、1 上海、2 北京。
pub fn market_of(code: &str) -> u16 {
    match super::market_of(code) {
        "SH" => 1,
        "BJ" => 2,
        _ => 0,
    }
}

fn config(addr: SocketAddr, timeout: Duration) -> TcpConfig {
    TcpConfig {
        timeout,
        ip: Some(addr),
        auto_reconnect: 2,
        retry_delay_ms: 200,
        // 服务器在高频请求下会静默返回空响应，空结果要二次确认。
        recheck_empty: true,
    }
}

/// 候选服务器：先试实测能出数据的，再退回依赖库内置的列表。
fn candidates() -> Vec<SocketAddr> {
    let mut out: Vec<SocketAddr> = SERVERS.iter().filter_map(|text| text.parse().ok()).collect();
    for addr in STOCK_IP.iter() {
        if !out.contains(addr) {
            out.push(*addr);
        }
    }
    out
}

/// 这台服务器能不能返回 K 线。
fn serves_data(addr: SocketAddr) -> bool {
    let Ok(mut client) = Client::with_config(&config(addr, PROBE_TIMEOUT)) else {
        return false;
    };
    matches!(
        client.bars(1, "600519", CATEGORY_DAY, 0, 5),
        Ok(bars) if !bars.is_empty()
    )
}

/// 挑一台能返回 K 线的服务器。
pub fn pick_server() -> Result<SocketAddr> {
    for addr in candidates() {
        if serves_data(addr) {
            return Ok(addr);
        }
    }
    Err(CoreError::Network(
        "没有可用的通达信行情服务器".to_string(),
    ))
}

/// 一台服务器上的一条连接。
pub struct Connection {
    client: Client,
}

impl Connection {
    /// 连到指定服务器并完成握手。
    pub fn open(addr: SocketAddr) -> Result<Self> {
        let client = Client::with_config(&config(addr, TIMEOUT))
            .map_err(|e| CoreError::Network(format!("连接通达信行情服务器 {addr} 失败: {e}")))?;
        Ok(Self { client })
    }

    /// 取某只股票自 `beg`（`YYYYMMDD`）以来的不复权日线，按日期升序。
    ///
    /// `beg` 传 `None` 表示取全部历史。
    pub fn daily_raw(&mut self, code: &str, beg: Option<&str>) -> Result<Vec<Bar>> {
        let begin = match beg {
            Some(text) => Some(parse_yyyymmdd(text)?),
            None => None,
        };
        let market = market_of(code);
        let bars = self
            .client
            .bars_range(market, code, CATEGORY_DAY, begin, None)
            .map_err(|e| CoreError::Network(format!("取 {code} 日线失败: {e}")))?;
        Ok(bars.iter().map(|bar| to_bar(code, bar)).collect())
    }

    /// 取某只股票某个周期的分钟线，按时间升序。
    pub fn minute_bars(&mut self, code: &str, period: u32) -> Result<Vec<MinuteBar>> {
        let category = category_of(period)
            .ok_or_else(|| CoreError::InvalidData(format!("通达信协议不支持 {period} 分钟周期")))?;
        let market = market_of(code);
        let bars = if period == 1 {
            self.client
                .bars(market, code, category, 0, MINUTE_1M_BARS)
                .map_err(|e| CoreError::Network(format!("取 {code} {period} 分钟线失败: {e}")))?
        } else {
            self.client
                .bars_range(market, code, category, Some(days_ago(MINUTE_WINDOW_DAYS)), None)
                .map_err(|e| CoreError::Network(format!("取 {code} {period} 分钟线失败: {e}")))?
        };
        let bars: Vec<_> = bars
            .iter()
            .map(|bar| MinuteBar {
                code: code.to_string(),
                ts: format!(
                    "{:04}-{:02}-{:02} {:02}:{:02}",
                    bar.dt.year, bar.dt.month, bar.dt.day, bar.dt.hour, bar.dt.minute
                ),
                period,
                open: bar.open,
                high: bar.high,
                low: bar.low,
                close: bar.close,
                // 分钟线的成交量单位是股。
                volume: bar.vol,
                amount: bar.amount,
            })
            .collect();
        // 1 分钟线取的是最近 240 根，开盘前与收盘后都会跨到前一个交易日，裁到最新那一天，
        // 与触发判定「看当前这个交易日」的语义一致。
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

    /// 取某只股票的除权除息事件，按除权日升序。
    ///
    /// 只保留种类 1（除权除息），送配股上市、股本变化这些不改变价格。
    pub fn corporate_actions(&mut self, code: &str) -> Result<Vec<CorporateAction>> {
        let market = market_of(code);
        let records = self
            .client
            .xdxr(market, code)
            .map_err(|e| CoreError::Network(format!("取 {code} 除权除息记录失败: {e}")))?;
        let recorded_at = crate::storage::now();
        let mut events: Vec<CorporateAction> = records
            .iter()
            .filter(|record| record.category == 1)
            .filter_map(|record| {
                let ex_date = yyyymmdd_to_date(record.date)?;
                Some(CorporateAction {
                    code: code.to_string(),
                    ex_date,
                    // 协议里每 10 股派现金、每 10 股送转股，换算成每股。
                    dividend_per_share: f64::from(record.fh_qltp) / 10.0,
                    split_ratio: f64::from(record.sg_hltp) / 10.0,
                    // 协议不提供除权日的前收盘价，由调用方按不复权日线补。
                    pre_close: None,
                    recorded_at: recorded_at.clone(),
                })
            })
            .collect();
        events.sort_by(|a, b| a.ex_date.cmp(&b.ex_date));
        Ok(events)
    }

    /// 取某个市场的全部证券。`market` 取 0 深圳、1 上海、2 北京。
    ///
    /// 列表里混着指数与基金，按 [`market_matches`] 剔掉与本市场不同号的那些。
    pub fn security_list(&mut self, market: u16) -> Result<Vec<Security>> {
        let listed = self
            .client
            .stocks(market)
            .map_err(|e| CoreError::Network(format!("取市场 {market} 证券列表失败: {e}")))?;
        Ok(listed
            .into_iter()
            .filter(|row| market_matches(&row.code, market))
            .map(|row| Security {
                code: row.code,
                name: row.name.trim().to_string(),
            })
            .collect())
    }
}

/// 服务器给的市场号，与代码前缀推出的交易所一致吗。
///
/// 沪深两边的列表里都混着指数：上证指数系列用的是 `000xxx` 代码，与深市股票同号。
/// 只按代码前缀判板块的话，这些指数会被当成主板股票进池，并顶掉同号的深市股票
/// （`000001` 既是平安银行也是上证指数）。两个号必须一致才收。
pub fn market_matches(code: &str, market: u16) -> bool {
    match market {
        0 => super::market_of(code) == "SZ",
        1 => super::market_of(code) == "SH",
        _ => false,
    }
}

/// 一组连接。并发调用时按需建连，用完放回。
///
/// 服务器在第一次取数时才挑，所以构造数据源这一步不会发网络请求，连不上服务器
/// 会在具体那次取数上报错。
pub struct Pool {
    addr: Mutex<Option<SocketAddr>>,
    idle: Mutex<Vec<Connection>>,
}

impl Pool {
    /// 建一个还没挑服务器的连接池。
    pub fn new() -> Self {
        Self {
            addr: Mutex::new(None),
            idle: Mutex::new(Vec::new()),
        }
    }

    /// 连到指定服务器建池，供测试与排查使用。
    pub fn at(addr: SocketAddr) -> Self {
        Self {
            addr: Mutex::new(Some(addr)),
            idle: Mutex::new(Vec::new()),
        }
    }

    /// 池子用的服务器，没挑过就现在挑。
    fn server(&self) -> Result<SocketAddr> {
        let mut guard = self.addr.lock().expect("服务器锁");
        if let Some(addr) = *guard {
            return Ok(addr);
        }
        let addr = pick_server()?;
        *guard = Some(addr);
        Ok(addr)
    }

    /// 借一条连接执行 `f`，成功后放回池子。
    ///
    /// 失败时这条连接被丢弃，下一次调用重新建连。
    pub fn with<T>(&self, f: impl FnOnce(&mut Connection) -> Result<T>) -> Result<T> {
        let addr = self.server()?;
        let mut connection = {
            let mut idle = self.idle.lock().expect("连接池锁");
            idle.pop()
        };
        let mut connection = match connection.take() {
            Some(connection) => connection,
            None => Connection::open(addr)?,
        };
        match f(&mut connection) {
            Ok(value) => {
                let mut idle = self.idle.lock().expect("连接池锁");
                idle.push(connection);
                Ok(value)
            }
            Err(error) => Err(error),
        }
    }
}

impl Default for Pool {
    fn default() -> Self {
        Self::new()
    }
}

/// 走真实网络的行情数据源。
pub struct TdxSource {
    pool: Pool,
}

impl TdxSource {
    /// 建一个行情数据源。服务器在第一次取数时才挑。
    pub fn new() -> Self {
        Self { pool: Pool::new() }
    }
}

impl Default for TdxSource {
    fn default() -> Self {
        Self::new()
    }
}

impl super::QuoteSource for TdxSource {
    /// 沪深两个市场的证券列表。北京证券交易所的证券不在股票池范围内，不去取。
    fn security_list(&self) -> Result<Vec<Security>> {
        let mut out = Vec::new();
        for market in [0u16, 1] {
            out.extend(self.pool.with(|connection| connection.security_list(market))?);
        }
        Ok(out)
    }

    fn daily_raw(&self, code: &str, beg: &str) -> Result<Vec<Bar>> {
        self.pool
            .with(|connection| connection.daily_raw(code, Some(beg)))
    }

    fn corporate_actions(&self, code: &str) -> Result<Vec<CorporateAction>> {
        self.pool.with(|connection| connection.corporate_actions(code))
    }

    fn minute_bars(&self, code: &str, period: u32) -> Result<Vec<MinuteBar>> {
        self.pool
            .with(|connection| connection.minute_bars(code, period))
    }
}

fn to_bar(code: &str, bar: &rustdx_complete::tcp::stock::KlineData<'_>) -> Bar {
    Bar {
        code: code.to_string(),
        date: format!("{:04}-{:02}-{:02}", bar.dt.year, bar.dt.month, bar.dt.day),
        open: bar.open,
        high: bar.high,
        low: bar.low,
        close: bar.close,
        // 日线的成交量单位是手。
        volume: bar.vol * LOTS,
        amount: bar.amount,
    }
}

/// 今天往前 `days` 天的 `YYYYMMDD`。
fn days_ago(days: u32) -> u32 {
    let today = chrono::Local::now().date_naive();
    today
        .checked_sub_days(chrono::Days::new(u64::from(days)))
        .unwrap_or(today)
        .format("%Y%m%d")
        .to_string()
        .parse()
        .expect("八位日期总能解析成 u32")
}

/// `YYYYMMDD` 转 `u32`，非八位数字时返回错误。
fn parse_yyyymmdd(text: &str) -> Result<u32> {
    text.parse::<u32>()
        .map_err(|_| CoreError::InvalidData(format!("起始日期不是 YYYYMMDD: {text}")))
}

/// `20260626` 转 `2026-06-26`。
fn yyyymmdd_to_date(value: u32) -> Option<String> {
    if !(19000101..=29991231).contains(&value) {
        return None;
    }
    let text = value.to_string();
    if text.len() != 8 {
        return None;
    }
    Some(format!(
        "{}-{}-{}",
        &text[0..4],
        &text[4..6],
        &text[6..8]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minute_periods_map_to_protocol_categories() {
        assert_eq!(category_of(1), Some(7));
        assert_eq!(category_of(5), Some(0));
        assert_eq!(category_of(15), Some(1));
        assert_eq!(category_of(30), Some(2));
        assert_eq!(category_of(60), Some(3));
        assert_eq!(category_of(2), None);
    }

    #[test]
    fn market_numbers_follow_the_protocol() {
        assert_eq!(market_of("600519"), 1);
        assert_eq!(market_of("000001"), 0);
        assert_eq!(market_of("920001"), 2);
    }

    #[test]
    fn xdxr_date_converts_to_iso() {
        assert_eq!(yyyymmdd_to_date(20260626).as_deref(), Some("2026-06-26"));
        assert_eq!(yyyymmdd_to_date(0), None);
        assert_eq!(yyyymmdd_to_date(2026062), None);
    }

    /// 指数与基金不能按代码前缀混进另一个市场。
    #[test]
    fn market_number_must_agree_with_the_code_prefix() {
        // 深市列表只收深市代码。
        assert!(market_matches("000001", 0));
        assert!(market_matches("300750", 0));
        assert!(!market_matches("600000", 0));
        assert!(!market_matches("688981", 0));
        // 沪市列表只收沪市代码：上证指数系列的 000xxx 与深市股票同号，必须剔掉。
        assert!(market_matches("600000", 1));
        assert!(market_matches("688981", 1));
        assert!(!market_matches("000001", 1), "上证指数不能被当成深市股票");
        assert!(!market_matches("300750", 1));
        // 北京证券交易所的列表没取，一律不收。
        assert!(!market_matches("920821", 2));
    }

    #[test]
    fn candidate_servers_put_verified_hosts_first() {
        let list = candidates();
        assert_eq!(list[0].to_string(), SERVERS[0]);
        assert_eq!(list.len(), SERVERS.len() + STOCK_IP.len());
    }
}

//! 分钟线抓取与盘中轮询。
//!
//! 分钟线走东方财富 K 线接口，`klt` 取 `1/5/15/30/60`：`klt=1` 只返回当日，
//! `klt=5/15/30/60` 只返回最近 32 个交易日，这是服务端的固定窗口，与请求参数无关
//! （见 `docs/research/quote-api-spec.md` 第 1.8 节）。
//!
//! 轮询每分钟一次，对象是「已标注支撑区、处于等待触发状态的股票」加「持仓」；
//! 每只股票抓当日 1 分钟线写进 `bar_minute`，再交给 [`crate::signal`] 判定触发。

use crate::error::{CoreError, Result};
use crate::market::{eastmoney, MinuteBar};
use crate::storage::{bar_minute, position, signal as signal_store};
use rusqlite::Connection;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::io;
use std::time::Duration;

/// 1 分钟周期。触发判定用这个周期。
pub const PERIOD_1M: u32 = 1;
/// 接口支持的全部分钟周期。
pub const PERIODS: [u32; 5] = [1, 5, 15, 30, 60];
/// 单次请求的失败重试次数。
const RETRIES: usize = 2;

/// 抓取某只股票某个周期分钟线的 URL。
///
/// `klt=1` 会无视 `beg`/`end`/`lmt`，总是返回当前交易日的全部分钟线，所以这里只给
/// `end=20500101`，由服务端决定窗口。
pub fn minute_url(secid: &str, period: u32) -> String {
    eastmoney::kline_url(
        secid,
        period,
        eastmoney::FQT_QFQ,
        None,
        Some(eastmoney::END_LATEST),
        None,
    )
}

/// 解析分钟线响应。行格式与日线一致，`f51` 是 `YYYY-MM-DD HH:MM`。
pub fn parse_minute_bars(body: &str, period: u32) -> Result<Vec<MinuteBar>> {
    let page = eastmoney::parse_kline(body)?;
    Ok(page
        .bars
        .into_iter()
        .map(|bar| MinuteBar {
            code: bar.code,
            ts: bar.date,
            period,
            open: bar.open,
            high: bar.high,
            low: bar.low,
            close: bar.close,
            volume: bar.volume,
            amount: bar.amount,
        })
        .collect())
}

/// 分钟线抓取的抽象，便于用录制样本回放测试。
pub trait MinuteFetch: Send + Sync {
    /// 取一只股票某个周期（`1/5/15/30/60`）的分钟线原始响应。
    fn minute_kline(&self, secid: &str, period: u32) -> Result<String>;
}

/// 走真实网络的分钟线抓取。
pub struct HttpMinuteFetch {
    client: Client,
}

impl HttpMinuteFetch {
    /// 建一个带超时与有限重试的客户端。
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(60))
            .user_agent("springload/0.1")
            .build()
            .map_err(|e| CoreError::Io(io::Error::other(format!("构建 HTTP 客户端失败: {e}"))))?;
        Ok(Self { client })
    }
}

impl MinuteFetch for HttpMinuteFetch {
    fn minute_kline(&self, secid: &str, period: u32) -> Result<String> {
        get_text(&self.client, &minute_url(secid, period))
    }
}

/// 抓一只股票某个周期的分钟线并写进 `bar_minute`，返回写入的行数。
pub fn refresh_minute_bars(
    conn: &mut Connection,
    fetch: &dyn MinuteFetch,
    code: &str,
    period: u32,
) -> Result<usize> {
    let body = fetch.minute_kline(&eastmoney::secid(code), period)?;
    let bars = parse_minute_bars(&body, period)?;
    bar_minute::upsert_many(conn, &bars)
}

/// 一次轮询的结果，作为 `poll://tick` 事件的载荷。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PollReport {
    /// 本轮刷新的「已标注支撑区、处于等待触发状态」的股票。
    pub waiting: Vec<String>,
    /// 本轮刷新的持仓股票。
    pub positions: Vec<String>,
    /// 本轮新触发信号的股票。
    pub triggered: Vec<String>,
}

/// 本轮轮询的对象：等待触发的股票与持仓股票，各按代码升序。
pub fn poll_targets(conn: &Connection) -> Result<(Vec<String>, Vec<String>)> {
    let waiting = signal_store::codes_waiting(conn)?;
    let positions: Vec<String> = position::list_open(conn)?
        .into_iter()
        .map(|row| row.code)
        .collect();
    Ok((waiting, positions))
}

/// 轮询一次：刷新等待触发的股票与持仓的当日 1 分钟线，判定触发并落库。
///
/// 抓取失败直接向上返回，由命令层转成可展示文案，不静默吞掉。
pub fn poll_once(conn: &mut Connection, fetch: &dyn MinuteFetch) -> Result<PollReport> {
    let (waiting, positions) = poll_targets(conn)?;
    let mut codes: Vec<String> = Vec::new();
    for code in waiting.iter().chain(positions.iter()) {
        if !codes.contains(code) {
            codes.push(code.clone());
        }
    }

    let mut triggered = Vec::new();
    for code in &codes {
        refresh_minute_bars(conn, fetch, code, PERIOD_1M)?;
        let status = crate::signal::evaluate(conn, code)?;
        if status.stage == crate::signal::Stage::Triggered {
            if let Some(trigger) = &status.trigger {
                crate::signal::record_trigger(conn, code, status.zone.as_ref(), trigger)?;
                triggered.push(code.clone());
            }
        }
    }

    Ok(PollReport {
        waiting,
        positions,
        triggered,
    })
}

/// 走 HTTP 取文本，失败按 [`RETRIES`] 重试。
fn get_text(client: &Client, url: &str) -> Result<String> {
    let mut last_error = None;
    for _ in 0..=RETRIES {
        match client.get(url).send() {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    return response
                        .text()
                        .map_err(|e| CoreError::Io(io::Error::other(format!("读取响应失败: {e}"))));
                }
                last_error = Some(format!("HTTP {}", status.as_u16()));
            }
            Err(error) => last_error = Some(error.to_string()),
        }
    }
    Err(CoreError::Io(io::Error::other(format!(
        "请求分钟线失败: {}",
        last_error.unwrap_or_else(|| "未知错误".to_string())
    ))))
}

/// 一轮轮询结束后的回调。命令层用它向界面推送 `poll://tick` 事件。
pub type PollSink = std::sync::Arc<dyn Fn(PollReport) + Send + Sync>;

/// 每分钟轮询一次，直到进程退出。
///
/// 单轮失败只跳过这一轮，不终止循环；取不到数据库连接或建不出 HTTP 客户端时同样跳过。
pub fn spawn_poll_scheduler(db: crate::storage::Database, sink: PollSink) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(60));
        let Ok(mut conn) = db.conn() else {
            continue;
        };
        let Ok(fetch) = HttpMinuteFetch::new() else {
            continue;
        };
        if let Ok(report) = poll_once(&mut conn, &fetch) {
            sink(report);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-13 实抓的 1 分钟线样本（`klt=1`，只返回当日）。
    const KLT1: &str = include_str!("../tests/fixtures/eastmoney_minute_600000_klt1.json");
    /// 2026-10-13 实抓的 5 分钟线样本（`klt=5`，最近 32 个交易日）。
    const KLT5: &str = include_str!("../tests/fixtures/eastmoney_minute_600000_klt5.json");

    #[test]
    fn minute_url_carries_the_period_and_no_range_parameters() {
        let url = minute_url("1.600000", PERIOD_1M);
        assert!(url.contains("klt=1"), "URL 应带 klt=1：{url}");
        assert!(url.contains("fqt=1"), "分钟线取前复权：{url}");
        assert!(url.contains(&format!("end={}", eastmoney::END_LATEST)));
        assert!(!url.contains("beg="), "分钟线不给 beg：{url}");
        assert!(!url.contains("lmt="), "分钟线不给 lmt：{url}");
    }

    #[test]
    fn parses_the_recorded_one_minute_sample() {
        let bars = parse_minute_bars(KLT1, PERIOD_1M).expect("样本可解析");
        assert!(!bars.is_empty(), "样本应有分钟线");
        assert!(bars.iter().all(|bar| bar.code == "600000"));
        assert!(bars.iter().all(|bar| bar.period == PERIOD_1M));
        assert_eq!(bars[0].ts, "2026-10-13 09:31");
        assert_eq!(bars[0].open, 9.67);
        assert_eq!(bars[0].high, 9.70);
        assert_eq!(bars[0].low, 9.62);
        assert_eq!(bars[0].close, 9.66);
        // 成交量单位是股：接口给的是手（30558 手 → 3055800 股）。
        assert_eq!(bars[0].volume, 3_055_800.0);
    }

    #[test]
    fn parses_the_recorded_five_minute_sample() {
        let bars = parse_minute_bars(KLT5, 5).expect("样本可解析");
        assert_eq!(bars.len(), 3, "klt=5 的样本只有当日尾部的三根");
        assert!(bars.iter().all(|bar| bar.period == 5));
        assert_eq!(bars[0].ts, "2026-10-13 14:50");
    }
}

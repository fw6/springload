//! 分钟线抓取与盘中轮询。
//!
//! 分钟线走当前市场的行情源，周期取 `1/5/15/30/60`。A 股协议给的是不复权原始价，
//! 写入前按该股票的除权除息事件换算成前复权，与日线用同一套公式；美股给的已经是
//! 复权价，事件表为空，换算退化成原样写入。
//!
//! 轮询每分钟一次，对象是「已标注支撑区、处于等待触发状态的股票」加「持仓」；
//! 每只股票抓当日 1 分钟线写进 `bar_minute`，再交给 [`crate::signal`] 判定触发。

use crate::adjustment::adjust_price;
use crate::error::Result;
use crate::market::{MinuteBar, QuoteSource};
use crate::storage::corporate_action::{self, CorporateAction};
use crate::storage::{bar_minute, position, signal as signal_store};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// 1 分钟周期。触发判定用这个周期。
pub const PERIOD_1M: u32 = 1;
/// 接口支持的全部分钟周期。
pub const PERIODS: [u32; 5] = [1, 5, 15, 30, 60];

/// 把一根不复权分钟线按除权除息事件换算成前复权。
///
/// 除权日当天的价格已经是不复权价，所以事件比较用的是时间戳的日期部分。价格保留
/// 两位小数，与日线一致。
fn adjust_minute_bar(bar: MinuteBar, events: &[CorporateAction]) -> MinuteBar {
    let date = bar.ts.get(0..10).unwrap_or(bar.ts.as_str());
    let round = |value: f64| (value * 100.0).round() / 100.0;
    MinuteBar {
        open: round(adjust_price(bar.open, date, events)),
        high: round(adjust_price(bar.high, date, events)),
        low: round(adjust_price(bar.low, date, events)),
        close: round(adjust_price(bar.close, date, events)),
        ..bar
    }
}

/// 抓一只股票某个周期的分钟线并写进 `bar_minute`，返回写入的行数。
pub fn refresh_minute_bars(
    conn: &mut Connection,
    source: &dyn QuoteSource,
    code: &str,
    period: u32,
) -> Result<usize> {
    let bars = source.minute_bars(code, period)?;
    let events = corporate_action::list_for_code(conn, code)?;
    let adjusted: Vec<MinuteBar> = bars
        .into_iter()
        .map(|bar| adjust_minute_bar(bar, &events))
        .collect();
    bar_minute::upsert_many(conn, &adjusted)
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
pub fn poll_once(conn: &mut Connection, source: &dyn QuoteSource) -> Result<PollReport> {
    let (waiting, positions) = poll_targets(conn)?;
    let mut codes: Vec<String> = Vec::new();
    for code in waiting.iter().chain(positions.iter()) {
        if !codes.contains(code) {
            codes.push(code.clone());
        }
    }

    let mut triggered = Vec::new();
    for code in &codes {
        refresh_minute_bars(conn, source, code, PERIOD_1M)?;
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

/// 一轮轮询结束后的回调。命令层用它向界面推送 `poll://tick` 事件。
pub type PollSink = std::sync::Arc<dyn Fn(PollReport) + Send + Sync>;

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(ts: &str, close: f64) -> MinuteBar {
        MinuteBar {
            code: "600519".to_string(),
            ts: ts.to_string(),
            period: PERIOD_1M,
            open: close,
            high: close,
            low: close,
            close,
            volume: 1000.0,
            amount: 1_000_000.0,
        }
    }

    fn action(ex_date: &str, dividend_per_share: f64) -> CorporateAction {
        CorporateAction {
            code: "600519".to_string(),
            ex_date: ex_date.to_string(),
            dividend_per_share,
            split_ratio: 0.0,
            pre_close: None,
            recorded_at: "2026-10-15T00:00:00Z".to_string(),
        }
    }

    /// 除权日之后的价格要减掉每股分红，除权日当天及之前的减。
    #[test]
    fn minute_bars_before_the_ex_date_are_adjusted() {
        let events = vec![action("2026-10-14", 2.0)];
        let before = adjust_minute_bar(bar("2026-10-13 09:31", 100.0), &events);
        assert_eq!(before.close, 98.0);
        let on_ex_date = adjust_minute_bar(bar("2026-10-14 09:31", 100.0), &events);
        assert_eq!(on_ex_date.close, 100.0, "除权日当天已经是不复权价");
        let after = adjust_minute_bar(bar("2026-10-15 09:31", 100.0), &events);
        assert_eq!(after.close, 100.0);
    }

    /// 成交量与成交额不参与复权换算。
    #[test]
    fn adjustment_keeps_volume_and_amount() {
        let adjusted = adjust_minute_bar(bar("2026-10-13 09:31", 100.0), &[action("2026-10-14", 2.0)]);
        assert_eq!(adjusted.volume, 1000.0);
        assert_eq!(adjusted.amount, 1_000_000.0);
        assert_eq!(adjusted.period, PERIOD_1M);
    }
}

//! 两级触发判定：日线的前置条件与分钟线的触发条件。
//!
//! - 前置条件（日线）：价格站上高置信支撑区。高置信指 `support_zone.confidence = '高'`，
//!   由 #16 计算写入，这里只读。
//! - 触发条件（分钟线）：先出现一次下探后回升，随后某一根 K 线的最高价超过前一根
//!   K 线的最高价。
//! - 价格跌穿支撑区且没有回升时保持等待，不发出任何提示。
//!
//! 「站上」与「跌穿」以支撑区下沿为界：价格不低于下沿算站上，低于下沿算跌穿。

use crate::error::Result;
use crate::market::MinuteBar;
use crate::minute::PERIOD_1M;
use crate::storage::support_zone::SupportZone;
use crate::storage::{bar, bar_minute, now, signal, support_zone};
use rusqlite::Connection;

/// 高置信支撑区的置信度取值。
pub const HIGH_CONFIDENCE: &str = "高";
/// 触发信号的时间周期。
pub const TIMEFRAME_1M: &str = "1m";
/// 已触发信号在 `signal.status` 里的取值。
pub const STATUS_TRIGGERED: &str = "triggered";

/// 当前卡在哪一级。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    /// 还没有触发，等待前置条件或分钟线触发。
    Waiting,
    /// 已触发。
    Triggered,
    /// 价格跌穿支撑区且没有回升，保持等待。
    Failed,
}

/// 一次触发：某根分钟线的最高价超过了前一根的最高价。
#[derive(Debug, Clone, PartialEq)]
pub struct Trigger {
    /// 触发那根分钟线的时间，`YYYY-MM-DD HH:MM`。
    pub ts: String,
    /// 触发那根分钟线的最高价。
    pub broken_high: f64,
    /// 前一根分钟线的最高价。
    pub pre_high: f64,
}

/// 一次判定的结果。
#[derive(Debug, Clone, PartialEq)]
pub struct SignalStatus {
    pub stage: Stage,
    /// 判定所依据的高置信支撑区。
    pub zone: Option<SupportZone>,
    /// 当前卡在哪一级的可展示说明。
    pub detail: String,
    /// 本次新判定出的触发；已经落库或尚未触发时为 `None`。
    pub trigger: Option<Trigger>,
}

/// 在分钟线上找「下探回升 + 突破前一根高点」。
///
/// 1. 下探：窗口内（不含最后一根，给回升留出空间）最低价的那一根。
/// 2. 回升：下探之后某一根的收盘价重新站上下探那根的最高价。
/// 3. 突破：回升之后某一根的最高价超过前一根的最高价。
///
/// 返回最早满足的一条；条件不齐时返回 `None`。
pub fn detect_trigger(bars: &[MinuteBar]) -> Option<Trigger> {
    if bars.len() < 3 {
        return None;
    }
    let dip = bars[..bars.len() - 1]
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.low.total_cmp(&b.low))
        .map(|(index, _)| index)?;
    let recovery = (dip + 1..bars.len()).find(|&index| bars[index].close > bars[dip].high)?;
    (recovery..bars.len())
        .find(|&index| bars[index].high > bars[index - 1].high)
        .map(|index| Trigger {
            ts: bars[index].ts.clone(),
            broken_high: bars[index].high,
            pre_high: bars[index - 1].high,
        })
}

/// 从高置信支撑区里挑出与现价相关的一块。
///
/// 取「下沿不高于现价、下沿最高」的一块（离现价最近的支撑）；现价低于全部支撑时，
/// 取下沿最高的一块（刚被跌穿的那块）。下沿相同时按 id 升序取第一块，保证结果稳定。
pub fn pick_zone<'a>(zones: &'a [SupportZone], price: f64) -> Option<&'a SupportZone> {
    let highest = |a: &&SupportZone, b: &&SupportZone| {
        a.low
            .total_cmp(&b.low)
            .then_with(|| b.id.cmp(&a.id))
    };
    zones
        .iter()
        .filter(|zone| zone.low <= price)
        .max_by(highest)
        .or_else(|| zones.iter().max_by(highest))
}

/// 判定某只股票当前卡在哪一级。
///
/// 已有触发信号时直接返回已触发；否则用日线收盘价与已入库的分钟线判定。
/// 不抓取网络，也不写库。
pub fn evaluate(conn: &Connection, code: &str) -> Result<SignalStatus> {
    let zones: Vec<SupportZone> = support_zone::list_for_code(conn, code)?
        .into_iter()
        .filter(|zone| zone.confidence == HIGH_CONFIDENCE)
        .collect();

    if let Some(existing) = signal::latest_for_code(conn, code)? {
        if existing.status == "triggered" {
            let zone = existing
                .zone_id
                .as_deref()
                .and_then(|id| zones.iter().find(|zone| zone.id == id).cloned())
                .or_else(|| pick_zone(&zones, existing.candle_high_broken).cloned());
            return Ok(SignalStatus {
                stage: Stage::Triggered,
                zone,
                detail: format!(
                    "已触发：{} · {} · 本根最高 {} 超过前一根最高 {}。",
                    existing.triggered_at,
                    existing.timeframe,
                    existing.candle_high_broken,
                    existing.pre_candle_high
                ),
                trigger: None,
            });
        }
    }

    let minutes = bar_minute::list(conn, code, PERIOD_1M)?;
    let daily_close = bar::list_for_code(conn, code)?
        .iter()
        .rev()
        .find(|row| row.volume > 0.0)
        .map(|row| row.close);
    let price = minutes.last().map(|bar| bar.close).or(daily_close);

    let Some(price) = price else {
        return Ok(SignalStatus {
            stage: Stage::Waiting,
            zone: None,
            detail: "还没有这只股票的日线数据，无法判断前置条件。".to_string(),
            trigger: None,
        });
    };

    let Some(zone) = pick_zone(&zones, price).cloned() else {
        return Ok(SignalStatus {
            stage: Stage::Waiting,
            zone: None,
            detail: "前置条件未满足：还没有高置信支撑区，先去第 2 步标注并让它达到高置信。"
                .to_string(),
            trigger: None,
        });
    };

    if price < zone.low {
        let detail = format!(
            "价格 {price} 已跌穿高置信支撑区下沿 {} 且没有回升，保持等待，不发出提示。",
            zone.low
        );
        return Ok(SignalStatus {
            stage: Stage::Failed,
            zone: Some(zone),
            detail,
            trigger: None,
        });
    }

    match detect_trigger(&minutes) {
        Some(trigger) => Ok(SignalStatus {
            stage: Stage::Triggered,
            detail: format!(
                "已触发：{} · 1 分钟 · 本根最高 {} 超过前一根最高 {}。",
                trigger.ts, trigger.broken_high, trigger.pre_high
            ),
            zone: Some(zone),
            trigger: Some(trigger),
        }),
        None => Ok(SignalStatus {
            stage: Stage::Waiting,
            zone: Some(zone),
            detail: "前置条件已满足：价格站上高置信支撑区，等待分钟图出现下探回升后突破前一根高点。"
                .to_string(),
            trigger: None,
        }),
    }
}

/// 把一次触发写进 `signal` 表。
///
/// id 由代码与触发时间推出，同一次触发重复写入只覆盖同一行。
pub fn record_trigger(
    conn: &Connection,
    code: &str,
    zone: Option<&SupportZone>,
    trigger: &Trigger,
) -> Result<signal::Signal> {
    let row = signal::Signal {
        id: format!("signal-{code}-{}", trigger.ts.replace([' ', ':'], "")),
        code: code.to_string(),
        triggered_at: trigger.ts.clone(),
        timeframe: TIMEFRAME_1M.to_string(),
        candle_high_broken: trigger.broken_high,
        pre_candle_high: trigger.pre_high,
        zone_id: zone.map(|zone| zone.id.clone()),
        status: STATUS_TRIGGERED.to_string(),
        created_at: now(),
    };
    signal::upsert(conn, &row)?;
    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 造一根分钟线。
    fn bar(ts: &str, high: f64, low: f64, close: f64) -> MinuteBar {
        MinuteBar {
            code: "600000".to_string(),
            ts: format!("2026-10-13 {ts}"),
            period: PERIOD_1M,
            open: close,
            high,
            low,
            close,
            volume: 1000.0,
            amount: close * 1000.0,
        }
    }

    /// 造一条支撑区，只有 id、上下沿与置信度是关心的。
    fn zone(id: &str, low: f64, high: f64, confidence: &str) -> SupportZone {
        SupportZone {
            id: id.to_string(),
            code: "600000".to_string(),
            kind: "support".to_string(),
            low,
            high,
            anchor_date: None,
            ema9: None,
            ema21: None,
            ema50: None,
            avwap_start_date: None,
            avwap_price: None,
            prev_high: None,
            prev_low: None,
            gap_top: None,
            gap_bottom: None,
            confidence: confidence.to_string(),
            spread: None,
            created_at: now(),
        }
    }

    #[test]
    fn needs_at_least_three_bars() {
        assert!(detect_trigger(&[]).is_none());
        assert!(detect_trigger(&[bar("09:31", 10.0, 9.9, 9.95)]).is_none());
        assert!(detect_trigger(&[bar("09:31", 10.0, 9.9, 9.95), bar("09:32", 10.1, 9.9, 10.0)]).is_none());
    }

    #[test]
    fn dip_recovery_and_breakout_triggers() {
        let bars = [
            bar("09:31", 10.03, 9.95, 9.97),
            bar("09:32", 10.00, 9.96, 9.99),
            bar("09:33", 10.04, 9.99, 10.02),
            bar("09:34", 10.06, 10.01, 10.05),
        ];
        let trigger = detect_trigger(&bars).expect("应触发");
        assert_eq!(trigger.ts, "2026-10-13 09:34");
        assert_eq!(trigger.broken_high, 10.06);
        assert_eq!(trigger.pre_high, 10.04);
    }

    #[test]
    fn recovery_without_breaking_the_previous_high_does_not_trigger() {
        let bars = [
            bar("09:31", 10.03, 9.95, 9.97),
            bar("09:32", 10.02, 9.96, 10.01),
            bar("09:33", 10.00, 9.99, 10.00),
            bar("09:34", 10.01, 9.99, 10.00),
        ];
        assert!(detect_trigger(&bars).is_none());
    }

    #[test]
    fn no_recovery_above_the_dip_high_does_not_trigger() {
        // 一路走低：下探之后收盘价始终没有回到下探那根的最高价之上。
        let bars = [
            bar("09:31", 10.03, 9.98, 9.99),
            bar("09:32", 9.99, 9.90, 9.92),
            bar("09:33", 9.93, 9.85, 9.86),
            bar("09:34", 9.87, 9.80, 9.81),
        ];
        assert!(detect_trigger(&bars).is_none());
    }

    #[test]
    fn picks_the_support_closest_below_the_price() {
        let zones = vec![zone("a", 9.0, 9.2, "高"), zone("b", 10.0, 10.2, "高")];
        assert_eq!(pick_zone(&zones, 10.5).unwrap().id, "b");
        // 现价低于全部支撑时取下沿最高的那块（刚被跌穿的那块）。
        assert_eq!(pick_zone(&zones, 8.0).unwrap().id, "b");
        // 下沿相同时按 id 升序取第一块，结果稳定。
        let tied = vec![zone("b", 10.0, 10.2, "高"), zone("a", 10.0, 10.2, "高")];
        assert_eq!(pick_zone(&tied, 10.5).unwrap().id, "a");
    }

    #[test]
    fn no_zone_picks_nothing() {
        assert!(pick_zone(&[], 10.0).is_none());
    }
}

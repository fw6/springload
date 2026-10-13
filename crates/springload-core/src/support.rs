//! 支撑区置信度：三个参考价（EMA21、AVWAP、前期高点）的离散度与落框计数。
//!
//! 这里是置信度计算的唯一实现，命令层（`support_zone_save` / `support_refs`）与
//! 收盘后批处理共用它。数值规则照原型 `data/market.ts` 的 `evaluateZone` 与
//! `flow/useFlow.ts` 的落框计数。

use crate::error::{CoreError, Result};
use crate::storage::bar::BarRow;
use serde::{Deserialize, Serialize};

/// AVWAP 未指定起点时，从末尾往前取这么多根交易日作为起点。
pub const AVWAP_DEFAULT_BARS_BACK: usize = 60;
/// 前期高点的回看窗口长度（交易日）。
const PREV_HIGH_LOOKBACK: usize = 60;
/// 前期高点的窗口距末尾跳过的交易日数，把最近的整理区排除在外。
const PREV_HIGH_SKIP: usize = 20;

/// 三个参考价。`None` 表示该参考价数据不足。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneRefs {
    pub ema21: Option<f64>,
    pub avwap: Option<f64>,
    pub prev_high: Option<f64>,
}

/// 置信度等级。序列化成中文，与 `support_zone.confidence` 列一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Confidence {
    #[serde(rename = "高")]
    High,
    #[serde(rename = "中")]
    Medium,
    #[serde(rename = "低")]
    Low,
}

impl Confidence {
    /// 存进 `support_zone.confidence` 的中文取值。
    pub fn as_str(self) -> &'static str {
        match self {
            Confidence::High => "高",
            Confidence::Medium => "中",
            Confidence::Low => "低",
        }
    }
}

/// 三个参考价的离散度评估。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RefEvaluation {
    pub confidence: Confidence,
    /// 离散度 `(max − min) / mean`；有效值少于两个时为 `None`。
    pub spread: Option<f64>,
}

/// 一条标注（框）的评估：置信度、离散度与落在框内的参考价个数。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxEvaluation {
    pub confidence: Confidence,
    pub spread: Option<f64>,
    /// 落在 `[low, high]` 内的参考价个数，最多 3。
    pub hits: u8,
}

/// 指标快照：三条均线、AVWAP 与前后期高低点。供写入 `support_zone` 行使用。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoneSnapshot {
    pub ema9: Option<f64>,
    pub ema21: Option<f64>,
    pub ema50: Option<f64>,
    pub avwap: Option<f64>,
    pub prev_high: Option<f64>,
    pub prev_low: Option<f64>,
}

impl ZoneSnapshot {
    /// 三个参考价。
    pub fn refs(&self) -> ZoneRefs {
        ZoneRefs {
            ema21: self.ema21,
            avwap: self.avwap,
            prev_high: self.prev_high,
        }
    }
}

/// 三个参考价的离散度等级（原型 `evaluateZone`）。
///
/// 有效值少于两个直接低置信、离散度为空；`≤ 2%` 高、`≤ 5%` 中、其余低。
pub fn evaluate_zone(refs: &ZoneRefs) -> RefEvaluation {
    let values = finite_values(refs);
    if values.len() < 2 {
        return RefEvaluation {
            confidence: Confidence::Low,
            spread: None,
        };
    }
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let spread = (max - min) / mean;
    let confidence = if spread <= 0.02 {
        Confidence::High
    } else if spread <= 0.05 {
        Confidence::Medium
    } else {
        Confidence::Low
    };
    RefEvaluation {
        confidence,
        spread: Some(spread),
    }
}

/// 一个框的置信度与落框计数（原型落框计数规则）。
///
/// 落在 `[low, high]` 内的参考价少于两个时直接低置信；达到两个时沿用三价离散度等级。
pub fn evaluate_box(refs: &ZoneRefs, low: f64, high: f64) -> BoxEvaluation {
    let ref_evaluation = evaluate_zone(refs);
    let hits = finite_values(refs)
        .into_iter()
        .filter(|value| *value >= low && *value <= high)
        .count() as u8;
    let confidence = if hits >= 2 {
        ref_evaluation.confidence
    } else {
        Confidence::Low
    };
    BoxEvaluation {
        confidence,
        spread: ref_evaluation.spread,
        hits,
    }
}

/// 从日线算出三条均线、AVWAP 与前后期高低点。
///
/// 停牌日（成交量为 0）不参与任何计算。`avwap_start_date` 为空时按
/// [`AVWAP_DEFAULT_BARS_BACK`] 根交易日取默认起点。
pub fn snapshot(bars: &[BarRow], avwap_start_date: Option<&str>) -> Result<ZoneSnapshot> {
    let traded: Vec<&BarRow> = bars.iter().filter(|bar| bar.volume > 0.0).collect();
    let closes: Vec<f64> = traded.iter().map(|bar| bar.close).collect();
    let (prev_high, prev_low) = prev_extremes(&traded);
    let avwap = match avwap_start_date {
        Some(date) => {
            let start = traded
                .iter()
                .position(|bar| bar.date.as_str() >= date)
                .ok_or_else(|| {
                    CoreError::InvalidData(format!("AVWAP 起点 {date} 之后没有交易日"))
                })?;
            avwap_from(&traded, start)
        }
        None => avwap_from(&traded, traded.len().saturating_sub(AVWAP_DEFAULT_BARS_BACK)),
    };
    Ok(ZoneSnapshot {
        ema9: ema_last(&closes, 9),
        ema21: ema_last(&closes, 21),
        ema50: ema_last(&closes, 50),
        avwap,
        prev_high,
        prev_low,
    })
}

/// 只取三个参考价。
pub fn reference_prices(bars: &[BarRow], avwap_start_date: Option<&str>) -> Result<ZoneRefs> {
    Ok(snapshot(bars, avwap_start_date)?.refs())
}

/// 三个参考价里有限的那些。
fn finite_values(refs: &ZoneRefs) -> Vec<f64> {
    [refs.ema21, refs.avwap, refs.prev_high]
        .into_iter()
        .flatten()
        .filter(|value| value.is_finite())
        .collect()
}

/// 收盘价序列的 EMA 末值。不足 `period` 根返回 `None`。
fn ema_last(closes: &[f64], period: usize) -> Option<f64> {
    if period == 0 || closes.len() < period {
        return None;
    }
    let alpha = 2.0 / (period as f64 + 1.0);
    let mut value = closes[..period].iter().sum::<f64>() / period as f64;
    for &close in &closes[period..] {
        value = close * alpha + value * (1.0 - alpha);
    }
    Some(value)
}

/// 从 `start` 起按成交量加权的平均成本价，典型价取 `(high + low + close) / 3`。
fn avwap_from(bars: &[&BarRow], start: usize) -> Option<f64> {
    if start >= bars.len() {
        return None;
    }
    let mut pv = 0.0;
    let mut volume = 0.0;
    for bar in &bars[start..] {
        let typical = (bar.high + bar.low + bar.close) / 3.0;
        pv += typical * bar.volume;
        volume += bar.volume;
    }
    if volume <= 0.0 {
        None
    } else {
        Some(pv / volume)
    }
}

/// 前期高点的窗口：末尾跳过 `PREV_HIGH_SKIP` 根，再往前取 `PREV_HIGH_LOOKBACK` 根。
fn prev_extremes(bars: &[&BarRow]) -> (Option<f64>, Option<f64>) {
    let end = bars.len().saturating_sub(PREV_HIGH_SKIP);
    let start = end.saturating_sub(PREV_HIGH_LOOKBACK);
    if start >= end {
        return (None, None);
    }
    let window = &bars[start..end];
    let high = window.iter().map(|bar| bar.high).fold(f64::NEG_INFINITY, f64::max);
    let low = window.iter().map(|bar| bar.low).fold(f64::INFINITY, f64::min);
    (Some(high), Some(low))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(date: &str, high: f64, low: f64, close: f64, volume: f64) -> BarRow {
        BarRow {
            code: "600000".to_string(),
            date: date.to_string(),
            open: close,
            high,
            low,
            close,
            volume,
            amount: volume * close,
        }
    }

    fn refs(ema21: f64, avwap: f64, prev_high: f64) -> ZoneRefs {
        ZoneRefs {
            ema21: Some(ema21),
            avwap: Some(avwap),
            prev_high: Some(prev_high),
        }
    }

    #[test]
    fn spread_within_two_percent_is_high() {
        // mean = 100，spread = 2 / 100 = 0.02，正好落在高置信边界上。
        let evaluation = evaluate_zone(&refs(99.0, 100.0, 101.0));
        assert_eq!(evaluation.confidence, Confidence::High);
        assert!((evaluation.spread.unwrap() - 0.02).abs() < 1e-12);
    }

    #[test]
    fn spread_just_above_two_percent_is_medium() {
        // mean = 100，spread = 2.2 / 100 = 0.022。
        let evaluation = evaluate_zone(&refs(98.9, 100.0, 101.1));
        assert_eq!(evaluation.confidence, Confidence::Medium);
    }

    #[test]
    fn spread_within_five_percent_is_medium_at_boundary() {
        // mean = 100，spread = 5 / 100 = 0.05。
        let evaluation = evaluate_zone(&refs(97.5, 100.0, 102.5));
        assert_eq!(evaluation.confidence, Confidence::Medium);
        assert!((evaluation.spread.unwrap() - 0.05).abs() < 1e-12);
    }

    #[test]
    fn spread_above_five_percent_is_low() {
        let evaluation = evaluate_zone(&refs(97.0, 100.0, 103.0));
        assert_eq!(evaluation.confidence, Confidence::Low);
    }

    #[test]
    fn fewer_than_two_references_is_low_without_spread() {
        let evaluation = evaluate_zone(&ZoneRefs {
            ema21: Some(100.0),
            avwap: None,
            prev_high: None,
        });
        assert_eq!(evaluation.confidence, Confidence::Low);
        assert_eq!(evaluation.spread, None);
    }

    #[test]
    fn box_with_two_or_more_hits_keeps_spread_level() {
        // 三价 spread = 0.02 → 高置信。
        let refs = refs(99.0, 100.0, 101.0);
        // 三个都在框内。
        let all = evaluate_box(&refs, 98.5, 101.5);
        assert_eq!(all.hits, 3);
        assert_eq!(all.confidence, Confidence::High);
        // 只有 avwap 与 prevHigh 在框内。
        let two = evaluate_box(&refs, 99.5, 101.5);
        assert_eq!(two.hits, 2);
        assert_eq!(two.confidence, Confidence::High);
    }

    #[test]
    fn box_with_fewer_than_two_hits_is_low() {
        // 三价 spread = 0.02 → 高置信，但框只裹住一个参考价。
        let refs = refs(99.0, 100.0, 101.0);
        let one = evaluate_box(&refs, 100.5, 101.5);
        assert_eq!(one.hits, 1);
        assert_eq!(one.confidence, Confidence::Low);

        let none = evaluate_box(&refs, 90.0, 95.0);
        assert_eq!(none.hits, 0);
        assert_eq!(none.confidence, Confidence::Low);
    }

    #[test]
    fn box_keeps_medium_when_hits_are_enough() {
        // 三价 spread = 0.03 → 中置信。
        let refs = refs(99.0, 100.0, 102.0);
        let evaluation = evaluate_box(&refs, 98.0, 103.0);
        assert_eq!(evaluation.hits, 3);
        assert_eq!(evaluation.confidence, Confidence::Medium);
    }

    #[test]
    fn ema_seed_is_simple_average_then_recursive() {
        // closes = 1..=30，period = 3：种子 = (1+2+3)/3 = 2，α = 0.5。
        let closes: Vec<f64> = (1..=30).map(|n| n as f64).collect();
        // EMA_4 = 4×0.5 + 2×0.5 = 3，之后每根加 1，末值 = 30 − 1 = 29。
        assert_eq!(ema_last(&closes, 3), Some(29.0));
    }

    #[test]
    fn ema_needs_a_full_period() {
        let closes: Vec<f64> = (1..=20).map(|n| n as f64).collect();
        assert_eq!(ema_last(&closes, 21), None);
    }

    #[test]
    fn avwap_weights_by_typical_price_and_volume() {
        let rows = [
            bar("2026-01-01", 10.0, 10.0, 10.0, 100.0), // 典型价 10
            bar("2026-01-02", 20.0, 10.0, 15.0, 100.0), // 典型价 15
            bar("2026-01-03", 30.0, 20.0, 25.0, 300.0), // 典型价 25
        ];
        let owned: Vec<&BarRow> = rows.iter().collect();
        // 从索引 1 起：(15×100 + 25×300) / 400 = 9000 / 400 = 22.5。
        assert_eq!(avwap_from(&owned, 1), Some(22.5));
        // 从索引 0 起：(10×100 + 9000) / 500 = 10000 / 500 = 20。
        assert_eq!(avwap_from(&owned, 0), Some(20.0));
    }

    #[test]
    fn snapshot_skips_suspended_days() {
        let mut rows: Vec<BarRow> = (0..25)
            .map(|i| bar(&format!("2026-01-{:02}", i + 1), 20.0, 10.0, 15.0, 100.0))
            .collect();
        // 中间插一根停牌日（成交量为 0），不影响均线与 AVWAP。
        rows[10] = bar("2026-01-11", 999.0, 1.0, 999.0, 0.0);
        let result = snapshot(&rows, None).unwrap();
        // 25 根里剔除停牌日后剩 24 根，EMA21 可用。
        assert!(result.ema21.is_some());
        // 全为典型价 15 的成交量加权，AVWAP = 15。
        assert!((result.avwap.unwrap() - 15.0).abs() < 1e-12);
    }

    #[test]
    fn snapshot_rejects_start_date_after_all_bars() {
        let rows = vec![bar("2026-01-01", 10.0, 10.0, 10.0, 100.0)];
        let error = snapshot(&rows, Some("2027-01-01")).unwrap_err();
        assert!(error.to_string().contains("2027-01-01"));
    }
}

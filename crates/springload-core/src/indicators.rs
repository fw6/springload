//! 指标计算：EMA、AVWAP、涨幅、波动收敛与 R 倍数盈亏。
//!
//! 停牌日（成交量为 0 或日期缺口）由调用方在传入前剔除，指标不把它当成 0 参与。
//! [`traded_bars`] 负责这一步。

use crate::market::Bar;

/// 剔除没有成交的 bar。`volume` 为 0 表示当天没有交易。
pub fn traded_bars(bars: &[Bar]) -> Vec<Bar> {
    bars.iter().filter(|bar| bar.traded()).cloned().collect()
}

/// EMA(n)：α = 2/(n+1)，首值取前 n 个收盘价的简单平均。
///
/// 前 n−1 位是「数据不足」，用 `NaN` 表示；序列长度不足 n 时整段都是 `NaN`。
pub fn ema(values: &[f64], period: usize) -> Vec<f64> {
    assert!(period > 0, "EMA 的周期必须大于 0");
    let mut out = Vec::with_capacity(values.len());
    if values.is_empty() {
        return out;
    }
    let alpha = 2.0 / (period as f64 + 1.0);
    let mut prev = f64::NAN;
    for (index, value) in values.iter().copied().enumerate() {
        if index + 1 < period {
            out.push(f64::NAN);
        } else if index + 1 == period {
            prev = values[..period].iter().sum::<f64>() / period as f64;
            out.push(prev);
        } else {
            prev = value * alpha + prev * (1.0 - alpha);
            out.push(prev);
        }
    }
    out
}

/// 涨幅：(最后一根收盘价 − N 日前收盘价) ÷ N 日前收盘价。
///
/// 数据不足（长度 ≤ N）或基准价非正时返回 `None`，表示「数据不足」。
pub fn change(closes: &[f64], n: usize) -> Option<f64> {
    if closes.len() <= n {
        return None;
    }
    let last = *closes.last()?;
    let base = closes[closes.len() - 1 - n];
    if base <= 0.0 {
        return None;
    }
    Some((last - base) / base)
}

/// 波动收敛：近 `window` 日的（最高价最大值 − 最低价最小值）÷ 收盘价均值。
///
/// 数据不足（bar 数少于 `window`）或均价非正时返回 `None`。
pub fn convergence(bars: &[Bar], window: usize) -> Option<f64> {
    if window == 0 || bars.len() < window {
        return None;
    }
    let slice = &bars[bars.len() - window..];
    let high = slice.iter().map(|bar| bar.high).fold(f64::NEG_INFINITY, f64::max);
    let low = slice.iter().map(|bar| bar.low).fold(f64::INFINITY, f64::min);
    let mean = slice.iter().map(|bar| bar.close).sum::<f64>() / window as f64;
    if mean <= 0.0 {
        return None;
    }
    Some((high - low) / mean)
}

/// AVWAP：从 `start_index` 起，累计（典型价 × 成交量）÷ 累计成交量。
///
/// 典型价 = (最高 + 最低 + 收盘) ÷ 3。起点越界或累计成交量为 0 时返回 `None`。
pub fn avwap(bars: &[Bar], start_index: usize) -> Option<f64> {
    if start_index >= bars.len() {
        return None;
    }
    let mut price_volume = 0.0;
    let mut volume = 0.0;
    for bar in &bars[start_index..] {
        let typical = (bar.high + bar.low + bar.close) / 3.0;
        price_volume += typical * bar.volume;
        volume += bar.volume;
    }
    if volume == 0.0 {
        return None;
    }
    Some(price_volume / volume)
}

/// R 倍数盈亏：(出场价 − 买入价) ÷ per_r。
///
/// `per_r` 必须为正，否则是调用方的程序性错误。
pub fn r_multiple(exit: f64, entry: f64, per_r: f64) -> f64 {
    assert!(per_r > 0.0, "per_r 必须为正");
    (exit - entry) / per_r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(high: f64, low: f64, close: f64, volume: f64) -> Bar {
        Bar {
            code: "600000".to_string(),
            date: "2026-01-01".to_string(),
            open: close,
            high,
            low,
            close,
            volume,
            amount: close * volume,
        }
    }

    #[test]
    fn ema_seeds_with_simple_average_and_uses_alpha() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0];
        let out = ema(&values, 9);

        // 前 8 位数据不足。
        for value in &out[..8] {
            assert!(value.is_nan());
        }
        // 第 9 位是前 9 个收盘价的简单平均：(1+…+9)/9 = 5。
        assert!((out[8] - 5.0).abs() < 1e-12);
        // 其后 α = 2/10 = 0.2：EMA_10 = 10×0.2 + 5×0.8 = 6；EMA_11 = 11×0.2 + 6×0.8 = 7。
        assert!((out[9] - 6.0).abs() < 1e-12);
        assert!((out[10] - 7.0).abs() < 1e-12);
    }

    #[test]
    fn ema_returns_all_nan_when_shorter_than_period() {
        let out = ema(&[1.0, 2.0, 3.0], 9);
        assert_eq!(out.len(), 3);
        assert!(out.iter().all(|value| value.is_nan()));
    }

    #[test]
    fn ema_of_empty_slice_is_empty() {
        assert!(ema(&[], 9).is_empty());
    }

    #[test]
    fn change_uses_n_bars_back() {
        let closes = vec![10.0, 11.0, 12.0, 13.0];
        // 3 个交易日前的收盘价是 10，涨到 13 → +30%。
        let value = change(&closes, 3).unwrap();
        assert!((value - 0.3).abs() < 1e-12);
    }

    #[test]
    fn change_is_none_when_data_is_insufficient() {
        let closes = vec![10.0, 11.0, 12.0];
        assert_eq!(change(&closes, 3), None);
        assert_eq!(change(&closes, 4), None);
    }

    #[test]
    fn convergence_uses_window_extremes_over_mean_close() {
        // 窗口内最高 11、最低 9，收盘价均值 10 → (11−9)/10 = 0.2。
        let bars = vec![
            bar(10.0, 10.0, 10.0, 100.0),
            bar(11.0, 10.0, 10.0, 100.0),
            bar(10.0, 9.0, 10.0, 100.0),
        ];
        let value = convergence(&bars, 3).unwrap();
        assert!((value - 0.2).abs() < 1e-12);
    }

    #[test]
    fn convergence_is_none_when_window_exceeds_bars() {
        let bars = vec![bar(10.0, 9.0, 10.0, 100.0)];
        assert_eq!(convergence(&bars, 20), None);
    }

    #[test]
    fn avwap_weights_typical_price_by_volume() {
        let bars = vec![
            bar(12.0, 10.0, 11.0, 100.0), // 典型价 11，量 100
            bar(21.0, 19.0, 20.0, 300.0), // 典型价 20，量 300
        ];
        let value = avwap(&bars, 0).unwrap();
        let expected = (11.0 * 100.0 + 20.0 * 300.0) / 400.0;
        assert!((value - expected).abs() < 1e-12);
    }

    #[test]
    fn avwap_from_start_index_ignores_earlier_bars() {
        let bars = vec![
            bar(100.0, 100.0, 100.0, 1000.0),
            bar(12.0, 10.0, 11.0, 100.0),
        ];
        let value = avwap(&bars, 1).unwrap();
        assert!((value - 11.0).abs() < 1e-12);
    }

    #[test]
    fn avwap_is_none_when_start_is_out_of_range_or_volume_is_zero() {
        let bars = vec![bar(12.0, 10.0, 11.0, 0.0)];
        assert_eq!(avwap(&bars, 0), None);
        assert_eq!(avwap(&bars, 1), None);
    }

    #[test]
    fn r_multiple_at_stop_is_minus_one() {
        assert!((r_multiple(19.5, 20.0, 0.5) + 1.0).abs() < 1e-12);
        assert!((r_multiple(21.5, 20.0, 0.5) - 3.0).abs() < 1e-12);
    }

    #[test]
    fn traded_bars_drops_zero_volume_days() {
        let bars = vec![bar(10.0, 9.0, 10.0, 100.0), bar(10.0, 9.0, 10.0, 0.0)];
        let kept = traded_bars(&bars);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].volume, 100.0);
    }
}

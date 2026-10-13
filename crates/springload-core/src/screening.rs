//! 强势股筛选：C1~C4 四条判定、成交额剔除与全市场排名。
//!
//! 四条同时满足才进观察名单；未入选的股票保留每条判定的取值，界面据此写出差哪一条。
//! 判定用的日线先剔除没有成交的 bar（[`crate::indicators::traded_bars`]）。

use crate::config::Settings;
use crate::error::Result;
use crate::indicators;
use crate::market::Bar;
use crate::storage::setting;
use crate::storage::watchlist::{self, WatchlistItem};
use crate::storage::Database;
use chrono::{Local, NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

/// 近 1 个月的交易日数。
pub const CHANGE_1M: usize = 21;
/// 近 3 个月的交易日数。
pub const CHANGE_3M: usize = 63;
/// 近 6 个月的交易日数。
pub const CHANGE_6M: usize = 126;
/// 波动收敛与日均成交额的窗口长度。
pub const CONVERGENCE_WINDOW: usize = 20;
/// 摆动高低点判定时左右各取的 bar 数。
pub const SWING_WINDOW: usize = 3;

/// `setting` 表里存最近一次筛选结果的键。
pub const SCREEN_LAST_KEY: &str = "screen_last";

/// 筛选参数，取自 [`Settings`]。
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenParams {
    /// 近 6 个月涨幅阈值。
    pub chg_6m_threshold: f64,
    /// 近 1 个月涨幅排名的分位区间。
    pub rank_band: (f64, f64),
    /// 近 20 日极差阈值。
    pub convergence_threshold: f64,
    /// 日均成交额下限（元）。
    pub liquidity_threshold: f64,
}

impl ScreenParams {
    /// 从配置取筛选参数。
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            chg_6m_threshold: settings.chg_6m_threshold,
            rank_band: settings.rank_band,
            convergence_threshold: settings.convergence_threshold,
            liquidity_threshold: settings.liquidity_threshold,
        }
    }
}

/// 一只股票的指标与判定。
///
/// `rank_pct` 与 `c1` 要等全市场排名后才定，所以不在这个结构里。
#[derive(Debug, Clone, PartialEq)]
pub struct StockMetrics {
    /// 最新收盘价。
    pub price: f64,
    pub chg_1m: Option<f64>,
    pub chg_3m: Option<f64>,
    pub chg_6m: Option<f64>,
    /// 近 20 日日均成交额（元）。
    pub avg_amount: Option<f64>,
    /// 近 20 日极差。
    pub convergence: Option<f64>,
    /// C2 结构条件。
    pub c2: bool,
    /// C3 均线条件。
    pub c3: bool,
    /// C4 排除条件。
    pub c4: bool,
    /// 近 6 个月涨幅是否过阈值（C1 的前半段）。
    pub chg_6m_ok: bool,
    /// 日均成交额是否不低于阈值。
    pub liquid: bool,
}

/// 一只待筛选的股票。
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub code: String,
    pub name: String,
    pub sector: Option<String>,
    pub metrics: StockMetrics,
}

/// 筛选结果里的一行。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenRow {
    pub code: String,
    pub name: String,
    pub sector: Option<String>,
    /// 最新收盘价。
    pub price: f64,
    pub chg_1m: Option<f64>,
    pub chg_3m: Option<f64>,
    pub chg_6m: Option<f64>,
    /// 近 1 个月涨幅的全市场排名分位。
    pub rank_pct: Option<f64>,
    pub avg_amount: Option<f64>,
    pub convergence: Option<f64>,
    pub c1: bool,
    pub c2: bool,
    pub c3: bool,
    pub c4: bool,
    /// 日均成交额是否达到阈值。
    pub liquid: bool,
}

impl ScreenRow {
    /// 转成观察名单的一行。`added_at` 是加进名单的时刻，`screen_run_at` 是产生这条判定的筛选时刻。
    pub fn to_watchlist_item(&self, added_at: &str, screen_run_at: &str) -> WatchlistItem {
        WatchlistItem {
            code: self.code.clone(),
            added_at: added_at.to_string(),
            c1: self.c1,
            c2: self.c2,
            c3: self.c3,
            c4: self.c4,
            chg_1m: self.chg_1m,
            chg_3m: self.chg_3m,
            chg_6m: self.chg_6m,
            rank_pct: self.rank_pct,
            avg_amount: self.avg_amount,
            convergence: self.convergence,
            screen_run_at: screen_run_at.to_string(),
            manual: false,
        }
    }
}

/// 一次筛选的结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenOutcome {
    /// 参与筛选的股票数。
    pub universe: usize,
    pub picked: Vec<ScreenRow>,
    pub rejected: Vec<ScreenRow>,
}

/// 一次筛选的完整报告，`screen_last` 命令的返回。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenReport {
    pub ran_at: String,
    pub universe: u64,
    pub elapsed_ms: u64,
    pub picked: Vec<ScreenRow>,
    pub rejected: Vec<ScreenRow>,
}

/// 算一只股票的指标与四条判定。
pub fn metrics(bars: &[Bar], params: &ScreenParams) -> StockMetrics {
    let traded = indicators::traded_bars(bars);
    let closes: Vec<f64> = traded.iter().map(|bar| bar.close).collect();
    let price = closes.last().copied().unwrap_or(0.0);

    let chg_1m = indicators::change(&closes, CHANGE_1M);
    let chg_3m = indicators::change(&closes, CHANGE_3M);
    let chg_6m = indicators::change(&closes, CHANGE_6M);
    let convergence = indicators::convergence(&traded, CONVERGENCE_WINDOW);
    let avg_amount = average_amount(&traded, CONVERGENCE_WINDOW);

    let ema9 = indicators::ema(&closes, 9);
    let ema21 = indicators::ema(&closes, 21);
    let ema9_last = ema9.last().copied().unwrap_or(f64::NAN);
    let ema21_last = ema21.last().copied().unwrap_or(f64::NAN);
    let ema9_prev = second_last(&ema9);
    let ema21_prev = second_last(&ema21);

    // C3：收盘价 > EMA9，且 EMA9 与 EMA21 走平或向上。
    let c3 = price > ema9_last
        && ema9_last >= ema9_prev
        && ema21_last >= ema21_prev
        && ema9_last.is_finite()
        && ema21_last.is_finite();

    // C4（排除）：高点与低点均下降，且收盘价 < 向下的 EMA21。
    let lows = swing_lows(&traded, SWING_WINDOW);
    let highs = swing_highs(&traded, SWING_WINDOW);
    let lows_falling = last_two(&lows).is_some_and(|(prev, last)| {
        traded[last].low < traded[prev].low
    });
    let highs_falling = last_two(&highs).is_some_and(|(prev, last)| {
        traded[last].high < traded[prev].high
    });
    let c4 = highs_falling && lows_falling && price < ema21_last && ema21_last < ema21_prev;

    // C2：当前回调低点高于上一次回调低点，且近 20 日极差低于阈值。
    let pullback_low_rising = last_two(&lows).is_some_and(|(prev, last)| {
        traded[last].low > traded[prev].low
    });
    let c2 = pullback_low_rising
        && convergence.is_some_and(|value| value < params.convergence_threshold);

    StockMetrics {
        price,
        chg_1m,
        chg_3m,
        chg_6m,
        avg_amount,
        convergence,
        c2,
        c3,
        c4,
        chg_6m_ok: chg_6m.is_some_and(|value| value > params.chg_6m_threshold),
        liquid: avg_amount.is_some_and(|value| value >= params.liquidity_threshold),
    }
}

/// 按近 1 个月涨幅做全市场排名，赋分位并判定 C1，分成入选与未入选两组。
///
/// 排名按涨幅降序；没有涨幅的排在最后。分位取 `(名次) / 总数`，
/// C1 要求分位落在 `(下限, 上限]`，即最强的下限那一段被排除在外。
pub fn classify(mut candidates: Vec<Candidate>, params: &ScreenParams) -> ScreenOutcome {
    candidates.sort_by(|a, b| {
        match (a.metrics.chg_1m, b.metrics.chg_1m) {
            (Some(left), Some(right)) => right.partial_cmp(&left).unwrap_or(Ordering::Equal),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
        .then_with(|| a.code.cmp(&b.code))
    });

    let universe = candidates.len();
    let mut picked = Vec::new();
    let mut rejected = Vec::new();
    for (index, candidate) in candidates.into_iter().enumerate() {
        let rank_pct = if universe == 0 {
            None
        } else {
            Some((index + 1) as f64 / universe as f64)
        };
        let in_band = rank_pct
            .is_some_and(|value| value > params.rank_band.0 && value <= params.rank_band.1);
        let c1 = candidate.metrics.chg_6m_ok && candidate.metrics.chg_1m.is_some() && in_band;
        let row = ScreenRow {
            code: candidate.code,
            name: candidate.name,
            sector: candidate.sector,
            price: candidate.metrics.price,
            chg_1m: candidate.metrics.chg_1m,
            chg_3m: candidate.metrics.chg_3m,
            chg_6m: candidate.metrics.chg_6m,
            rank_pct,
            avg_amount: candidate.metrics.avg_amount,
            convergence: candidate.metrics.convergence,
            c1,
            c2: candidate.metrics.c2,
            c3: candidate.metrics.c3,
            c4: candidate.metrics.c4,
            liquid: candidate.metrics.liquid,
        };
        if row.c1 && row.c2 && row.c3 && !row.c4 && row.liquid {
            picked.push(row);
        } else {
            rejected.push(row);
        }
    }
    ScreenOutcome {
        universe,
        picked,
        rejected,
    }
}

/// 跑一遍筛选：读全市场日线，判定，把入选写进观察名单（手工补充的行保留），
/// 并把报告存进 `setting` 表。
pub fn run_screen(db: &Database, params: &ScreenParams) -> Result<ScreenReport> {
    let started = Instant::now();
    let conn = db.conn()?;
    let stocks = crate::storage::stock::list(&conn)?;
    let mut candidates = Vec::with_capacity(stocks.len());
    for stock in stocks {
        let rows = crate::storage::bar::list_for_code(&conn, &stock.code)?;
        let bars: Vec<Bar> = rows.into_iter().map(Bar::from).collect();
        candidates.push(Candidate {
            code: stock.code,
            name: stock.name,
            sector: stock.sector,
            metrics: metrics(&bars, params),
        });
    }
    let outcome = classify(candidates, params);
    let ran_at = crate::storage::now();

    watchlist::delete_auto(&conn)?;
    for row in &outcome.picked {
        watchlist::upsert(&conn, &row.to_watchlist_item(&ran_at, &ran_at))?;
    }

    let report = ScreenReport {
        ran_at,
        universe: outcome.universe as u64,
        elapsed_ms: started.elapsed().as_millis() as u64,
        picked: outcome.picked,
        rejected: outcome.rejected,
    };
    setting::set(
        &conn,
        SCREEN_LAST_KEY,
        &serde_json::to_string(&report)?,
        &crate::storage::now(),
    )?;
    Ok(report)
}

/// 读最近一次筛选报告；还没跑过时返回 `None`。
pub fn last_report(conn: &rusqlite::Connection) -> Result<Option<ScreenReport>> {
    let Some(text) = setting::get(conn, SCREEN_LAST_KEY)? else {
        return Ok(None);
    };
    Ok(Some(serde_json::from_str(&text)?))
}

/// 收盘时间。
pub fn close_time() -> NaiveTime {
    NaiveTime::from_hms_opt(15, 0, 0).expect("15:00 是合法时刻")
}

/// 是否该在收盘后自动跑一次筛选：过了收盘时间，且今天还没跑过。
pub fn should_run_auto_screen(now: NaiveTime, today: NaiveDate, last_run: Option<NaiveDate>) -> bool {
    now >= close_time() && last_run != Some(today)
}

static LAST_AUTO_SCREEN: LazyLock<Mutex<Option<NaiveDate>>> = LazyLock::new(|| Mutex::new(None));

/// 起一个后台线程，收盘后自动跑一次筛选。
///
/// 筛选参数每次运行时从配置文件重新读，改过阈值不用重启应用。
pub fn spawn_daily_scheduler(db: Database, config_path: PathBuf) {
    std::thread::spawn(move || loop {
        let now = Local::now();
        let today = now.date_naive();
        let last_run = *LAST_AUTO_SCREEN.lock().expect("自动筛选锁");
        if should_run_auto_screen(now.time(), today, last_run) {
            if let Ok(load) = crate::config::load_settings(&config_path) {
                let params = ScreenParams::from_settings(&load.settings);
                if run_screen(&db, &params).is_ok() {
                    *LAST_AUTO_SCREEN.lock().expect("自动筛选锁") = Some(today);
                }
            }
        }
        std::thread::sleep(Duration::from_secs(60));
    });
}

/// 近 `window` 日日均成交额。
fn average_amount(bars: &[Bar], window: usize) -> Option<f64> {
    if window == 0 || bars.len() < window {
        return None;
    }
    let slice = &bars[bars.len() - window..];
    Some(slice.iter().map(|bar| bar.amount).sum::<f64>() / window as f64)
}

/// 倒数第二个元素；不足两个时返回 `NaN`。
fn second_last(values: &[f64]) -> f64 {
    if values.len() < 2 {
        f64::NAN
    } else {
        values[values.len() - 2]
    }
}

/// 摆动低点的下标：该 bar 的最低价严格低于左右各 `k` 根的最低价。
fn swing_lows(bars: &[Bar], k: usize) -> Vec<usize> {
    if k == 0 || bars.len() < 2 * k + 1 {
        return Vec::new();
    }
    (k..bars.len() - k)
        .filter(|&index| {
            let low = bars[index].low;
            (index - k..index).all(|j| low < bars[j].low)
                && (index + 1..=index + k).all(|j| low < bars[j].low)
        })
        .collect()
}

/// 摆动高点的下标：该 bar 的最高价严格高于左右各 `k` 根的最高价。
fn swing_highs(bars: &[Bar], k: usize) -> Vec<usize> {
    if k == 0 || bars.len() < 2 * k + 1 {
        return Vec::new();
    }
    (k..bars.len() - k)
        .filter(|&index| {
            let high = bars[index].high;
            (index - k..index).all(|j| high > bars[j].high)
                && (index + 1..=index + k).all(|j| high > bars[j].high)
        })
        .collect()
}

/// 一串下标里的最后两个。
fn last_two(indices: &[usize]) -> Option<(usize, usize)> {
    if indices.len() < 2 {
        return None;
    }
    Some((indices[indices.len() - 2], indices[indices.len() - 1]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::Bar;

    /// 用收盘价序列造一组 bar：最高价 +0.5%、最低价 −0.5%、成交额固定。
    fn from_closes(closes: &[f64]) -> Vec<Bar> {
        closes
            .iter()
            .enumerate()
            .map(|(index, close)| Bar {
                code: "600000".to_string(),
                date: format!("2026-01-{:02}", index % 28 + 1),
                open: *close,
                high: close * 1.005,
                low: close * 0.995,
                close: *close,
                volume: 1000.0,
                amount: 2e8,
            })
            .collect()
    }

    fn params() -> ScreenParams {
        ScreenParams::from_settings(&Settings::default())
    }

    fn rising(len: usize) -> Vec<f64> {
        (0..len).map(|index| 10.0 + index as f64 * 0.1).collect()
    }

    #[test]
    fn ema9_above_and_rising_marks_c3() {
        let bars = from_closes(&rising(200));
        let result = metrics(&bars, &params());
        assert!(result.c3, "收盘价在向上的 EMA9 之上时 C3 成立");
        assert!(!result.c4, "向上趋势里收盘价高于 EMA21，C4 不成立");
    }

    #[test]
    fn falling_series_marks_c4() {
        // 逐级下移的高点与低点，末端跌破向下的 EMA21。
        let closes = path(
            200,
            &[
                (0, 31.0),
                (20, 29.0),
                (40, 34.0),
                (60, 28.0),
                (80, 31.0),
                (100, 25.0),
                (120, 27.0),
                (140, 22.0),
                (160, 24.0),
                (199, 20.0),
            ],
        );
        let bars = from_closes(&closes);
        let result = metrics(&bars, &params());
        assert!(!result.c3);
        assert!(result.c4, "高点低点均下降且跌破向下 EMA21 时 C4 成立");
    }

    #[test]
    fn c3_requires_both_emas_flat_or_up() {
        // 前半段上涨、最后一根大幅回落：收盘价仍在 EMA9 上方，但 EMA9 转下。
        let mut closes = rising(200);
        let last = closes.len() - 1;
        closes[last] = closes[last - 1] * 0.90;
        let bars = from_closes(&closes);
        let result = metrics(&bars, &params());
        assert!(!result.c3, "EMA9 转向下时 C3 不成立");
    }

    #[test]
    fn convergence_uses_the_last_twenty_traded_bars() {
        // 前 180 根宽幅波动、最后 20 根收窄，收敛只看最后 20 根。
        let mut closes: Vec<f64> = (0..180).map(|i| if i % 2 == 0 { 8.0 } else { 14.0 }).collect();
        closes.extend((0..20).map(|index| 10.0 + index as f64 * 0.01));
        let bars = from_closes(&closes);
        let result = metrics(&bars, &params());
        let value = result.convergence.unwrap();
        assert!(value < 0.25, "最后 20 根收窄时收敛值应低于阈值，实际 {value}");
    }

    #[test]
    fn rising_pullback_lows_mark_c2() {
        // 两段回踩，第二段低点更高，最后一段收窄上行。
        let closes = path(
            200,
            &[
                (0, 5.0),
                (69, 12.0),
                (89, 9.5),
                (119, 14.0),
                (139, 12.5),
                (199, 13.4),
            ],
        );
        let bars = from_closes(&closes);
        let result = metrics(&bars, &params());
        assert!(result.c2, "回调低点抬高且波动收窄时 C2 成立");
    }

    #[test]
    fn lower_pullback_low_fails_c2() {
        // 第二次回踩比第一次更低。
        let closes = path(
            200,
            &[
                (0, 5.0),
                (69, 12.0),
                (89, 8.0),
                (119, 14.0),
                (139, 7.0),
                (199, 7.4),
            ],
        );
        let bars = from_closes(&closes);
        let result = metrics(&bars, &params());
        assert!(!result.c2, "回调低点下移时 C2 不成立");
    }

    #[test]
    fn non_traded_bars_do_not_shift_indicators() {
        let mut bars = from_closes(&rising(200));
        let plain = metrics(&bars, &params());
        // 在中间插入一根成交量为 0 的停牌日，指标不应被拉偏。
        bars.insert(
            100,
            Bar {
                code: "600000".to_string(),
                date: "2026-06-01".to_string(),
                open: 1.0,
                high: 1.0,
                low: 1.0,
                close: 1.0,
                volume: 0.0,
                amount: 0.0,
            },
        );
        let with_gap = metrics(&bars, &params());
        assert_eq!(plain, with_gap);
    }

    #[test]
    fn liquidity_compares_average_amount_with_threshold() {
        let mut bars = from_closes(&rising(200));
        for bar in bars.iter_mut() {
            bar.amount = 5e7;
        }
        let result = metrics(&bars, &params());
        assert!(!result.liquid, "日均成交额低于 1 亿应被剔除");
        assert_eq!(result.avg_amount, Some(5e7));
    }

    #[test]
    fn rank_band_excludes_the_strongest_slice() {
        // 20 只股票，涨幅严格递减；分位是 1/20…20/20，只有 5/20=0.25 与 6/20=0.30 落在区间内。
        let candidates = (0..20)
            .map(|index| Candidate {
                code: format!("6000{index:02}"),
                name: format!("股票{index}"),
                sector: None,
                metrics: StockMetrics {
                    price: 10.0,
                    chg_1m: Some(1.0 - index as f64 * 0.01),
                    chg_3m: Some(0.5),
                    chg_6m: Some(0.5),
                    avg_amount: Some(2e8),
                    convergence: Some(0.1),
                    c2: true,
                    c3: true,
                    c4: false,
                    chg_6m_ok: true,
                    liquid: true,
                },
            })
            .collect();
        let outcome = classify(candidates, &params());
        assert_eq!(outcome.universe, 20);
        assert_eq!(outcome.picked.len(), 2, "只有分位 0.25 与 0.30 落在排名区间内");
        assert_eq!(outcome.picked[0].rank_pct, Some(5.0 / 20.0));
        assert_eq!(outcome.picked[1].rank_pct, Some(6.0 / 20.0));
        // 最强的两只是 c1=false，落在未入选里。
        assert!(!outcome.rejected[0].c1);
        assert_eq!(outcome.rejected[0].rank_pct, Some(1.0 / 20.0));
    }

    #[test]
    fn picked_requires_all_four_conditions_and_liquidity() {
        let base = |code: &str, c2: bool, c3: bool, c4: bool, liquid: bool| Candidate {
            code: code.to_string(),
            name: code.to_string(),
            sector: None,
            metrics: StockMetrics {
                price: 10.0,
                chg_1m: Some(0.2),
                chg_3m: Some(0.3),
                chg_6m: Some(0.5),
                avg_amount: Some(2e8),
                convergence: Some(0.1),
                c2,
                c3,
                c4,
                chg_6m_ok: true,
                liquid,
            },
        };
        // 4 只股票，分位 0.25/0.50/0.75/1.00；只有 0.25 在区间内。
        let candidates = vec![
            base("600001", true, true, false, true),
            base("600002", false, true, false, true),
            base("600003", true, true, false, true),
            base("600004", true, true, false, true),
        ];
        let outcome = classify(candidates, &params());
        assert_eq!(outcome.picked.len(), 1);
        assert_eq!(outcome.picked[0].code, "600001");
        assert_eq!(outcome.rejected.len(), 3);
    }

    #[test]
    fn c4_hit_or_low_liquidity_keeps_the_stock_out() {
        let make = |code: &str, c4: bool, liquid: bool, chg_1m: f64| Candidate {
            code: code.to_string(),
            name: code.to_string(),
            sector: None,
            metrics: StockMetrics {
                price: 10.0,
                chg_1m: Some(chg_1m),
                chg_3m: Some(0.3),
                chg_6m: Some(0.5),
                avg_amount: Some(2e8),
                convergence: Some(0.1),
                c2: true,
                c3: true,
                c4,
                chg_6m_ok: true,
                liquid,
            },
        };
        let candidates = vec![
            make("600001", true, true, 0.40),
            make("600002", false, false, 0.30),
            make("600003", false, true, 0.20),
            make("600004", false, true, 0.10),
        ];
        let outcome = classify(candidates, &params());
        // 分位 0.25/0.50/0.75/1.00：0.25 那只命中 C4，0.50 那只成交额不足。
        assert!(outcome.picked.is_empty());
        assert_eq!(outcome.rejected.len(), 4);
        assert!(outcome.rejected[0].c4);
        assert!(!outcome.rejected[1].liquid);
    }

    #[test]
    fn auto_screen_runs_after_close_once_a_day() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();
        assert!(!should_run_auto_screen(
            NaiveTime::from_hms_opt(14, 59, 0).unwrap(),
            today,
            None
        ));
        assert!(should_run_auto_screen(
            NaiveTime::from_hms_opt(15, 0, 0).unwrap(),
            today,
            None
        ));
        assert!(!should_run_auto_screen(
            NaiveTime::from_hms_opt(15, 30, 0).unwrap(),
            today,
            Some(today)
        ));
    }

    /// 按锚点做分段线性插值，造一条指定长度、无重复点的价格路径。
    fn path(len: usize, anchors: &[(usize, f64)]) -> Vec<f64> {
        (0..len)
            .map(|index| {
                for window in anchors.windows(2) {
                    let (start, start_value) = window[0];
                    let (end, end_value) = window[1];
                    if index >= start && index <= end {
                        let ratio = if end == start {
                            0.0
                        } else {
                            (index - start) as f64 / (end - start) as f64
                        };
                        return start_value + (end_value - start_value) * ratio;
                    }
                }
                anchors.last().expect("至少一个锚点").1
            })
            .collect()
    }
}

//! 解释上下文：把五段解释要用到的事实从数据库与日线里取出来。
//!
//! 指标（`indicators.rs`）、筛选（`screening.rs`）、支撑区（`support.rs`）、信号
//! （`signal.rs`）与仓位（`sizing.rs`）分属别的 ticket。本文件只读取它们写进数据库的
//! 结果（`watchlist_item`、`support_zone`、`signal`、`trade_plan`、`trade_log`），
//! 并自己算解释文案要用到的最小指标（收盘价、EMA9/EMA21、区间低点）。

use super::Scope;
use crate::error::Result;
use crate::storage::bar::{self, BarRow};
use crate::storage::{signal, stock, support_zone, trade_log, trade_plan, watchlist};
use rusqlite::Connection;

/// 取区间低点时的窗口长度（交易日）。
const LOW_WINDOW: usize = 20;
/// 交易日志页解释最多带上的最近成交笔数。
const JOURNAL_RECENT: usize = 10;

/// 一次解释需要的全部事实。按 scope 只填对应的小节。
#[derive(Debug, Clone, PartialEq)]
pub struct ExplainContext {
    pub code: String,
    /// 股票名称；`stock` 表里没有该代码时回落到代码本身。
    pub name: String,
    pub scope: Scope,
    /// 数据截止日：主流程四步取该股最新日线日期，交易日志页取最近一笔交易日期。
    pub data_as_of: String,
    pub step1: Option<Step1Facts>,
    pub step2: Option<Step2Facts>,
    pub step3: Option<Step3Facts>,
    pub step4: Option<Step4Facts>,
    pub journal: Option<JournalFacts>,
}

impl ExplainContext {
    /// 该 scope 生成解释所需的数据是否齐备。
    pub fn is_complete(&self) -> bool {
        if self.data_as_of.is_empty() {
            return false;
        }
        match self.scope {
            Scope::Step1 => self
                .step1
                .as_ref()
                .is_some_and(|f| f.in_watchlist && f.close.is_some() && f.ema9.is_some()),
            Scope::Step2 => self.step2.as_ref().is_some_and(|f| !f.zones.is_empty()),
            Scope::Step3 => self.step3.as_ref().is_some_and(|f| f.zone.is_some()),
            Scope::Step4 => self.step4.is_some(),
            Scope::Journal => self.journal.as_ref().is_some_and(|f| f.trades > 0),
        }
    }

    /// 数据不齐备时，说明缺的是什么，供错误文案使用。
    pub fn missing_reason(&self) -> String {
        if self.data_as_of.is_empty() {
            return "还没有这只股票的日线数据".to_string();
        }
        match self.scope {
            Scope::Step1 => "第 1 步的解释需要先完成筛选，并具备足够的日线算均线".to_string(),
            Scope::Step2 => "第 2 步的解释需要先在图上标注至少一个支撑区".to_string(),
            Scope::Step3 => "第 3 步的解释需要先在第 2 步标注出支撑区".to_string(),
            Scope::Step4 => "第 4 步的解释需要先算出一份交易计划".to_string(),
            Scope::Journal => "交易日志页的解释需要至少一笔已出场的交易".to_string(),
        }
    }
}

/// 第 1 步（为什么入选）的事实。
#[derive(Debug, Clone, PartialEq)]
pub struct Step1Facts {
    /// 是否已有筛选结论（`watchlist_item` 里有这只股票）。
    pub in_watchlist: bool,
    pub c1: bool,
    pub c2: bool,
    pub c3: bool,
    pub c4: bool,
    /// 近 1 / 3 / 6 个月涨幅，来自 `watchlist_item`。
    pub chg_1m: Option<f64>,
    pub chg_3m: Option<f64>,
    pub chg_6m: Option<f64>,
    /// 近 1 个月涨幅的全市场排名分位。
    pub rank_pct: Option<f64>,
    /// 近 20 日极差。
    pub convergence: Option<f64>,
    /// 日均成交额（元）。
    pub avg_amount: Option<f64>,
    /// 最新收盘价。
    pub close: Option<f64>,
    pub ema9: Option<f64>,
    pub ema21: Option<f64>,
    /// 近 20 个交易日的最低价。
    pub recent_low: Option<f64>,
    /// 再往前 20 个交易日的最低价。
    pub prior_low: Option<f64>,
}

/// 一条支撑区标注的事实。
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneFacts {
    pub id: String,
    /// `hline` | `support` | `gap` | `avwap_start`。
    pub kind: String,
    pub low: f64,
    pub high: f64,
    /// `高` | `中` | `低`。
    pub confidence: String,
    /// 三个参考价的离散度。
    pub spread: Option<f64>,
    pub ema21: Option<f64>,
    pub avwap: Option<f64>,
    pub prev_high: Option<f64>,
}

/// 第 2 步（图上形态与置信度）的事实。
#[derive(Debug, Clone, PartialEq)]
pub struct Step2Facts {
    pub zones: Vec<ZoneFacts>,
    /// 其中高置信的条数。
    pub high_confidence: usize,
}

/// 第 3 步（触发条件卡在哪一级）的事实。
#[derive(Debug, Clone, PartialEq)]
pub struct Step3Facts {
    /// `waiting` | `triggered` | `failed`。
    pub stage: String,
    pub zone: Option<ZoneFacts>,
    pub timeframe: Option<String>,
    pub triggered_at: Option<String>,
    /// 被突破的前一根 K 线最高价。
    pub broken_high: Option<f64>,
    pub pre_high: Option<f64>,
}

/// 第 4 步（这单的风险结构）的事实，来自最近一份交易计划。
#[derive(Debug, Clone, PartialEq)]
pub struct Step4Facts {
    pub entry: f64,
    pub stop: f64,
    pub account: f64,
    pub risk_pct: f64,
    pub per_r: f64,
    pub shares: i64,
    pub capital: f64,
    pub pos_pct: f64,
    pub stop_distance: f64,
    pub t3: f64,
    pub t5: f64,
    /// `draft` | `triggered` | `executed` | `abandoned`。
    pub status: String,
}

/// 交易日志页（复盘总结）的事实。
#[derive(Debug, Clone, PartialEq)]
pub struct JournalFacts {
    /// 已出场的笔数。
    pub trades: usize,
    /// 其中盈利的笔数（按 R 倍数判定）。
    pub wins: usize,
    pub win_rate: Option<f64>,
    pub avg_r: Option<f64>,
    pub total_pnl: Option<f64>,
    /// 最近若干笔已出场的交易。
    pub recent: Vec<ClosedTradeFacts>,
}

/// 一笔已出场交易的事实。
#[derive(Debug, Clone, PartialEq)]
pub struct ClosedTradeFacts {
    pub code: String,
    pub name: Option<String>,
    pub entry_date: String,
    pub exit_date: Option<String>,
    pub entry_price: f64,
    pub exit_price: Option<f64>,
    pub r_multiple: Option<f64>,
    pub pnl: Option<f64>,
    pub note: Option<String>,
}

/// 按 scope 取齐一次解释需要的事实。
pub fn build_context(conn: &Connection, code: &str, scope: Scope) -> Result<ExplainContext> {
    let name = stock::get(conn, code)?
        .map(|row| row.name)
        .unwrap_or_else(|| code.to_string());

    match scope {
        Scope::Journal => build_journal(conn, code, name),
        _ => build_stock(conn, code, name, scope),
    }
}

fn build_stock(conn: &Connection, code: &str, name: String, scope: Scope) -> Result<ExplainContext> {
    let data_as_of = bar::latest_date(conn, code)?.unwrap_or_default();

    let mut context = ExplainContext {
        code: code.to_string(),
        name,
        scope,
        data_as_of,
        step1: None,
        step2: None,
        step3: None,
        step4: None,
        journal: None,
    };

    match scope {
        Scope::Step1 => {
            let bars = valid_bars(bar::list_for_code(conn, code)?);
            let closes: Vec<f64> = bars.iter().map(|row| row.close).collect();
            context.step1 = Some(Step1Facts {
                in_watchlist: false,
                c1: false,
                c2: false,
                c3: false,
                c4: false,
                chg_1m: None,
                chg_3m: None,
                chg_6m: None,
                rank_pct: None,
                convergence: None,
                avg_amount: None,
                close: closes.last().copied(),
                ema9: ema(&closes, 9),
                ema21: ema(&closes, 21),
                recent_low: window_low(&bars, 0),
                prior_low: window_low(&bars, LOW_WINDOW),
            });
            if let Some(item) = watchlist::get(conn, code)? {
                let facts = context.step1.as_mut().expect("刚写入的 step1");
                facts.in_watchlist = true;
                facts.c1 = item.c1;
                facts.c2 = item.c2;
                facts.c3 = item.c3;
                facts.c4 = item.c4;
                facts.chg_1m = item.chg_1m;
                facts.chg_3m = item.chg_3m;
                facts.chg_6m = item.chg_6m;
                facts.rank_pct = item.rank_pct;
                facts.convergence = item.convergence;
                facts.avg_amount = item.avg_amount;
            }
        }
        Scope::Step2 => {
            let zones = zones_for(conn, code)?;
            let high_confidence = zones.iter().filter(|zone| zone.confidence == "高").count();
            context.step2 = Some(Step2Facts {
                zones,
                high_confidence,
            });
        }
        Scope::Step3 => {
            let zones = zones_for(conn, code)?;
            let latest = signal::latest_for_code(conn, code)?;
            let zone = latest
                .as_ref()
                .and_then(|row| row.zone_id.as_deref())
                .and_then(|id| zones.iter().find(|zone| zone.id == id))
                .or_else(|| zones.iter().find(|zone| zone.confidence == "高"))
                .or_else(|| zones.first())
                .cloned();
            context.step3 = Some(Step3Facts {
                stage: latest
                    .as_ref()
                    .map(|row| row.status.clone())
                    .unwrap_or_else(|| "waiting".to_string()),
                zone,
                timeframe: latest.as_ref().map(|row| row.timeframe.clone()),
                triggered_at: latest.as_ref().map(|row| row.triggered_at.clone()),
                broken_high: latest.as_ref().map(|row| row.candle_high_broken),
                pre_high: latest.as_ref().map(|row| row.pre_candle_high),
            });
        }
        Scope::Step4 => {
            let plan = trade_plan::list(conn, None)?
                .into_iter()
                .find(|plan| plan.code == code);
            context.step4 = plan.map(|plan| Step4Facts {
                entry: plan.entry,
                stop: plan.stop,
                account: plan.account,
                risk_pct: plan.risk_pct,
                per_r: plan.per_r,
                shares: plan.shares,
                capital: plan.capital,
                pos_pct: plan.pos_pct,
                stop_distance: plan.stop_distance,
                t3: plan.t3,
                t5: plan.t5,
                status: plan.status,
            });
        }
        Scope::Journal => unreachable!("交易日志页由 build_journal 处理"),
    }

    Ok(context)
}

fn build_journal(conn: &Connection, code: &str, name: String) -> Result<ExplainContext> {
    let closed = trade_log::list_closed(conn)?;
    let data_as_of = closed
        .iter()
        .map(|entry| entry.exit_date.clone().unwrap_or_else(|| entry.entry_date.clone()))
        .max()
        .unwrap_or_default();

    let rated: Vec<f64> = closed.iter().filter_map(|entry| entry.r_multiple).collect();
    let wins = rated.iter().filter(|value| **value > 0.0).count();
    let win_rate = if rated.is_empty() {
        None
    } else {
        Some(wins as f64 / rated.len() as f64)
    };
    let avg_r = if rated.is_empty() {
        None
    } else {
        Some(rated.iter().sum::<f64>() / rated.len() as f64)
    };
    let pnls: Vec<f64> = closed.iter().filter_map(|entry| entry.pnl).collect();
    let total_pnl = if pnls.is_empty() {
        None
    } else {
        Some(pnls.iter().sum())
    };

    let recent = closed
        .iter()
        .take(JOURNAL_RECENT)
        .map(|entry| ClosedTradeFacts {
            code: entry.code.clone(),
            name: entry.name.clone(),
            entry_date: entry.entry_date.clone(),
            exit_date: entry.exit_date.clone(),
            entry_price: entry.entry_price,
            exit_price: entry.exit_price,
            r_multiple: entry.r_multiple,
            pnl: entry.pnl,
            note: entry.note.clone(),
        })
        .collect();

    Ok(ExplainContext {
        code: code.to_string(),
        name,
        scope: Scope::Journal,
        data_as_of,
        step1: None,
        step2: None,
        step3: None,
        step4: None,
        journal: Some(JournalFacts {
            trades: closed.len(),
            wins,
            win_rate,
            avg_r,
            total_pnl,
            recent,
        }),
    })
}

/// 把 `support_zone` 行转成解释用的事实。
fn zones_for(conn: &Connection, code: &str) -> Result<Vec<ZoneFacts>> {
    let zones = support_zone::list_for_code(conn, code)?
        .into_iter()
        .map(|zone| ZoneFacts {
            id: zone.id,
            kind: zone.kind,
            low: zone.low,
            high: zone.high,
            confidence: zone.confidence,
            spread: zone.spread,
            ema21: zone.ema21,
            avwap: zone.avwap_price,
            prev_high: zone.prev_high,
        })
        .collect();
    Ok(zones)
}

/// 丢掉没有成交的 bar（成交量为 0），指标只在有交易的 bar 上算。
fn valid_bars(bars: Vec<BarRow>) -> Vec<BarRow> {
    bars.into_iter().filter(|row| row.volume > 0.0).collect()
}

/// 指数移动平均：α = 2/(n+1)，首值取前 n 个收盘价的简单平均。数据不足返回 None。
fn ema(closes: &[f64], period: usize) -> Option<f64> {
    if period == 0 || closes.len() < period {
        return None;
    }
    let alpha = 2.0 / (period as f64 + 1.0);
    let seed = closes[..period].iter().sum::<f64>() / period as f64;
    let mut value = seed;
    for close in &closes[period..] {
        value = alpha * close + (1.0 - alpha) * value;
    }
    Some(value)
}

/// 从末尾往前数 `skip` 个 bar 之后，最近 `LOW_WINDOW` 个 bar 的最低价。
///
/// `skip` 为 0 时取最近 20 个交易日，为 `LOW_WINDOW` 时取再往前 20 个交易日。
fn window_low(bars: &[BarRow], skip: usize) -> Option<f64> {
    if bars.len() < skip + 1 {
        return None;
    }
    let end = bars.len() - skip;
    let start = end.saturating_sub(LOW_WINDOW);
    if start == end {
        return None;
    }
    bars[start..end]
        .iter()
        .map(|row| row.low)
        .reduce(f64::min)
}

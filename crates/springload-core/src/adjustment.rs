//! 前复权换算与过期判定。
//!
//! 事实来源见 `docs/research/suspension-and-adjustment.md` 第 2、3、5 节：
//!
//! - 除权（息）参考价 `(前收盘价 - D) / (1 + r)`，除权日行情里的前收盘价就是它；
//! - 前复权对日期 `d` 之后的每一次除权除息事件依次施加 `P ↦ (P - D) / (1 + r)`；
//! - 除权日之前的历史在除权日当日被整体改写，据此判断已存的前复权历史是否过期。
//!
//! 行情源给的是不复权原始价，前复权由本模块按除权除息事件换算，所以过期判据只看
//! 最近一次除权除息日与本地最后一条日线的先后。

use crate::error::Result;
use crate::market::{Bar, QuoteSource};
use crate::storage::bar::BarRow;
use crate::storage::bar_sync_state::{self, BarSyncState};
use crate::storage::corporate_action::{self, CorporateAction};
use crate::storage::{self, bar, stock, Database};
use chrono::{Months, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// 回补的历史长度（年），与日线回补一致。
pub const HISTORY_YEARS: u32 = crate::backfill::HISTORY_YEARS;
/// 检查与重算时的并发请求数。
pub const DEFAULT_CONCURRENCY: usize = crate::backfill::DEFAULT_CONCURRENCY;
/// 保留的错误条数上限。
const MAX_ERRORS: usize = 50;

/// 除权（息）参考价：`(前收盘价 - 每股现金红利) / (1 + 每股送转比例)`。
pub fn reference_price(prev_close: f64, dividend_per_share: f64, split_ratio: f64) -> f64 {
    (prev_close - dividend_per_share) / (1.0 + split_ratio)
}

/// 对日期 `date` 的一个价格施加 `date` 之后的所有除权除息事件。
///
/// 事件按除权日先后依次施加 `P ↦ (P - D) / (1 + r)`。`ex_date <= date` 的事件不施加，
/// 因为除权日当天及其后的价格已经是不复权价。
pub fn adjust_price(raw: f64, date: &str, events: &[CorporateAction]) -> f64 {
    let mut ordered: Vec<&CorporateAction> = events
        .iter()
        .filter(|event| event.ex_date.as_str() > date)
        .collect();
    ordered.sort_by(|a, b| a.ex_date.cmp(&b.ex_date));
    let mut price = raw;
    for event in ordered {
        price = (price - event.dividend_per_share) / (1.0 + event.split_ratio);
    }
    price
}

/// 对一根不复权日线的四个价格字段做前复权换算，成交量与成交额不变。
///
/// 价格保留两位小数，与行情源的前复权序列一致。
pub fn adjust_bar(row: &BarRow, events: &[CorporateAction]) -> BarRow {
    let round = |value: f64| (value * 100.0).round() / 100.0;
    BarRow {
        code: row.code.clone(),
        date: row.date.clone(),
        open: round(adjust_price(row.open, &row.date, events)),
        high: round(adjust_price(row.high, &row.date, events)),
        low: round(adjust_price(row.low, &row.date, events)),
        close: round(adjust_price(row.close, &row.date, events)),
        volume: row.volume,
        amount: row.amount,
    }
}

/// 刚取到的事件与库里记的不一致就判过期：新增或改动过的事件都会改写它之前的历史。
///
/// 库里多出来的事件不判过期，否则每次检查都要重算一遍。比较「最近一次除权除息日是否
/// 晚于本地最后一条日线」是不够的：同步发生在除权日之后时，除权日早于最后一条日线，
/// 但除权日之前的历史仍然是旧价，漏判会让这段历史一直停在错的数上。
pub fn stale_by_events(fetched: &[CorporateAction], stored: &[CorporateAction]) -> bool {
    fetched.iter().any(|event| {
        match stored
            .iter()
            .find(|row| row.ex_date == event.ex_date)
        {
            None => true,
            Some(row) => {
                (row.dividend_per_share - event.dividend_per_share).abs() > 1e-6
                    || (row.split_ratio - event.split_ratio).abs() > 1e-6
            }
        }
    })
}

/// 一批不复权日线按除权除息事件换算成前复权日线。
pub fn adjust_bars(raw: &[Bar], events: &[CorporateAction]) -> Vec<BarRow> {
    raw.iter()
        .map(|bar| {
            adjust_bar(
                &BarRow {
                    code: bar.code.clone(),
                    date: bar.date.clone(),
                    open: bar.open,
                    high: bar.high,
                    low: bar.low,
                    close: bar.close,
                    volume: bar.volume,
                    amount: bar.amount,
                },
                events,
            )
        })
        .collect()
}

/// 需要重算的股票。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaleStock {
    pub code: String,
    pub name: String,
    /// 触发过期的最近一次除权除息日。
    pub ex_date: Option<String>,
}

/// 一次重算的结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResyncReport {
    pub stocks: u64,
    pub bars: u64,
    pub elapsed_ms: u64,
    pub errors: Vec<String>,
}

/// 检查全部已回补股票的前复权历史是否过期。
///
/// 逐只取除权除息记录，比较最近一次已生效的除权除息日与本地最后一条日线。只读，
/// 不写库；过期股票的事件与历史由 [`resync`] 一起补上。
pub fn detect_stale(
    db: &Database,
    source: &dyn QuoteSource,
    concurrency: usize,
) -> Result<Vec<StaleStock>> {
    let names: HashMap<String, String> = {
        let conn = db.conn()?;
        stock::list(&conn)?
            .into_iter()
            .map(|item| (item.code, item.name))
            .collect()
    };
    let codes: Vec<String> = names.keys().cloned().collect();
    let (found, errors) = run_concurrent(codes, concurrency, |code| {
        let conn = db.conn()?;
        let name = names.get(code).cloned().unwrap_or_default();
        probe_stale(&conn, source, code, &name)
    });
    let mut stale: Vec<StaleStock> = found.into_iter().flatten().collect();
    stale.sort_by(|a, b| a.code.cmp(&b.code));
    for error in &errors {
        eprintln!("{error}");
    }
    Ok(stale)
}

/// 探测一只股票是否过期，`Ok(None)` 表示不过期。
fn probe_stale(
    conn: &rusqlite::Connection,
    source: &dyn QuoteSource,
    code: &str,
    name: &str,
) -> Result<Option<StaleStock>> {
    let Some(state) = bar_sync_state::get(conn, code)? else {
        return Ok(None);
    };
    if state.last_bar_date.is_none() {
        return Ok(None);
    }

    let fetched = source.corporate_actions(code)?;
    let stored = corporate_action::list_for_code(conn, code)?;
    if stale_by_events(&fetched, &stored) {
        let ex_date = fetched.iter().map(|event| event.ex_date.clone()).max();
        Ok(Some(StaleStock {
            code: code.to_string(),
            name: name.to_string(),
            ex_date,
        }))
    } else {
        Ok(None)
    }
}

/// 重算给定股票的前复权历史。
///
/// 取不复权日线与除权除息事件，把事件写进 `corporate_action`，再对整段历史施加前复权
/// 换算后覆盖 `bar` 表，并刷新 `bar_sync_state`。
pub fn resync(
    db: &Database,
    source: &dyn QuoteSource,
    codes: &[String],
    today: NaiveDate,
) -> Result<ResyncReport> {
    let started = Instant::now();
    let (written, errors) = run_concurrent(codes.to_vec(), DEFAULT_CONCURRENCY, |code| {
        let mut conn = db.conn()?;
        resync_one(&mut conn, source, code, today)
    });
    let bars: u64 = written.iter().map(|n| *n as u64).sum();
    for error in &errors {
        eprintln!("{error}");
    }
    Ok(ResyncReport {
        stocks: (written.len() + errors.len()) as u64,
        bars,
        elapsed_ms: started.elapsed().as_millis() as u64,
        errors,
    })
}

/// 重算一只股票，返回写入的日线条数。
fn resync_one(
    conn: &mut rusqlite::Connection,
    source: &dyn QuoteSource,
    code: &str,
    today: NaiveDate,
) -> Result<usize> {
    let beg = today
        .checked_sub_months(Months::new(HISTORY_YEARS * 12))
        .unwrap_or(today);
    let raw = source.daily_raw(code, &beg.format("%Y%m%d").to_string())?;
    let events = source.corporate_actions(code)?;
    record_events(conn, code, &events, &raw)?;

    let last = raw.last().map(|bar| bar.date.as_str());
    let effective = crate::backfill::effective_events(conn, code, last)?;
    let adjusted = adjust_bars(&raw, &effective);
    bar::delete_for_code(conn, code)?;
    let written = bar::upsert_many(conn, &adjusted)?;
    bar_sync_state::upsert(
        conn,
        &BarSyncState {
            code: code.to_string(),
            last_bar_date: adjusted.last().map(|bar| bar.date.clone()),
            fetched_at: Some(storage::now()),
            updated_at: storage::now(),
        },
    )?;
    Ok(written)
}

/// 把除权除息事件写进 `corporate_action`。
///
/// 除权日的前收盘价按参考价公式从不复权日线算出：取除权日前最后一根日线的收盘价。
/// 事件早于 `raw` 覆盖范围时保留表里已有的值。
pub fn record_events(
    conn: &rusqlite::Connection,
    code: &str,
    events: &[CorporateAction],
    raw: &[Bar],
) -> Result<usize> {
    let existing: HashMap<String, Option<f64>> = corporate_action::list_for_code(conn, code)?
        .into_iter()
        .map(|action| (action.ex_date, action.pre_close))
        .collect();
    let now = storage::now();
    for event in events {
        let pre_close = raw
            .iter()
            .filter(|bar| bar.date.as_str() < event.ex_date.as_str())
            .next_back()
            .map(|bar| reference_price(bar.close, event.dividend_per_share, event.split_ratio))
            .or_else(|| existing.get(&event.ex_date).copied().flatten());
        corporate_action::upsert(
            conn,
            &CorporateAction {
                code: code.to_string(),
                ex_date: event.ex_date.clone(),
                dividend_per_share: event.dividend_per_share,
                split_ratio: event.split_ratio,
                pre_close,
                recorded_at: now.clone(),
            },
        )?;
    }
    Ok(events.len())
}

/// 在若干线程上并发处理一批代码，收集成功结果与错误文案。
fn run_concurrent<T, F>(codes: Vec<String>, concurrency: usize, worker: F) -> (Vec<T>, Vec<String>)
where
    T: Clone + Send,
    F: Fn(&str) -> Result<T> + Sync,
{
    let queue = Arc::new(Mutex::new(VecDeque::from(codes)));
    let results = Arc::new(Mutex::new(Vec::<T>::new()));
    let errors = Arc::new(Mutex::new(Vec::<String>::new()));
    let worker = &worker;
    std::thread::scope(|scope| {
        for _ in 0..concurrency.max(1) {
            let queue = Arc::clone(&queue);
            let results = Arc::clone(&results);
            let errors = Arc::clone(&errors);
            scope.spawn(move || loop {
                let item = { queue.lock().expect("队列锁").pop_front() };
                let Some(code) = item else { break };
                match worker(&code) {
                    Ok(value) => results.lock().expect("结果锁").push(value),
                    Err(error) => {
                        let mut guard = errors.lock().expect("错误锁");
                        if guard.len() < MAX_ERRORS {
                            guard.push(format!("{code} 失败: {error}"));
                        }
                    }
                }
            });
        }
    });
    let results = results.lock().expect("结果锁").clone();
    let errors = errors.lock().expect("错误锁").clone();
    (results, errors)
}

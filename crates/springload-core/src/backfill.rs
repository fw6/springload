//! 股票池与日线回补。
//!
//! 抓本市场的证券列表写进 `stock` 表，再逐只抓最近三年的日线，换算成前复权写进
//! `bar` 表。回补可以在收盘后由定时任务触发一次，也可以由 `sync_market` 命令手动触发。
//!
//! A 股走通达信：拿到的是不复权原始价，按除权除息事件换算前复权。美股走 Yahoo：
//! 拿到的是已经复权过的价，除权除息事件为空，换算退化成原样写入。
//!
//! 上市日期由日线推出来：行情源都不提供上市日，而日线历史从上市首日开始，所以
//! 三年窗口没被填满时最早那根日线就是上市首日；窗口被填满说明上市已超过三年，
//! 这时上市日留空，反正「上市满六个月」的判据只用到「最早一根日线早于六个月分界日」。

use crate::adjustment::{adjust_bars, record_events};
use crate::error::{CoreError, Result};
use crate::market::{board_of, market_of, Bar, Market, QuoteSource, Security};
use crate::storage::bar_sync_state::BarSyncState;
use crate::storage::stock::Stock;
use crate::storage::bar::BarRow;
use crate::storage::{self, bar, bar_sync_state, corporate_action, stock, Database};
use chrono::{Days, Local, Months, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Instant;

/// 回补的历史长度（年）。
pub const HISTORY_YEARS: u32 = 3;
/// 回补窗口往前多取的天数。
///
/// 用来判断三年窗口是否覆盖到上市首日：最早一根日线晚于三年分界日时它才是上市首日，
/// 否则说明上市已超过三年。多取的这一个月要盖过最长的休市（春节与国庆各约十天），
/// 不然老股票会因为窗口起点落在休市日而被误判成新上市。
const LISTING_PROBE_DAYS: u32 = 30;
/// 默认的并发请求数。
pub const DEFAULT_CONCURRENCY: usize = 6;
/// 保留的错误条数上限。
const MAX_ERRORS: usize = 50;
/// 判断价格基准有没有变的容差。
///
/// 两边都走同一条换算与取整路径（A 股取到分、美股取到万分之一分），所以基准没变时
/// 两个数完全相同，这个容差只用来挡住浮点噪声。
const BASIS_TOLERANCE: f64 = 1e-6;

/// 同步进度，作为 `sync://progress` 事件的载荷。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub running: bool,
    pub done: u64,
    pub total: u64,
    pub last_sync_at: Option<String>,
    pub message: String,
}

/// 一次同步的结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub stocks: u64,
    pub bars: u64,
    pub elapsed_ms: u64,
    pub errors: Vec<String>,
}

/// 进度回调。命令层用它更新内存状态并向界面推送事件。
pub type ProgressSink = Arc<dyn Fn(SyncStatus) + Send + Sync>;

/// 什么也不做的进度回调，供测试使用。
pub fn noop_sink() -> ProgressSink {
    Arc::new(|_| {})
}

#[derive(Default)]
struct ProgressState {
    running: bool,
    done: u64,
    total: u64,
    message: String,
}

static PROGRESS: LazyLock<Mutex<ProgressState>> = LazyLock::new(|| Mutex::new(ProgressState::default()));

/// 记录最近一次同步进度，供 `sync_status` 读取。
pub fn publish(status: &SyncStatus) {
    let mut guard = PROGRESS.lock().expect("同步进度锁");
    guard.running = status.running;
    guard.done = status.done;
    guard.total = status.total;
    guard.message = status.message.clone();
}

/// 当前同步状态。`last_sync_at` 由调用方从数据库读出后填入。
pub fn snapshot(last_sync_at: Option<String>) -> SyncStatus {
    let guard = PROGRESS.lock().expect("同步进度锁");
    SyncStatus {
        running: guard.running,
        done: guard.done,
        total: guard.total,
        last_sync_at,
        message: guard.message.clone(),
    }
}

/// 股票池里的一只股票。
#[derive(Debug, Clone, PartialEq)]
pub struct StockInfo {
    pub code: String,
    pub name: String,
    /// A 股是 `SH` | `SZ`，美股是 `US`。
    pub market: String,
    /// A 股是 `main` | `star` | `chinext` | `bse`，美股是 `common`。
    pub board: String,
    pub listed_at: Option<String>,
    pub is_st: bool,
}

impl StockInfo {
    fn to_stock(&self, updated_at: &str) -> Stock {
        Stock {
            code: self.code.clone(),
            name: self.name.clone(),
            market: self.market.clone(),
            board: self.board.clone(),
            sector: None,
            listed_at: self.listed_at.clone(),
            is_st: self.is_st,
            updated_at: updated_at.to_string(),
        }
    }
}

/// 上市满六个月的分界日。
pub fn listed_cutoff(today: NaiveDate) -> NaiveDate {
    today.checked_sub_months(Months::new(6)).unwrap_or(today)
}

/// 从证券列表里筛出股票池。
///
/// A 股只留沪深主板并剔除 ST，科创、创业与北交所的证券不进池。美股只留纳斯达克、
/// 纽交所与美交所的普通股与存托凭证，目录层面的过滤在
/// [`crate::market::us::parse_directory`] 里做完，这里只补上交易所与板块。
/// 上市不满六个月的股票要等抓到日线才知道，在 [`backfill_one`] 里剔除。
pub fn filter_universe(rows: &[Security], market: Market) -> Vec<StockInfo> {
    rows.iter()
        .filter_map(|row| match market {
            Market::Cn => {
                if board_of(&row.code)? != "main" {
                    return None;
                }
                if row.name.contains("ST") {
                    return None;
                }
                Some(StockInfo {
                    code: row.code.clone(),
                    name: row.name.clone(),
                    market: market_of(&row.code).to_string(),
                    board: "main".to_string(),
                    listed_at: None,
                    is_st: false,
                })
            }
            Market::Us => Some(StockInfo {
                code: row.code.clone(),
                name: row.name.clone(),
                market: "US".to_string(),
                board: "common".to_string(),
                listed_at: None,
                is_st: false,
            }),
        })
        .collect()
}

/// 一次同步的参数。
#[derive(Debug, Clone)]
pub struct SyncOptions {
    /// 是否忽略已有进度、重新抓取整段历史。
    pub full: bool,
    /// 判断上市时长与回补起点用的当天日期。
    pub today: NaiveDate,
    /// 并发请求数。
    pub max_concurrency: usize,
}

impl SyncOptions {
    /// 按本机当天日期构造。
    pub fn new(full: bool) -> Self {
        Self {
            full,
            today: Local::now().date_naive(),
            max_concurrency: DEFAULT_CONCURRENCY,
        }
    }

    /// 指定当天日期构造，供测试使用。
    pub fn with_today(full: bool, today: NaiveDate) -> Self {
        Self {
            full,
            today,
            max_concurrency: DEFAULT_CONCURRENCY,
        }
    }
}

/// 抓证券列表并回补日线。
pub fn sync_market(
    db: &Database,
    source: &dyn QuoteSource,
    market: Market,
    options: &SyncOptions,
    progress: &ProgressSink,
) -> Result<SyncReport> {
    progress(SyncStatus {
        running: true,
        done: 0,
        total: 0,
        last_sync_at: None,
        message: "抓取证券列表".to_string(),
    });
    match run_sync(db, source, market, options, progress) {
        Ok(report) => Ok(report),
        Err(error) => {
            // 失败时也推一条终止状态，界面不会一直停在「同步中」。
            progress(SyncStatus {
                running: false,
                done: 0,
                total: 0,
                last_sync_at: None,
                message: format!("同步失败: {error}"),
            });
            Err(error)
        }
    }
}

fn run_sync(
    db: &Database,
    source: &dyn QuoteSource,
    market: Market,
    options: &SyncOptions,
    progress: &ProgressSink,
) -> Result<SyncReport> {
    let started = Instant::now();
    let universe = fetch_universe(source, market)?;
    let total = universe.len() as u64;
    progress(SyncStatus {
        running: true,
        done: 0,
        total,
        last_sync_at: None,
        message: format!("回补日线 0/{total}"),
    });

    let queue = Arc::new(Mutex::new(VecDeque::from(universe)));
    let done = Arc::new(AtomicU64::new(0));
    let bar_count = Arc::new(AtomicU64::new(0));
    let stock_count = Arc::new(AtomicU64::new(0));
    let error_sink = Arc::new(Mutex::new(Vec::<String>::new()));
    let concurrency = options.max_concurrency.max(1);

    std::thread::scope(|scope| {
        for _ in 0..concurrency {
            let queue = Arc::clone(&queue);
            let done = Arc::clone(&done);
            let bar_count = Arc::clone(&bar_count);
            let stock_count = Arc::clone(&stock_count);
            let error_sink = Arc::clone(&error_sink);
            let progress = Arc::clone(progress);
            scope.spawn(move || {
                let mut conn = match db.conn() {
                    Ok(conn) => conn,
                    Err(error) => {
                        error_sink
                            .lock()
                            .expect("错误锁")
                            .push(format!("数据库连接失败: {error}"));
                        return;
                    }
                };
                loop {
                    let item = { queue.lock().expect("队列锁").pop_front() };
                    let Some(info) = item else { break };
                    match backfill_one(&mut conn, source, &info, options) {
                        Ok(Backfilled::Written(bars)) => {
                            bar_count.fetch_add(bars as u64, Ordering::SeqCst);
                            stock_count.fetch_add(1, Ordering::SeqCst);
                        }
                        // 取不到日线或上市不满六个月的股票不进股票池，也不算失败。
                        Ok(Backfilled::Skipped) => {}
                        Err(error) => {
                            let mut guard = error_sink.lock().expect("错误锁");
                            if guard.len() < MAX_ERRORS {
                                guard.push(format!("{} 回补失败: {error}", info.code));
                            }
                        }
                    }
                    let finished = done.fetch_add(1, Ordering::SeqCst) + 1;
                    progress(SyncStatus {
                        running: true,
                        done: finished,
                        total,
                        last_sync_at: None,
                        message: format!("回补日线 {finished}/{total}"),
                    });
                }
            });
        }
    });

    let errors: Vec<String> = error_sink.lock().expect("错误锁").drain(..).collect();
    let written = stock_count.load(Ordering::SeqCst);
    let bars = bar_count.load(Ordering::SeqCst);
    // 一只都没写进去说明行情源没在供数据，不能报成「同步完成」。
    if written == 0 && total > 0 {
        return Err(CoreError::Network(format!(
            "{} 只股票一只都没取到日线，行情源可能不可用",
            total
        )));
    }
    let last_sync_at = Some(storage::now());
    let message = if errors.is_empty() {
        "同步完成".to_string()
    } else {
        format!("同步完成，{} 只失败", errors.len())
    };
    progress(SyncStatus {
        running: false,
        done: total,
        total,
        last_sync_at: last_sync_at.clone(),
        message,
    });

    Ok(SyncReport {
        stocks: written,
        bars,
        elapsed_ms: started.elapsed().as_millis() as u64,
        errors,
    })
}

/// 取本市场的证券列表并筛出候选股票池。
pub fn fetch_universe(source: &dyn QuoteSource, market: Market) -> Result<Vec<StockInfo>> {
    Ok(filter_universe(&source.security_list()?, market))
}

/// 回补起点：三年分界日再往前多取 [`LISTING_PROBE_DAYS`] 天。
fn probe_start(fallback: NaiveDate) -> NaiveDate {
    fallback
        .checked_sub_days(Days::new(u64::from(LISTING_PROBE_DAYS)))
        .unwrap_or(fallback)
}

/// 回补一只股票的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Backfilled {
    /// 已写入 `stock` 与 `bar`，携带日线条数。
    Written(usize),
    /// 不进股票池：取不到日线，或者上市不满六个月。
    Skipped,
}

/// 回补一只股票：抓日线，判断上市时长，抓除权除息事件，换算前复权后写库。
///
/// 增量回补取的是「最后一条日线到今天」这一段，只有全量回补（或者库里还没有历史）
/// 取到的窗口才从三年分界日开始，上市时长与上市日这两项因此只在全量那一次判定。
fn backfill_one(
    conn: &mut storage::DbConn,
    source: &dyn QuoteSource,
    info: &StockInfo,
    options: &SyncOptions,
) -> Result<Backfilled> {
    let state = bar_sync_state::get(conn, &info.code)?;
    let fallback = options
        .today
        .checked_sub_months(Months::new(HISTORY_YEARS * 12))
        .unwrap_or(options.today);
    // 全量窗口的起点。三年分界日再往前多取一个月，用来判断三年窗口有没有被填满。
    let full_from = probe_start(fallback);
    let stored_last = state
        .as_ref()
        .and_then(|state| state.last_bar_date.as_deref())
        .filter(|date| !date.is_empty())
        .map(str::to_string);

    // 增量回补从最后一条日线开始，与已有历史接上；库里没有历史时按全量取。
    let mut window_from = match (options.full, stored_last.as_deref()) {
        (false, Some(last)) => NaiveDate::parse_from_str(last, "%Y-%m-%d").unwrap_or(full_from),
        _ => full_from,
    };
    let mut fetched = fetch_adjusted(conn, source, &info.code, window_from)?;
    // 证券列表里也有已退市与长期停牌的代码，它们取不到日线。
    if fetched.raw.is_empty() {
        return Ok(Backfilled::Skipped);
    }

    // 价格基准检查：增量取到的那一段与库里旧的那一段在最后一条日线那天重叠，
    // 同一天的价格对不上就说明中间发生过改变基准的事，整段历史要按新基准重写。
    if window_from > full_from && rebased(conn, &info.code, stored_last.as_deref(), &fetched.rows)?
    {
        window_from = full_from;
        fetched = fetch_adjusted(conn, source, &info.code, full_from)?;
        // 重写整段之前先把旧行删掉：新窗口之外可能还留着旧基准的行。
        bar::delete_for_code(conn, &info.code)?;
    }

    let Some(first) = fetched.raw.first() else {
        return Ok(Backfilled::Skipped);
    };
    let first_date = NaiveDate::parse_from_str(&first.date, "%Y-%m-%d")
        .map_err(|e| CoreError::InvalidData(format!("日线日期不是 YYYY-MM-DD: {e}")))?;
    let listed_at = if window_from > full_from {
        // 增量窗口的起点是最后一条日线，判不出上市日，沿用库里已有的值。
        stock::get(conn, &info.code)?.and_then(|row| row.listed_at)
    } else {
        // 最早一根日线晚于六个月分界日，说明上市不满六个月。
        if first_date > listed_cutoff(options.today) {
            return Ok(Backfilled::Skipped);
        }
        // 最早一根日线晚于三年分界日时它就是上市首日；否则上市已超过三年，上市日无从得知。
        if first_date > fallback {
            Some(first.date.clone())
        } else {
            None
        }
    };

    let now = storage::now();
    let mut info = info.clone();
    info.listed_at = listed_at;
    stock::upsert(conn, &info.to_stock(&now))?;
    let written = bar::upsert_many(conn, &fetched.rows)?;
    bar_sync_state::upsert(
        conn,
        &BarSyncState {
            code: info.code.clone(),
            last_bar_date: fetched.rows.last().map(|row| row.date.clone()),
            fetched_at: Some(now.clone()),
            updated_at: now,
        },
    )?;
    Ok(Backfilled::Written(written))
}

/// 取一只股票从 `from` 起的日线，记下除权除息事件，换算成前复权。
fn fetch_adjusted(
    conn: &mut storage::DbConn,
    source: &dyn QuoteSource,
    code: &str,
    from: NaiveDate,
) -> Result<FetchedWindow> {
    let raw = source.daily_raw(code, &from.format("%Y%m%d").to_string())?;
    let events = source.corporate_actions(code)?;
    record_events(conn, code, &events, &raw)?;
    let effective = effective_events(conn, code, raw.last().map(|bar| bar.date.as_str()))?;
    let rows = adjust_bars(&raw, &effective);
    Ok(FetchedWindow { raw, rows })
}

/// 一次取数：行情源给的原始窗口，以及换算成前复权之后的行。
struct FetchedWindow {
    raw: Vec<Bar>,
    rows: Vec<BarRow>,
}

/// 价格基准变没变：拿库里最后一条日线的收盘价，与新取到的那一段里同一天的值比。
///
/// 两边走同一条换算与取整路径，所以基准没变时两个数完全相同；真出现差别就是基准变了
/// （拆股、除权除息，或者行情源改了复权口径）。容差只用来挡住浮点噪声。
fn rebased(
    conn: &storage::DbConn,
    code: &str,
    stored_last: Option<&str>,
    fresh: &[BarRow],
) -> Result<bool> {
    let Some(last) = stored_last else {
        return Ok(false);
    };
    let Some(stored) = bar::close_at(conn, code, last)? else {
        return Ok(false);
    };
    Ok(fresh
        .iter()
        .find(|row| row.date == last)
        .is_some_and(|row| (row.close - stored).abs() > BASIS_TOLERANCE))
}

/// 尚未到除权日的事件不改写历史，与行情源一致。
pub(crate) fn effective_events(
    conn: &rusqlite::Connection,
    code: &str,
    last_bar_date: Option<&str>,
) -> Result<Vec<crate::storage::corporate_action::CorporateAction>> {
    Ok(corporate_action::list_for_code(conn, code)?
        .into_iter()
        .filter(|event| last_bar_date.is_some_and(|last| event.ex_date.as_str() <= last))
        .collect())
}

/// 是否该在收盘后自动回补一次：过了本市场的收盘时间，且今天还没跑过。
///
/// `now` 与 `today` 都取本市场的当地时间，`close` 是本市场的收盘时刻。
pub fn should_run_auto_sync(
    now: chrono::NaiveTime,
    close: chrono::NaiveTime,
    today: NaiveDate,
    last_run: Option<NaiveDate>,
) -> bool {
    now >= close && last_run != Some(today)
}

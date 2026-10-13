//! 股票池与日线回补。
//!
//! 抓沪深主板的股票列表写进 `stock` 表，再逐只抓最近三年的前复权日线写进 `bar`
//! 表。回补可以在收盘后由定时任务触发一次，也可以由 `sync_market` 命令手动触发。

use crate::error::{CoreError, Result};
use crate::market::{eastmoney, Bar};
use crate::storage::bar::BarRow;
use crate::storage::bar_sync_state::BarSyncState;
use crate::storage::stock::Stock;
use crate::storage::{self, bar, bar_sync_state, stock, Database};
use chrono::{Local, Months, NaiveDate, NaiveTime};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

/// 回补的历史长度（年）。
pub const HISTORY_YEARS: u32 = 3;
/// 默认的并发请求数。
pub const DEFAULT_CONCURRENCY: usize = 6;
/// 保留的错误条数上限。
const MAX_ERRORS: usize = 50;
/// 股票列表的分页上限，防御服务端返回异常。
const MAX_PAGES: u32 = 200;
/// 单次请求的失败重试次数。
const RETRIES: usize = 2;

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
    /// `SH` | `SZ` | `BJ`。
    pub market: String,
    /// `main` | `star` | `chinext` | `bse`。
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

/// 六位代码所属的板块。只认沪深主板、科创板、创业板与北交所。
pub fn board_of(code: &str) -> Option<&'static str> {
    let prefix = code.get(0..3)?;
    match prefix {
        "600" | "601" | "603" | "605" | "000" | "001" | "002" | "003" => Some("main"),
        "688" | "689" => Some("star"),
        "300" | "301" => Some("chinext"),
        _ if prefix.starts_with('4') || prefix.starts_with('8') || prefix == "920" => Some("bse"),
        _ => None,
    }
}

/// 六位代码所属的市场。
pub fn market_of(code: &str) -> &'static str {
    if code.starts_with("920") || code.starts_with('4') || code.starts_with('8') {
        "BJ"
    } else if code.starts_with('6') || code.starts_with('9') {
        "SH"
    } else {
        "SZ"
    }
}

/// 上市满六个月的分界日。
pub fn listed_cutoff(today: NaiveDate) -> NaiveDate {
    today.checked_sub_months(Months::new(6)).unwrap_or(today)
}

/// 从列表里筛出沪深主板的股票池。
///
/// 排除 ST（含 `*ST`）、上市不足六个月的、以及科创板创业板北交所的证券。
pub fn filter_universe(rows: &[eastmoney::ClistRow], today: NaiveDate) -> Vec<StockInfo> {
    let cutoff = listed_cutoff(today);
    rows.iter()
        .filter_map(|row| {
            if board_of(&row.code)? != "main" {
                return None;
            }
            if row.name.contains("ST") {
                return None;
            }
            // 没有上市日就无法判断是否满六个月。
            let listed_date = NaiveDate::parse_from_str(row.listed_at.as_deref()?, "%Y-%m-%d").ok()?;
            if listed_date > cutoff {
                return None;
            }
            Some(StockInfo {
                code: row.code.clone(),
                name: row.name.clone(),
                market: market_of(&row.code).to_string(),
                board: "main".to_string(),
                listed_at: row.listed_at.clone(),
                is_st: false,
            })
        })
        .collect()
}

/// 行情抓取的抽象，便于用录制样本回放测试。
pub trait MarketFetch: Send + Sync {
    /// 取全市场列表的一页。
    fn clist_page(&self, page: u32, page_size: u32) -> Result<String>;
    /// 取一只股票从 `beg`（`YYYYMMDD`）到最新的前复权日线。
    fn kline(&self, secid: &str, beg: &str) -> Result<String>;
}

/// 走真实网络的抓取实现。
pub struct HttpFetch {
    client: Client,
}

impl HttpFetch {
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

impl MarketFetch for HttpFetch {
    fn clist_page(&self, page: u32, page_size: u32) -> Result<String> {
        let url = eastmoney::clist_url(page, page_size, eastmoney::MAIN_BOARD_FS);
        get_text(&self.client, &url, &[])
    }

    fn kline(&self, secid: &str, beg: &str) -> Result<String> {
        let url = eastmoney::kline_url(
            secid,
            eastmoney::KLT_DAY,
            eastmoney::FQT_QFQ,
            Some(beg),
            Some(eastmoney::END_LATEST),
            None,
        );
        get_text(&self.client, &url, &[])
    }
}

fn get_text(client: &Client, url: &str, headers: &[(&str, &str)]) -> Result<String> {
    let mut last_error = None;
    for attempt in 0..=RETRIES {
        let mut request = client.get(url);
        for (key, value) in headers {
            request = request.header(*key, *value);
        }
        match request.send() {
            Ok(response) => {
                let status = response.status();
                if !status.is_success() {
                    last_error = Some(format!("HTTP {}", status.as_u16()));
                } else {
                    return response
                        .text()
                        .map_err(|e| CoreError::Io(io::Error::other(format!("读取响应失败: {e}"))));
                }
            }
            Err(error) => last_error = Some(error.to_string()),
        }
        if attempt < RETRIES {
            std::thread::sleep(Duration::from_millis(300 * (attempt as u64 + 1)));
        }
    }
    Err(CoreError::Io(io::Error::other(format!(
        "请求 {url} 失败: {}",
        last_error.unwrap_or_else(|| "未知错误".to_string())
    ))))
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

/// 抓股票列表并回补日线。
pub fn sync_market(
    db: &Database,
    fetch: &dyn MarketFetch,
    options: &SyncOptions,
    progress: &ProgressSink,
) -> Result<SyncReport> {
    progress(SyncStatus {
        running: true,
        done: 0,
        total: 0,
        last_sync_at: None,
        message: "抓取股票列表".to_string(),
    });
    match run_sync(db, fetch, options, progress) {
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
    fetch: &dyn MarketFetch,
    options: &SyncOptions,
    progress: &ProgressSink,
) -> Result<SyncReport> {
    let started = Instant::now();
    let universe = fetch_universe(fetch, options.today)?;
    {
        let conn = db.conn()?;
        let now = storage::now();
        for info in &universe {
            stock::upsert(&conn, &info.to_stock(&now))?;
        }
    }

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
    let error_sink = Arc::new(Mutex::new(Vec::<String>::new()));
    let concurrency = options.max_concurrency.max(1);

    std::thread::scope(|scope| {
        for _ in 0..concurrency {
            let queue = Arc::clone(&queue);
            let done = Arc::clone(&done);
            let bar_count = Arc::clone(&bar_count);
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
                    match backfill_one(&mut conn, fetch, &info, options) {
                        Ok(written) => {
                            bar_count.fetch_add(written as u64, Ordering::SeqCst);
                        }
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
        stocks: total,
        bars: bar_count.load(Ordering::SeqCst),
        elapsed_ms: started.elapsed().as_millis() as u64,
        errors,
    })
}

/// 分页取完股票列表并筛选出股票池。
pub fn fetch_universe(fetch: &dyn MarketFetch, today: NaiveDate) -> Result<Vec<StockInfo>> {
    let mut rows: Vec<eastmoney::ClistRow> = Vec::new();
    let mut total: Option<i64> = None;
    let mut page = 1u32;
    loop {
        let body = fetch.clist_page(page, eastmoney::PAGE_SIZE)?;
        let parsed = eastmoney::parse_clist(&body)?;
        let reported = *total.get_or_insert(parsed.total);
        if parsed.rows.is_empty() {
            break;
        }
        rows.extend(parsed.rows);
        if rows.len() as i64 >= reported {
            break;
        }
        page += 1;
        if page > MAX_PAGES {
            return Err(CoreError::InvalidData(format!(
                "股票列表分页超过 {MAX_PAGES} 页"
            )));
        }
    }
    let reported = total.unwrap_or(0);
    if rows.len() as i64 != reported {
        return Err(CoreError::InvalidData(format!(
            "股票列表取到 {} 条，服务端报告 {reported} 条",
            rows.len()
        )));
    }
    Ok(filter_universe(&rows, today))
}

/// 回补一只股票，返回写入的日线条数。
fn backfill_one(
    conn: &mut storage::DbConn,
    fetch: &dyn MarketFetch,
    info: &StockInfo,
    options: &SyncOptions,
) -> Result<usize> {
    let state = bar_sync_state::get(conn, &info.code)?;
    let fallback = options
        .today
        .checked_sub_months(Months::new(HISTORY_YEARS * 12))
        .unwrap_or(options.today);
    let beg = if options.full {
        fallback
    } else {
        state
            .as_ref()
            .and_then(|state| state.last_bar_date.as_deref())
            .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
            .unwrap_or(fallback)
    };
    let beg_text = beg.format("%Y%m%d").to_string();

    let body = fetch.kline(&eastmoney::secid(&info.code), &beg_text)?;
    let page = eastmoney::parse_kline(&body)?;
    let rows: Vec<BarRow> = page
        .bars
        .iter()
        .map(|bar| BarRow {
            code: bar.code.clone(),
            date: bar.date.clone(),
            open: bar.open,
            high: bar.high,
            low: bar.low,
            close: bar.close,
            volume: bar.volume,
            amount: bar.amount,
        })
        .collect();
    let written = bar::upsert_many(conn, &rows)?;

    let last_bar_date = rows
        .last()
        .map(|row| row.date.clone())
        .or_else(|| state.as_ref().and_then(|state| state.last_bar_date.clone()));
    bar_sync_state::upsert(
        conn,
        &BarSyncState {
            code: info.code.clone(),
            last_bar_date,
            fetched_at: Some(storage::now()),
            dktotal: Some(page.dktotal),
            tail_qfq: Some(tail_qfq_json(&page.bars)),
            updated_at: storage::now(),
        },
    )?;
    Ok(written)
}

/// 最近十条前复权收盘价的 JSON：`[[date, close], ...]`。
fn tail_qfq_json(bars: &[Bar]) -> String {
    let start = bars.len().saturating_sub(10);
    let tail: Vec<serde_json::Value> = bars[start..]
        .iter()
        .map(|bar| serde_json::json!([bar.date, bar.close]))
        .collect();
    serde_json::Value::Array(tail).to_string()
}

/// 收盘时间。
pub fn close_time() -> NaiveTime {
    NaiveTime::from_hms_opt(15, 0, 0).expect("15:00 是合法时刻")
}

/// 是否该在收盘后自动回补一次：过了收盘时间，且今天还没跑过。
pub fn should_run_auto_sync(now: NaiveTime, today: NaiveDate, last_run: Option<NaiveDate>) -> bool {
    now >= close_time() && last_run != Some(today)
}

static LAST_AUTO_SYNC: LazyLock<Mutex<Option<NaiveDate>>> = LazyLock::new(|| Mutex::new(None));

/// 起一个后台线程，收盘后自动回补一次。
pub fn spawn_daily_scheduler(db: Database, progress: ProgressSink) {
    std::thread::spawn(move || loop {
        let now = Local::now();
        let today = now.date_naive();
        let last_run = *LAST_AUTO_SYNC.lock().expect("自动回补锁");
        if should_run_auto_sync(now.time(), today, last_run) {
            if let Ok(fetch) = HttpFetch::new() {
                let options = SyncOptions::new(false);
                if sync_market(&db, &fetch, &options, &progress).is_ok() {
                    *LAST_AUTO_SYNC.lock().expect("自动回补锁") = Some(today);
                }
            }
        }
        std::thread::sleep(Duration::from_secs(60));
    });
}

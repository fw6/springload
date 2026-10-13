//! 前复权换算与过期判定。
//!
//! 事实来源见 `docs/research/suspension-and-adjustment.md` 第 2、3、5 节：
//!
//! - 除权（息）参考价 `(前收盘价 - D) / (1 + r)`，除权日行情里的前收盘价就是它；
//! - 前复权对日期 `d` 之后的每一次除权除息事件依次施加 `P ↦ (P - D) / (1 + r)`；
//! - 除权日之前的历史在除权日当日被整体改写，据此判断已存的前复权历史是否过期。

use crate::error::{CoreError, Result};
use crate::market::{eastmoney, Bar};
use crate::storage::bar::BarRow;
use crate::storage::bar_sync_state::{self, BarSyncState};
use crate::storage::corporate_action::{self, CorporateAction};
use crate::storage::{self, bar, stock, Database};
use chrono::{Months, NaiveDate};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 回补的历史长度（年），与日线回补一致。
pub const HISTORY_YEARS: u32 = crate::backfill::HISTORY_YEARS;
/// 检查与重算时的并发请求数。
pub const DEFAULT_CONCURRENCY: usize = crate::backfill::DEFAULT_CONCURRENCY;
/// 尾部比对取多少条前复权收盘价。
pub const TAIL_LEN: u32 = 10;
/// 保留的错误条数上限。
const MAX_ERRORS: usize = 50;
/// 单次请求的失败重试次数。
const RETRIES: usize = 2;
/// 不复权（原始）日线的 `fqt` 取值。
pub const FQT_RAW: u32 = 0;

/// 东方财富分红送配数据接口。
pub const DIVIDEND_ENDPOINT: &str = "https://datacenter-web.eastmoney.com/api/data/v1/get";
/// 分红送配单页记录数。
pub const DIVIDEND_PAGE_SIZE: u32 = 50;

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

/// 判据 A：最近一次除权除息日早于本地最后一条日线时不判过期；晚于它则本地前复权历史过期。
pub fn stale_by_event(latest_ex: Option<&str>, last_bar_date: Option<&str>) -> bool {
    match (latest_ex, last_bar_date) {
        (Some(ex), Some(last)) => ex > last,
        _ => false,
    }
}

/// 判据 B：服务端尾部前复权收盘价与本地保存的同一日期逐条比对，任一条不同即过期。
pub fn tail_overlap_changed(server: &[(String, f64)], local: &[(String, f64)]) -> bool {
    if local.is_empty() {
        return false;
    }
    server.iter().any(|(date, close)| {
        local
            .iter()
            .find(|(local_date, _)| local_date == date)
            .is_some_and(|(_, old)| old != close)
    })
}

/// 解析 `bar_sync_state.tail_qfq` 的 JSON：`[[date, close], ...]`。
pub fn parse_tail_qfq(json: &str) -> Vec<(String, f64)> {
    let Ok(value) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let Some(rows) = value.as_array() else {
        return Vec::new();
    };
    rows.iter()
        .filter_map(|row| {
            let pair = row.as_array()?;
            let date = pair.first()?.as_str()?.to_string();
            let close = pair.get(1)?.as_f64()?;
            Some((date, close))
        })
        .collect()
}

/// 分红送配接口里的一次除权除息。
#[derive(Debug, Clone, PartialEq)]
pub struct DividendRecord {
    /// 除权除息日 YYYY-MM-DD。
    pub ex_date: String,
    /// 每股现金分红 D。
    pub dividend_per_share: f64,
    /// 每股送转比例 r。
    pub split_ratio: f64,
}

/// 分红送配请求的 URL。`filter` 里的引号必须编码，原样引号会被服务端判 400。
pub fn dividend_url(code: &str) -> String {
    format!(
        "{DIVIDEND_ENDPOINT}?sortColumns=EX_DIVIDEND_DATE&sortTypes=-1\
         &pageSize={DIVIDEND_PAGE_SIZE}&pageNumber=1&reportName=RPT_SHAREBONUS_DET&columns=ALL\
         &filter=(SECURITY_CODE%3D%22{code}%22)"
    )
}

/// 解析 `RPT_SHAREBONUS_DET` 响应。
///
/// 每 10 股税前派现元除以 10 得到每股现金红利 D，每 10 股送转股数除以 10 得到 r。
/// `EX_DIVIDEND_DATE` 为空（尚未实施）的记录跳过；同一天出现多条时保留第一条。
pub fn parse_dividends(body: &str) -> Result<Vec<DividendRecord>> {
    let root: Value = serde_json::from_str(body)?;
    let result = root.get("result");
    let data = match result {
        // `result` 为 null（例如 `success:false, code:9201`）表示该股票没有分红记录。
        None => {
            return Err(CoreError::InvalidData(
                "分红送配接口没有返回 result 字段".to_string(),
            ))
        }
        Some(Value::Null) => return Ok(Vec::new()),
        Some(result) => result
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
    };
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in &data {
        let Some(ex_date) = item.get("EX_DIVIDEND_DATE").and_then(Value::as_str) else {
            continue;
        };
        let Some(date) = ex_date.get(0..10) else {
            continue;
        };
        if !seen.insert(date.to_string()) {
            continue;
        }
        let per_ten_bonus = number(item.get("PRETAX_BONUS_RMB")).unwrap_or(0.0);
        let per_ten_split = number(item.get("IT_RATIO"))
            .or_else(|| number(item.get("BONUS_IT_RATIO")))
            .unwrap_or(0.0);
        out.push(DividendRecord {
            ex_date: date.to_string(),
            dividend_per_share: per_ten_bonus / 10.0,
            split_ratio: per_ten_split / 10.0,
        });
    }
    out.sort_by(|a, b| a.ex_date.cmp(&b.ex_date));
    Ok(out)
}

/// 数值字段可能是数字，也可能是字符串。
fn number(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

/// 需要重算的股票。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaleStock {
    pub code: String,
    pub name: String,
    /// 触发过期的最近一次除权除息日；仅由尾部数值比对发现时为 `None`。
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

/// 检查与重算需要的行情抓取抽象，便于用录制样本回放测试。
pub trait AdjustmentFetch: Send + Sync {
    /// 取一只股票最近 `lmt` 条前复权日线。
    fn qfq_tail(&self, code: &str, lmt: u32) -> Result<String>;
    /// 取一只股票从 `beg`（`YYYYMMDD`）到最新的不复权日线。
    fn raw_history(&self, code: &str, beg: &str) -> Result<String>;
    /// 取一只股票的分红送配记录。
    fn dividends(&self, code: &str) -> Result<String>;
}

/// 走真实网络的抓取实现。
pub struct HttpAdjustmentFetch {
    client: Client,
}

impl HttpAdjustmentFetch {
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

impl AdjustmentFetch for HttpAdjustmentFetch {
    fn qfq_tail(&self, code: &str, lmt: u32) -> Result<String> {
        let url = eastmoney::kline_url(
            &eastmoney::secid(code),
            eastmoney::KLT_DAY,
            eastmoney::FQT_QFQ,
            None,
            Some(eastmoney::END_LATEST),
            Some(lmt),
        );
        get_text(&self.client, &url)
    }

    fn raw_history(&self, code: &str, beg: &str) -> Result<String> {
        let url = eastmoney::kline_url(
            &eastmoney::secid(code),
            eastmoney::KLT_DAY,
            FQT_RAW,
            Some(beg),
            Some(eastmoney::END_LATEST),
            None,
        );
        get_text(&self.client, &url)
    }

    fn dividends(&self, code: &str) -> Result<String> {
        get_text(&self.client, &dividend_url(code))
    }
}

fn get_text(client: &Client, url: &str) -> Result<String> {
    let mut last_error = None;
    for attempt in 0..=RETRIES {
        match client.get(url).send() {
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

/// 检查全部已回补股票的前复权历史是否过期。
///
/// 对每只股票先取服务端尾部（代价很小）与分红送配记录：判据 A 比较最近一次除权除息日
/// 与本地最后一条日线；分红送配接口不可用时退回判据 B，逐条比对尾部前复权收盘价。
pub fn detect_stale(
    db: &Database,
    fetch: &dyn AdjustmentFetch,
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
        probe_stale(&conn, fetch, code, &name)
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
    fetch: &dyn AdjustmentFetch,
    code: &str,
    name: &str,
) -> Result<Option<StaleStock>> {
    let Some(state) = bar_sync_state::get(conn, code)? else {
        return Ok(None);
    };
    let Some(last_bar_date) = state.last_bar_date.clone() else {
        return Ok(None);
    };
    let local_tail = state
        .tail_qfq
        .as_deref()
        .map(parse_tail_qfq)
        .unwrap_or_default();

    let tail_page = eastmoney::parse_kline(&fetch.qfq_tail(code, TAIL_LEN)?)?;
    let server_tail: Vec<(String, f64)> = tail_page
        .bars
        .iter()
        .map(|bar| (bar.date.clone(), bar.close))
        .collect();
    let server_last = tail_page.bars.last().map(|bar| bar.date.clone());

    let mut ex_date = None;
    let mut stale_by_ex = false;
    // 分红送配接口取不到或解析失败时退回判据 B。
    match fetch
        .dividends(code)
        .and_then(|body| parse_dividends(&body))
    {
        Ok(records) => {
            record_events(conn, code, &records, &HashMap::new())?;
            let cur_last_ex = server_last.as_deref().and_then(|last| {
                records
                    .iter()
                    .filter(|record| record.ex_date.as_str() <= last)
                    .map(|record| record.ex_date.clone())
                    .max()
            });
            stale_by_ex = stale_by_event(cur_last_ex.as_deref(), Some(&last_bar_date));
            ex_date = cur_last_ex;
        }
        Err(error) => {
            eprintln!("{code} 分红送配接口不可用，改用尾部数值比对: {error}");
        }
    }

    if stale_by_ex || tail_overlap_changed(&server_tail, &local_tail) {
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
/// 取不复权日线与分红送配记录，把除权除息事件写进 `corporate_action`，再对整段历史
/// 施加前复权换算后覆盖 `bar` 表，并刷新 `bar_sync_state`。
pub fn resync(
    db: &Database,
    fetch: &dyn AdjustmentFetch,
    codes: &[String],
    today: NaiveDate,
) -> Result<ResyncReport> {
    let started = Instant::now();
    let (written, errors) = run_concurrent(codes.to_vec(), DEFAULT_CONCURRENCY, |code| {
        let mut conn = db.conn()?;
        resync_one(&mut conn, fetch, code, today)
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
    fetch: &dyn AdjustmentFetch,
    code: &str,
    today: NaiveDate,
) -> Result<usize> {
    let beg = today
        .checked_sub_months(Months::new(HISTORY_YEARS * 12))
        .unwrap_or(today);
    let beg_text = beg.format("%Y%m%d").to_string();
    let raw_page = eastmoney::parse_kline(&fetch.raw_history(code, &beg_text)?)?;
    let records = parse_dividends(&fetch.dividends(code)?)?;

    // 除权日的前收盘价 = 上一根不复权收盘价按参考价公式换算。
    let mut pre_closes = HashMap::new();
    for record in &records {
        let prev = raw_page
            .bars
            .iter()
            .filter(|bar| bar.date.as_str() < record.ex_date.as_str())
            .next_back();
        if let Some(prev) = prev {
            pre_closes.insert(
                record.ex_date.clone(),
                reference_price(prev.close, record.dividend_per_share, record.split_ratio),
            );
        }
    }
    record_events(conn, code, &records, &pre_closes)?;

    let server_last = raw_page.bars.last().map(|bar| bar.date.clone());
    let events: Vec<CorporateAction> = corporate_action::list_for_code(conn, code)?
        .into_iter()
        // 尚未到除权日的事件不改写历史，与行情源一致。
        .filter(|event| server_last.as_deref().is_some_and(|last| event.ex_date.as_str() <= last))
        .collect();

    let adjusted: Vec<BarRow> = raw_page
        .bars
        .iter()
        .map(|bar| adjust_bar(&bar_row(bar), &events))
        .collect();
    bar::delete_for_code(conn, code)?;
    let written = bar::upsert_many(conn, &adjusted)?;
    bar_sync_state::upsert(
        conn,
        &BarSyncState {
            code: code.to_string(),
            last_bar_date: adjusted.last().map(|bar| bar.date.clone()),
            fetched_at: Some(storage::now()),
            dktotal: Some(raw_page.dktotal),
            tail_qfq: Some(tail_qfq_json(&adjusted)),
            updated_at: storage::now(),
        },
    )?;
    Ok(written)
}

/// 把分红送配记录写进 `corporate_action`，保留已有的 `pre_close`。
fn record_events(
    conn: &rusqlite::Connection,
    code: &str,
    records: &[DividendRecord],
    pre_closes: &HashMap<String, f64>,
) -> Result<usize> {
    let existing: HashMap<String, Option<f64>> = corporate_action::list_for_code(conn, code)?
        .into_iter()
        .map(|action| (action.ex_date, action.pre_close))
        .collect();
    let now = storage::now();
    for record in records {
        let pre_close = pre_closes
            .get(&record.ex_date)
            .copied()
            .or_else(|| existing.get(&record.ex_date).copied().flatten());
        corporate_action::upsert(
            conn,
            &CorporateAction {
                code: code.to_string(),
                ex_date: record.ex_date.clone(),
                dividend_per_share: record.dividend_per_share,
                split_ratio: record.split_ratio,
                pre_close,
                recorded_at: now.clone(),
            },
        )?;
    }
    Ok(records.len())
}

fn bar_row(bar: &Bar) -> BarRow {
    BarRow {
        code: bar.code.clone(),
        date: bar.date.clone(),
        open: bar.open,
        high: bar.high,
        low: bar.low,
        close: bar.close,
        volume: bar.volume,
        amount: bar.amount,
    }
}

/// 最近 `TAIL_LEN` 条前复权收盘价的 JSON：`[[date, close], ...]`。
fn tail_qfq_json(bars: &[BarRow]) -> String {
    let start = bars.len().saturating_sub(TAIL_LEN as usize);
    let tail: Vec<Value> = bars[start..]
        .iter()
        .map(|bar| serde_json::json!([bar.date, bar.close]))
        .collect();
    Value::Array(tail).to_string()
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

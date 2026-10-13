//! `backfill` 模块：股票池筛选、分页抓取、日线回补与收盘后自动回补的判定。
//!
//! 股票列表与日线样本取自 `tests/fixtures/`，来源 URL 见各测试注释。

use chrono::NaiveDate;
use springload_core::backfill::{
    self, MarketFetch, SyncOptions, SyncStatus, DEFAULT_CONCURRENCY,
};
use springload_core::error::{CoreError, Result};
use springload_core::market::eastmoney;
use springload_core::storage::{bar, bar_sync_state, stock, Database};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("读取样本 {name} 失败: {error}"))
}

/// 用录制样本回放的抓取实现。
struct FixtureFetch {
    pages: Vec<String>,
    klines: HashMap<String, String>,
    requested_begs: Mutex<Vec<(String, String)>>,
}

impl FixtureFetch {
    fn new(pages: Vec<String>, klines: HashMap<String, String>) -> Self {
        Self {
            pages,
            klines,
            requested_begs: Mutex::new(Vec::new()),
        }
    }
}

impl MarketFetch for FixtureFetch {
    fn clist_page(&self, page: u32, _page_size: u32) -> Result<String> {
        self.pages
            .get((page - 1) as usize)
            .cloned()
            .ok_or_else(|| CoreError::InvalidData(format!("没有第 {page} 页样本")))
    }

    fn kline(&self, secid: &str, beg: &str) -> Result<String> {
        self.requested_begs
            .lock()
            .expect("记录锁")
            .push((secid.to_string(), beg.to_string()));
        self.klines
            .get(secid)
            .cloned()
            .ok_or_else(|| CoreError::InvalidData(format!("没有 {secid} 的日线样本")))
    }
}

/// 由第 1 页样本里的真实行拼出的一份最小列表响应，只保留给定代码。
///
/// 只用于驱动回补流水线；完整页面的解析由 `market_test.rs` 用原始样本断言。
fn clist_subset(codes: &[&str]) -> String {
    clist_subset_with_total(codes, codes.len())
}

/// 同上，但把服务端报告的 `total` 改成给定值，用来构造条数不一致的响应。
fn clist_subset_with_total(codes: &[&str], total: usize) -> String {
    let mut value: serde_json::Value =
        serde_json::from_str(&fixture("eastmoney_clist_page1.json")).expect("解析样本");
    let diff = value["data"]["diff"].as_array_mut().expect("diff 是数组");
    diff.retain(|row| {
        codes
            .iter()
            .any(|code| row["f12"].as_str() == Some(*code))
    });
    assert_eq!(diff.len(), codes.len(), "样本里应能找齐给定代码");
    value["data"]["total"] = serde_json::json!(total);
    value.to_string()
}

fn kline_samples() -> HashMap<String, String> {
    let mut map = HashMap::new();
    for code in ["605599", "605598", "605589"] {
        map.insert(
            format!("1.{code}"),
            fixture(&format!("eastmoney_kline_{code}.json")),
        );
    }
    map
}

fn open_db(dir: &tempfile::TempDir) -> Database {
    Database::open(&dir.path().join("springload.db")).expect("打开数据库")
}

#[test]
fn board_of_classifies_main_star_chinext_and_bse() {
    for code in ["600000", "601398", "603188", "605599", "000001", "001309", "002388", "003037"] {
        assert_eq!(backfill::board_of(code), Some("main"), "{code}");
    }
    assert_eq!(backfill::board_of("688981"), Some("star"));
    assert_eq!(backfill::board_of("300750"), Some("chinext"));
    assert_eq!(backfill::board_of("301660"), Some("chinext"));
    assert_eq!(backfill::board_of("920821"), Some("bse"));
    assert_eq!(backfill::board_of("900948"), None, "上海 B 股不在范围里");
    assert_eq!(backfill::board_of("200530"), None, "深圳 B 股不在范围里");
}

#[test]
fn market_of_maps_code_to_exchange() {
    assert_eq!(backfill::market_of("600000"), "SH");
    assert_eq!(backfill::market_of("688981"), "SH");
    assert_eq!(backfill::market_of("000001"), "SZ");
    assert_eq!(backfill::market_of("300750"), "SZ");
    assert_eq!(backfill::market_of("920821"), "BJ");
}

#[test]
fn listed_cutoff_is_six_months_before() {
    let today = NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();
    assert_eq!(
        backfill::listed_cutoff(today),
        NaiveDate::from_ymd_opt(2026, 4, 13).unwrap()
    );
}

#[test]
fn filter_universe_keeps_only_main_board_listed_six_months_without_st() {
    let today = NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();
    let row = |code: &str, name: &str, listed: Option<&str>| eastmoney::ClistRow {
        code: code.to_string(),
        market: if code.starts_with('6') { 1 } else { 0 },
        name: name.to_string(),
        listed_at: listed.map(str::to_string),
        last: Some(10.0),
        prev_close: Some(10.0),
    };
    let rows = vec![
        row("600000", "浦发银行", Some("1999-11-10")),
        row("000001", "平安银行", Some("1991-04-03")),
        // ST 与 *ST 都排除。
        row("600001", "ST 某股", Some("2000-01-01")),
        row("600002", "*ST 某股", Some("2000-01-01")),
        // 上市不足六个月。
        row("600003", "新股", Some("2026-06-01")),
        // 上市刚满六个月，保留。
        row("600004", "临界股", Some("2026-04-13")),
        // 科创板、创业板、北交所排除。
        row("688981", "中芯国际", Some("2020-07-16")),
        row("300750", "宁德时代", Some("2018-06-11")),
        row("920821", "北交所股", Some("2021-01-01")),
        // 缺少上市日，无法判断，排除。
        row("600005", "无上市日", None),
    ];
    let kept = backfill::filter_universe(&rows, today);
    let codes: Vec<&str> = kept.iter().map(|info| info.code.as_str()).collect();
    assert_eq!(codes, vec!["600000", "000001", "600004"]);

    let first = &kept[0];
    assert_eq!(first.market, "SH");
    assert_eq!(first.board, "main");
    assert!(!first.is_st);
}

#[test]
fn fetch_universe_pages_until_total_and_checks_count() {
    let today = NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();
    let fetch = FixtureFetch::new(vec![clist_subset(&["605599", "605598", "605589"])], HashMap::new());
    let universe = backfill::fetch_universe(&fetch, today).expect("抓取股票池");
    assert_eq!(universe.len(), 3);
    let mut codes: Vec<&str> = universe.iter().map(|info| info.code.as_str()).collect();
    codes.sort();
    assert_eq!(codes, vec!["605589", "605598", "605599"]);
}

#[test]
fn fetch_universe_rejects_count_mismatch() {
    let today = NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();
    // 第二页为空，但服务端报告 5 条。
    let empty_page = r#"{"rc":0,"data":{"total":5,"diff":[]}}"#.to_string();
    let fetch = FixtureFetch::new(
        vec![clist_subset_with_total(&["605599", "605598", "605589"], 5), empty_page],
        HashMap::new(),
    );
    let error = backfill::fetch_universe(&fetch, today).expect_err("条数不一致应报错");
    assert!(error.to_string().contains("3 条"), "{error}");
    assert!(error.to_string().contains("5 条"), "{error}");
}

#[test]
fn sync_market_writes_stocks_bars_and_sync_state() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(&dir);
    let fetch = FixtureFetch::new(
        vec![clist_subset(&["605599", "605598", "605589"])],
        kline_samples(),
    );
    let today = NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();
    let options = SyncOptions::with_today(true, today);

    let events: Arc<Mutex<Vec<SyncStatus>>> = Arc::new(Mutex::new(Vec::new()));
    let sink_events = Arc::clone(&events);
    let sink: backfill::ProgressSink = Arc::new(move |status| {
        sink_events.lock().expect("事件锁").push(status);
    });

    let report = backfill::sync_market(&db, &fetch, &options, &sink).expect("同步成功");
    assert_eq!(report.stocks, 3);
    assert!(report.bars > 0);
    assert!(report.errors.is_empty(), "{:?}", report.errors);

    let conn = db.conn().unwrap();
    assert_eq!(stock::count(&conn).unwrap(), 3);
    assert!(stock::get(&conn, "605599").unwrap().is_some());
    assert!(bar::count(&conn).unwrap() > 0);

    let bars = bar::list_for_code(&conn, "605599").unwrap();
    assert!(!bars.is_empty());
    assert_eq!(bars[0].date, "2026-09-01");
    assert_eq!(bars.last().unwrap().date, "2026-10-13");

    let state = bar_sync_state::get(&conn, "605599").unwrap().expect("有回补状态");
    assert_eq!(state.dktotal, Some(1229));
    assert!(state.last_bar_date.is_some());
    let tail: serde_json::Value =
        serde_json::from_str(state.tail_qfq.as_deref().expect("有尾部收盘价")).expect("尾部是 JSON");
    assert_eq!(tail.as_array().unwrap().len(), 10);

    // 回补范围是最近三年。
    let begs = fetch.requested_begs.lock().unwrap().clone();
    assert_eq!(begs.len(), 3);
    for (_, beg) in &begs {
        assert_eq!(beg, "20231013");
    }

    // 进度事件最后一条是终止状态。
    let events = events.lock().unwrap();
    assert!(!events.is_empty());
    let last = events.last().unwrap();
    assert!(!last.running);
    assert_eq!(last.total, 3);
    assert_eq!(last.done, 3);
    assert!(last.last_sync_at.is_some());
}

#[test]
fn incremental_sync_starts_from_last_bar_date() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(&dir);
    let fetch = FixtureFetch::new(
        vec![clist_subset(&["605599"])],
        kline_samples(),
    );
    let today = NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();

    backfill::sync_market(
        &db,
        &fetch,
        &SyncOptions::with_today(true, today),
        &backfill::noop_sink(),
    )
    .expect("首次同步");

    backfill::sync_market(
        &db,
        &fetch,
        &SyncOptions::with_today(false, today),
        &backfill::noop_sink(),
    )
    .expect("增量同步");

    let begs = fetch.requested_begs.lock().unwrap().clone();
    assert_eq!(begs.len(), 2);
    assert_eq!(begs[0].1, "20231013", "首次按三年回补");
    // 样本最后一根是 2026-10-13，增量从它开始。
    assert_eq!(begs[1].1, "20261013");
}

#[test]
fn sync_market_keeps_going_when_one_stock_fails() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(&dir);
    // 只提供两只股票的日线样本，第三只取不到。
    let mut klines = kline_samples();
    klines.remove("1.605589");
    let fetch = FixtureFetch::new(
        vec![clist_subset(&["605599", "605598", "605589"])],
        klines,
    );
    let today = NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();
    let report = backfill::sync_market(
        &db,
        &fetch,
        &SyncOptions::with_today(true, today),
        &backfill::noop_sink(),
    )
    .expect("整体同步成功");
    assert_eq!(report.stocks, 3);
    assert_eq!(report.errors.len(), 1);
    assert!(report.errors[0].contains("605589"), "{:?}", report.errors);

    let conn = db.conn().unwrap();
    assert!(bar::count(&conn).unwrap() > 0);
    assert!(bar_sync_state::get(&conn, "605589").unwrap().is_none());
}

#[test]
fn sync_options_use_default_concurrency() {
    let options = SyncOptions::new(false);
    assert_eq!(options.max_concurrency, DEFAULT_CONCURRENCY);
}

#[test]
fn auto_sync_runs_once_after_close() {
    let today = NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();
    let before_close = chrono::NaiveTime::from_hms_opt(14, 59, 0).unwrap();
    let at_close = chrono::NaiveTime::from_hms_opt(15, 0, 0).unwrap();
    let after_close = chrono::NaiveTime::from_hms_opt(16, 30, 0).unwrap();

    assert!(!backfill::should_run_auto_sync(before_close, today, None));
    assert!(backfill::should_run_auto_sync(at_close, today, None));
    assert!(backfill::should_run_auto_sync(after_close, today, None));
    // 今天已经跑过就不再跑。
    assert!(!backfill::should_run_auto_sync(after_close, today, Some(today)));
    // 昨天跑过，今天照常跑。
    let yesterday = NaiveDate::from_ymd_opt(2026, 10, 12).unwrap();
    assert!(backfill::should_run_auto_sync(after_close, today, Some(yesterday)));
}

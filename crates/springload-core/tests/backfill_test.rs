//! `backfill` 模块：股票池筛选、日线回补、前复权换算与收盘后自动回补的判定。
//!
//! 用录制样本回放，不联网。

use chrono::NaiveDate;
use springload_core::backfill::{self, SyncOptions, SyncStatus, DEFAULT_CONCURRENCY};
use springload_core::error::{CoreError, Result};
use springload_core::market::{Bar, Market, MinuteBar, QuoteSource, Security};
use springload_core::storage::corporate_action::CorporateAction;
use springload_core::storage::{bar, bar_sync_state, corporate_action, stock, Database};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// 用录制样本回放的行情数据源。
struct FixtureSource {
    lists: Vec<Security>,
    daily: HashMap<String, Vec<Bar>>,
    events: HashMap<String, Vec<CorporateAction>>,
    requested_begs: Mutex<Vec<(String, String)>>,
}

impl FixtureSource {
    fn new() -> Self {
        Self {
            lists: Vec::new(),
            daily: HashMap::new(),
            events: HashMap::new(),
            requested_begs: Mutex::new(Vec::new()),
        }
    }

    fn with_list(mut self, rows: &[(&str, &str)]) -> Self {
        self.lists.extend(rows.iter().map(|(code, name)| Security {
            code: code.to_string(),
            name: name.to_string(),
        }));
        self
    }

    fn with_daily(mut self, code: &str, bars: Vec<Bar>) -> Self {
        self.daily.insert(code.to_string(), bars);
        self
    }

    fn with_events(mut self, code: &str, events: Vec<CorporateAction>) -> Self {
        self.events.insert(code.to_string(), events);
        self
    }
}

impl QuoteSource for FixtureSource {
    fn security_list(&self) -> Result<Vec<Security>> {
        Ok(self.lists.clone())
    }

    /// 真实行情源只回 `beg` 之后的日线，样本也照做——忽略 `beg` 会让增量回补这条
    /// 路径测不出来。
    fn daily_raw(&self, code: &str, beg: &str) -> Result<Vec<Bar>> {
        self.requested_begs
            .lock()
            .expect("记录锁")
            .push((code.to_string(), beg.to_string()));
        let from = NaiveDate::parse_from_str(beg, "%Y%m%d")
            .map_err(|e| CoreError::InvalidData(format!("起始日期不是 YYYYMMDD: {e}")))?;
        let bars = self
            .daily
            .get(code)
            .cloned()
            .ok_or_else(|| CoreError::InvalidData(format!("没有 {code} 的日线样本")))?;
        Ok(bars
            .into_iter()
            .filter(|bar| {
                NaiveDate::parse_from_str(&bar.date, "%Y-%m-%d").is_ok_and(|date| date >= from)
            })
            .collect())
    }

    fn corporate_actions(&self, code: &str) -> Result<Vec<CorporateAction>> {
        Ok(self.events.get(code).cloned().unwrap_or_default())
    }

    fn minute_bars(&self, _code: &str, _period: u32) -> Result<Vec<MinuteBar>> {
        Ok(Vec::new())
    }
}

fn bar(code: &str, date: &str, close: f64) -> Bar {
    Bar {
        code: code.to_string(),
        date: date.to_string(),
        open: close,
        high: close,
        low: close,
        close,
        volume: 1_000_000.0,
        amount: close * 1_000_000.0,
    }
}

fn event(code: &str, ex_date: &str, dividend_per_share: f64) -> CorporateAction {
    CorporateAction {
        code: code.to_string(),
        ex_date: ex_date.to_string(),
        dividend_per_share,
        split_ratio: 0.0,
        pre_close: None,
        recorded_at: "2026-10-13T08:00:00Z".to_string(),
    }
}

fn open_db(dir: &tempfile::TempDir) -> Database {
    Database::open(&dir.path().join("springload.db")).expect("打开数据库")
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
fn filter_universe_keeps_only_main_board_without_st() {
    let rows = vec![
        Security { code: "600000".into(), name: "浦发银行".into() },
        Security { code: "000001".into(), name: "平安银行".into() },
        // ST 与 *ST 都排除。
        Security { code: "600001".into(), name: "ST 某股".into() },
        Security { code: "600002".into(), name: "*ST 某股".into() },
        // 科创板、创业板、北交所排除。
        Security { code: "688981".into(), name: "中芯国际".into() },
        Security { code: "300750".into(), name: "宁德时代".into() },
        Security { code: "920821".into(), name: "北交所股".into() },
        // 指数与 B 股排除。
        Security { code: "395001".into(), name: "主板Ａ股".into() },
        Security { code: "900948".into(), name: "伊泰Ｂ股".into() },
    ];
    let kept = backfill::filter_universe(&rows, Market::Cn);
    let codes: Vec<&str> = kept.iter().map(|info| info.code.as_str()).collect();
    assert_eq!(codes, vec!["600000", "000001"]);

    let first = &kept[0];
    assert_eq!(first.market, "SH");
    assert_eq!(first.board, "main");
    assert!(!first.is_st);
    // 上市日要等抓到日线才知道。
    assert_eq!(first.listed_at, None);
}

#[test]
fn fetch_universe_reads_the_list_of_the_current_market() {
    let source = FixtureSource::new()
        .with_list(&[("600000", "浦发银行"), ("000001", "平安银行")]);
    let universe = backfill::fetch_universe(&source, Market::Cn).expect("抓取股票池");
    let mut codes: Vec<&str> = universe.iter().map(|info| info.code.as_str()).collect();
    codes.sort();
    assert_eq!(codes, vec!["000001", "600000"]);
}

/// 美股的证券目录已经筛过一遍，进池时只补上交易所与板块。
#[test]
fn us_universe_is_tagged_as_us_common_stock() {
    let rows = vec![
        Security { code: "AAPL".into(), name: "Apple Inc. - Common Stock".into() },
        Security { code: "BRK.B".into(), name: "Berkshire Hathaway Inc. Class B Common Stock".into() },
    ];
    let kept = backfill::filter_universe(&rows, Market::Us);
    let codes: Vec<&str> = kept.iter().map(|info| info.code.as_str()).collect();
    assert_eq!(codes, vec!["AAPL", "BRK.B"]);
    assert_eq!(kept[0].market, "US");
    assert_eq!(kept[0].board, "common");
}

#[test]
fn sync_market_writes_stocks_bars_and_sync_state() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(&dir);
    let source = FixtureSource::new()
        .with_list(&[("600000", "浦发银行")])
        .with_daily(
            "600000",
            vec![
                bar("600000", "2023-10-16", 10.0),
                bar("600000", "2026-06-25", 20.0),
                bar("600000", "2026-06-26", 18.0),
                bar("600000", "2026-10-13", 19.0),
            ],
        )
        // 2026-06-26 除息 2 元：这天之前的收盘价要减 2。
        .with_events("600000", vec![event("600000", "2026-06-26", 2.0)]);
    let today = NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();
    let options = SyncOptions::with_today(true, today);

    let events: Arc<Mutex<Vec<SyncStatus>>> = Arc::new(Mutex::new(Vec::new()));
    let sink_events = Arc::clone(&events);
    let sink: backfill::ProgressSink = Arc::new(move |status| {
        sink_events.lock().expect("事件锁").push(status);
    });

    let report = backfill::sync_market(&db, &source, Market::Cn, &options, &sink).expect("同步成功");
    assert_eq!(report.stocks, 1);
    assert_eq!(report.bars, 4);
    assert!(report.errors.is_empty(), "{:?}", report.errors);

    let conn = db.conn().unwrap();
    assert_eq!(stock::count(&conn).unwrap(), 1);
    assert!(bar::count(&conn).unwrap() > 0);

    // 除权日之前的收盘价减掉每股分红，除权日当天与之后不动。
    let bars = bar::list_for_code(&conn, "600000").unwrap();
    assert_eq!(bars[0].date, "2023-10-16");
    assert_eq!(bars[0].close, 8.0);
    assert_eq!(bars[1].close, 18.0);
    assert_eq!(bars[2].close, 18.0);
    assert_eq!(bars[3].close, 19.0);

    // 除权除息事件写进了 `corporate_action`，除权日的前收盘价按参考价算出。
    let actions = corporate_action::list_for_code(&conn, "600000").unwrap();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].ex_date, "2026-06-26");
    assert_eq!(actions[0].pre_close, Some(18.0), "20 元减 2 元分红");

    let state = bar_sync_state::get(&conn, "600000").unwrap().expect("有回补状态");
    assert_eq!(state.last_bar_date.as_deref(), Some("2026-10-13"));

    // 回补范围是最近三年。
    let begs = source.requested_begs.lock().unwrap().clone();
    assert_eq!(begs, vec![("600000".to_string(), "20230913".to_string())]);

    // 进度事件最后一条是终止状态。
    let events = events.lock().unwrap();
    assert!(!events.is_empty());
    let last = events.last().unwrap();
    assert!(!last.running);
    assert_eq!(last.total, 1);
    assert_eq!(last.done, 1);
    assert!(last.last_sync_at.is_some());
}

/// 三年窗口没被填满时最早那根日线就是上市首日；填满了说明上市已超过三年。
#[test]
fn listed_at_comes_from_the_earliest_bar() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(&dir);
    let today = NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();
    let source = FixtureSource::new()
        .with_list(&[("600000", "老股"), ("600004", "次新股")])
        // 老股的三年窗口是填满的：行情源只回 `beg` 之后的日线，所以「填满」表现为
        // 最早一根就落在窗口起点（2026-10-13 往前三年再多取一个月）上。
        .with_daily(
            "600000",
            vec![bar("600000", "2023-09-13", 8.0), bar("600000", "2026-10-13", 12.0)],
        )
        .with_daily(
            "600004",
            vec![bar("600004", "2025-06-02", 30.0), bar("600004", "2026-10-13", 31.0)],
        );

    backfill::sync_market(
        &db,
        &source,
        Market::Cn,
        &SyncOptions::with_today(true, today),
        &backfill::noop_sink(),
    )
    .expect("同步成功");

    let conn = db.conn().unwrap();
    assert_eq!(stock::get(&conn, "600000").unwrap().unwrap().listed_at, None);
    assert_eq!(
        stock::get(&conn, "600004").unwrap().unwrap().listed_at.as_deref(),
        Some("2025-06-02")
    );
}

/// 最早一根日线晚于六个月分界日的股票不进股票池，也不算失败。
#[test]
fn newly_listed_stocks_are_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(&dir);
    let today = NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();
    let source = FixtureSource::new()
        .with_list(&[("600000", "老股"), ("600003", "新股")])
        .with_daily(
            "600000",
            vec![bar("600000", "2023-10-16", 10.0), bar("600000", "2026-10-13", 12.0)],
        )
        .with_daily(
            "600003",
            vec![bar("600003", "2026-06-01", 20.0), bar("600003", "2026-10-13", 21.0)],
        );

    let report = backfill::sync_market(
        &db,
        &source,
        Market::Cn,
        &SyncOptions::with_today(true, today),
        &backfill::noop_sink(),
    )
    .expect("同步成功");
    assert_eq!(report.stocks, 1);
    assert!(report.errors.is_empty(), "{:?}", report.errors);

    let conn = db.conn().unwrap();
    assert!(stock::get(&conn, "600000").unwrap().is_some());
    assert!(stock::get(&conn, "600003").unwrap().is_none(), "新股不进股票池");
    assert!(bar::list_for_code(&conn, "600003").unwrap().is_empty());
}

#[test]
fn incremental_sync_starts_from_last_bar_date() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(&dir);
    let source = FixtureSource::new()
        .with_list(&[("600000", "浦发银行")])
        .with_daily(
            "600000",
            vec![bar("600000", "2023-10-16", 10.0), bar("600000", "2026-10-13", 12.0)],
        );
    let today = NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();

    backfill::sync_market(
        &db,
        &source,
        Market::Cn,
        &SyncOptions::with_today(true, today),
        &backfill::noop_sink(),
    )
    .expect("首次同步");

    backfill::sync_market(
        &db,
        &source,
        Market::Cn,
        &SyncOptions::with_today(false, today),
        &backfill::noop_sink(),
    )
    .expect("增量同步");

    let begs = source.requested_begs.lock().unwrap().clone();
    assert_eq!(begs.len(), 2, "基准没变时不该多取一次");
    assert_eq!(begs[0].1, "20230913", "首次按三年回补，另往前多取一个月");
    // 样本最后一根是 2026-10-13，增量从它开始。
    assert_eq!(begs[1].1, "20261013");

    // 增量这一段确实写进去了，库里的历史仍然完整。
    let conn = db.conn().unwrap();
    let bars = bar::list_for_code(&conn, "600000").unwrap();
    assert_eq!(bars.len(), 2, "增量之后历史应仍是两根");
    assert_eq!(bars.last().unwrap().date, "2026-10-13");
    let state = bar_sync_state::get(&conn, "600000").unwrap().expect("有回补状态");
    assert_eq!(state.last_bar_date.as_deref(), Some("2026-10-13"));
}

#[test]
fn sync_market_keeps_going_when_one_stock_fails() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(&dir);
    // 只提供一只股票的日线样本，另一只取不到。
    let source = FixtureSource::new()
        .with_list(&[("600000", "浦发银行"), ("600001", "取不到日线")])
        .with_daily(
            "600000",
            vec![bar("600000", "2023-10-16", 10.0), bar("600000", "2026-10-13", 12.0)],
        );
    let today = NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();
    let report = backfill::sync_market(
        &db,
        &source,
        Market::Cn,
        &SyncOptions::with_today(true, today),
        &backfill::noop_sink(),
    )
    .expect("整体同步成功");
    assert_eq!(report.stocks, 1);
    assert_eq!(report.errors.len(), 1);
    assert!(report.errors[0].contains("600001"), "{:?}", report.errors);

    let conn = db.conn().unwrap();
    assert!(bar::count(&conn).unwrap() > 0);
    assert!(bar_sync_state::get(&conn, "600001").unwrap().is_none());
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

    let cn_close = Market::Cn.close_time();
    let us_close = Market::Us.close_time();
    assert!(!backfill::should_run_auto_sync(before_close, cn_close, today, None));
    assert!(backfill::should_run_auto_sync(at_close, cn_close, today, None));
    assert!(backfill::should_run_auto_sync(after_close, cn_close, today, None));
    // 今天已经跑过就不再跑。
    assert!(!backfill::should_run_auto_sync(after_close, cn_close, today, Some(today)));
    // 昨天跑过，今天照常跑。
    let yesterday = NaiveDate::from_ymd_opt(2026, 10, 12).unwrap();
    assert!(backfill::should_run_auto_sync(after_close, cn_close, today, Some(yesterday)));
    // 美股的收盘时间不同：15:00 还没收盘，16:30 已经收盘。
    assert!(!backfill::should_run_auto_sync(at_close, us_close, today, None));
    assert!(backfill::should_run_auto_sync(after_close, us_close, today, None));
}

/// 行情源把整段历史的价格换了个基准（拆股、或者改了复权口径）时，增量回补要能发现，
/// 并把整段历史按新基准重写，接缝处不留台阶。
#[test]
fn a_price_basis_change_rewrites_the_whole_history() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(&dir);
    let today = NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();

    // 第一次：价格在 10 元一线。
    let before = FixtureSource::new().with_list(&[("600000", "浦发银行")]).with_daily(
        "600000",
        vec![
            bar("600000", "2023-10-16", 10.0),
            bar("600000", "2026-06-25", 20.0),
            bar("600000", "2026-10-13", 19.0),
        ],
    );
    backfill::sync_market(
        &db,
        &before,
        Market::Cn,
        &SyncOptions::with_today(true, today),
        &backfill::noop_sink(),
    )
    .expect("首次同步");

    // 第二次：行情源把整段历史的价格都砍半，相当于发生了 2 拆 1。
    let after = FixtureSource::new().with_list(&[("600000", "浦发银行")]).with_daily(
        "600000",
        vec![
            bar("600000", "2023-10-16", 5.0),
            bar("600000", "2026-06-25", 10.0),
            bar("600000", "2026-10-13", 9.5),
        ],
    );
    backfill::sync_market(
        &db,
        &after,
        Market::Cn,
        &SyncOptions::with_today(false, today),
        &backfill::noop_sink(),
    )
    .expect("增量同步");

    // 第三次取数是整段重取，不是接着增量。
    let begs = after.requested_begs.lock().unwrap().clone();
    assert_eq!(begs.len(), 2);
    assert_eq!(begs[0].1, "20261013", "增量那一次从最后一条日线开始");
    assert_eq!(begs[1].1, "20230913", "发现基准变了之后重取整段");

    let conn = db.conn().unwrap();
    let bars = bar::list_for_code(&conn, "600000").unwrap();
    let series: Vec<(String, f64)> = bars
        .iter()
        .map(|row| (row.date.clone(), row.close))
        .collect();
    assert_eq!(
        series,
        vec![
            ("2023-10-16".to_string(), 5.0),
            ("2026-06-25".to_string(), 10.0),
            ("2026-10-13".to_string(), 9.5),
        ],
        "整段历史都换成新基准"
    );
}

/// 除权除息让历史基准变了时同样要整体重写。这是 A 股那条路：事件落在窗口内，
/// 重叠那一天的换算结果与库里对不上。
#[test]
fn a_new_corporate_action_rewrites_the_whole_history() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(&dir);
    let today = NaiveDate::from_ymd_opt(2026, 10, 14).unwrap();

    // 最早一根要早于六个月分界日，否则会被当成次新股剔除。
    let before = FixtureSource::new().with_list(&[("600000", "浦发银行")]).with_daily(
        "600000",
        vec![
            bar("600000", "2026-01-05", 20.0),
            bar("600000", "2026-06-25", 18.0),
            bar("600000", "2026-10-13", 19.0),
        ],
    );
    backfill::sync_market(
        &db,
        &before,
        Market::Cn,
        &SyncOptions::with_today(true, today),
        &backfill::noop_sink(),
    )
    .expect("首次同步");

    // 2026-10-14 除息 2 元，同时补上当天那根日线。
    let after = FixtureSource::new()
        .with_list(&[("600000", "浦发银行")])
        .with_daily(
            "600000",
            vec![
                bar("600000", "2026-01-05", 20.0),
                bar("600000", "2026-06-25", 18.0),
                bar("600000", "2026-10-13", 19.0),
                bar("600000", "2026-10-14", 19.5),
            ],
        )
        .with_events("600000", vec![event("600000", "2026-10-14", 2.0)]);
    backfill::sync_market(
        &db,
        &after,
        Market::Cn,
        &SyncOptions::with_today(false, today),
        &backfill::noop_sink(),
    )
    .expect("增量同步");

    let conn = db.conn().unwrap();
    let bars = bar::list_for_code(&conn, "600000").unwrap();
    let series: Vec<(String, f64)> = bars
        .iter()
        .map(|row| (row.date.clone(), row.close))
        .collect();
    assert_eq!(
        series,
        vec![
            ("2026-01-05".to_string(), 18.0),
            ("2026-06-25".to_string(), 16.0),
            ("2026-10-13".to_string(), 17.0),
            ("2026-10-14".to_string(), 19.5),
        ],
        "除息日之前的历史整体减掉每股分红"
    );
}

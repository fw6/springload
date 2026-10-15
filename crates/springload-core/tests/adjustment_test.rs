//! 前复权换算、过期判定与重算的集成测试。
//!
//! 场景取 001309 德明利 2026-10-13 的除权除息，方案 `10转4股派15.00元`：每股现金
//! 红利 1.5 元、每股送转 0.4 股，除权日前收盘 321.43，除权日收盘 233.78。存储用
//! 临时 SQLite 文件，行情用样本数据源回放。

use chrono::NaiveDate;
use springload_core::adjustment::{
    self, adjust_bar, adjust_bars, record_events, reference_price, stale_by_events,
};
use springload_core::error::Result;
use springload_core::market::{Bar, MinuteBar, QuoteSource};
use springload_core::storage::bar::BarRow;
use springload_core::storage::bar_sync_state::BarSyncState;
use springload_core::storage::corporate_action::{self, CorporateAction};
use springload_core::storage::stock::Stock;
use springload_core::storage::{bar, bar_sync_state, stock, Database};

/// 除权日。
const EX_DATE: &str = "2026-10-13";
/// 除权日前的最后一根不复权日线。
const PRE_EX_DATE: &str = "2026-10-12";
/// 除权日前收盘价（不复权）。
const PRE_EX_CLOSE: f64 = 321.43;
/// 除权日收盘价（不复权），除权后价格不再换算。
const EX_CLOSE: f64 = 233.78;
/// 每股现金红利。
const DIVIDEND: f64 = 1.5;
/// 每股送转比例。
const SPLIT: f64 = 0.4;

fn raw_bar(code: &str, date: &str, close: f64) -> Bar {
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

/// 复权前的一小段历史：除权日前一根与除权日当天。
fn raw_history() -> Vec<Bar> {
    vec![
        raw_bar("001309", PRE_EX_DATE, PRE_EX_CLOSE),
        raw_bar("001309", EX_DATE, EX_CLOSE),
    ]
}

fn ex_event() -> CorporateAction {
    CorporateAction {
        code: "001309".to_string(),
        ex_date: EX_DATE.to_string(),
        dividend_per_share: DIVIDEND,
        split_ratio: SPLIT,
        pre_close: None,
        recorded_at: "2026-10-13T00:00:00Z".to_string(),
    }
}

fn rows(bars: &[Bar]) -> Vec<BarRow> {
    bars.iter()
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
        .collect()
}

/// 用样本回放的行情数据源。
struct FakeSource {
    raw: Vec<Bar>,
    events: Vec<CorporateAction>,
}

impl QuoteSource for FakeSource {
    fn security_list(&self) -> Result<Vec<springload_core::market::Security>> {
        Ok(Vec::new())
    }

    fn daily_raw(&self, _code: &str, _beg: &str) -> Result<Vec<Bar>> {
        Ok(self.raw.clone())
    }

    fn corporate_actions(&self, _code: &str) -> Result<Vec<CorporateAction>> {
        Ok(self.events.clone())
    }

    fn minute_bars(&self, _code: &str, _period: u32) -> Result<Vec<MinuteBar>> {
        Ok(Vec::new())
    }
}

fn open_db(dir: &std::path::Path) -> Database {
    Database::open(&dir.join("springload.db")).expect("打开数据库")
}

fn seed_stock(db: &Database, code: &str, name: &str) {
    let conn = db.conn().unwrap();
    stock::upsert(
        &conn,
        &Stock {
            code: code.to_string(),
            name: name.to_string(),
            market: "SZ".to_string(),
            board: "main".to_string(),
            sector: None,
            listed_at: Some("2020-01-01".to_string()),
            is_st: false,
            updated_at: "2026-10-13T00:00:00Z".to_string(),
        },
    )
    .unwrap();
}

fn seed_state(db: &Database, code: &str, last_bar_date: &str) {
    let conn = db.conn().unwrap();
    bar_sync_state::upsert(
        &conn,
        &BarSyncState {
            code: code.to_string(),
            last_bar_date: Some(last_bar_date.to_string()),
            fetched_at: Some("2026-10-12T08:00:00Z".to_string()),
            updated_at: "2026-10-12T08:00:00Z".to_string(),
        },
    )
    .unwrap();
}

#[test]
fn reference_price_matches_exchange_formula() {
    // 001309：前收盘 321.43，D = 1.50，r = 0.40 → 228.52。
    let price = reference_price(321.43, 1.5, 0.4);
    assert!((price - 228.521428571).abs() < 1e-6, "{price}");
}

#[test]
fn adjust_bar_rewrites_history_before_the_ex_date_only() {
    let events = vec![ex_event()];
    let before = adjust_bar(&rows(&raw_history())[0], &events);
    // (321.43 - 1.50) / 1.40 = 228.5214…，保留两位小数。
    assert_eq!(before.close, 228.52);
    assert_eq!(before.open, 228.52);

    let on_ex_date = adjust_bar(&rows(&raw_history())[1], &events);
    assert_eq!(on_ex_date.close, EX_CLOSE, "除权日当天不换算");
}

#[test]
fn adjust_bars_keeps_volume_and_amount() {
    let adjusted = adjust_bars(&raw_history(), &[ex_event()]);
    assert_eq!(adjusted.len(), 2);
    assert_eq!(adjusted[0].volume, 1_000_000.0);
    assert_eq!(adjusted[0].amount, PRE_EX_CLOSE * 1_000_000.0);
    assert_eq!(adjusted[1].close, EX_CLOSE);
}

#[test]
fn stale_by_events_compares_the_event_sets() {
    let stored = vec![ex_event()];
    // 库里没有这个事件：新事件会改写它之前的历史。
    assert!(stale_by_events(&stored, &[]));
    // 库里已经有同一个事件。
    assert!(!stale_by_events(&stored, &stored));

    // 同一天的分红改了：历史要按新的数重算。
    let mut changed = ex_event();
    changed.dividend_per_share = 2.0;
    assert!(stale_by_events(&[changed], &stored));

    // 库里多出来的事件不判过期，否则每次检查都要重算。
    assert!(!stale_by_events(&[], &stored));
}

#[test]
fn record_events_computes_the_ex_reference_price() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(dir.path());
    let conn = db.conn().unwrap();
    record_events(&conn, "001309", &[ex_event()], &raw_history()).unwrap();

    let list = corporate_action::list_for_code(&conn, "001309").unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].dividend_per_share, DIVIDEND);
    assert_eq!(list[0].split_ratio, SPLIT);
    let pre_close = list[0].pre_close.expect("应记录除权参考价");
    assert!((pre_close - 228.521428571).abs() < 1e-6, "{pre_close}");
}

/// 事件早于不复权日线的覆盖范围时，保留表里已有的前收盘价。
#[test]
fn record_events_keeps_the_stored_reference_price_for_older_events() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(dir.path());
    let conn = db.conn().unwrap();
    let mut old = ex_event();
    old.ex_date = "2020-06-01".to_string();
    record_events(&conn, "001309", std::slice::from_ref(&old), &raw_history()).unwrap();

    let mut refreshed = old.clone();
    refreshed.pre_close = Some(999.0);
    corporate_action::upsert(&conn, &refreshed).unwrap();
    // 再写一次：日线覆盖不到 2020 年，保留表里已有的值。
    record_events(&conn, "001309", std::slice::from_ref(&old), &raw_history()).unwrap();

    let list = corporate_action::list_for_code(&conn, "001309").unwrap();
    assert_eq!(list[0].pre_close, Some(999.0));
}

#[test]
fn corporate_action_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(dir.path());
    let conn = db.conn().unwrap();
    corporate_action::upsert(
        &conn,
        &CorporateAction {
            code: "001309".to_string(),
            ex_date: EX_DATE.to_string(),
            dividend_per_share: DIVIDEND,
            split_ratio: SPLIT,
            pre_close: Some(228.52),
            recorded_at: "2026-10-13T00:00:00Z".to_string(),
        },
    )
    .unwrap();
    let list = corporate_action::list_for_code(&conn, "001309").unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].dividend_per_share, DIVIDEND);
    assert_eq!(list[0].split_ratio, SPLIT);
    assert_eq!(list[0].pre_close, Some(228.52));
    assert_eq!(
        corporate_action::latest_ex_date(&conn, "001309").unwrap(),
        Some(EX_DATE.to_string())
    );
    assert_eq!(corporate_action::count(&conn).unwrap(), 1);
}

#[test]
fn detect_stale_flags_stock_whose_event_is_missing_from_the_store() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(dir.path());
    seed_stock(&db, "001309", "德明利");
    seed_state(&db, "001309", PRE_EX_DATE);

    let source = FakeSource {
        raw: raw_history(),
        events: vec![ex_event()],
    };
    let stale = adjustment::detect_stale(&db, &source, 2).expect("检查过期");

    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].code, "001309");
    assert_eq!(stale[0].name, "德明利");
    assert_eq!(stale[0].ex_date.as_deref(), Some(EX_DATE));
}

#[test]
fn detect_stale_ignores_stock_whose_events_are_already_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(dir.path());
    seed_stock(&db, "001309", "德明利");
    seed_state(&db, "001309", EX_DATE);
    {
        let conn = db.conn().unwrap();
        record_events(&conn, "001309", &[ex_event()], &raw_history()).unwrap();
    }

    let source = FakeSource {
        raw: raw_history(),
        events: vec![ex_event()],
    };
    let stale = adjustment::detect_stale(&db, &source, 2).expect("检查过期");
    assert!(stale.is_empty(), "事件已经记过，不应判过期");
}

/// 没有回补状态的股票不参与检查。
#[test]
fn detect_stale_skips_stocks_without_sync_state() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(dir.path());
    seed_stock(&db, "001309", "德明利");

    let source = FakeSource {
        raw: raw_history(),
        events: vec![ex_event()],
    };
    assert!(adjustment::detect_stale(&db, &source, 2).unwrap().is_empty());
}

#[test]
fn resync_rewrites_history_and_records_events() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(dir.path());
    seed_stock(&db, "001309", "德明利");
    let raw_bars = raw_history();
    {
        // 存的是除权前抓的前复权历史，等于当时的不复权序列。
        let mut conn = db.conn().unwrap();
        bar::upsert_many(&mut conn, &rows(&raw_bars)).unwrap();
    }
    seed_state(&db, "001309", PRE_EX_DATE);

    let source = FakeSource {
        raw: raw_bars.clone(),
        events: vec![ex_event()],
    };
    let report = adjustment::resync(
        &db,
        &source,
        &["001309".to_string()],
        NaiveDate::from_ymd_opt(2026, 10, 13).unwrap(),
    )
    .expect("重算");
    assert_eq!(report.stocks, 1);
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert_eq!(report.bars as usize, raw_bars.len());

    // 同一段历史被整体改写：除权日之前换算成前复权，除权日当天不变。
    let conn = db.conn().unwrap();
    let stored = bar::list_for_code(&conn, "001309").unwrap();
    assert_eq!(stored.len(), 2);
    assert_eq!(stored[0].date, PRE_EX_DATE);
    assert_eq!(stored[0].close, 228.52);
    assert_eq!(stored[1].date, EX_DATE);
    assert_eq!(stored[1].close, EX_CLOSE);

    // 除权除息事件写进表，除权日的前收盘价是参考价 (321.43 - 1.50) / 1.40 = 228.52。
    let actions = corporate_action::list_for_code(&conn, "001309").unwrap();
    let latest = actions
        .iter()
        .find(|action| action.ex_date == EX_DATE)
        .expect("应记录除权日事件");
    assert_eq!(latest.dividend_per_share, DIVIDEND);
    assert_eq!(latest.split_ratio, SPLIT);
    let pre_close = latest.pre_close.expect("应记录除权参考价");
    assert!((pre_close - 228.521428571).abs() < 1e-6, "{pre_close}");

    // 回补状态被刷新到最新交易日。
    let state = bar_sync_state::get(&conn, "001309").unwrap().unwrap();
    assert_eq!(state.last_bar_date.as_deref(), Some(EX_DATE));
}

#[test]
fn resync_after_rewrite_leaves_nothing_stale() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(dir.path());
    seed_stock(&db, "001309", "德明利");
    {
        let mut conn = db.conn().unwrap();
        bar::upsert_many(&mut conn, &rows(&raw_history())).unwrap();
    }
    seed_state(&db, "001309", PRE_EX_DATE);

    let source = FakeSource {
        raw: raw_history(),
        events: vec![ex_event()],
    };
    let today = NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();
    assert_eq!(adjustment::detect_stale(&db, &source, 2).unwrap().len(), 1);

    adjustment::resync(&db, &source, &["001309".to_string()], today).unwrap();

    assert!(adjustment::detect_stale(&db, &source, 2).unwrap().is_empty());
}

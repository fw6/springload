//! 前复权换算、过期判定与重算的集成测试。
//!
//! 行情走 `tests/fixtures/` 下录制好的 001309 德明利响应样本（2026-10-13 除权除息，
//! 方案 `10转4股派15.00元`），存储用临时 SQLite 文件。

use springload_core::adjustment::{
    self, adjust_bar, parse_dividends, parse_tail_qfq, reference_price, stale_by_event,
    tail_overlap_changed, AdjustmentFetch, DividendRecord, HttpAdjustmentFetch,
};
use springload_core::error::{CoreError, Result};
use springload_core::market::{eastmoney, Bar};
use springload_core::storage::bar::BarRow;
use springload_core::storage::bar_sync_state::BarSyncState;
use springload_core::storage::corporate_action::{self, CorporateAction};
use springload_core::storage::stock::Stock;
use springload_core::storage::{bar, bar_sync_state, stock, Database};
use chrono::NaiveDate;

const RAW: &str = "eastmoney_kline_001309_raw.json";
const QFQ: &str = "eastmoney_kline_001309_qfq.json";
const TAIL: &str = "eastmoney_kline_001309_tail_qfq.json";
const TAIL_BEFORE_EX: &str = "eastmoney_kline_001309_tail_before_ex.json";
const BONUS: &str = "eastmoney_sharebonus_001309.json";

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("读取样本 {path}: {error}"))
}

fn bars(name: &str) -> Vec<Bar> {
    eastmoney::parse_kline(&fixture(name))
        .unwrap_or_else(|error| panic!("解析样本 {name}: {error}"))
        .bars
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

/// 从分红送配样本构造事件，与生产路径一致。
fn events_from_bonus() -> Vec<CorporateAction> {
    parse_dividends(&fixture(BONUS))
        .expect("解析分红送配样本")
        .into_iter()
        .map(|record| CorporateAction {
            code: "001309".to_string(),
            ex_date: record.ex_date,
            dividend_per_share: record.dividend_per_share,
            split_ratio: record.split_ratio,
            pre_close: None,
            recorded_at: "2026-10-13T00:00:00Z".to_string(),
        })
        .collect()
}

/// 用录制样本回放的抓取实现。
struct FakeFetch {
    tail: String,
    raw: String,
    dividends: String,
    dividends_fail: bool,
}

impl FakeFetch {
    fn new() -> Self {
        Self {
            tail: fixture(TAIL),
            raw: fixture(RAW),
            dividends: fixture(BONUS),
            dividends_fail: false,
        }
    }
}

impl AdjustmentFetch for FakeFetch {
    fn qfq_tail(&self, _code: &str, _lmt: u32) -> Result<String> {
        Ok(self.tail.clone())
    }

    fn raw_history(&self, _code: &str, _beg: &str) -> Result<String> {
        Ok(self.raw.clone())
    }

    fn dividends(&self, _code: &str) -> Result<String> {
        if self.dividends_fail {
            return Err(CoreError::InvalidData("分红送配接口不可用".to_string()));
        }
        Ok(self.dividends.clone())
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

fn seed_state(db: &Database, code: &str, last_bar_date: &str, tail_qfq: &str) {
    let conn = db.conn().unwrap();
    bar_sync_state::upsert(
        &conn,
        &BarSyncState {
            code: code.to_string(),
            last_bar_date: Some(last_bar_date.to_string()),
            fetched_at: Some("2026-10-12T08:00:00Z".to_string()),
            dktotal: Some(1037),
            tail_qfq: Some(tail_qfq.to_string()),
            updated_at: "2026-10-12T08:00:00Z".to_string(),
        },
    )
    .unwrap();
}

#[test]
fn parse_dividends_reads_per_share_amounts() {
    let records = parse_dividends(&fixture(BONUS)).expect("解析分红送配样本");
    assert_eq!(records.len(), 5, "样本里有五条已实施记录");
    assert_eq!(
        records.last().unwrap(),
        &DividendRecord {
            ex_date: "2026-10-13".to_string(),
            dividend_per_share: 1.5,
            split_ratio: 0.4,
        }
    );
    assert_eq!(
        records[3],
        DividendRecord {
            ex_date: "2026-04-01".to_string(),
            dividend_per_share: 0.4,
            split_ratio: 0.0,
        }
    );
}

#[test]
fn parse_dividends_treats_null_result_as_empty() {
    let body = r#"{"version":null,"result":null,"success":false,"message":"返回数据为空","code":9201}"#;
    assert!(parse_dividends(body).unwrap().is_empty());
}

#[test]
fn reference_price_matches_exchange_formula() {
    // 001309：前收盘 321.43，D = 1.50，r = 0.40 → 228.52。
    let price = reference_price(321.43, 1.5, 0.4);
    assert!((price - 228.521428571).abs() < 1e-6, "{price}");
}

#[test]
fn adjust_bar_reproduces_server_qfq_series() {
    let events = events_from_bonus();
    let raw = bars(RAW);
    let expected = bars(QFQ);
    assert_eq!(raw.len(), expected.len());

    let mut changed = 0;
    for (raw_bar, expected_bar) in raw.iter().zip(expected.iter()) {
        assert_eq!(raw_bar.date, expected_bar.date);
        let adjusted = adjust_bar(&rows(std::slice::from_ref(raw_bar))[0], &events);
        // 前复权序列保留两位小数，逐条与行情源一致。
        assert!(
            (adjusted.open - expected_bar.open).abs() < 0.005,
            "{} open {} != {}",
            raw_bar.date,
            adjusted.open,
            expected_bar.open
        );
        assert!(
            (adjusted.close - expected_bar.close).abs() < 0.005,
            "{} close {} != {}",
            raw_bar.date,
            adjusted.close,
            expected_bar.close
        );
        if (adjusted.close - raw_bar.close).abs() > 1e-9 {
            changed += 1;
        }
    }
    // 除权日 2026-10-13 之前的历史全部被改写，除权日当天不变。
    assert_eq!(changed, raw.len() - 1);
    let last = adjust_bar(&rows(&raw[raw.len() - 1..])[0], &events);
    assert_eq!(last.close, raw[raw.len() - 1].close, "除权日当天不换算");
}

#[test]
fn stale_by_event_compares_latest_ex_with_local_last_bar() {
    assert!(stale_by_event(Some("2026-10-13"), Some("2026-10-12")));
    assert!(!stale_by_event(Some("2026-10-13"), Some("2026-10-13")));
    assert!(!stale_by_event(Some("2026-04-01"), Some("2026-10-12")));
    assert!(!stale_by_event(None, Some("2026-10-12")));
    assert!(!stale_by_event(Some("2026-10-13"), None));
}

#[test]
fn tail_overlap_changed_detects_rewritten_history() {
    let server = vec![
        ("2026-10-12".to_string(), 228.52),
        ("2026-10-13".to_string(), 233.78),
    ];
    let same = server.clone();
    assert!(!tail_overlap_changed(&server, &same));

    // 本地保存的是除权前的不复权价，同一日期数值不同。
    let before = vec![
        ("2026-10-12".to_string(), 321.43),
        ("2026-10-13".to_string(), 233.78),
    ];
    assert!(tail_overlap_changed(&server, &before));

    // 没有重叠日期时不判过期。
    let disjoint = vec![("2026-09-30".to_string(), 254.97)];
    assert!(!tail_overlap_changed(&server, &disjoint));
    assert!(!tail_overlap_changed(&server, &[]));
}

#[test]
fn parse_tail_qfq_reads_pairs() {
    let tail = parse_tail_qfq(r#"[["2026-10-12",228.52],["2026-10-13",233.78]]"#);
    assert_eq!(
        tail,
        vec![
            ("2026-10-12".to_string(), 228.52),
            ("2026-10-13".to_string(), 233.78),
        ]
    );
    assert!(parse_tail_qfq("[]").is_empty());
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
            ex_date: "2026-10-13".to_string(),
            dividend_per_share: 1.5,
            split_ratio: 0.4,
            pre_close: Some(228.52),
            recorded_at: "2026-10-13T00:00:00Z".to_string(),
        },
    )
    .unwrap();
    let list = corporate_action::list_for_code(&conn, "001309").unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].dividend_per_share, 1.5);
    assert_eq!(list[0].split_ratio, 0.4);
    assert_eq!(list[0].pre_close, Some(228.52));
    assert_eq!(
        corporate_action::latest_ex_date(&conn, "001309").unwrap(),
        Some("2026-10-13".to_string())
    );
    assert_eq!(corporate_action::count(&conn).unwrap(), 1);
}

#[test]
fn detect_stale_flags_stock_whose_history_precedes_the_ex_date() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(dir.path());
    seed_stock(&db, "001309", "德明利");
    // 本地最后一条是除权日的前一天，尾部保存的是除权前的不复权价。
    let raw_tail = tail_json(&bars(RAW));
    seed_state(&db, "001309", "2026-10-12", &raw_tail);

    let fetch = FakeFetch::new();
    let stale = adjustment::detect_stale(&db, &fetch, 2).expect("检查过期");

    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].code, "001309");
    assert_eq!(stale[0].name, "德明利");
    assert_eq!(stale[0].ex_date.as_deref(), Some("2026-10-13"));
}

#[test]
fn detect_stale_ignores_stock_already_synced_past_the_ex_date() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(dir.path());
    seed_stock(&db, "001309", "德明利");
    // 本地最后一条就是除权日，尾部保存的正是服务端当前的前复权价。
    let server_tail = tail_json(&bars(TAIL));
    seed_state(&db, "001309", "2026-10-13", &server_tail);

    let fetch = FakeFetch::new();
    let stale = adjustment::detect_stale(&db, &fetch, 2).expect("检查过期");
    assert!(stale.is_empty(), "已经是除权日之后的序列，不应判过期");
}

#[test]
fn detect_stale_falls_back_to_tail_comparison_without_dividends() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(dir.path());
    seed_stock(&db, "001309", "德明利");
    let raw_tail = tail_json(&bars(RAW));
    seed_state(&db, "001309", "2026-10-12", &raw_tail);

    let fetch = FakeFetch {
        dividends_fail: true,
        ..FakeFetch::new()
    };
    let stale = adjustment::detect_stale(&db, &fetch, 2).expect("检查过期");

    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].code, "001309");
    assert_eq!(stale[0].ex_date, None, "退回判据 B 时没有除权日信息");
}

#[test]
fn detect_stale_ignores_future_ex_date_before_it_takes_effect() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(dir.path());
    seed_stock(&db, "001309", "德明利");
    // 服务端尾部止于除权日前一天，除权日 2026-10-13 还是未来，本地历史仍是当时的前复权价。
    let server_tail = tail_json(&bars(TAIL_BEFORE_EX));
    seed_state(&db, "001309", "2026-10-12", &server_tail);

    let fetch = FakeFetch {
        tail: fixture(TAIL_BEFORE_EX),
        ..FakeFetch::new()
    };
    let stale = adjustment::detect_stale(&db, &fetch, 2).expect("检查过期");
    assert!(stale.is_empty(), "除权日尚未到来，历史未被改写");
}

#[test]
fn resync_rewrites_history_and_records_events() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(dir.path());
    seed_stock(&db, "001309", "德明利");
    let raw_bars = bars(RAW);
    {
        // 存的是除权前抓的前复权历史，等于当时的不复权序列。
        let mut conn = db.conn().unwrap();
        bar::upsert_many(&mut conn, &rows(&raw_bars)).unwrap();
    }
    seed_state(&db, "001309", "2026-10-12", &tail_json(&raw_bars));

    let fetch = FakeFetch::new();
    let report = adjustment::resync(
        &db,
        &fetch,
        &["001309".to_string()],
        NaiveDate::from_ymd_opt(2026, 10, 13).unwrap(),
    )
    .expect("重算");
    assert_eq!(report.stocks, 1);
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert_eq!(report.bars as usize, raw_bars.len());

    // 同一段历史被整体改写：除权日之前对齐行情源的前复权序列，除权日当天不变。
    let stored = bar::list_for_code(&db.conn().unwrap(), "001309").unwrap();
    let expected = bars(QFQ);
    assert_eq!(stored.len(), expected.len());
    for (row, want) in stored.iter().zip(expected.iter()) {
        assert_eq!(row.date, want.date);
        assert!(
            (row.close - want.close).abs() < 0.005,
            "{} close {} != {}",
            row.date,
            row.close,
            want.close
        );
    }
    assert_eq!(stored[stored.len() - 1].close, 233.78);

    // 除权除息事件写进表，除权日的前收盘价是参考价 (321.43 - 1.50) / 1.40 = 228.52。
    let conn = db.conn().unwrap();
    let actions = corporate_action::list_for_code(&conn, "001309").unwrap();
    let latest = actions
        .iter()
        .find(|action| action.ex_date == "2026-10-13")
        .expect("应记录 2026-10-13 事件");
    assert_eq!(latest.dividend_per_share, 1.5);
    assert_eq!(latest.split_ratio, 0.4);
    let pre_close = latest.pre_close.expect("应记录除权参考价");
    assert!((pre_close - 228.521428571).abs() < 1e-6, "{pre_close}");

    // 回补状态被刷新到服务端最新交易日。
    let state = bar_sync_state::get(&conn, "001309").unwrap().unwrap();
    assert_eq!(state.last_bar_date.as_deref(), Some("2026-10-13"));
    assert_eq!(state.dktotal, Some(1037));
    let tail = parse_tail_qfq(state.tail_qfq.as_deref().unwrap());
    assert_eq!(tail.len(), 10);
    assert_eq!(tail.last().unwrap().1, 233.78);
}

#[test]
fn resync_after_rewrite_leaves_nothing_stale() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_db(dir.path());
    seed_stock(&db, "001309", "德明利");
    {
        let mut conn = db.conn().unwrap();
        bar::upsert_many(&mut conn, &rows(&bars(RAW))).unwrap();
    }
    seed_state(&db, "001309", "2026-10-12", &tail_json(&bars(RAW)));

    let fetch = FakeFetch::new();
    assert_eq!(adjustment::detect_stale(&db, &fetch, 2).unwrap().len(), 1);

    adjustment::resync(
        &db,
        &fetch,
        &["001309".to_string()],
        NaiveDate::from_ymd_opt(2026, 10, 13).unwrap(),
    )
    .unwrap();

    // 重算后本地尾部等于服务端尾部，判据 A、B 都不再判过期。
    assert!(adjustment::detect_stale(&db, &fetch, 2).unwrap().is_empty());
}

#[test]
fn dividend_url_encodes_filter_quotes() {
    let url = adjustment::dividend_url("001309");
    assert!(url.contains("filter=(SECURITY_CODE%3D%22001309%22)"), "{url}");
    assert!(url.contains("reportName=RPT_SHAREBONUS_DET"));
}

/// 联网实测：确认尾部、不复权日线与分红送配三个请求都能通。
///
/// 运行：`cargo test -p springload-core --test adjustment_test -- --ignored --test-threads=1`
#[test]
#[ignore = "联网实测，需要公网"]
fn live_tail_raw_and_dividend_endpoints_are_reachable() {
    let fetch = HttpAdjustmentFetch::new().expect("建抓取客户端");

    let tail = eastmoney::parse_kline(&fetch.qfq_tail("001309", 10).expect("取前复权尾部"))
        .expect("解析前复权尾部");
    assert_eq!(tail.code, "001309");
    assert!(!tail.bars.is_empty());

    let raw = eastmoney::parse_kline(&fetch.raw_history("001309", "20260320").expect("取不复权日线"))
        .expect("解析不复权日线");
    assert!(!raw.bars.is_empty());

    let records = parse_dividends(&fetch.dividends("001309").expect("取分红送配")).expect("解析分红送配");
    assert!(
        records.iter().any(|record| record.ex_date == "2026-10-13"),
        "001309 应有 2026-10-13 的除权除息记录"
    );
}

/// 与 `bar_sync_state.tail_qfq` 一致的尾部 JSON。
fn tail_json(bars: &[Bar]) -> String {
    let start = bars.len().saturating_sub(adjustment::TAIL_LEN as usize);
    let tail: Vec<serde_json::Value> = bars[start..]
        .iter()
        .map(|bar| serde_json::json!([bar.date, bar.close]))
        .collect();
    serde_json::Value::Array(tail).to_string()
}

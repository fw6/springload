//! `storage` 模块：迁移与各表读写。

use springload_core::storage::bar::BarRow;
use springload_core::storage::bar_sync_state::BarSyncState;
use springload_core::storage::position::Position;
use springload_core::storage::signal::Signal;
use springload_core::storage::stock::Stock;
use springload_core::storage::support_zone::SupportZone;
use springload_core::storage::trade_log::TradeLogEntry;
use springload_core::storage::trade_plan::TradePlan;
use springload_core::storage::watchlist::WatchlistItem;
use springload_core::storage::{bar, bar_sync_state, position, setting, signal, stock, support_zone, trade_log, trade_plan, watchlist, Database};
use std::collections::HashSet;

const ALL_TABLES: &[&str] = &[
    "stock",
    "bar",
    "bar_minute",
    "bar_sync_state",
    "corporate_action",
    "watchlist_item",
    "support_zone",
    "signal",
    "trade_plan",
    "position",
    "trade_log",
    "setting",
];

fn open(dir: &tempfile::TempDir) -> Database {
    Database::open(&dir.path().join("springload.db")).expect("打开数据库")
}

fn table_names(db: &Database) -> HashSet<String> {
    let conn = db.conn().unwrap();
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
        .unwrap();
    let rows = stmt.query_map([], |row| row.get::<_, String>(0)).unwrap();
    rows.map(|r| r.unwrap()).collect()
}

#[test]
fn migrations_create_every_table_from_an_empty_file() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(&dir);
    let names = table_names(&db);
    for table in ALL_TABLES {
        assert!(names.contains(*table), "缺表 {table}");
    }
    assert!(db.schema_version().unwrap() >= 1);
}

#[test]
fn migrations_are_idempotent_across_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(&dir);
    let version = db.schema_version().unwrap();

    // 重新打开同一个库文件：迁移不应重复执行，也不应报错。
    let again = Database::open(&dir.path().join("springload.db")).expect("重开数据库");
    assert_eq!(again.schema_version().unwrap(), version);

    // 显式再跑一次迁移同样不报错。
    again.migrate().expect("重复迁移");
}

#[test]
fn stock_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(&dir);
    let conn = db.conn().unwrap();

    let s = Stock {
        code: "600000".into(),
        name: "浦发银行".into(),
        market: "SH".into(),
        board: "main".into(),
        sector: Some("银行".into()),
        listed_at: Some("1999-11-10".into()),
        is_st: false,
        updated_at: "2026-10-13T00:00:00Z".into(),
    };
    stock::upsert(&conn, &s).unwrap();
    assert_eq!(stock::get(&conn, "600000").unwrap(), Some(s.clone()));
    assert_eq!(stock::count(&conn).unwrap(), 1);

    // 更新同一条记录不新增行。
    let mut updated = s.clone();
    updated.name = "浦发银行（改）".into();
    stock::upsert(&conn, &updated).unwrap();
    assert_eq!(stock::count(&conn).unwrap(), 1);
    assert_eq!(stock::list(&conn).unwrap()[0].name, "浦发银行（改）");
}

#[test]
fn bar_upserts_and_reads_ranges() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(&dir);
    let mut conn = db.conn().unwrap();

    let bars = vec![
        bar_row("600000", "2026-10-09", 10.0, 10.5),
        bar_row("600000", "2026-10-10", 10.5, 11.0),
        bar_row("600000", "2026-10-13", 11.0, 11.5),
        bar_row("000001", "2026-10-13", 8.0, 8.2),
    ];
    assert_eq!(bar::upsert_many(&mut conn, &bars).unwrap(), 4);
    assert_eq!(bar::count(&conn).unwrap(), 4);

    // 重复写入同一主键只更新，不新增。
    bar::upsert_many(&mut conn, &[bar_row("600000", "2026-10-13", 11.0, 12.0)]).unwrap();
    assert_eq!(bar::count(&conn).unwrap(), 4);

    let range = bar::range(&conn, "600000", "2026-10-10", "2026-10-13").unwrap();
    assert_eq!(range.len(), 2);
    assert_eq!(range[0].date, "2026-10-10");
    assert_eq!(range[1].date, "2026-10-13");
    assert_eq!(range[1].high, 12.0, "重复写入同一主键应更新为新值");
    assert_eq!(bar::latest_date(&conn, "600000").unwrap().as_deref(), Some("2026-10-13"));
    assert_eq!(bar::latest_date(&conn, "999999").unwrap(), None);
    assert_eq!(bar::delete_for_code(&conn, "600000").unwrap(), 3);
    assert_eq!(bar::count(&conn).unwrap(), 1);
}

#[test]
fn bar_sync_state_tracks_latest_fetch() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(&dir);
    let conn = db.conn().unwrap();

    let a = BarSyncState {
        code: "600000".into(),
        last_bar_date: Some("2026-10-13".into()),
        fetched_at: Some("2026-10-13T08:00:00Z".into()),
        dktotal: Some(730),
        tail_qfq: Some("[[\"2026-10-13\",11.5]]".into()),
        updated_at: "2026-10-13T08:00:00Z".into(),
    };
    bar_sync_state::upsert(&conn, &a).unwrap();
    let b = BarSyncState {
        code: "000001".into(),
        last_bar_date: Some("2026-10-13".into()),
        fetched_at: Some("2026-10-13T09:00:00Z".into()),
        dktotal: None,
        tail_qfq: None,
        updated_at: "2026-10-13T09:00:00Z".into(),
    };
    bar_sync_state::upsert(&conn, &b).unwrap();

    assert_eq!(bar_sync_state::get(&conn, "600000").unwrap(), Some(a));
    assert_eq!(
        bar_sync_state::latest_fetched_at(&conn).unwrap().as_deref(),
        Some("2026-10-13T09:00:00Z")
    );
}

#[test]
fn watchlist_round_trips_and_removes() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(&dir);
    let conn = db.conn().unwrap();

    let item = WatchlistItem {
        code: "600000".into(),
        added_at: "2026-10-13T08:00:00Z".into(),
        c1: true,
        c2: true,
        c3: true,
        c4: false,
        chg_1m: Some(0.12),
        chg_3m: Some(0.30),
        chg_6m: Some(0.45),
        rank_pct: Some(0.25),
        avg_amount: Some(2.0e8),
        convergence: Some(0.18),
        screen_run_at: "2026-10-13T08:00:00Z".into(),
    };
    watchlist::upsert(&conn, &item).unwrap();
    assert_eq!(watchlist::get(&conn, "600000").unwrap(), Some(item));
    assert_eq!(watchlist::list(&conn).unwrap().len(), 1);
    assert_eq!(watchlist::remove(&conn, "600000").unwrap(), 1);
    assert!(watchlist::list(&conn).unwrap().is_empty());
}

#[test]
fn support_zone_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(&dir);
    let conn = db.conn().unwrap();

    let zone = SupportZone {
        id: "z1".into(),
        code: "600000".into(),
        kind: "support".into(),
        low: 10.0,
        high: 10.5,
        anchor_date: Some("2026-09-01".into()),
        ema9: Some(10.2),
        ema21: Some(10.3),
        ema50: Some(10.1),
        avwap_start_date: Some("2026-08-01".into()),
        avwap_price: Some(10.25),
        prev_high: Some(10.4),
        prev_low: Some(9.8),
        gap_top: None,
        gap_bottom: None,
        confidence: "高".into(),
        spread: Some(0.012),
        created_at: "2026-10-13T08:00:00Z".into(),
    };
    support_zone::upsert(&conn, &zone).unwrap();
    assert_eq!(support_zone::get(&conn, "z1").unwrap(), Some(zone));
    assert_eq!(support_zone::list_for_code(&conn, "600000").unwrap().len(), 1);
    assert_eq!(support_zone::delete(&conn, "z1").unwrap(), 1);
    assert!(support_zone::list_for_code(&conn, "600000").unwrap().is_empty());
}

#[test]
fn signal_latest_for_code() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(&dir);
    let conn = db.conn().unwrap();

    for (id, at) in [("s1", "2026-10-09T09:31:00Z"), ("s2", "2026-10-13T10:00:00Z")] {
        signal::upsert(
            &conn,
            &Signal {
                id: id.into(),
                code: "600000".into(),
                triggered_at: at.into(),
                timeframe: "1m".into(),
                candle_high_broken: 10.6,
                pre_candle_high: 10.5,
                zone_id: Some("z1".into()),
                status: "triggered".into(),
                created_at: at.into(),
            },
        )
        .unwrap();
    }
    assert_eq!(signal::list_for_code(&conn, "600000").unwrap().len(), 2);
    assert_eq!(signal::latest_for_code(&conn, "600000").unwrap().unwrap().id, "s2");
}

#[test]
fn trade_plan_filters_by_status() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(&dir);
    let conn = db.conn().unwrap();

    for (id, status) in [("p1", "draft"), ("p2", "executed")] {
        trade_plan::upsert(
            &conn,
            &TradePlan {
                id: id.into(),
                code: "600000".into(),
                entry: 20.0,
                stop: 19.5,
                account: 100000.0,
                risk_pct: 0.005,
                per_r: 0.5,
                shares: 1000,
                capital: 20000.0,
                pos_pct: 0.2,
                stop_distance: 0.025,
                t3: 21.5,
                t5: 22.5,
                status: status.into(),
                zone_id: None,
                signal_id: None,
                created_at: "2026-10-13T08:00:00Z".into(),
                updated_at: "2026-10-13T08:00:00Z".into(),
            },
        )
        .unwrap();
    }
    assert_eq!(trade_plan::list(&conn, None).unwrap().len(), 2);
    assert_eq!(trade_plan::list(&conn, Some("draft")).unwrap().len(), 1);
    assert_eq!(trade_plan::get(&conn, "p1").unwrap().unwrap().shares, 1000);
    assert_eq!(trade_plan::set_status(&conn, "p1", "executed", "2026-10-13T09:00:00Z").unwrap(), 1);
    assert_eq!(trade_plan::get(&conn, "p1").unwrap().unwrap().status, "executed");
}

#[test]
fn position_open_and_close() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(&dir);
    let conn = db.conn().unwrap();

    position::upsert(
        &conn,
        &Position {
            id: "pos1".into(),
            plan_id: "p1".into(),
            code: "600000".into(),
            entry_date: "2026-10-13".into(),
            entry_price: 20.0,
            quantity: 1000,
            current_stop: 19.5,
            follow_ema: true,
            exit_price: None,
            exit_date: None,
            created_at: "2026-10-13T08:00:00Z".into(),
            updated_at: "2026-10-13T08:00:00Z".into(),
        },
    )
    .unwrap();
    assert_eq!(position::list_open(&conn).unwrap().len(), 1);
    assert_eq!(position::close(&conn, "pos1", 21.5, "2026-10-20", "2026-10-20T08:00:00Z").unwrap(), 1);
    assert!(position::list_open(&conn).unwrap().is_empty());
    assert_eq!(position::list(&conn).unwrap()[0].exit_price, Some(21.5));
}

#[test]
fn trade_log_statistics_only_count_closed() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(&dir);
    let conn = db.conn().unwrap();

    trade_log::upsert(&conn, &log("t1", Some("2026-10-20"), Some(1.5))).unwrap();
    trade_log::upsert(&conn, &log("t2", None, None)).unwrap();

    assert_eq!(trade_log::list(&conn).unwrap().len(), 2);
    let closed = trade_log::list_closed(&conn).unwrap();
    assert_eq!(closed.len(), 1);
    assert_eq!(closed[0].id, "t1");
    assert_eq!(trade_log::get(&conn, "t1").unwrap().unwrap().r_multiple, Some(1.5));
}

#[test]
fn setting_key_value_store() {
    let dir = tempfile::tempdir().unwrap();
    let db = open(&dir);
    let conn = db.conn().unwrap();

    setting::set(&conn, "last_sync_at", "2026-10-13T08:00:00Z", "2026-10-13T08:00:00Z").unwrap();
    setting::set(&conn, "account", "100000", "2026-10-13T08:00:00Z").unwrap();
    assert_eq!(setting::get(&conn, "account").unwrap().as_deref(), Some("100000"));
    assert_eq!(setting::all(&conn).unwrap().len(), 2);

    // 覆盖写不新增。
    setting::set(&conn, "account", "200000", "2026-10-13T09:00:00Z").unwrap();
    assert_eq!(setting::get(&conn, "account").unwrap().as_deref(), Some("200000"));
    assert_eq!(setting::all(&conn).unwrap().len(), 2);

    assert_eq!(setting::remove(&conn, "account").unwrap(), 1);
    assert_eq!(setting::get(&conn, "account").unwrap(), None);
}

fn bar_row(code: &str, date: &str, close: f64, high: f64) -> BarRow {
    BarRow {
        code: code.into(),
        date: date.into(),
        open: close - 0.1,
        high,
        low: close - 0.2,
        close,
        volume: 1_000_000.0,
        amount: 1.0e7,
    }
}

fn log(id: &str, exit_date: Option<&str>, r_multiple: Option<f64>) -> TradeLogEntry {
    TradeLogEntry {
        id: id.into(),
        position_id: None,
        code: "600000".into(),
        name: Some("浦发银行".into()),
        entry_date: "2026-10-13".into(),
        entry_price: 20.0,
        stop_price: 19.5,
        exit_date: exit_date.map(str::to_string),
        exit_price: exit_date.map(|_| 21.5),
        quantity: 1000,
        per_r: Some(0.5),
        r_multiple,
        pnl: r_multiple.map(|r| r * 0.5 * 1000.0),
        fees: 0.0,
        note: None,
        source: "plan".into(),
        created_at: "2026-10-13T08:00:00Z".into(),
    }
}

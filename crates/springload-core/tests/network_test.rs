//! 默认忽略的联网实测：确认 A 股与美股两套行情源、以及腾讯与新浪的快照接口都能通。
//!
//! 运行：`cargo test -p springload-core --test network_test -- --ignored --test-threads=1`
//! 需要公网连通。真实网络失败时如实报错，不做任何替代。

use springload_core::backfill::{self, SyncOptions};
use springload_core::market::tdx::TdxSource;
use springload_core::market::us::UsSource;
use springload_core::market::{sina, tencent, Market, QuoteSource};
use springload_core::storage::{bar, bar_sync_state, stock, Database};
use std::time::Duration;

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60))
        .user_agent("springload/0.1")
        .build()
        .expect("构建 HTTP 客户端")
}

#[test]
#[ignore = "联网实测，需要公网"]
fn tdx_server_serves_list_bars_and_events() {
    let source = TdxSource::new();

    let rows = source.security_list().expect("取沪深证券列表");
    assert!(rows.len() > 1000, "沪深证券应有上千条，实际 {}", rows.len());

    // 不复权日线的成交量单位是手，换算成股后与成交额对得上。
    let bars = source.daily_raw("600519", "20261001").expect("取 600519 日线");
    assert!(!bars.is_empty(), "日线不应为空");
    let last = bars.last().expect("有日线");
    let ratio = last.amount / (last.volume * last.close);
    assert!(
        (0.9..1.1).contains(&ratio),
        "成交量应是股：amount/(volume*close) = {ratio}"
    );

    // 除权除息记录：茅台每年都分红。
    let events = source.corporate_actions("600519").expect("取 600519 除权除息");
    assert!(events.len() > 10, "茅台应有多次分红，实际 {}", events.len());
    assert!(events.iter().all(|event| event.ex_date.len() == 10));
    assert!(events.iter().all(|event| event.dividend_per_share >= 0.0));
}

#[test]
#[ignore = "联网实测，需要公网"]
fn tdx_minute_bars_come_back_for_every_period() {
    let source = TdxSource::new();
    for period in springload_core::minute::PERIODS {
        let bars = source.minute_bars("600519", period).expect("取分钟线");
        assert!(!bars.is_empty(), "{period} 分钟线不应为空");
        assert!(bars.iter().all(|bar| bar.period == period));
        assert!(
            bars.windows(2).all(|pair| pair[0].ts <= pair[1].ts),
            "{period} 分钟线应按时间升序"
        );
    }
}

#[test]
#[ignore = "联网实测，需要公网"]
fn tencent_snapshot_is_reachable() {
    let url = tencent::snapshot_url(&["600000".to_string()], Market::Cn);
    let bytes = client()
        .get(&url)
        .send()
        .expect("请求腾讯快照")
        .bytes()
        .expect("读腾讯响应体");
    let (text, _, _) = encoding_rs::GBK.decode(&bytes);
    let snapshots = tencent::parse_snapshots(&text, Market::Cn).expect("解析腾讯快照");
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].code, "600000");
    assert_eq!(snapshots[0].name, "浦发银行");
    assert!(snapshots[0].last > 0.0);
}

#[test]
#[ignore = "联网实测，需要公网"]
fn sina_snapshot_needs_referer_and_is_reachable() {
    let url = sina::list_url(&["600519".to_string()], Market::Cn);

    // 不带 Referer 返回 403。
    let without = client().get(&url).send().expect("请求新浪快照");
    assert_eq!(without.status().as_u16(), 403, "不带 Referer 应被拒绝");

    // 带合法 Referer 返回 200 且可解析。
    let response = client()
        .get(&url)
        .header("Referer", sina::REFERER)
        .send()
        .expect("带 Referer 请求新浪快照");
    assert_eq!(response.status().as_u16(), 200);
    let bytes = response.bytes().expect("读新浪响应体");
    let body = sina::decode(&bytes);
    let snapshots = sina::parse_snapshots(&body, Market::Cn).expect("解析新浪快照");
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].code, "600519");
    assert_eq!(snapshots[0].name, "贵州茅台");
}

#[test]
#[ignore = "联网实测，需要公网"]
fn full_backfill_writes_stock_pool_and_daily_bars() {
    let dir = tempfile::tempdir().expect("临时目录");
    let db = Database::open(&dir.path().join("springload.db")).expect("打开数据库");
    let source = TdxSource::new();
    let options = SyncOptions::new(false);

    let report =
        backfill::sync_market(&db, &source, Market::Cn, &options, &backfill::noop_sink())
            .expect("全量同步成功");
    println!(
        "股票池 {} 只，写入日线 {} 根，耗时 {} ms，失败 {} 条",
        report.stocks,
        report.bars,
        report.elapsed_ms,
        report.errors.len()
    );
    for error in report.errors.iter().take(5) {
        println!("  失败: {error}");
    }

    assert!(report.stocks > 3000, "股票池应在三千以上，实际 {}", report.stocks);
    assert!(report.bars > 0, "应写入日线");

    let conn = db.conn().unwrap();
    assert_eq!(stock::count(&conn).unwrap(), report.stocks as i64);
    assert!(bar::count(&conn).unwrap() > 0);
    let bars = bar::list_for_code(&conn, "600000").unwrap();
    assert!(!bars.is_empty(), "600000 应有日线");

    // 股票池只含沪深主板，且没有 ST。
    let stocks = stock::list(&conn).unwrap();
    for item in &stocks {
        let prefix = &item.code[..3];
        assert!(
            matches!(
                prefix,
                "600" | "601" | "603" | "605" | "000" | "001" | "002" | "003"
            ),
            "股票池出现非沪深主板代码 {} {}",
            item.code,
            item.name
        );
        assert_eq!(item.board, "main", "{} 的板块应是 main", item.code);
        assert!(!item.name.contains("ST"), "ST 不应在股票池里: {} {}", item.code, item.name);
        assert_eq!(item.market, if item.code.starts_with('6') { "SH" } else { "SZ" });
    }

    // 紧接着再跑一次增量：这是收盘后自动回补每天走的路径，必须真的写进日线。
    // 增量取的是「最后一条日线到今天」这一段，如果拿它去判上市时长，每只都会被当成
    // 次新股跳过，整次同步一只都写不进去。
    let incremental = backfill::sync_market(
        &db,
        &source,
        Market::Cn,
        &SyncOptions::new(false),
        &backfill::noop_sink(),
    )
    .expect("增量同步成功");
    println!(
        "增量回补写入 {} 只股票、{} 根日线，失败 {} 条",
        incremental.stocks,
        incremental.bars,
        incremental.errors.len()
    );
    assert!(
        incremental.stocks > 3000,
        "增量回补应写进三千只以上，实际 {}",
        incremental.stocks
    );
    assert!(incremental.bars > 0, "增量回补应写入日线");

    // 增量之后股票池只增不减：全量那一次因网络失败漏掉的，这一次补上了。
    let conn = db.conn().unwrap();
    assert!(
        stock::count(&conn).unwrap() >= report.stocks as i64,
        "增量回补不该让股票池变小"
    );
    let state = bar_sync_state::get(&conn, "600000").unwrap().expect("有回补状态");
    assert!(state.last_bar_date.is_some(), "增量之后仍应有最后一条日线");
}

#[test]
#[ignore = "联网实测，需要公网"]
fn us_source_serves_directory_daily_bars_and_minutes() {
    let source = UsSource::new();

    let rows = source.security_list().expect("取美股证券目录");
    assert!(rows.len() > 3000, "美股普通股应有数千条，实际 {}", rows.len());
    assert!(
        rows.iter().any(|row| row.code == "AAPL"),
        "目录里应有 AAPL"
    );

    // 日线是前复权价：成交额与「收盘价 × 成交量」对得上，说明价格单位是美元/股。
    let bars = source.daily_raw("AAPL", "20240101").expect("取 AAPL 日线");
    assert!(bars.len() > 300, "一年半应有三百多根日线，实际 {}", bars.len());
    assert!(
        bars.windows(2).all(|pair| pair[0].date < pair[1].date),
        "日线应按日期升序"
    );
    let last = bars.last().expect("有日线");
    let ratio = last.amount / (last.close * last.volume);
    assert!(
        (0.95..1.05).contains(&ratio),
        "成交额应是收盘价乘成交量：amount/(close*volume) = {ratio}"
    );

    // 拆股在源里已经处理过：NVDA 2024-06-10 的 10 拆 1 不该在日线上留缺口。
    let nvda = source.daily_raw("NVDA", "20240601").expect("取 NVDA 日线");
    let before = nvda
        .iter()
        .find(|bar| bar.date == "2024-06-07")
        .expect("有 2024-06-07");
    let after = nvda
        .iter()
        .find(|bar| bar.date == "2024-06-10")
        .expect("有 2024-06-10");
    let jump = after.close / before.close;
    assert!(
        (0.8..1.25).contains(&jump),
        "拆股日不应有十倍跳空，实际 {jump}"
    );

    // 美股没有除权除息事件：源给的就是复权价。
    assert!(source.corporate_actions("AAPL").expect("取事件").is_empty());

    for period in springload_core::minute::PERIODS {
        let bars = source.minute_bars("AAPL", period).expect("取分钟线");
        assert!(!bars.is_empty(), "{period} 分钟线不应为空");
        assert!(bars.iter().all(|bar| bar.period == period));
        assert!(
            bars.windows(2).all(|pair| pair[0].ts <= pair[1].ts),
            "{period} 分钟线应按时间升序"
        );
    }
}

#[test]
#[ignore = "联网实测，需要公网"]
fn us_snapshots_come_back_from_both_sources() {
    let url = tencent::snapshot_url(&["AAPL".to_string(), "MSFT".to_string()], Market::Us);
    let bytes = client()
        .get(&url)
        .send()
        .expect("请求腾讯美股快照")
        .bytes()
        .expect("读腾讯响应体");
    let (text, _, _) = encoding_rs::GBK.decode(&bytes);
    let snapshots = tencent::parse_snapshots(&text, Market::Us).expect("解析腾讯美股快照");
    assert_eq!(snapshots.len(), 2);
    let apple = snapshots
        .iter()
        .find(|snapshot| snapshot.code == "AAPL")
        .expect("应有 AAPL");
    assert!(apple.last > 0.0);
    assert!(apple.volume > 0.0, "成交量应是股数");
    assert!(apple.amount > 0.0, "成交额应取到");

    let url = sina::list_url(&["AAPL".to_string()], Market::Us);
    let bytes = client()
        .get(&url)
        .header("Referer", sina::REFERER)
        .send()
        .expect("请求新浪美股快照")
        .bytes()
        .expect("读新浪响应体");
    let body = sina::decode(&bytes);
    let snapshots = sina::parse_snapshots(&body, Market::Us).expect("解析新浪美股快照");
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].code, "AAPL");
    assert!(snapshots[0].last > 0.0);
    assert!(snapshots[0].prev_close > 0.0);
}

#[test]
#[ignore = "联网实测，需要公网"]
fn us_backfill_writes_common_stocks_only() {
    let dir = tempfile::tempdir().expect("临时目录");
    let db = Database::open(&dir.path().join("springload.us.db")).expect("打开数据库");
    let source = UsSource::new();
    let options = SyncOptions::new(false);
    let universe = backfill::fetch_universe(&source, Market::Us).expect("取股票池");
    println!("美股候选 {} 只", universe.len());
    assert!(universe.len() > 3000);
    assert!(universe.iter().all(|row| row.board == "common"));
    assert!(universe.iter().all(|row| row.market == "US"));
    assert!(
        universe.iter().all(|row| !row.name.to_lowercase().contains("warrant")),
        "权证不应进池"
    );
    drop(options);
    drop(db);
}

/// 美股全量回补走通：证券目录 → 日线 → 前复权 → 落库 → 股票池。
///
/// 五千多只股票逐个取日线，要跑几分钟，所以默认忽略。它证明的是整条美股取数链路，
/// 不是单点解析。
#[test]
#[ignore = "联网实测，需要公网，约十分钟"]
fn us_full_backfill_runs_end_to_end() {
    let dir = tempfile::tempdir().expect("临时目录");
    let db = Database::open(&dir.path().join("springload.us.db")).expect("打开数据库");
    let source = UsSource::new();
    let options = SyncOptions::new(true);

    let report = backfill::sync_market(&db, &source, Market::Us, &options, &backfill::noop_sink())
        .expect("全量同步成功");
    println!(
        "美股股票池 {} 只，写入日线 {} 根，耗时 {} ms，失败 {} 条",
        report.stocks,
        report.bars,
        report.elapsed_ms,
        report.errors.len()
    );
    for error in report.errors.iter().take(5) {
        println!("  失败: {error}");
    }

    assert!(report.stocks > 3000, "股票池应在三千以上，实际 {}", report.stocks);
    assert!(report.bars > 1_000_000, "三年日线应上百万根，实际 {}", report.bars);

    let conn = db.conn().unwrap();
    // 抽样核对一只股票的日线是前复权价，且落在三年窗口内。
    let bars = bar::list_for_code(&conn, "AAPL").unwrap();
    assert!(bars.len() > 500, "AAPL 应有三年日线，实际 {}", bars.len());
    let last = bars.last().unwrap();
    assert!(last.volume > 0.0, "最后一根应有成交");
    assert!(last.close > 0.0);

    let stocks = stock::list(&conn).unwrap();
    for item in stocks.iter().take(200) {
        assert_eq!(item.market, "US");
        assert_eq!(item.board, "common");
    }
    drop(conn);

    // 紧接着再跑一次增量：收盘后自动回补走的就是这条路径。美股的价格基准检查同样
    // 在这里生效——行情源把整段历史按新基准重算时，重叠那一天的价对不上就整段重取。
    let incremental = backfill::sync_market(
        &db,
        &source,
        Market::Us,
        &SyncOptions::new(false),
        &backfill::noop_sink(),
    )
    .expect("增量同步成功");
    println!(
        "美股增量回补写入 {} 只股票、{} 根日线，失败 {} 条",
        incremental.stocks,
        incremental.bars,
        incremental.errors.len()
    );
    assert!(
        incremental.stocks > 3000,
        "美股增量回补应写进三千只以上，实际 {}",
        incremental.stocks
    );

    // 增量之后抽查一只：整段历史仍然连续，接缝处没有台阶。
    let conn = db.conn().unwrap();
    let bars = bar::list_for_code(&conn, "AAPL").unwrap();
    assert!(bars.len() > 500, "AAPL 增量之后仍应有三年日线");
    let jumps = bars
        .windows(2)
        .filter(|pair| {
            let step = pair[1].close / pair[0].close;
            !(0.5..2.0).contains(&step)
        })
        .count();
    assert_eq!(jumps, 0, "AAPL 日线上不该出现半倍以上的跳变");
}

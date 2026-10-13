//! 默认忽略的联网实测：确认东方财富、腾讯、新浪三家接口都能通。
//!
//! 运行：`cargo test -p springload-core --test network_test -- --ignored --test-threads=1`
//! 需要公网连通。真实网络失败时如实报错，不做任何替代。

use springload_core::backfill::{self, HttpFetch, MarketFetch, SyncOptions};
use springload_core::market::{eastmoney, sina, tencent};
use springload_core::storage::{bar, stock, Database};
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
fn eastmoney_clist_and_kline_are_reachable() {
    let fetch = HttpFetch::new().expect("建抓取客户端");
    let body = fetch.clist_page(1, eastmoney::PAGE_SIZE).expect("取股票列表第 1 页");
    let page = eastmoney::parse_clist(&body).expect("解析股票列表");
    assert_eq!(page.rows.len(), 100, "每页应返回 100 条");
    assert!(page.total > 3000, "沪深主板总数应在三千以上，实际 {}", page.total);

    let kline = fetch.kline("1.600000", "20260901").expect("取 600000 日线");
    let parsed = eastmoney::parse_kline(&kline).expect("解析日线");
    assert_eq!(parsed.code, "600000");
    assert!(!parsed.bars.is_empty(), "日线不应为空");
    assert!(parsed.dktotal > 0);
}

#[test]
#[ignore = "联网实测，需要公网"]
fn tencent_snapshot_is_reachable() {
    let url = tencent::snapshot_url(&["600000".to_string()]);
    let bytes = client()
        .get(&url)
        .send()
        .expect("请求腾讯快照")
        .bytes()
        .expect("读腾讯响应体");
    let (text, _, _) = encoding_rs::GBK.decode(&bytes);
    let snapshots = tencent::parse_snapshots(&text).expect("解析腾讯快照");
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].code, "600000");
    assert_eq!(snapshots[0].name, "浦发银行");
    assert!(snapshots[0].last > 0.0);
}

#[test]
#[ignore = "联网实测，需要公网"]
fn sina_snapshot_needs_referer_and_is_reachable() {
    let url = sina::list_url(&["600519".to_string()]);

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
    let snapshots = sina::parse_snapshots(&body).expect("解析新浪快照");
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].code, "600519");
    assert_eq!(snapshots[0].name, "贵州茅台");
}

#[test]
#[ignore = "联网实测，需要公网"]
fn full_backfill_writes_stock_pool_and_daily_bars() {
    let dir = tempfile::tempdir().expect("临时目录");
    let db = Database::open(&dir.path().join("springload.db")).expect("打开数据库");
    let fetch = HttpFetch::new().expect("建抓取客户端");
    let options = SyncOptions::new(false);

    let report = backfill::sync_market(&db, &fetch, &options, &backfill::noop_sink())
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
        assert!(item.listed_at.is_some(), "{} 应有上市日", item.code);
    }
}

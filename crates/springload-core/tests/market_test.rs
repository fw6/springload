//! `market` 模块：三家适配器的解析与批量切分。
//!
//! 样本取自 `crates/springload-core/tests/fixtures/`，来源 URL 见每个测试的注释；
//! 全部为 2026-10-13 实际请求的响应。

use springload_core::market::url_budget::Budget;
use springload_core::market::{eastmoney, sina, tencent, TradeStatus};

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("读取样本 {name} 失败: {error}"))
}

fn fixture_text(name: &str) -> String {
    String::from_utf8(fixture(name)).unwrap_or_else(|error| panic!("样本 {name} 不是 UTF-8: {error}"))
}

#[test]
fn eastmoney_parses_qfq_daily_klines() {
    // https://push2his.eastmoney.com/api/qt/stock/kline/get?secid=1.600000&ut=fa5fd1943c7b386f172d6893dbfba10b&fields1=f1,f2,f3,f4,f5,f6&fields2=f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61&klt=101&fqt=1&beg=20240101&end=20240110
    let page = eastmoney::parse_kline(&fixture_text("eastmoney_kline_600000.json")).expect("解析日线");
    assert_eq!(page.code, "600000");
    assert_eq!(page.name, "浦发银行");
    assert_eq!(page.dktotal, 6404);
    assert!(!page.bars.is_empty());

    let first = &page.bars[0];
    assert_eq!(first.code, "600000");
    assert_eq!(first.date, "2024-01-02");
    assert_eq!(first.open, 5.48);
    assert_eq!(first.close, 5.45);
    assert_eq!(first.high, 5.50);
    assert_eq!(first.low, 5.45);
    // f56 是手，换算成股。
    assert_eq!(first.volume, 220667.0 * 100.0);
    assert_eq!(first.amount, 146066304.00);
    assert!(first.traded());
}

#[test]
fn eastmoney_kline_reports_bad_parameter_as_error() {
    // 同一接口，secid 的 market 写错时 rc=100。
    let error = eastmoney::parse_kline(&fixture_text("eastmoney_kline_rc100.json"))
        .expect_err("rc 非 0 应返回错误");
    assert!(error.to_string().contains("rc=100"), "{error}");
}

#[test]
fn eastmoney_kline_of_unlisted_stock_is_empty() {
    // https://push2his.eastmoney.com/api/qt/stock/kline/get?secid=0.001381&...&klt=101&fqt=1&end=20500101&lmt=3
    let page = eastmoney::parse_kline(&fixture_text("eastmoney_kline_empty.json")).expect("解析日线");
    assert_eq!(page.code, "001381");
    assert_eq!(page.dktotal, 0);
    assert!(page.bars.is_empty());
}

#[test]
fn eastmoney_kline_skips_suspended_days() {
    // https://push2his.eastmoney.com/api/qt/stock/kline/get?secid=1.600095&...&klt=101&fqt=0&beg=20250314&end=20250401
    // 600095 在 2025-03-17 至 2025-03-28 停牌，样本里只有 03-14、03-31、04-01 三根。
    let page = eastmoney::parse_kline(&fixture_text("eastmoney_kline_suspension_600095.json"))
        .expect("解析日线");
    let dates: Vec<&str> = page.bars.iter().map(|bar| bar.date.as_str()).collect();
    assert_eq!(dates, vec!["2025-03-14", "2025-03-31", "2025-04-01"]);
    // 复牌首日正常成交。
    assert_eq!(page.bars[1].date, "2025-03-31");
    assert!(page.bars[1].traded());
}

#[test]
fn eastmoney_parses_latest_bar_from_lmt_request() {
    // https://push2his.eastmoney.com/api/qt/stock/kline/get?secid=1.600000&...&klt=101&fqt=1&end=20500101&lmt=1
    let page = eastmoney::parse_kline(&fixture_text("eastmoney_kline_600000_latest.json"))
        .expect("解析日线");
    assert_eq!(page.bars.len(), 1);
    let bar = &page.bars[0];
    assert_eq!(bar.date, "2026-10-13");
    assert_eq!(bar.open, 9.67);
    assert_eq!(bar.close, 9.59);
    assert_eq!(bar.high, 9.70);
    assert_eq!(bar.low, 9.57);
    assert_eq!(bar.volume, 680290.0 * 100.0);
    assert_eq!(bar.amount, 654402287.00);
}

#[test]
fn eastmoney_kline_url_carries_frozen_parameters() {
    let url = eastmoney::kline_url("1.600000", 101, 1, Some("20230101"), Some("20500101"), None);
    assert!(url.contains("secid=1.600000"));
    assert!(url.contains("klt=101"));
    assert!(url.contains("fqt=1"));
    assert!(url.contains("beg=20230101"));
    assert!(url.contains("end=20500101"));
    assert!(!url.contains("lmt="));

    // secid 的市场前缀：上海 1、深圳 0。
    assert_eq!(eastmoney::secid("600000"), "1.600000");
    assert_eq!(eastmoney::secid("000001"), "0.000001");
}

#[test]
fn eastmoney_parses_clist_page_and_listing_date() {
    // https://push2.eastmoney.com/api/qt/clist/get?pn=1&pz=100&po=1&np=1&fltt=2&invt=2&fid=f12&fs=m:0+t:6,m:1+t:2&fields=f12,f13,f14,f26,f2,f18
    let page = eastmoney::parse_clist(&fixture_text("eastmoney_clist_page1.json")).expect("解析列表");
    assert_eq!(page.total, 3488);
    assert_eq!(page.rows.len(), 100);

    let row = page
        .rows
        .iter()
        .find(|row| row.code == "605599")
        .expect("样本里有 605599");
    assert_eq!(row.name, "菜百股份");
    assert_eq!(row.market, 1);
    assert_eq!(row.listed_at.as_deref(), Some("2021-09-09"));
    assert_eq!(row.last, Some(13.95));

    // 第 35 页是最后一页，返回 88 条。
    let last = eastmoney::parse_clist(&fixture_text("eastmoney_clist_page35.json")).expect("解析列表");
    assert_eq!(last.total, 3488);
    assert_eq!(last.rows.len(), 88);
}

#[test]
fn eastmoney_clist_reads_dash_as_missing_number() {
    // 第 35 页有停牌与未上市的行，f2 是字符串 `-`。
    let page = eastmoney::parse_clist(&fixture_text("eastmoney_clist_page35.json")).expect("解析列表");
    assert!(
        page.rows.iter().any(|row| row.last.is_none()),
        "停牌或未上市的行最新价应为 None"
    );
}

#[test]
fn tencent_parses_snapshot_by_code_field() {
    // https://qt.gtimg.cn/q=sh600000（响应按 GBK 编码）
    let body = decode_gbk(&fixture("tencent_snapshot_600000.gbk"));
    let snapshots = tencent::parse_snapshots(&body).expect("解析快照");
    assert_eq!(snapshots.len(), 1);
    let snapshot = &snapshots[0];
    assert_eq!(snapshot.code, "600000");
    assert_eq!(snapshot.name, "浦发银行");
    assert_eq!(snapshot.last, 9.59);
    assert_eq!(snapshot.prev_close, 9.67);
    assert_eq!(snapshot.open, 9.67);
    assert_eq!(snapshot.high, 9.70);
    assert_eq!(snapshot.low, 9.57);
    // 第 6 项是手，换算成股。
    assert_eq!(snapshot.volume, 680290.0 * 100.0);
    assert_eq!(snapshot.amount, 654402287.0);
    assert_eq!(snapshot.status, TradeStatus::Trading);
    assert_eq!(snapshot.captured_at, "2026-10-13 16:14:48");
}

#[test]
fn tencent_snapshot_status_maps_suspension_delisting_and_unlisted() {
    // https://qt.gtimg.cn/q=sh600000,sh601198,sz002388,sh600001,sz001381
    let body = decode_gbk(&fixture("tencent_snapshot_statuses.gbk"));
    let snapshots = tencent::parse_snapshots(&body).expect("解析快照");
    let status = |code: &str| {
        snapshots
            .iter()
            .find(|snapshot| snapshot.code == code)
            .unwrap_or_else(|| panic!("样本里没有 {code}"))
            .status
    };
    assert_eq!(status("600000"), TradeStatus::Trading);
    assert_eq!(status("601198"), TradeStatus::Suspended);
    assert_eq!(status("002388"), TradeStatus::Suspended);
    assert_eq!(status("600001"), TradeStatus::Delisted);
    assert_eq!(status("001381"), TradeStatus::Unknown);
}

#[test]
fn tencent_none_match_returns_no_snapshot() {
    // https://qt.gtimg.cn/q=sh999999,sz888888
    let body = decode_gbk(&fixture("tencent_none_match.gbk"));
    let snapshots = tencent::parse_snapshots(&body).expect("解析快照");
    assert!(snapshots.is_empty());
}

#[test]
fn tencent_over_limit_body_is_an_error() {
    // https://qt.gtimg.cn/q=<901 只代码>
    let body = decode_gbk(&fixture("tencent_too_many_codes.txt"));
    let error = tencent::parse_snapshots(&body).expect_err("超过 900 只应报错");
    assert!(error.to_string().contains("900"), "{error}");
}

#[test]
fn tencent_parses_qfq_daily_kline() {
    // https://web.ifzq.gtimg.cn/appstock/app/fqkline/get?param=sz001965,day,2026-07-27,2026-08-04,320,qfq
    let bars = tencent::parse_fqkline(&fixture_text("tencent_fqkline_001965.json")).expect("解析日线");
    assert_eq!(bars.len(), 7);
    let first = &bars[0];
    assert_eq!(first.code, "001965");
    assert_eq!(first.date, "2026-07-27");
    assert_eq!(first.open, 9.747);
    assert_eq!(first.close, 9.917);
    assert_eq!(first.high, 9.977);
    assert_eq!(first.low, 9.737);
    // 量单位是手。
    assert_eq!(first.volume, 119968.0 * 100.0);
}

#[test]
fn tencent_snapshot_url_prefixes_markets() {
    let url = tencent::snapshot_url(&["600000".into(), "000001".into(), "920821".into()]);
    assert_eq!(url, "https://qt.gtimg.cn/q=sh600000,sz000001,bj920821");
}

#[test]
fn sina_referer_must_be_sina_domain() {
    for allowed in [
        "https://finance.sina.com.cn",
        "https://finance.sina.com.cn/",
        "https://www.sina.com.cn",
        "https://sina.com.cn",
        "https://foo.sina.com.cn",
        "http://finance.sina.com.cn",
    ] {
        assert!(sina::referer_allowed(allowed), "{allowed} 应通过");
    }
    for rejected in [
        "",
        "Referer:",
        "https://evil.example.com",
        "https://notsina.com.cn",
        "https://xsina.com.cn",
        "https://sinacom.cn",
        "https://sina.com.cn.evil.example.com",
        "https://finance.sina.com.cn.evil.example.com",
        "https://sina.com.cn:8443",
        "not a url at all",
    ] {
        assert!(!sina::referer_allowed(rejected), "{rejected} 应被拒绝");
    }
}

#[test]
fn sina_decodes_gb18030_and_parses_snapshot() {
    // https://hq.sinajs.cn/list=sh600519，Referer: https://finance.sina.com.cn
    let body = sina::decode(&fixture("sina_snapshot_600519.gb18030"));
    assert!(body.contains("贵州茅台"), "应正确转码: {body}");
    let snapshots = sina::parse_snapshots(&body).expect("解析快照");
    assert_eq!(snapshots.len(), 1);
    let snapshot = &snapshots[0];
    assert_eq!(snapshot.code, "600519");
    assert_eq!(snapshot.name, "贵州茅台");
    assert_eq!(snapshot.open, 1287.000);
    assert_eq!(snapshot.prev_close, 1288.000);
    assert_eq!(snapshot.last, 1274.890);
    assert_eq!(snapshot.high, 1288.480);
    assert_eq!(snapshot.low, 1268.110);
    // 新浪的成交量已经是股。
    assert_eq!(snapshot.volume, 2657276.0);
    assert_eq!(snapshot.amount, 3393315572.000);
    assert_eq!(snapshot.captured_at, "2026-10-13 15:34:59");
}

#[test]
fn sina_skips_empty_placeholder_and_keeps_real_rows() {
    // https://hq.sinajs.cn/list=sh600013,sh600519（600013 不存在，返回空占位）
    let body = sina::decode(&fixture("sina_snapshot_empty_placeholder.gb18030"));
    assert!(body.contains(r#"hq_str_sh600013="";"#), "样本应含空占位: {body}");
    let snapshots = sina::parse_snapshots(&body).expect("解析快照");
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].code, "600519");
}

#[test]
fn sina_list_url_prefixes_markets() {
    let url = sina::list_url(&["600519".into(), "000001".into()]);
    assert_eq!(url, "https://hq.sinajs.cn/list=sh600519,sz000001");
}

#[test]
fn url_budget_splits_tencent_batches_within_limits() {
    let codes: Vec<String> = (0..2000).map(|i| format!("{:06}", 600000 + i)).collect();
    let batches = tencent::BATCH_BUDGET_HTTP1.batches(&codes);
    assert!(batches.len() > 1);
    let mut seen = 0;
    for batch in &batches {
        assert!(batch.len() <= tencent::MAX_CODES, "单批代码数超上限");
        assert!(
            tencent::BATCH_BUDGET_HTTP1.url_len(batch) <= tencent::HTTP1_URL_LIMIT,
            "单批 URL 超上限"
        );
        seen += batch.len();
    }
    assert_eq!(seen, codes.len(), "切分不丢代码");
}

#[test]
fn url_budget_splits_sina_batches_within_limits() {
    let codes: Vec<String> = (0..2000).map(|i| format!("{:06}", 600000 + i)).collect();
    let batches = sina::BATCH_BUDGET.batches(&codes);
    assert!(batches.len() > 2);
    for batch in &batches {
        assert!(batch.len() <= sina::MAX_CODES);
        assert!(sina::BATCH_BUDGET.url_len(batch) <= sina::URL_LIMIT);
    }
    let total: usize = batches.iter().map(Vec::len).sum();
    assert_eq!(total, codes.len());
}

#[test]
fn url_budget_splits_eastmoney_batches_within_limits() {
    let codes: Vec<String> = (0..2000).map(|i| format!("{:06}", 600000 + i)).collect();
    let batches = eastmoney::BATCH_BUDGET.batches(&codes);
    assert!(batches.len() > 2);
    for batch in &batches {
        assert!(batch.len() <= eastmoney::BATCH_BUDGET.max_codes);
        assert!(eastmoney::BATCH_BUDGET.url_len(batch) <= eastmoney::BATCH_BUDGET.max_url_len);
    }
    let total: usize = batches.iter().map(Vec::len).sum();
    assert_eq!(total, codes.len());
}

#[test]
fn tencent_batch_helpers_stay_within_url_limits() {
    let codes: Vec<String> = (0..2000).map(|i| format!("{:06}", 600000 + i)).collect();

    // HTTP/1.1 的默认切分。
    for batch in tencent::snapshot_batches(&codes) {
        let url = tencent::snapshot_url(&batch);
        assert!(url.len() <= tencent::HTTP1_URL_LIMIT, "URL 超上限: {} 字符", url.len());
        assert!(batch.len() <= tencent::MAX_CODES);
    }
    // HTTP/2 的切分更碎。
    let http2 = tencent::BATCH_BUDGET_HTTP2.batches(&codes);
    let http1 = tencent::snapshot_batches(&codes);
    assert!(http2.len() > http1.len());
    for batch in &http2 {
        assert!(tencent::snapshot_url(batch).len() <= tencent::HTTP2_URL_LIMIT);
    }
}

#[test]
fn sina_batch_helper_stays_within_url_limit() {
    let codes: Vec<String> = (0..2000).map(|i| format!("{:06}", 600000 + i)).collect();
    for batch in sina::list_batches(&codes) {
        assert!(batch.len() <= sina::MAX_CODES);
        assert!(sina::list_url(&batch).len() <= sina::URL_LIMIT);
    }
}

#[test]
fn url_budget_keeps_single_long_code_in_its_own_batch() {
    let budget = Budget {
        base_len: 10,
        prefix_len: 0,
        max_url_len: 20,
        max_codes: 100,
    };
    let codes = vec!["aaaaaaaaaaaaaaaaaaaaaaaa".to_string(), "b".to_string()];
    let batches = budget.batches(&codes);
    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0], vec!["aaaaaaaaaaaaaaaaaaaaaaaa".to_string()]);
    assert_eq!(batches[1], vec!["b".to_string()]);
}

/// 腾讯响应按 GBK 编码。
fn decode_gbk(bytes: &[u8]) -> String {
    let (text, _, _) = encoding_rs::GBK.decode(bytes);
    text.into_owned()
}

//! `market` 模块：板块与市场归属、通达信协议常量、腾讯与新浪快照的解析与批量切分，
//! 以及美股 Yahoo 适配器的日线与分钟线解析。
//!
//! 快照样本取自 `crates/springload-core/tests/fixtures/`，来源 URL 见每个测试的注释；
//! 全部为 2026-10-13 实际请求的响应。

use springload_core::market::url_budget::Budget;
use springload_core::market::{board_of, market_of, sina, tdx, tencent, us, Market, TradeStatus};

/// 代码到板块与市场的归属：只认沪深主板、科创板、创业板与北交所。
#[test]
fn code_prefix_decides_board_and_market() {
    for code in ["600000", "601398", "603000", "605081", "000001", "001979", "002415", "003816"] {
        assert_eq!(board_of(code), Some("main"), "{code} 应是主板");
    }
    assert_eq!(board_of("688981"), Some("star"));
    assert_eq!(board_of("300750"), Some("chinext"));
    assert_eq!(board_of("301660"), Some("chinext"));
    assert_eq!(board_of("920821"), Some("bse"));
    assert_eq!(board_of("900948"), None, "上海 B 股不在范围里");
    assert_eq!(board_of("200530"), None, "深圳 B 股不在范围里");

    assert_eq!(market_of("600000"), "SH");
    assert_eq!(market_of("688981"), "SH");
    assert_eq!(market_of("000001"), "SZ");
    assert_eq!(market_of("300750"), "SZ");
    assert_eq!(market_of("920821"), "BJ");
}

/// 通达信协议的市场号：0 深圳、1 上海、2 北京。
#[test]
fn tdx_market_numbers_follow_the_protocol() {
    assert_eq!(tdx::market_of("600519"), 1);
    assert_eq!(tdx::market_of("000001"), 0);
    assert_eq!(tdx::market_of("300750"), 0);
    assert_eq!(tdx::market_of("920001"), 2);
}

/// 分钟周期到协议 K 线种类的映射。
#[test]
fn tdx_minute_periods_map_to_categories() {
    assert_eq!(tdx::category_of(1), Some(7));
    assert_eq!(tdx::category_of(5), Some(0));
    assert_eq!(tdx::category_of(15), Some(1));
    assert_eq!(tdx::category_of(30), Some(2));
    assert_eq!(tdx::category_of(60), Some(3));
    assert_eq!(tdx::category_of(2), None, "两分钟不是协议支持的周期");
}

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
fn tencent_parses_snapshot_by_code_field() {
    // https://qt.gtimg.cn/q=sh600000（响应按 GBK 编码）
    let body = decode_gbk(&fixture("tencent_snapshot_600000.gbk"));
    let snapshots = tencent::parse_snapshots(&body, Market::Cn).expect("解析快照");
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
    let snapshots = tencent::parse_snapshots(&body, Market::Cn).expect("解析快照");
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
    let snapshots = tencent::parse_snapshots(&body, Market::Cn).expect("解析快照");
    assert!(snapshots.is_empty());
}

#[test]
fn tencent_over_limit_body_is_an_error() {
    // https://qt.gtimg.cn/q=<901 只代码>
    let body = decode_gbk(&fixture("tencent_too_many_codes.txt"));
    let error = tencent::parse_snapshots(&body, Market::Cn).expect_err("超过 900 只应报错");
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
    let url = tencent::snapshot_url(&["600000".into(), "000001".into(), "920821".into()], Market::Cn);
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
    let snapshots = sina::parse_snapshots(&body, Market::Cn).expect("解析快照");
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
    let snapshots = sina::parse_snapshots(&body, Market::Cn).expect("解析快照");
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].code, "600519");
}

#[test]
fn sina_list_url_prefixes_markets() {
    let url = sina::list_url(&["600519".into(), "000001".into()], Market::Cn);
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
fn tencent_batch_helpers_stay_within_url_limits() {
    let codes: Vec<String> = (0..2000).map(|i| format!("{:06}", 600000 + i)).collect();

    // HTTP/1.1 的默认切分。
    for batch in tencent::snapshot_batches(&codes) {
        let url = tencent::snapshot_url(&batch, Market::Cn);
        assert!(url.len() <= tencent::HTTP1_URL_LIMIT, "URL 超上限: {} 字符", url.len());
        assert!(batch.len() <= tencent::MAX_CODES);
    }
    // HTTP/2 的切分更碎。
    let http2 = tencent::BATCH_BUDGET_HTTP2.batches(&codes);
    let http1 = tencent::snapshot_batches(&codes);
    assert!(http2.len() > http1.len());
    for batch in &http2 {
        assert!(tencent::snapshot_url(batch, Market::Cn).len() <= tencent::HTTP2_URL_LIMIT);
    }
}

#[test]
fn sina_batch_helper_stays_within_url_limit() {
    let codes: Vec<String> = (0..2000).map(|i| format!("{:06}", 600000 + i)).collect();
    for batch in sina::list_batches(&codes) {
        assert!(batch.len() <= sina::MAX_CODES);
        assert!(sina::list_url(&batch, Market::Cn).len() <= sina::URL_LIMIT);
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

/// 美股代码调 Yahoo 时把类别股的点换成连字符。
#[test]
fn us_symbols_are_translated_for_yahoo() {
    assert_eq!(us::yahoo_symbol("aapl"), "AAPL");
    assert_eq!(us::yahoo_symbol("BRK.B"), "BRK-B");
    assert_eq!(us::yahoo_symbol("BF.B"), "BF-B");
}

/// 每个分钟周期都要配到一条 Yahoo 的 interval 与 range。
#[test]
fn us_minute_windows_cover_every_period() {
    let periods: Vec<u32> = us::MINUTE_WINDOWS.iter().map(|(period, _, _)| *period).collect();
    assert_eq!(periods, springload_core::minute::PERIODS.to_vec());
    for (_, interval, range) in us::MINUTE_WINDOWS {
        assert!(!interval.is_empty() && !range.is_empty());
    }
}

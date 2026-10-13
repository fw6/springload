//! 东方财富适配器：前复权日线 K 线与全市场股票列表。
//!
//! 事实来源见 `docs/research/quote-api-spec.md` 第 1、2 节。

use super::url_budget::Budget;
use super::{parse_f64, yyyymmdd_to_date, Bar};
use crate::error::{CoreError, Result};
use serde_json::Value;

/// 日线 K 线接口。
pub const KLINE_ENDPOINT: &str = "https://push2his.eastmoney.com/api/qt/stock/kline/get";
/// 全市场列表接口。
pub const CLIST_ENDPOINT: &str = "https://push2.eastmoney.com/api/qt/clist/get";
/// K 线接口的 ut 参数（不带也能返回，带上与文档一致）。
pub const UT: &str = "fa5fd1943c7b386f172d6893dbfba10b";
/// 沪深主板 A 股的 fs 过滤条件。
pub const MAIN_BOARD_FS: &str = "m:0+t:6,m:1+t:2";
/// `clist` 单页上限，超过 100 也只返回 100 条。
pub const PAGE_SIZE: u32 = 100;
/// 批量快照接口的 URL 长度上限：8179 字符通过，8404 字符返回 502。
///
/// `base_len` 是 `https://push2.eastmoney.com/api/qt/ulist.np/get?secids=` 加
/// 末尾 `&fields=f2,f3,f4,f12,f13,f14` 的长度；每个代码前另有 `市场号.` 两个字符。
pub const BATCH_BUDGET: Budget = Budget {
    base_len: 84,
    prefix_len: 2,
    max_url_len: 8179,
    max_codes: 900,
};

/// 日线周期。
pub const KLT_DAY: u32 = 101;
/// 前复权。
pub const FQT_QFQ: u32 = 1;
/// 取到最新的结束日期。
pub const END_LATEST: &str = "20500101";

/// 由六位代码推出 `secid` 的市场前缀：上海为 1，深圳为 0。
pub fn secid(code: &str) -> String {
    let market = if code.starts_with('6') || code.starts_with('9') {
        1
    } else {
        0
    };
    format!("{market}.{code}")
}

/// 前复权日线请求的 URL。
///
/// `beg` 与 `end` 都给定时 `lmt` 被服务端忽略；只给 `end` 时 `lmt` 生效。
pub fn kline_url(
    secid: &str,
    klt: u32,
    fqt: u32,
    beg: Option<&str>,
    end: Option<&str>,
    lmt: Option<u32>,
) -> String {
    let mut url = format!(
        "{KLINE_ENDPOINT}?secid={secid}&ut={UT}\
         &fields1=f1,f2,f3,f4,f5,f6\
         &fields2=f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61\
         &klt={klt}&fqt={fqt}"
    );
    if let Some(beg) = beg {
        url.push_str(&format!("&beg={beg}"));
    }
    if let Some(end) = end {
        url.push_str(&format!("&end={end}"));
    }
    if let Some(lmt) = lmt {
        url.push_str(&format!("&lmt={lmt}"));
    }
    url
}

/// 全市场列表请求的 URL。
pub fn clist_url(page: u32, page_size: u32, fs: &str) -> String {
    format!(
        "{CLIST_ENDPOINT}?pn={page}&pz={page_size}&po=1&np=1&fltt=2&invt=2&fid=f12&fs={fs}\
         &fields=f12,f13,f14,f26,f2,f18"
    )
}

/// 一只股票的 K 线响应。
#[derive(Debug, Clone, PartialEq)]
pub struct KlinePage {
    pub code: String,
    pub name: String,
    /// 该证券历史 K 线总条数，与请求范围无关。
    pub dktotal: i64,
    pub bars: Vec<Bar>,
}

/// 解析日线 K 线响应。`rc` 非 0 或没有 `data` 时返回错误。
pub fn parse_kline(body: &str) -> Result<KlinePage> {
    let root: Value = serde_json::from_str(body)?;
    let rc = root.get("rc").and_then(Value::as_i64).unwrap_or(-1);
    if rc != 0 {
        return Err(CoreError::InvalidData(format!("东方财富 K 线接口返回 rc={rc}")));
    }
    let data = root
        .get("data")
        .filter(|value| !value.is_null())
        .ok_or_else(|| CoreError::InvalidData("东方财富 K 线接口没有返回数据".to_string()))?;
    let code = data
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let name = data
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let dktotal = data.get("dktotal").and_then(Value::as_i64).unwrap_or(0);
    let mut bars = Vec::new();
    if let Some(klines) = data.get("klines").and_then(Value::as_array) {
        for row in klines {
            let text = row
                .as_str()
                .ok_or_else(|| CoreError::InvalidData("K 线行不是字符串".to_string()))?;
            bars.push(parse_kline_row(&code, text)?);
        }
    }
    Ok(KlinePage {
        code,
        name,
        dktotal,
        bars,
    })
}

/// 一行 `f51..f61`：日期、开、收、高、低、量（手）、额（元）、振幅、涨跌幅、涨跌额、换手率。
fn parse_kline_row(code: &str, text: &str) -> Result<Bar> {
    let fields: Vec<&str> = text.split(',').collect();
    if fields.len() < 7 {
        return Err(CoreError::InvalidData(format!("K 线行字段不足: {text}")));
    }
    Ok(Bar {
        code: code.to_string(),
        date: fields[0].to_string(),
        open: parse_f64("开盘", fields[1])?,
        close: parse_f64("收盘", fields[2])?,
        high: parse_f64("最高", fields[3])?,
        low: parse_f64("最低", fields[4])?,
        // f56 的单位是手，换算成股。
        volume: parse_f64("成交量", fields[5])? * 100.0,
        amount: parse_f64("成交额", fields[6])?,
    })
}

/// 全市场列表里的一行。
#[derive(Debug, Clone, PartialEq)]
pub struct ClistRow {
    pub code: String,
    /// 1 表示上海，0 表示深圳。
    pub market: i64,
    pub name: String,
    /// 上市日 YYYY-MM-DD。
    pub listed_at: Option<String>,
    /// 最新价，停牌或未上市时为 `None`。
    pub last: Option<f64>,
    /// 昨收，未上市时为 `None`。
    pub prev_close: Option<f64>,
}

/// 一页全市场列表。
#[derive(Debug, Clone, PartialEq)]
pub struct ClistPage {
    /// 服务端报告的总条数。
    pub total: i64,
    pub rows: Vec<ClistRow>,
}

/// 解析全市场列表响应。
pub fn parse_clist(body: &str) -> Result<ClistPage> {
    let root: Value = serde_json::from_str(body)?;
    let rc = root.get("rc").and_then(Value::as_i64).unwrap_or(-1);
    if rc != 0 {
        return Err(CoreError::InvalidData(format!("东方财富列表接口返回 rc={rc}")));
    }
    let data = root
        .get("data")
        .filter(|value| !value.is_null())
        .ok_or_else(|| CoreError::InvalidData("东方财富列表接口没有返回数据".to_string()))?;
    let total = data.get("total").and_then(Value::as_i64).unwrap_or(0);
    let mut rows = Vec::new();
    if let Some(diff) = data.get("diff").and_then(Value::as_array) {
        for item in diff {
            rows.push(parse_clist_row(item)?);
        }
    }
    Ok(ClistPage { total, rows })
}

fn parse_clist_row(item: &Value) -> Result<ClistRow> {
    let code = item
        .get("f12")
        .and_then(Value::as_str)
        .ok_or_else(|| CoreError::InvalidData("列表行缺 f12".to_string()))?
        .to_string();
    let name = item
        .get("f14")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let market = item.get("f13").and_then(Value::as_i64).unwrap_or(0);
    Ok(ClistRow {
        code,
        market,
        name,
        listed_at: optional_yyyymmdd(item.get("f26")),
        last: optional_number(item.get("f2")),
        prev_close: optional_number(item.get("f18")),
    })
}

/// `f26` 是 `YYYYMMDD` 的整数。
fn optional_yyyymmdd(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::Number(n) => yyyymmdd_to_date(&n.to_string()),
        Value::String(s) => yyyymmdd_to_date(s),
        _ => None,
    }
}

/// 数值字段可能是数字，也可能是停牌时的字符串 `-`。
fn optional_number(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

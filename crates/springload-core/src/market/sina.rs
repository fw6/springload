//! 新浪适配器：`hq.sinajs.cn` 实时快照。
//!
//! 事实来源见 `docs/research/quote-api-limits.md` 第一节与 `docs/research/us-quote-source.md`。
//! 请求必须携带主机名为 `sina.com.cn` 或其子域的 `Referer`，响应编码是 GB18030。
//! 不存在的代码返回空占位 `=""`，响应按代码字段重新配对。
//!
//! 两个市场的字段位置完全不同：A 股是逗号分隔的三十多项、美股是三十六项的另一套
//! 排法，所以各写一个解析函数。美股没有涨跌停与停牌状态位，返回非空即视为正常交易。

use super::url_budget::Budget;
use super::{parse_f64, Market, Snapshot, TradeStatus};
use crate::error::{CoreError, Result};
use encoding_rs::GB18030;

/// 快照接口前缀。
pub const LIST_ENDPOINT: &str = "https://hq.sinajs.cn/list=";
/// 请求必须携带的 `Referer`。
pub const REFERER: &str = "https://finance.sina.com.cn";
/// 单次请求的代码数上限：890 只通过，900 只返回 431。
pub const MAX_CODES: usize = 890;
/// URL 长度上限：8035 字符通过，8121 字符返回 431。
pub const URL_LIMIT: usize = 8035;

/// 批量约束。
pub const BATCH_BUDGET: Budget = Budget {
    base_len: LIST_ENDPOINT.len(),
    prefix_len: 2,
    max_url_len: URL_LIMIT,
    max_codes: MAX_CODES,
};

/// 代码前缀：A 股与腾讯一致（`sh`/`sz`/`bj`），美股是 `gb_` 加小写代码。
///
/// 新浪的美股代码区分大小写：`gb_aapl` 有行情，`gb_AAPL` 返回空占位。
pub fn prefix(code: &str, market: Market) -> String {
    match market {
        Market::Cn => format!("{}{}", super::tencent::prefix(code, Market::Cn), code),
        Market::Us => format!("gb_{}", code.trim().to_ascii_lowercase()),
    }
}

/// 请求 URL。
pub fn list_url(codes: &[String], market: Market) -> String {
    let joined = codes
        .iter()
        .map(|code| prefix(code, market))
        .collect::<Vec<_>>()
        .join(",");
    format!("{LIST_ENDPOINT}{joined}")
}

/// 按 URL 长度与代码数上限把一批代码切成若干批。
pub fn list_batches(codes: &[String]) -> Vec<Vec<String>> {
    BATCH_BUDGET.batches(codes)
}

/// `Referer` 的主机名是否是 `sina.com.cn` 或其子域。
///
/// 只接受 http 与 https；带端口号不接受；字符串包含关系不通过。
pub fn referer_allowed(referer: &str) -> bool {
    let rest = match referer
        .strip_prefix("https://")
        .or_else(|| referer.strip_prefix("http://"))
    {
        Some(rest) => rest,
        None => return false,
    };
    let host_port = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host_port.is_empty() || host_port.contains(':') {
        return false;
    }
    let host = host_port.to_ascii_lowercase();
    host == "sina.com.cn" || host.ends_with(".sina.com.cn")
}

/// 按 GB18030 解码响应体。
pub fn decode(body: &[u8]) -> String {
    let (text, _, _) = GB18030.decode(body);
    text.into_owned()
}

/// 解析快照响应。空占位（`=""`）的代码被跳过。
pub fn parse_snapshots(body: &str, market: Market) -> Result<Vec<Snapshot>> {
    let mut out = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if !line.starts_with("var hq_str_") {
            continue;
        }
        let Some(eq) = line.find('=') else {
            continue;
        };
        let var = &line[..eq];
        // 变量名形如 `var hq_str_sh600519`，末段是带市场前缀的代码。
        let Some(symbol) = var.rsplit('_').next() else {
            continue;
        };
        let content = line[eq + 1..]
            .trim()
            .trim_end_matches(';')
            .trim()
            .trim_matches('"');
        // 不存在的代码返回空占位，跳过。
        if content.is_empty() {
            continue;
        }
        out.push(match market {
            Market::Cn => cn_snapshot(symbol, content)?,
            Market::Us => us_snapshot(symbol, content)?,
        });
    }
    Ok(out)
}

/// A 股快照的一行。
fn cn_snapshot(symbol: &str, content: &str) -> Result<Snapshot> {
    let Some(code) = symbol
        .strip_prefix("sh")
        .or_else(|| symbol.strip_prefix("sz"))
        .or_else(|| symbol.strip_prefix("bj"))
    else {
        return Err(CoreError::InvalidData(format!(
            "新浪快照的代码字段不认识: {symbol}"
        )));
    };
    let fields: Vec<&str> = content.split(',').collect();
    // 最后一项状态码在索引 32，字段数不足 33 时截断了。
    if fields.len() < 33 {
        return Err(CoreError::InvalidData(format!(
            "新浪快照字段数 {} 少于 33",
            fields.len()
        )));
    }
    Ok(Snapshot {
        code: code.to_string(),
        name: fields[0].trim().to_string(),
        open: parse_f64("今开", fields[1])?,
        prev_close: parse_f64("昨收", fields[2])?,
        last: parse_f64("最新价", fields[3])?,
        high: parse_f64("最高", fields[4])?,
        low: parse_f64("最低", fields[5])?,
        // 第 8 项单位是股。
        volume: parse_f64("成交量", fields[8])?,
        amount: parse_f64("成交额", fields[9])?,
        status: sina_status(fields[32]),
        captured_at: format!("{} {}", fields[30].trim(), fields[31].trim()),
    })
}

/// 美股快照的一行。变量名形如 `var hq_str_gb_aapl`，末段已经是 `gb_` 之后的小写
/// 交易代码本身，不含市场前缀。
fn us_snapshot(symbol: &str, content: &str) -> Result<Snapshot> {
    if symbol.is_empty() {
        return Err(CoreError::InvalidData(
            "新浪快照的代码字段是空的".to_string(),
        ));
    }
    let fields: Vec<&str> = content.split(',').collect();
    // 成交额在索引 30，字段数不足 31 时截断了。
    if fields.len() < 31 {
        return Err(CoreError::InvalidData(format!(
            "新浪美股快照字段数 {} 少于 31",
            fields.len()
        )));
    }
    Ok(Snapshot {
        code: symbol.to_ascii_uppercase(),
        name: fields[0].trim().to_string(),
        last: parse_f64("最新价", fields[1])?,
        open: parse_f64("今开", fields[5])?,
        high: parse_f64("最高", fields[6])?,
        low: parse_f64("最低", fields[7])?,
        // 第 10 项单位是股。
        volume: parse_f64("成交量", fields[10])?,
        amount: parse_f64("成交额", fields[30])?,
        prev_close: parse_f64("昨收", fields[26])?,
        status: TradeStatus::Trading,
        captured_at: fields[3].trim().to_string(),
    })
}

/// 第 32 项状态码：`00` 为正常，其余取值未在调研中核实。
fn sina_status(value: &str) -> TradeStatus {
    match value.trim() {
        "" | "00" => TradeStatus::Trading,
        _ => TradeStatus::Unknown,
    }
}

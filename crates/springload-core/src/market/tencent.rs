//! 腾讯适配器：`qt.gtimg.cn` 实时快照与 `web.ifzq.gtimg.cn` 前复权日线。
//!
//! 事实来源见 `docs/research/quote-api-spec.md` 第 3、4 节。腾讯对不存在的代码整条
//! 省略，响应必须按返回的代码字段重新配对，不按下标取值。
//!
//! A 股与美股的快照字段位置基本相同，三处不同：成交量 A 股是手、美股是股；成交额
//! A 股要从第 35 项里切、美股直接在第 37 项；代码字段 A 股是六位数字、美股带交易所
//! 后缀（`AAPL.OQ`）。

use super::url_budget::Budget;
use super::{parse_f64, yyyymmddhhmmss_to_datetime, Bar, Market, Snapshot, TradeStatus};
use crate::error::{CoreError, Result};
use serde_json::Value;

/// 实时快照接口前缀。
pub const SNAPSHOT_ENDPOINT: &str = "https://qt.gtimg.cn/q=";
/// 前复权日线接口。
pub const FQKLINE_ENDPOINT: &str = "https://web.ifzq.gtimg.cn/appstock/app/fqkline/get";
/// 单次请求的代码数硬上限，超过返回 `too many codes`。
pub const MAX_CODES: usize = 900;
/// HTTP/1.1 下的 URL 长度上限：8121 字符通过，8571 字符返回 414。
pub const HTTP1_URL_LIMIT: usize = 8100;
/// HTTP/2 下的 URL 长度上限：5691 字符通过，5736 字符起连接层拒绝。
pub const HTTP2_URL_LIMIT: usize = 5700;

/// HTTP/1.1 客户端的批量约束。
pub const BATCH_BUDGET_HTTP1: Budget = Budget {
    base_len: SNAPSHOT_ENDPOINT.len(),
    prefix_len: 2,
    max_url_len: HTTP1_URL_LIMIT,
    max_codes: MAX_CODES,
};

/// HTTP/2 客户端的批量约束。
pub const BATCH_BUDGET_HTTP2: Budget = Budget {
    base_len: SNAPSHOT_ENDPOINT.len(),
    prefix_len: 2,
    max_url_len: HTTP2_URL_LIMIT,
    max_codes: MAX_CODES,
};

/// 代码前缀：A 股上海 `sh`、深圳 `sz`、北京 `bj`；美股统一 `us`。
pub fn prefix(code: &str, market: Market) -> &'static str {
    match market {
        Market::Us => "us",
        Market::Cn if code.starts_with("920") || code.starts_with('4') || code.starts_with('8') => {
            "bj"
        }
        Market::Cn if code.starts_with('6') || code.starts_with('9') => "sh",
        Market::Cn => "sz",
    }
}

/// 快照请求的 URL。
pub fn snapshot_url(codes: &[String], market: Market) -> String {
    let joined = codes
        .iter()
        .map(|code| format!("{}{}", prefix(code, market), code))
        .collect::<Vec<_>>()
        .join(",");
    format!("{SNAPSHOT_ENDPOINT}{joined}")
}

/// 按 URL 长度与代码数上限把一批代码切成若干批。
///
/// 默认走 HTTP/1.1 客户端，因此用 [`BATCH_BUDGET_HTTP1`]；改用 HTTP/2 时传
/// [`BATCH_BUDGET_HTTP2`]。两个市场的代码前缀都是两个字符，切分规则不分市场。
pub fn snapshot_batches(codes: &[String]) -> Vec<Vec<String>> {
    BATCH_BUDGET_HTTP1.batches(codes)
}

/// 解析快照响应。响应体应先按 GBK 解码。
///
/// 不存在的代码不会出现在响应里；全部查不到时返回 `v_pv_none_match="1";`。
pub fn parse_snapshots(body: &str, market: Market) -> Result<Vec<Snapshot>> {
    if body.trim() == "too many codes" {
        return Err(CoreError::InvalidData(
            "腾讯快照一次请求的代码数超过 900".to_string(),
        ));
    }
    let mut out = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some(eq) = line.find('=') else {
            continue;
        };
        let var = line[..eq].trim();
        if var == "v_pv_none_match" {
            continue;
        }
        let content = line[eq + 1..].trim().trim_end_matches(';').trim();
        let content = content
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .unwrap_or(content);
        if content.is_empty() {
            continue;
        }
        let fields: Vec<&str> = content.split('~').collect();
        if fields.len() < 41 {
            return Err(CoreError::InvalidData(format!(
                "腾讯快照字段数 {} 少于 41",
                fields.len()
            )));
        }
        out.push(match market {
            Market::Cn => cn_snapshot(&fields)?,
            Market::Us => us_snapshot(&fields)?,
        });
    }
    Ok(out)
}

/// A 股快照的一行。
fn cn_snapshot(fields: &[&str]) -> Result<Snapshot> {
    Ok(Snapshot {
        // 第 2 项是代码；按它配对，不按下标。
        code: fields[2].trim().to_string(),
        name: fields[1].trim().to_string(),
        last: parse_f64("最新价", fields[3])?,
        prev_close: parse_f64("昨收", fields[4])?,
        open: parse_f64("今开", fields[5])?,
        high: parse_f64("最高", fields[33])?,
        low: parse_f64("最低", fields[34])?,
        // 第 6 项单位是手。
        volume: parse_f64("成交量", fields[6])? * 100.0,
        amount: cn_amount(fields)?,
        status: tencent_status(fields[40]),
        captured_at: yyyymmddhhmmss_to_datetime(fields[30]),
    })
}

/// 美股快照的一行。
fn us_snapshot(fields: &[&str]) -> Result<Snapshot> {
    Ok(Snapshot {
        // 第 2 项带交易所后缀，形如 `AAPL.OQ`。
        code: fields[2]
            .trim()
            .split('.')
            .next()
            .unwrap_or("")
            .to_ascii_uppercase(),
        name: fields[1].trim().to_string(),
        last: parse_f64("最新价", fields[3])?,
        prev_close: parse_f64("昨收", fields[4])?,
        open: parse_f64("今开", fields[5])?,
        high: parse_f64("最高", fields[33])?,
        low: parse_f64("最低", fields[34])?,
        // 第 6 项单位是股。
        volume: parse_f64("成交量", fields[6])?,
        amount: parse_f64("成交额", fields[37])?,
        status: tencent_status(fields[40]),
        captured_at: yyyymmddhhmmss_to_datetime(fields[30]),
    })
}

/// A 股成交额：优先取第 35 项的第三段（元），缺失时用第 37 项（万元）换算。
fn cn_amount(fields: &[&str]) -> Result<f64> {
    if let Some(segment) = fields[35].split('/').nth(2) {
        if let Ok(value) = segment.trim().parse::<f64>() {
            return Ok(value);
        }
    }
    Ok(parse_f64("成交额", fields[37])? * 10_000.0)
}

/// 第 40 项交易状态：空为正常，`S` 停牌，`D` 退市，`U` 未上市。
fn tencent_status(value: &str) -> TradeStatus {
    match value.trim() {
        "" => TradeStatus::Trading,
        "S" => TradeStatus::Suspended,
        "D" => TradeStatus::Delisted,
        _ => TradeStatus::Unknown,
    }
}

/// 解析前复权日线响应。
///
/// `data` 下是 `{"sz001965": {"qfqday": [[日期, 开, 收, 高, 低, 量（手）], ...]}}`。
pub fn parse_fqkline(body: &str) -> Result<Vec<Bar>> {
    let root: Value = serde_json::from_str(body)?;
    let code = root
        .get("code")
        .and_then(Value::as_i64)
        .unwrap_or(-1);
    if code != 0 {
        return Err(CoreError::InvalidData(format!("腾讯日线接口返回 code={code}")));
    }
    let data = root
        .get("data")
        .and_then(Value::as_object)
        .ok_or_else(|| CoreError::InvalidData("腾讯日线接口没有返回数据".to_string()))?;
    let (symbol, payload) = data
        .iter()
        .next()
        .ok_or_else(|| CoreError::InvalidData("腾讯日线接口没有返回数据".to_string()))?;
    let code = symbol
        .strip_prefix("sh")
        .or_else(|| symbol.strip_prefix("sz"))
        .or_else(|| symbol.strip_prefix("bj"))
        .unwrap_or(symbol)
        .to_string();
    let rows = payload
        .get("qfqday")
        .or_else(|| payload.get("day"))
        .and_then(Value::as_array)
        .ok_or_else(|| CoreError::InvalidData("腾讯日线接口没有 qfqday".to_string()))?;
    let mut bars = Vec::new();
    for row in rows {
        let cells = row
            .as_array()
            .ok_or_else(|| CoreError::InvalidData("腾讯日线行不是数组".to_string()))?;
        if cells.len() < 6 {
            return Err(CoreError::InvalidData("腾讯日线行字段不足".to_string()));
        }
        let text = |index: usize| -> Result<&str> {
            cells[index]
                .as_str()
                .ok_or_else(|| CoreError::InvalidData("腾讯日线字段不是字符串".to_string()))
        };
        bars.push(Bar {
            code: code.clone(),
            date: text(0)?.to_string(),
            open: parse_f64("开盘", text(1)?)?,
            close: parse_f64("收盘", text(2)?)?,
            high: parse_f64("最高", text(3)?)?,
            low: parse_f64("最低", text(4)?)?,
            // 量单位是手。
            volume: parse_f64("成交量", text(5)?)? * 100.0,
            // 腾讯前复权日线不返回成交额。
            amount: 0.0,
        });
    }
    Ok(bars)
}

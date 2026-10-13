//! 持仓跟踪：三类离场提示、T+1 的可执行日与持仓列表。
//!
//! 三类提示互不排斥，一次刷新可能同时给出多条。提示只描述已经发生的事与计划里的
//! 数字，不给买卖结论。

use crate::error::Result;
use crate::indicators;
use crate::storage::bar::{self, BarRow};
use crate::storage::position::{self, Position};
use crate::storage::stock;
use crate::storage::trade_plan;
use crate::storage::Database;
use chrono::{Datelike, Duration, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};

/// 止损提示的文案。
pub const STOP_LOSS_MESSAGE: &str = "触发止损，按计划离场";
/// 跌破 9 日均线的提示文案。
pub const BELOW_EMA9_MESSAGE: &str = "趋势可能走坏，建议评估离场";
/// 到达 3R 的提示文案。
pub const REDUCE_3R_MESSAGE: &str = "到达 3R 减仓目标位";
/// 到达 5R 的提示文案。
pub const REDUCE_5R_MESSAGE: &str = "到达 5R 减仓目标位";

/// 提示的种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AlertKind {
    /// 跌破止损价。
    StopLoss,
    /// 跌破 9 日均线。
    BelowEma9,
    /// 到达 3R。
    Reduce3R,
    /// 到达 5R。
    Reduce5R,
}

/// 一条离场提示。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alert {
    pub kind: AlertKind,
    pub message: String,
    /// 观察到条件的那一天。
    pub signal_date: String,
    /// 最早可以卖出的那一天，不早于买入日的次一交易日。
    pub executable_date: String,
}

/// 持仓列表里的一行。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionView {
    pub id: String,
    pub plan_id: String,
    pub code: String,
    pub name: Option<String>,
    pub entry_date: String,
    pub entry_price: f64,
    pub stop: f64,
    pub quantity: i64,
    /// 最新价；没有行情时为 `None`。
    pub last: Option<f64>,
    /// 9 日均线；日线不足 9 根时为 `None`。
    pub ema9: Option<f64>,
    pub per_r: f64,
    pub t3: f64,
    pub t5: f64,
    /// 剩余仓位是否跟随 9 日均线。
    pub follow_ema: bool,
    pub alerts: Vec<Alert>,
}

/// 买入日之后的下一个交易日。
///
/// 日历里找不到更晚的交易日时（例如还没回补到那天），退回买入日之后的第一个工作日。
pub fn next_trading_day(calendar: &[String], date: &str) -> String {
    if let Some(found) = calendar.iter().find(|day| day.as_str() > date) {
        return found.clone();
    }
    next_weekday(date)
}

/// 可执行日：不早于买入日的次一交易日。
///
/// 信号日已经晚于那个日子时就是信号日本身。
pub fn executable_date(calendar: &[String], entry_date: &str, signal_date: &str) -> String {
    let earliest = next_trading_day(calendar, entry_date);
    if signal_date >= earliest.as_str() {
        signal_date.to_string()
    } else {
        earliest
    }
}

/// 买入日之后的第一个工作日，跳过周六周日。
fn next_weekday(date: &str) -> String {
    let parsed = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap_or_else(|_| {
        NaiveDate::from_ymd_opt(1970, 1, 1).expect("固定的日期总是合法的")
    });
    let mut cursor = parsed + Duration::days(1);
    while matches!(cursor.weekday(), Weekday::Sat | Weekday::Sun) {
        cursor += Duration::days(1);
    }
    cursor.format("%Y-%m-%d").to_string()
}

/// 三类提示的判定。条件同时成立时全部返回。
///
/// 传入的价格与均线都可能是 `None`（没有行情或日线不足），此时对应的提示不产生。
pub fn evaluate(
    stop: f64,
    t3: f64,
    t5: f64,
    last: Option<f64>,
    ema9: Option<f64>,
    follow_ema: bool,
    calendar: &[String],
    entry_date: &str,
    signal_date: &str,
) -> Vec<Alert> {
    let mut alerts = Vec::new();
    let Some(last) = last else {
        return alerts;
    };
    let executable = executable_date(calendar, entry_date, signal_date);

    let mut push = |kind: AlertKind, message: &str| {
        alerts.push(Alert {
            kind,
            message: message.to_string(),
            signal_date: signal_date.to_string(),
            executable_date: executable.clone(),
        });
    };

    if last <= stop {
        push(AlertKind::StopLoss, STOP_LOSS_MESSAGE);
    }
    // 剩余仓位跟随 9 日均线时才给这条提示。
    if follow_ema {
        if let Some(ema9) = ema9 {
            if last < ema9 {
                push(AlertKind::BelowEma9, BELOW_EMA9_MESSAGE);
            }
        }
    }
    if last >= t5 {
        push(AlertKind::Reduce5R, REDUCE_5R_MESSAGE);
    } else if last >= t3 {
        push(AlertKind::Reduce3R, REDUCE_3R_MESSAGE);
    }
    alerts
}

/// 某只股票最近一根已交易日线的收盘价。
fn latest_close(bars: &[BarRow]) -> Option<f64> {
    bars.iter().rev().find(|row| row.volume > 0.0).map(|row| row.close)
}

/// 某只股票的 9 日均线（用已交易的日线算）。
fn ema9_of(bars: &[BarRow]) -> Option<f64> {
    let closes: Vec<f64> = bars
        .iter()
        .filter(|row| row.volume > 0.0)
        .map(|row| row.close)
        .collect();
    let series = indicators::ema(&closes, 9);
    series.last().copied().filter(|value| value.is_finite())
}

/// 读全部未平仓持仓，用已入库的日线当最新价（没有实时快照时的兜底）。
pub fn list_positions(db: &Database) -> Result<Vec<PositionView>> {
    let conn = db.conn()?;
    let calendar = bar::trading_days(&conn)?;
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let mut out = Vec::new();
    for row in position::list_open(&conn)? {
        out.push(build_view(&conn, &row, &calendar, &today, None)?);
    }
    Ok(out)
}

/// 用实时快照刷新持仓的最新价，并给出提示。
pub fn refresh_positions(db: &Database, quotes: &[(String, f64)]) -> Result<Vec<PositionView>> {
    let conn = db.conn()?;
    let calendar = bar::trading_days(&conn)?;
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let mut out = Vec::new();
    for row in position::list_open(&conn)? {
        let quote = quotes
            .iter()
            .find(|(code, _)| code == &row.code)
            .map(|(_, price)| *price);
        out.push(build_view(&conn, &row, &calendar, &today, quote)?);
    }
    Ok(out)
}

/// 抓一批代码的实时快照，返回（代码，最新价）。
///
/// 主来源是腾讯 `qt.gtimg.cn`，取不到时切到新浪 `hq.sinajs.cn` 这个备用来源。
/// 两家的批量上限与 URL 长度上限不同，各自按自己的约束切分；腾讯对不存在的代码
/// 整条省略，新浪返回空占位，两家的结果都按代码字段配对，不按位置对应。
pub fn fetch_quotes(codes: &[String]) -> Result<Vec<(String, f64)>> {
    use crate::error::CoreError;
    use crate::market::{sina, tencent};
    use std::io;
    use std::time::Duration;

    if codes.is_empty() {
        return Ok(Vec::new());
    }
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| CoreError::Io(io::Error::other(format!("构建 HTTP 客户端失败: {error}"))))?;

    let mut out = Vec::new();
    for batch in tencent::snapshot_batches(codes) {
        match tencent_quotes(&client, &batch) {
            Ok(quotes) => out.extend(quotes),
            Err(error) => {
                eprintln!("腾讯快照不可用，改用新浪: {error}");
                // 新浪的批量上限比腾讯小，切过去时要按新浪的约束重新分批。
                for sina_batch in sina::list_batches(&batch) {
                    out.extend(sina_quotes(&client, &sina_batch)?);
                }
            }
        }
    }
    Ok(out)
}

/// 走腾讯抓一批快照。
fn tencent_quotes(
    client: &reqwest::blocking::Client,
    batch: &[String],
) -> Result<Vec<(String, f64)>> {
    use crate::error::CoreError;
    use crate::market::tencent;
    use std::io;

    let response = client
        .get(tencent::snapshot_url(batch))
        .send()
        .map_err(|error| CoreError::Io(io::Error::other(format!("请求腾讯快照失败: {error}"))))?;
    let status = response.status();
    if !status.is_success() {
        return Err(CoreError::InvalidData(format!(
            "腾讯快照返回 HTTP {}",
            status.as_u16()
        )));
    }
    let bytes = response
        .bytes()
        .map_err(|error| CoreError::Io(io::Error::other(format!("读取腾讯快照失败: {error}"))))?;
    let (body, _, _) = encoding_rs::GB18030.decode(&bytes);
    Ok(tencent::parse_snapshots(&body)?
        .into_iter()
        .map(|snapshot| (snapshot.code, snapshot.last))
        .collect())
}

/// 走新浪抓一批快照。请求必须带 `sina.com.cn` 域的 `Referer`，响应是 GB18030。
fn sina_quotes(
    client: &reqwest::blocking::Client,
    batch: &[String],
) -> Result<Vec<(String, f64)>> {
    use crate::error::CoreError;
    use crate::market::sina;
    use std::io;

    let response = client
        .get(sina::list_url(batch))
        .header(reqwest::header::REFERER, sina::REFERER)
        .send()
        .map_err(|error| CoreError::Io(io::Error::other(format!("请求新浪快照失败: {error}"))))?;
    let status = response.status();
    if !status.is_success() {
        return Err(CoreError::InvalidData(format!(
            "新浪快照返回 HTTP {}",
            status.as_u16()
        )));
    }
    let bytes = response
        .bytes()
        .map_err(|error| CoreError::Io(io::Error::other(format!("读取新浪快照失败: {error}"))))?;
    Ok(sina::parse_snapshots(&sina::decode(&bytes))?
        .into_iter()
        .map(|snapshot| (snapshot.code, snapshot.last))
        .collect())
}

/// 拼一行持仓视图。
fn build_view(
    conn: &rusqlite::Connection,
    row: &Position,
    calendar: &[String],
    signal_date: &str,
    quote: Option<f64>,
) -> Result<PositionView> {
    let plan = trade_plan::get(conn, &row.plan_id)?;
    let per_r = plan
        .as_ref()
        .map(|plan| plan.per_r)
        .unwrap_or_else(|| row.entry_price - row.current_stop);
    let t3 = plan
        .as_ref()
        .map(|plan| plan.t3)
        .unwrap_or_else(|| row.entry_price + 3.0 * per_r);
    let t5 = plan
        .as_ref()
        .map(|plan| plan.t5)
        .unwrap_or_else(|| row.entry_price + 5.0 * per_r);
    let bars = bar::list_for_code(conn, &row.code)?;
    let ema9 = ema9_of(&bars);
    let last = quote.or_else(|| latest_close(&bars));
    let alerts = evaluate(
        row.current_stop,
        t3,
        t5,
        last,
        ema9,
        row.follow_ema,
        calendar,
        &row.entry_date,
        signal_date,
    );
    Ok(PositionView {
        id: row.id.clone(),
        plan_id: row.plan_id.clone(),
        code: row.code.clone(),
        name: stock::get(conn, &row.code)?.map(|stock| stock.name),
        entry_date: row.entry_date.clone(),
        entry_price: row.entry_price,
        stop: row.current_stop,
        quantity: row.quantity,
        last,
        ema9,
        per_r,
        t3,
        t5,
        follow_ema: row.follow_ema,
        alerts,
    })
}

/// 填实际成交价，平掉一条持仓并写出场记录。
///
/// 出场记录的 R 倍数与盈亏按计划的每股风险算；计划的每股风险拿不到时用买入价与止损价之差。
pub fn close_position(
    conn: &mut rusqlite::Connection,
    id: &str,
    exit_price: f64,
    exit_date: &str,
) -> Result<crate::storage::trade_log::TradeLogEntry> {
    let row = position::get(conn, id)?
        .ok_or_else(|| crate::error::CoreError::InvalidData(format!("找不到持仓 {id}")))?;
    if !exit_price.is_finite() || exit_price <= 0.0 {
        return Err(crate::error::CoreError::InvalidData(
            "实际成交价必须是正数".to_string(),
        ));
    }
    let timestamp = crate::storage::now();
    position::close(conn, id, exit_price, exit_date, &timestamp)?;

    let plan = trade_plan::get(conn, &row.plan_id)?;
    let per_r = plan
        .as_ref()
        .map(|plan| plan.per_r)
        .unwrap_or_else(|| row.entry_price - row.current_stop);
    let r_multiple = crate::sizing::r_multiple(exit_price, row.entry_price, per_r);
    let pnl = (exit_price - row.entry_price) * row.quantity as f64;

    let existing = crate::storage::trade_log::list(conn)?
        .into_iter()
        .find(|entry| entry.position_id.as_deref() == Some(id));
    let entry = crate::storage::trade_log::TradeLogEntry {
        id: existing
            .as_ref()
            .map(|entry| entry.id.clone())
            .unwrap_or_else(|| format!("log-{}", row.plan_id)),
        position_id: Some(row.id.clone()),
        code: row.code.clone(),
        name: stock::get(conn, &row.code)?.map(|stock| stock.name),
        entry_date: row.entry_date.clone(),
        entry_price: row.entry_price,
        stop_price: row.current_stop,
        exit_date: Some(exit_date.to_string()),
        exit_price: Some(exit_price),
        quantity: row.quantity,
        per_r: Some(per_r),
        r_multiple: Some(r_multiple),
        pnl: Some(pnl),
        fees: existing.as_ref().map(|entry| entry.fees).unwrap_or(0.0),
        note: existing.as_ref().and_then(|entry| entry.note.clone()),
        source: existing
            .as_ref()
            .map(|entry| entry.source.clone())
            .unwrap_or_else(|| "plan".to_string()),
        created_at: existing
            .as_ref()
            .map(|entry| entry.created_at.clone())
            .unwrap_or_else(|| timestamp.clone()),
    };
    crate::storage::trade_log::upsert(conn, &entry)?;
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 一串连续的交易日，跳过周末。
    fn calendar() -> Vec<String> {
        [
            "2026-10-09",
            "2026-10-12",
            "2026-10-13",
            "2026-10-14",
            "2026-10-15",
            "2026-10-16",
            "2026-10-19",
        ]
        .iter()
        .map(|day| day.to_string())
        .collect()
    }

    #[test]
    fn next_trading_day_follows_the_calendar() {
        let days = calendar();
        assert_eq!(next_trading_day(&days, "2026-10-09"), "2026-10-12");
        assert_eq!(next_trading_day(&days, "2026-10-13"), "2026-10-14");
    }

    #[test]
    fn next_trading_day_falls_back_to_the_next_weekday() {
        // 日历里没有更晚的交易日时退回下一个工作日：10-16 是周五，下一个是周一。
        let days = calendar();
        assert_eq!(next_trading_day(&days, "2026-10-19"), "2026-10-20");
        assert_eq!(next_trading_day(&days, "2026-10-17"), "2026-10-19");
    }

    #[test]
    fn executable_date_is_never_before_the_next_trading_day() {
        let days = calendar();
        // 当天买入当天出信号：可执行日推到次一交易日。
        assert_eq!(
            executable_date(&days, "2026-10-13", "2026-10-13"),
            "2026-10-14"
        );
        // 信号日已经晚于次一交易日时用信号日本身。
        assert_eq!(
            executable_date(&days, "2026-10-13", "2026-10-16"),
            "2026-10-16"
        );
    }

    #[test]
    fn stop_loss_fires_at_or_below_the_stop() {
        let days = calendar();
        let alerts = evaluate(
            19.5,
            21.5,
            22.5,
            Some(19.5),
            Some(19.8),
            true,
            &days,
            "2026-10-13",
            "2026-10-14",
        );
        assert!(alerts.iter().any(|alert| alert.kind == AlertKind::StopLoss));
        let stop = alerts
            .iter()
            .find(|alert| alert.kind == AlertKind::StopLoss)
            .unwrap();
        assert_eq!(stop.message, STOP_LOSS_MESSAGE);
        assert_eq!(stop.executable_date, "2026-10-14");
    }

    #[test]
    fn below_ema9_only_fires_when_the_remainder_follows_the_ema() {
        let days = calendar();
        let with_follow = evaluate(
            19.0,
            21.5,
            22.5,
            Some(19.8),
            Some(19.9),
            true,
            &days,
            "2026-10-13",
            "2026-10-14",
        );
        assert!(with_follow.iter().any(|alert| alert.kind == AlertKind::BelowEma9));

        let without_follow = evaluate(
            19.0,
            21.5,
            22.5,
            Some(19.8),
            Some(19.9),
            false,
            &days,
            "2026-10-13",
            "2026-10-14",
        );
        assert!(without_follow.is_empty());
    }

    #[test]
    fn reaching_3r_and_5r_give_the_reduce_hints() {
        let days = calendar();
        let at_3r = evaluate(
            19.5,
            21.5,
            22.5,
            Some(21.5),
            Some(21.0),
            true,
            &days,
            "2026-10-13",
            "2026-10-14",
        );
        assert!(at_3r.iter().any(|alert| alert.kind == AlertKind::Reduce3R));
        assert!(!at_3r.iter().any(|alert| alert.kind == AlertKind::Reduce5R));

        let at_5r = evaluate(
            19.5,
            21.5,
            22.5,
            Some(22.5),
            Some(21.0),
            true,
            &days,
            "2026-10-13",
            "2026-10-14",
        );
        assert!(at_5r.iter().any(|alert| alert.kind == AlertKind::Reduce5R));
        assert!(!at_5r.iter().any(|alert| alert.kind == AlertKind::Reduce3R));
    }

    #[test]
    fn no_quote_means_no_alerts() {
        let days = calendar();
        let alerts = evaluate(
            19.5,
            21.5,
            22.5,
            None,
            None,
            true,
            &days,
            "2026-10-13",
            "2026-10-14",
        );
        assert!(alerts.is_empty());
    }
}

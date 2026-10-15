//! 交易日志：记录表、统计摘要、手动补录、费用模型与 CSV 导出。
//!
//! 统计只计已出场的交易。费用模型默认关闭；开启后逐笔扣减，扣减额写进记录的
//! `fees` 列，`pnl` 也按扣减后的金额算。费用口径随市场变，见 [`crate::fees`]。

use crate::error::{CoreError, Result};
use crate::fees;
use crate::market::Market;
use crate::storage::stock;
use crate::storage::trade_log::{self, TradeLogEntry};
use crate::storage::{now, Database};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// 统计口径的固定标注。
pub const STATS_NOTE: &str = "未计入交易成本与滑点";

/// 统计摘要。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalStats {
    /// 交易笔数。
    pub trades: u64,
    /// 盈利笔数。
    pub wins: u64,
    /// 胜率 = 盈利笔数 ÷ 总笔数。
    pub win_rate: f64,
    /// 平均 R 倍数。
    pub avg_r: f64,
    /// 总盈亏（本市场货币）。
    pub total_pnl: f64,
    /// 逐笔扣减的费用合计（本市场货币）。
    pub total_fees: f64,
    /// 统计口径的固定标注。
    pub note: String,
}

/// 交易日志页需要的全部数据。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalView {
    /// 全部记录，最新的在前；未出场的记录也在里面。
    pub entries: Vec<TradeLogEntry>,
    pub stats: JournalStats,
    pub fee_model_enabled: bool,
    pub commission_rate: f64,
}

/// 手动补录一笔历史交易。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualEntry {
    pub code: String,
    pub entry_date: String,
    pub entry_price: f64,
    pub stop_price: f64,
    pub exit_price: Option<f64>,
    pub exit_date: Option<String>,
    pub quantity: i64,
    pub note: Option<String>,
}

/// 按固定的列顺序导出 CSV。字段里有逗号或引号时按 CSV 规则转义。
pub fn export_csv(entries: &[TradeLogEntry]) -> String {
    let mut out = String::from(
        "代码,名称,入场日,买入价,止损价,出场日,出场价,股数,每股风险,R倍数,盈亏,费用,备注\n",
    );
    for entry in entries {
        let cells = [
            entry.code.clone(),
            entry.name.clone().unwrap_or_default(),
            entry.entry_date.clone(),
            format!("{:.2}", entry.entry_price),
            format!("{:.2}", entry.stop_price),
            entry.exit_date.clone().unwrap_or_default(),
            entry
                .exit_price
                .map(|value| format!("{value:.2}"))
                .unwrap_or_default(),
            entry.quantity.to_string(),
            entry
                .per_r
                .map(|value| format!("{value:.4}"))
                .unwrap_or_default(),
            entry
                .r_multiple
                .map(|value| format!("{value:.4}"))
                .unwrap_or_default(),
            entry
                .pnl
                .map(|value| format!("{value:.2}"))
                .unwrap_or_default(),
            format!("{:.2}", entry.fees),
            entry.note.clone().unwrap_or_default(),
        ];
        out.push_str(
            &cells
                .iter()
                .map(|cell| escape_csv(cell))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push('\n');
    }
    out
}

/// 字段里有逗号、引号或换行时用双引号包起来，内部的引号翻倍。
fn escape_csv(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// 算统计摘要。只计已出场的记录。
pub fn stats(entries: &[TradeLogEntry]) -> JournalStats {
    let closed: Vec<&TradeLogEntry> = entries
        .iter()
        .filter(|entry| entry.exit_date.is_some() && entry.r_multiple.is_some())
        .collect();
    let trades = closed.len() as u64;
    if trades == 0 {
        return JournalStats {
            trades: 0,
            wins: 0,
            win_rate: 0.0,
            avg_r: 0.0,
            total_pnl: 0.0,
            total_fees: 0.0,
            note: STATS_NOTE.to_string(),
        };
    }
    let wins = closed
        .iter()
        .filter(|entry| entry.r_multiple.unwrap_or(0.0) > 0.0)
        .count() as u64;
    let sum_r: f64 = closed.iter().filter_map(|entry| entry.r_multiple).sum();
    let total_pnl: f64 = closed.iter().filter_map(|entry| entry.pnl).sum();
    let total_fees: f64 = closed.iter().map(|entry| entry.fees).sum();
    JournalStats {
        trades,
        wins,
        win_rate: wins as f64 / trades as f64,
        avg_r: sum_r / trades as f64,
        total_pnl,
        total_fees,
        note: STATS_NOTE.to_string(),
    }
}

/// 读交易日志页需要的全部数据。
///
/// 费用模型关闭时不改任何数字；开启时按当前费率逐笔重算 `fees` 与扣减后的 `pnl`，
/// 重算结果写回库里，这样导出与统计用的是同一份数字。
pub fn view(db: &Database, market: Market, fee_model_enabled: bool, commission_rate: f64) -> Result<JournalView> {
    let conn = db.conn()?;
    let mut entries = trade_log::list(&conn)?;
    if fee_model_enabled {
        for entry in entries.iter_mut() {
            if entry.exit_price.is_none() {
                continue;
            }
            let exit = entry.exit_price.unwrap_or(entry.entry_price);
            let charged =
                fees::round_trip(entry.entry_price, exit, entry.quantity, commission_rate, market);
            let gross = (exit - entry.entry_price) * entry.quantity as f64;
            entry.fees = charged.total;
            entry.pnl = Some(gross - charged.total);
            trade_log::upsert(&conn, entry)?;
        }
    }
    let stats = stats(&entries);
    Ok(JournalView {
        entries,
        stats,
        fee_model_enabled,
        commission_rate,
    })
}

/// 手动补录一笔历史交易。出场价与出场日要给就都给，否则当成还没出场。
pub fn add_manual(conn: &Connection, input: &ManualEntry) -> Result<TradeLogEntry> {
    if input.code.trim().is_empty() {
        return Err(CoreError::InvalidData("标的不为空".to_string()));
    }
    if !(input.entry_price > 0.0) || !(input.stop_price > 0.0) {
        return Err(CoreError::InvalidData(
            "买入价与止损价必须是正数".to_string(),
        ));
    }
    if input.quantity <= 0 {
        return Err(CoreError::InvalidData("股数必须是正整数".to_string()));
    }
    match (&input.exit_price, &input.exit_date) {
        (Some(_), None) | (None, Some(_)) => {
            return Err(CoreError::InvalidData(
                "出场价与出场日要一起给，或者都不给".to_string(),
            ));
        }
        _ => {}
    }

    let per_r = input.entry_price - input.stop_price;
    let (r_multiple, pnl) = match input.exit_price {
        Some(exit) if per_r > 0.0 => (
            Some(crate::sizing::r_multiple(exit, input.entry_price, per_r)),
            Some((exit - input.entry_price) * input.quantity as f64),
        ),
        Some(exit) => (None, Some((exit - input.entry_price) * input.quantity as f64)),
        None => (None, None),
    };

    let entry = TradeLogEntry {
        id: format!("log-manual-{}-{}", input.code, millis()),
        position_id: None,
        code: input.code.clone(),
        name: stock::get(conn, &input.code)?.map(|row| row.name),
        entry_date: input.entry_date.clone(),
        entry_price: input.entry_price,
        stop_price: input.stop_price,
        exit_date: input.exit_date.clone(),
        exit_price: input.exit_price,
        quantity: input.quantity,
        per_r: Some(per_r),
        r_multiple,
        pnl,
        fees: 0.0,
        note: input.note.clone(),
        source: "manual".to_string(),
        created_at: now(),
    };
    trade_log::upsert(conn, &entry)?;
    Ok(entry)
}

/// 毫秒时间戳，用来拼补录记录的 id。
fn millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn closed(code: &str, r: f64, pnl: f64) -> TradeLogEntry {
        TradeLogEntry {
            id: format!("log-{code}"),
            position_id: None,
            code: code.to_string(),
            name: None,
            entry_date: "2026-10-01".to_string(),
            entry_price: 20.0,
            stop_price: 19.5,
            exit_date: Some("2026-10-10".to_string()),
            exit_price: Some(20.0 + r * 0.5),
            quantity: 1000,
            per_r: Some(0.5),
            r_multiple: Some(r),
            pnl: Some(pnl),
            fees: 0.0,
            note: None,
            source: "manual".to_string(),
            created_at: "2026-10-10T00:00:00Z".to_string(),
        }
    }

    fn open(code: &str) -> TradeLogEntry {
        TradeLogEntry {
            exit_date: None,
            exit_price: None,
            r_multiple: None,
            pnl: None,
            ..closed(code, 0.0, 0.0)
        }
    }

    #[test]
    fn statistics_only_count_closed_trades() {
        let entries = vec![closed("600001", 3.7, 8200.0), closed("600002", -1.0, -2400.0), open("600003")];
        let stats = stats(&entries);
        assert_eq!(stats.trades, 2);
        assert_eq!(stats.wins, 1);
        assert_eq!(stats.win_rate, 0.5);
        assert!((stats.avg_r - 1.35).abs() < 1e-9);
        assert!((stats.total_pnl - 5800.0).abs() < 1e-9);
        assert_eq!(stats.note, STATS_NOTE);
    }

    #[test]
    fn an_empty_journal_has_zeroed_statistics() {
        let stats = stats(&[]);
        assert_eq!(stats.trades, 0);
        assert_eq!(stats.win_rate, 0.0);
        assert_eq!(stats.note, STATS_NOTE);
    }

    #[test]
    fn csv_has_a_header_and_one_line_per_entry() {
        let entries = vec![closed("600001", 3.0, 1500.0), open("600002")];
        let csv = export_csv(&entries);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("代码,名称,入场日,买入价,止损价,出场日,出场价,股数"));
        assert!(lines[1].starts_with("600001,,2026-10-01,20.00,19.50,2026-10-10,21.50,1000,0.5000,3.0000,1500.00,0.00,"));
        // 未出场的记录里出场日与出场价是空的。
        assert!(lines[2].starts_with("600002,,2026-10-01,20.00,19.50,,,1000"));
    }

    #[test]
    fn csv_escapes_commas_and_quotes() {
        let mut entry = closed("600001", 1.0, 100.0);
        entry.note = Some("到 3R 减仓，剩余跟随\"9 日均线\"".to_string());
        let csv = export_csv(&[entry]);
        assert!(csv.contains("\"到 3R 减仓，剩余跟随\"\"9 日均线\"\"\""));
    }
}

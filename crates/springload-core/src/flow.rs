//! 四步解锁判定：读 `watchlist_item`、`support_zone`、`signal` 三张表。
//!
//! 第 1 步永远可进；第 2 步需观察名单非空；第 3 步需观察名单非空且该股有高置信支撑区；
//! 第 4 步再加该股已有触发信号。未解锁的步骤带上该步的阻止提示文案。

use crate::error::Result;
use crate::glossary;
use crate::storage::{signal, support_zone, watchlist};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// `step_unlock` 命令的返回：四步是否解锁，以及各自的阻止原因。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepUnlock {
    pub unlocked: [bool; 4],
    pub reasons: [String; 4],
}

/// 判定某只股票当前已解锁的步骤。
///
/// `code` 为空时按「没有选中股票」处理：第 3、4 步都不可进。
pub fn step_unlock(conn: &Connection, code: &str) -> Result<StepUnlock> {
    let watchlist_non_empty = !watchlist::list(conn)?.is_empty();
    let has_high_zone = support_zone::list_for_code(conn, code)?
        .iter()
        .any(|zone| zone.confidence == "高");
    let has_signal = signal::list_for_code(conn, code)?
        .iter()
        .any(|row| row.status == "triggered");

    let unlocked = [
        true,
        watchlist_non_empty,
        watchlist_non_empty && has_high_zone,
        watchlist_non_empty && has_high_zone && has_signal,
    ];

    let steps = glossary::steps();
    let mut reasons: [String; 4] = Default::default();
    for index in 1..4 {
        if !unlocked[index] {
            reasons[index] = steps
                .get(index)
                .map(|step| step.blocked_hint.clone())
                .unwrap_or_default();
        }
    }

    Ok(StepUnlock { unlocked, reasons })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::migrations;
    use crate::storage::signal::Signal;
    use crate::storage::support_zone::SupportZone;
    use crate::storage::watchlist::WatchlistItem;
    use rusqlite::Connection;

    fn conn() -> Connection {
        let mut conn = Connection::open_in_memory().expect("内存库");
        migrations::run(&mut conn).expect("建表");
        conn
    }

    fn add_to_watchlist(conn: &Connection, code: &str) {
        watchlist::upsert(
            conn,
            &WatchlistItem {
                code: code.to_string(),
                added_at: "2026-10-13T09:00:00Z".to_string(),
                c1: true,
                c2: true,
                c3: true,
                c4: false,
                chg_1m: Some(0.2),
                chg_3m: Some(0.4),
                chg_6m: Some(0.5),
                rank_pct: Some(0.25),
                avg_amount: Some(2e8),
                convergence: Some(0.1),
                screen_run_at: "2026-10-13T09:00:00Z".to_string(),
            },
        )
        .expect("写观察名单");
    }

    fn add_zone(conn: &Connection, code: &str, confidence: &str) {
        support_zone::upsert(
            conn,
            &SupportZone {
                id: format!("{code}-{confidence}"),
                code: code.to_string(),
                kind: "support".to_string(),
                low: 10.0,
                high: 10.5,
                anchor_date: None,
                ema9: None,
                ema21: None,
                ema50: None,
                avwap_start_date: None,
                avwap_price: None,
                prev_high: None,
                prev_low: None,
                gap_top: None,
                gap_bottom: None,
                confidence: confidence.to_string(),
                spread: Some(0.01),
                created_at: "2026-10-13T09:00:00Z".to_string(),
            },
        )
        .expect("写支撑区");
    }

    fn add_signal(conn: &Connection, code: &str, status: &str) {
        signal::upsert(
            conn,
            &Signal {
                id: format!("{code}-{status}"),
                code: code.to_string(),
                triggered_at: "2026-10-13 14:32".to_string(),
                timeframe: "1m".to_string(),
                candle_high_broken: 10.5,
                pre_candle_high: 10.4,
                zone_id: None,
                status: status.to_string(),
                created_at: "2026-10-13T09:00:00Z".to_string(),
            },
        )
        .expect("写信号");
    }

    #[test]
    fn first_step_is_always_unlocked() {
        let conn = conn();
        let unlock = step_unlock(&conn, "600000").unwrap();
        assert!(unlock.unlocked[0]);
        assert_eq!(unlock.reasons[0], "");
    }

    #[test]
    fn second_step_needs_a_non_empty_watchlist() {
        let conn = conn();
        let empty = step_unlock(&conn, "600000").unwrap();
        assert!(!empty.unlocked[1]);
        assert!(empty.reasons[1].contains("第 1 步"));

        add_to_watchlist(&conn, "600000");
        let filled = step_unlock(&conn, "600000").unwrap();
        assert!(filled.unlocked[1]);
        assert_eq!(filled.reasons[1], "");
    }

    #[test]
    fn third_step_needs_a_high_confidence_zone_for_that_stock() {
        let conn = conn();
        add_to_watchlist(&conn, "600000");
        add_zone(&conn, "600000", "中");

        let medium = step_unlock(&conn, "600000").unwrap();
        assert!(!medium.unlocked[2], "中置信支撑区不解锁第 3 步");
        assert!(medium.reasons[2].contains("高置信"));

        // 别的股票有高置信区，不影响这只。
        add_zone(&conn, "600001", "高");
        assert!(!step_unlock(&conn, "600000").unwrap().unlocked[2]);

        add_zone(&conn, "600000", "高");
        assert!(step_unlock(&conn, "600000").unwrap().unlocked[2]);
    }

    #[test]
    fn fourth_step_needs_a_triggered_signal() {
        let conn = conn();
        add_to_watchlist(&conn, "600000");
        add_zone(&conn, "600000", "高");
        add_signal(&conn, "600000", "waiting");

        let waiting = step_unlock(&conn, "600000").unwrap();
        assert!(!waiting.unlocked[3], "等待中的信号不解锁第 4 步");

        add_signal(&conn, "600000", "triggered");
        let triggered = step_unlock(&conn, "600000").unwrap();
        assert!(triggered.unlocked[3]);
        assert_eq!(triggered.reasons[3], "");
    }

    #[test]
    fn empty_code_keeps_steps_three_and_four_locked() {
        let conn = conn();
        add_to_watchlist(&conn, "600000");
        add_zone(&conn, "600000", "高");
        add_signal(&conn, "600000", "triggered");

        let unlock = step_unlock(&conn, "").unwrap();
        assert!(unlock.unlocked[1], "观察名单非空时第 2 步可进");
        assert!(!unlock.unlocked[2], "没有选中股票时第 3 步不可进");
        assert!(!unlock.unlocked[3], "没有选中股票时第 4 步不可进");
    }
}

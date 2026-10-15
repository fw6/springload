//! 仓位计算与减仓股数。
//!
//! 纯计算，不碰数据库：`plan_calc` 命令与交易计划卡、持仓跟踪共用这里的函数。
//! 数值规则取自规格「仓位计算与目标位」一节。
//!
//! 买入股数按市场的最小申报单位向下取整：A 股一手 100 股，美股按整股。

use crate::error::{CoreError, Result};
use crate::market::Market;
use serde::{Deserialize, Serialize};

/// 止损距离的警告阈值：超过它给出显著警告，但不阻止提交。
pub const STOP_DISTANCE_WARN: f64 = 0.05;

/// `plan_calc` 的入参。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SizingInput {
    /// 买入价（本市场货币/股）。
    pub entry: f64,
    /// 止损价（本市场货币/股）。
    pub stop: f64,
    /// 账户金额（本市场货币）。
    pub account: f64,
    /// 单笔风险比例。
    pub risk_pct: f64,
}

/// `plan_calc` 的返回。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SizingResult {
    /// 每股风险 = 买入价 − 止损价。
    pub per_r: f64,
    /// 本单最大可亏金额 = 账户金额 × 单笔风险比例。
    pub max_loss: f64,
    /// 可买股数：向下取整到最小申报单位的整数倍。
    pub shares: i64,
    /// 投入金额 = 股数 × 买入价。
    pub capital: f64,
    /// 仓位占比 = 投入金额 ÷ 账户金额。
    pub pos_pct: f64,
    /// 止损距离 = 每股风险 ÷ 买入价。
    pub stop_distance: f64,
    /// 3R 目标位 = 买入价 + 3 × 每股风险。
    pub t3: f64,
    /// 5R 目标位 = 买入价 + 5 × 每股风险。
    pub t5: f64,
    /// 因最小申报单位取整而未能用满的风险金额。
    pub unused_risk: f64,
    /// 本市场的最小申报单位（股）。
    pub lot_size: i64,
}

/// 减仓安排：3R 与 5R 各减一部分，剩余跟随 9 日均线。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReducePlan {
    /// 3R 减仓股数。
    pub shares_3r: i64,
    /// 5R 减仓股数。
    pub shares_5r: i64,
    /// 剩余跟随 9 日均线的股数。
    pub remaining: i64,
}

/// 算仓位。买入价不高于止损价时返回错误。
pub fn size(input: &SizingInput, market: Market) -> Result<SizingResult> {
    if !(input.entry > input.stop) {
        return Err(CoreError::InvalidData(
            "买入价必须高于止损价，请检查输入".to_string(),
        ));
    }
    let lot = market.lot_size();
    let per_r = input.entry - input.stop;
    let max_loss = input.account * input.risk_pct;
    let raw_shares = (max_loss / per_r).floor();
    let shares = if raw_shares.is_finite() && raw_shares > 0.0 {
        (raw_shares as i64 / lot) * lot
    } else {
        0
    };
    let capital = shares as f64 * input.entry;
    Ok(SizingResult {
        per_r,
        max_loss,
        shares,
        capital,
        pos_pct: if input.account == 0.0 {
            0.0
        } else {
            capital / input.account
        },
        stop_distance: per_r / input.entry,
        t3: input.entry + 3.0 * per_r,
        t5: input.entry + 5.0 * per_r,
        unused_risk: max_loss - shares as f64 * per_r,
        lot_size: lot,
    })
}

/// 按 3R 与 5R 各减 `floor(shares ÷ 3)` 股算减仓股数，剩余跟随 9 日均线。
/// 按整股取整，不按手取整：卖出申报不受 100 股整数倍限制。
pub fn reduce_shares(shares: i64) -> ReducePlan {
    let each = shares.max(0) / 3;
    ReducePlan {
        shares_3r: each,
        shares_5r: each,
        remaining: shares.max(0) - 2 * each,
    }
}

/// 按出场价算 R 倍数：(出场价 − 买入价) ÷ per_r。
///
/// `per_r` 必须为正；出场价等于止损价时结果为 −1。
pub fn r_multiple(exit_price: f64, entry: f64, per_r: f64) -> f64 {
    (exit_price - entry) / per_r
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 规格里的示例：账户 100,000，entry 20.00、stop 19.50。
    fn example() -> SizingInput {
        SizingInput {
            entry: 20.00,
            stop: 19.50,
            account: 100_000.0,
            risk_pct: 0.005,
        }
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "期望 {expected}，实际 {actual}"
        );
    }

    #[test]
    fn example_numbers_match_spec() {
        let result = size(&example(), Market::Cn).expect("示例输入合法");
        assert_close(result.per_r, 0.50);
        assert_close(result.max_loss, 500.0);
        assert_eq!(result.shares, 1000);
        assert_close(result.capital, 20_000.0);
        assert_close(result.pos_pct, 0.20);
        assert_close(result.stop_distance, 0.025);
        assert_close(result.t3, 21.50);
        assert_close(result.t5, 22.50);
        assert_close(result.unused_risk, 0.0);
        assert_eq!(result.lot_size, 100);
    }

    #[test]
    fn rejects_entry_not_above_stop() {
        let mut input = example();
        input.entry = 19.50;
        input.stop = 19.50;
        assert!(size(&input, Market::Cn).is_err(), "买入价等于止损价应报错");

        input.entry = 19.00;
        assert!(size(&input, Market::Cn).is_err(), "买入价低于止损价应报错");
    }

    #[test]
    fn stop_distance_over_five_percent_is_reported_not_rejected() {
        // per_r = 1.10，止损距离 = 1.10 ÷ 20 = 5.5% > 5%。
        let input = SizingInput {
            entry: 20.00,
            stop: 18.90,
            account: 100_000.0,
            risk_pct: 0.005,
        };
        let result = size(&input, Market::Cn).expect("止损距离超阈值不阻止计算");
        assert!(result.stop_distance > STOP_DISTANCE_WARN);
        assert!(result.shares > 0);
    }

    #[test]
    fn zero_shares_when_budget_smaller_than_one_lot() {
        // A 股：最大可亏 5 元，每股风险 0.50 元 → 原始股数 10 股，取整到 100 股后为 0。
        let input = SizingInput {
            entry: 20.00,
            stop: 19.50,
            account: 1_000.0,
            risk_pct: 0.005,
        };
        let result = size(&input, Market::Cn).expect("输入合法，只是股数为 0");
        assert_eq!(result.shares, 0);
        assert_close(result.capital, 0.0);
        assert_close(result.pos_pct, 0.0);
        assert_close(result.unused_risk, 5.0);

        // 美股同一笔输入按整股取整，能买到 10 股。
        let us = size(&input, Market::Us).expect("输入合法");
        assert_eq!(us.shares, 10);
        assert_eq!(us.lot_size, 1);
        assert_close(us.capital, 200.0);
        assert_close(us.unused_risk, 0.0);
    }

    #[test]
    fn unused_risk_reflects_lot_rounding() {
        // A 股：最大可亏 375 元，每股风险 0.50 元 → 原始 750 股，取整到 700 股，
        // 未用满的风险 = 375 − 700 × 0.50 = 25 元。
        let input = SizingInput {
            entry: 20.00,
            stop: 19.50,
            account: 75_000.0,
            risk_pct: 0.005,
        };
        let result = size(&input, Market::Cn).expect("输入合法");
        assert_eq!(result.shares, 700);
        assert_close(result.unused_risk, 25.0);

        // 美股不按手取整，同一笔输入用满风险：750 股，未用满的风险为 0。
        let us = size(&input, Market::Us).expect("输入合法");
        assert_eq!(us.shares, 750);
        assert_close(us.unused_risk, 0.0);
    }

    #[test]
    fn reduce_shares_uses_whole_shares() {
        let plan = reduce_shares(1000);
        assert_eq!(plan.shares_3r, 333);
        assert_eq!(plan.shares_5r, 333);
        assert_eq!(plan.remaining, 334);

        let plan = reduce_shares(100);
        assert_eq!(plan.shares_3r, 33);
        assert_eq!(plan.shares_5r, 33);
        assert_eq!(plan.remaining, 34);
    }

    #[test]
    fn r_multiple_at_stop_is_minus_one() {
        // 出场价等于止损价 → (19.50 − 20.00) ÷ 0.50 = −1。
        assert_close(r_multiple(19.50, 20.00, 0.50), -1.0);
        assert_close(r_multiple(21.50, 20.00, 0.50), 3.0);
        assert_close(r_multiple(22.50, 20.00, 0.50), 5.0);
    }
}

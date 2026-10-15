//! 可选费用模型：开启后逐笔扣减。
//!
//! A 股的费率取自规格「交易日志与费用」一节：印花税、过户费、经手费、证管费与佣金，
//! 佣金含单笔最低 5 元，买卖两侧都收。
//!
//! 美股只收佣金，没有印花税与各项规费；佣金费率由配置给出，常见券商为零。

use crate::market::Market;
use serde::{Deserialize, Serialize};

/// 印花税：0.05%，仅卖出侧。
pub const STAMP_DUTY_RATE: f64 = 0.0005;
/// 过户费：0.01‰，双向。
pub const TRANSFER_FEE_RATE: f64 = 0.00001;
/// 交易所经手费：0.00341%，双向。
pub const HANDLING_FEE_RATE: f64 = 0.0000341;
/// 证管费：0.02‰，双向。
pub const SUPERVISION_FEE_RATE: f64 = 0.00002;
/// A 股佣金的单笔最低金额（元）。
pub const MIN_COMMISSION: f64 = 5.0;

/// 一侧的费用明细。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fees {
    /// 印花税，仅卖出侧不为零，只有 A 股收取。
    pub stamp_duty: f64,
    pub transfer_fee: f64,
    pub handling_fee: f64,
    pub supervision_fee: f64,
    pub commission: f64,
    pub total: f64,
}

impl Fees {
    /// 两侧相加。
    pub fn plus(self, other: Fees) -> Fees {
        Fees {
            stamp_duty: self.stamp_duty + other.stamp_duty,
            transfer_fee: self.transfer_fee + other.transfer_fee,
            handling_fee: self.handling_fee + other.handling_fee,
            supervision_fee: self.supervision_fee + other.supervision_fee,
            commission: self.commission + other.commission,
            total: self.total + other.total,
        }
    }
}

/// 买入侧的费用。成交额是价格 × 股数。
pub fn buy(amount: f64, commission_rate: f64, market: Market) -> Fees {
    match market {
        Market::Us => {
            let commission = amount * commission_rate;
            Fees {
                stamp_duty: 0.0,
                transfer_fee: 0.0,
                handling_fee: 0.0,
                supervision_fee: 0.0,
                commission,
                total: commission,
            }
        }
        Market::Cn => {
            let commission = (amount * commission_rate).max(MIN_COMMISSION);
            let transfer_fee = amount * TRANSFER_FEE_RATE;
            let handling_fee = amount * HANDLING_FEE_RATE;
            let supervision_fee = amount * SUPERVISION_FEE_RATE;
            Fees {
                stamp_duty: 0.0,
                transfer_fee,
                handling_fee,
                supervision_fee,
                commission,
                total: commission + transfer_fee + handling_fee + supervision_fee,
            }
        }
    }
}

/// 卖出侧的费用，A 股比买入侧多一道印花税。
pub fn sell(amount: f64, commission_rate: f64, market: Market) -> Fees {
    let base = buy(amount, commission_rate, market);
    let stamp_duty = match market {
        Market::Cn => amount * STAMP_DUTY_RATE,
        Market::Us => 0.0,
    };
    Fees {
        stamp_duty,
        total: base.total + stamp_duty,
        ..base
    }
}

/// 一买一卖的总费用。
pub fn round_trip(entry: f64, exit: f64, quantity: i64, commission_rate: f64, market: Market) -> Fees {
    let shares = quantity.max(0) as f64;
    buy(entry * shares, commission_rate, market).plus(sell(exit * shares, commission_rate, market))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "期望 {expected}，实际 {actual}"
        );
    }

    #[test]
    fn the_buy_side_has_no_stamp_duty() {
        let fees = buy(20_000.0, 0.00025, Market::Cn);
        close(fees.stamp_duty, 0.0);
        // 过户费 20000 × 0.00001 = 0.2；经手费 20000 × 0.0000341 = 0.682；
        // 证管费 20000 × 0.00002 = 0.4；佣金 20000 × 0.00025 = 5。
        close(fees.transfer_fee, 0.2);
        close(fees.handling_fee, 0.682);
        close(fees.supervision_fee, 0.4);
        close(fees.commission, 5.0);
        close(fees.total, 6.282);
    }

    #[test]
    fn the_sell_side_adds_the_stamp_duty() {
        let fees = sell(20_000.0, 0.00025, Market::Cn);
        // 印花税 20000 × 0.0005 = 10。
        close(fees.stamp_duty, 10.0);
        close(fees.total, 16.282);
    }

    #[test]
    fn commission_never_goes_below_five_yuan() {
        // 成交额 1000 元时按费率算只有 0.25 元，取最低 5 元。
        let fees = buy(1_000.0, 0.00025, Market::Cn);
        close(fees.commission, MIN_COMMISSION);
    }

    #[test]
    fn a_round_trip_sums_both_sides() {
        // 规格里的示例：20.00 买入、21.50 卖出、1000 股。
        let fees = round_trip(20.0, 21.5, 1000, 0.00025, Market::Cn);
        // 印花税只在卖出侧：21500 × 0.0005 = 10.75。
        close(fees.stamp_duty, 10.75);
        // 佣金两侧各按成交额算：5 + 5.375。
        close(fees.commission, 10.375);
        close(fees.total, 6.282 + 5.375 + 10.75 + 0.215 + 0.73315 + 0.43);
    }

    #[test]
    fn the_us_side_charges_commission_only() {
        // 默认费率为零的券商：一买一卖都不产生费用。
        let fees = round_trip(20.0, 21.5, 1000, 0.0, Market::Us);
        close(fees.total, 0.0);

        // 给出费率时只按成交额收佣金，没有印花税与各项规费，也没有最低佣金。
        let fees = round_trip(20.0, 21.5, 10, 0.001, Market::Us);
        close(fees.stamp_duty, 0.0);
        close(fees.transfer_fee, 0.0);
        close(fees.handling_fee, 0.0);
        close(fees.supervision_fee, 0.0);
        close(fees.commission, 0.2 + 0.215);
        close(fees.total, 0.415);
    }
}

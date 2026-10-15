//! 五段解释的提示词。
//!
//! 提示词由两部分组成：交给 Rig extractor 的 [`PREAMBLE`]（系统提示），以及按
//! 当前 scope 与上下文拼出的用户提示 [`build`]。上下文里只放已经发生的事实，
//! 提示词要求模型只描述这些事实，不出现判断词、不给买卖结论。金额一律带上当前
//! 市场的货币名，模型不会自己加。

use super::context::{
    ExplainContext, JournalFacts, Step1Facts, Step2Facts, Step3Facts, Step4Facts, ZoneFacts,
};
use super::Scope;
use crate::market::Market;

/// 交给 Rig extractor 的系统提示。
///
/// 末尾要求以 JSON 提交，配合 `additional_params` 里 DeepSeek 的 `json_object`
/// 响应格式（该模式要求提示词里出现 “JSON” 一词）。
pub const PREAMBLE: &str = "\
你是「蓄势」交易工作台里的解释器，只做一件事：把已经发生的行情形态与数据，用股票新手能看懂的白话复述一遍。

硬性要求：
- 只描述已经发生的形态与数据，不预测涨跌，不判断这只股票值不值得买。
- 不出现判断词与结论性措辞，例如「值得」「建议」「推荐」「看好」「机会」「应该买」「会涨」「必涨」「风险很大」。
- 只使用下面给出的事实与数字，不补充任何未给出的信息，不编造价格、日期或指标。
- 用中文，2 到 4 句话，直接给出解释正文，不加标题、不加免责声明。

把最终结果通过 submit 工具以 JSON 提交。";

/// 按 scope 拼出用户提示。
pub fn build(scope: Scope, context: &ExplainContext, market: Market) -> String {
    let currency = market.currency();
    let mut lines = Vec::new();
    lines.push(format!(
        "市场：{}，金额单位为{}。",
        market.label(),
        currency
    ));
    lines.push(format!(
        "股票：{} {}（数据截止日 {}）。",
        context.code, context.name, context.data_as_of
    ));
    match scope {
        Scope::Step1 => {
            lines.push(
                "任务：把下面 C1~C4 四条判定逐条翻成白话，说明这条为什么入选。"
                    .to_string(),
            );
            if let Some(facts) = &context.step1 {
                lines.extend(step1_facts(facts, currency));
            }
        }
        Scope::Step2 => {
            lines.push(
                "任务：描述图上的形态与支撑区的置信度，说明置信度是怎么算出来的。"
                    .to_string(),
            );
            if let Some(facts) = &context.step2 {
                lines.extend(step2_facts(facts));
            }
        }
        Scope::Step3 => {
            lines.push(
                "任务：说明买入信号卡在哪一级：日线的前置条件是否满足，分钟线的触发条件是否出现。"
                    .to_string(),
            );
            if let Some(facts) = &context.step3 {
                lines.extend(step3_facts(facts));
            }
        }
        Scope::Step4 => {
            lines.push(
                "任务：说明这一单的风险结构：亏多少、买多少股、仓位占比多少、目标位在哪。"
                    .to_string(),
            );
            if let Some(facts) = &context.step4 {
                lines.extend(step4_facts(facts, currency));
            }
        }
        Scope::Journal => {
            lines.push("任务：做一段复盘总结，说明交易记录里的统计与反复出现的形态。".to_string());
            if let Some(facts) = &context.journal {
                lines.extend(journal_facts(facts, currency));
            }
        }
    }
    lines.join("\n")
}

fn step1_facts(facts: &Step1Facts, currency: &str) -> Vec<String> {
    let mut lines = vec![
        format!("C1 涨幅条件：{}", verdict(facts.c1)),
        format!("C2 结构条件：{}", verdict(facts.c2)),
        format!("C3 均线条件：{}", verdict(facts.c3)),
        format!("C4 排除条件：{}", verdict(facts.c4)),
    ];
    if let Some(value) = facts.chg_6m {
        lines.push(format!("近 6 个月涨幅：{}", percent(value)));
    }
    if let Some(value) = facts.chg_3m {
        lines.push(format!("近 3 个月涨幅：{}", percent(value)));
    }
    if let Some(value) = facts.chg_1m {
        lines.push(format!("近 1 个月涨幅：{}", percent(value)));
    }
    if let Some(value) = facts.rank_pct {
        lines.push(format!("近 1 个月涨幅排名分位：{}", percent(value)));
    }
    if let Some(value) = facts.convergence {
        lines.push(format!("近 20 日极差：{}", percent(value)));
    }
    if let Some(value) = facts.avg_amount {
        lines.push(format!("日均成交额：{}", money(value, currency)));
    }
    if let Some(value) = facts.close {
        lines.push(format!("最新收盘价：{}", price(value)));
    }
    if let Some(value) = facts.ema9 {
        lines.push(format!("EMA9：{}", price(value)));
    }
    if let Some(value) = facts.ema21 {
        lines.push(format!("EMA21：{}", price(value)));
    }
    if let Some(value) = facts.recent_low {
        lines.push(format!("近 20 个交易日的最低价：{}", price(value)));
    }
    if let Some(value) = facts.prior_low {
        lines.push(format!("再往前 20 个交易日的最低价：{}", price(value)));
    }
    lines
}

fn step2_facts(facts: &Step2Facts) -> Vec<String> {
    let mut lines = vec![format!(
        "已标注 {} 条，其中高置信 {} 条。",
        facts.zones.len(),
        facts.high_confidence
    )];
    for zone in &facts.zones {
        lines.push(format!("标注：{}", zone_line(zone)));
    }
    lines
}

fn step3_facts(facts: &Step3Facts) -> Vec<String> {
    let mut lines = vec![format!("当前状态：{}", stage_label(&facts.stage))];
    match &facts.zone {
        Some(zone) => lines.push(format!("参照的支撑区：{}", zone_line(zone))),
        None => lines.push("还没有可参照的支撑区。".to_string()),
    }
    if let Some(timeframe) = &facts.timeframe {
        lines.push(format!("观察周期：{}", timeframe));
    }
    if let Some(value) = facts.pre_high {
        lines.push(format!("前一根 K 线最高价：{}", price(value)));
    }
    if let Some(value) = facts.broken_high {
        lines.push(format!("被突破的那一根最高价：{}", price(value)));
    }
    if let Some(value) = &facts.triggered_at {
        lines.push(format!("触发时间：{}", value));
    }
    lines
}

fn step4_facts(facts: &Step4Facts, currency: &str) -> Vec<String> {
    vec![
        format!("买入价：{}", price(facts.entry)),
        format!("止损价：{}", price(facts.stop)),
        format!("每股风险：{}", price(facts.per_r)),
        format!("账户金额：{}", money(facts.account, currency)),
        format!("单笔风险比例：{}", percent(facts.risk_pct)),
        format!("最大可亏金额：{}", money(facts.account * facts.risk_pct, currency)),
        format!("股数：{} 股", facts.shares),
        format!("投入金额：{}", money(facts.capital, currency)),
        format!("仓位占比：{}", percent(facts.pos_pct)),
        format!("止损距离：{}", percent(facts.stop_distance)),
        format!("3R 目标位：{}", price(facts.t3)),
        format!("5R 目标位：{}", price(facts.t5)),
        format!("计划状态：{}", facts.status),
        "减仓规则：到 3R 卖三分之一，到 5R 再卖三分之一，剩余三分之一跟随 9 日均线。"
            .to_string(),
    ]
}

fn journal_facts(facts: &JournalFacts, currency: &str) -> Vec<String> {
    let mut lines = vec![format!("已出场 {} 笔，其中盈利 {} 笔。", facts.trades, facts.wins)];
    if let Some(value) = facts.win_rate {
        lines.push(format!("胜率：{}", percent(value)));
    }
    if let Some(value) = facts.avg_r {
        lines.push(format!("平均 R 倍数：{}", number(value)));
    }
    if let Some(value) = facts.total_pnl {
        lines.push(format!("总盈亏：{} {}", number(value), currency));
    }
    lines.push("统计口径：未计入交易成本与滑点。".to_string());
    for trade in &facts.recent {
        let name = trade.name.clone().unwrap_or_else(|| trade.code.clone());
        let mut line = format!(
            "交易：{} {} 入场 {} {} {}，出场 {}",
            trade.code,
            name,
            price(trade.entry_price),
            currency,
            trade.entry_date,
            match (trade.exit_price, &trade.exit_date) {
                (Some(exit), Some(date)) => format!("{} {} {}", price(exit), currency, date),
                _ => "尚未填写".to_string(),
            }
        );
        if let Some(r) = trade.r_multiple {
            line.push_str(&format!("，R 倍数 {}", number(r)));
        }
        if let Some(pnl) = trade.pnl {
            line.push_str(&format!("，盈亏 {} {}", number(pnl), currency));
        }
        if let Some(note) = &trade.note {
            line.push_str(&format!("，备注 {}", note));
        }
        lines.push(line);
    }
    lines
}

fn zone_line(zone: &ZoneFacts) -> String {
    let mut parts = vec![
        format!("{}（{}）", zone.kind, zone.id),
        format!("下沿 {} 上沿 {}", price(zone.low), price(zone.high)),
        format!("置信度 {}", zone.confidence),
    ];
    if let Some(value) = zone.spread {
        parts.push(format!("离散度 {}", percent(value)));
    }
    if let Some(value) = zone.ema21 {
        parts.push(format!("EMA21 {}", price(value)));
    }
    if let Some(value) = zone.avwap {
        parts.push(format!("AVWAP {}", price(value)));
    }
    if let Some(value) = zone.prev_high {
        parts.push(format!("前期高点 {}", price(value)));
    }
    parts.join("，")
}

fn verdict(value: bool) -> &'static str {
    if value {
        "满足"
    } else {
        "不满足"
    }
}

fn stage_label(stage: &str) -> &'static str {
    match stage {
        "triggered" => "已触发",
        "failed" => "已失效",
        _ => "等待中",
    }
}

fn price(value: f64) -> String {
    format!("{value:.2}")
}

fn percent(value: f64) -> String {
    format!("{:.1}%", value * 100.0)
}

fn money(value: f64, currency: &str) -> String {
    format!("{value:.0} {currency}")
}

fn number(value: f64) -> String {
    format!("{value:.2}")
}

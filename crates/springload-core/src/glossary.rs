//! 十个术语与四步文案的唯一定稿来源。
//!
//! 术语的白话解释取自需求规格说明书第 2 章；四步文案取自原型
//! `prototype/src/data/glossary.ts`，并按规格第 10 条补足筛选条件与减仓比例。
//!
//! 术语表两个市场共用：回调买入法的定义与市场无关。四步文案里提到股票池范围与
//! 买入申报单位的两处按市场取不同的说法。

use crate::market::Market;
use serde::Serialize;

/// 一个术语：键、显示名与白话解释。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Term {
    pub key: String,
    pub label: String,
    pub plain: String,
}

/// 一步的文案：标题、目标、一句话说明、完整说明、关键术语与前序未完成时的阻止提示。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepCopy {
    pub index: u8,
    pub title: String,
    pub goal: String,
    /// 一句话说明这一步在干什么。
    pub one_line: String,
    /// 展开后的完整说明。
    pub detail: String,
    /// 本步的关键术语。
    pub term_keys: Vec<String>,
    /// 前序未完成时的阻止提示。
    pub blocked_hint: String,
}

/// `glossary` 命令的返回：全部术语与全部步骤文案。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Glossary {
    pub terms: Vec<Term>,
    pub steps: Vec<StepCopy>,
}

fn term(key: &str, label: &str, plain: &str) -> Term {
    Term {
        key: key.to_string(),
        label: label.to_string(),
        plain: plain.to_string(),
    }
}

/// 十个术语，顺序与需求文档第 2 章一致。
pub fn terms() -> Vec<Term> {
    vec![
        term(
            "pullback",
            "回调买入法",
            "股票涨过一波后休息（横盘），等它再次走强时上车，而不是追着已经涨高的价格买。",
        ),
        term(
            "ema",
            "EMA 9/21/50",
            "最近 9/21/50 天股价的平均成本线。9 日看短线、21 日看中短线、50 日看中期。",
        ),
        term(
            "support",
            "支撑区",
            "股价跌到这个价格附近就容易跌不动的区域，通常叠在均线、前期高点、AVWAP 附近。",
        ),
        term(
            "gap",
            "跳空缺口",
            "昨天收盘价与今天开盘价之间那段没有成交的价格区间，常成为支撑/压力。",
        ),
        term(
            "avwap",
            "AVWAP",
            "从某个指定起点开始，按成交量加权算出的平均成本价，约等于那段时间参与者的平均成本。价格回到这里附近常获得支撑。",
        ),
        term(
            "firstRetest",
            "第一次回测",
            "股价刚站上某个关键位，回头又踩它一脚。踩住 = 支撑有效，是这套策略最看重的触发场景。",
        ),
        term(
            "r",
            "R（风险单位）",
            "这一单预设的最大每股亏损额（买入价 − 止损价）。全程用它衡量盈亏，不用百分比。",
        ),
        term(
            "target",
            "3R / 5R 目标位",
            "盈利达到 3 倍或 5 倍初始风险的价位。3R 卖出一部分，5R 再卖一部分。",
        ),
        term(
            "slippage",
            "滑点",
            "按下买入那一刻，实际成交价比看到的价格差了一点。会导致实际亏损大于预设值。",
        ),
        term(
            "trialCost",
            "试错成本",
            "这单如果错了，会亏掉的钱。策略的核心是用极小的试错成本换大段上涨。",
        ),
    ]
}

/// 股票池范围的一句话描述。
fn universe(market: Market) -> &'static str {
    match market {
        Market::Cn => "沪深主板",
        Market::Us => "纳斯达克、纽交所与美交所的普通股与存托凭证",
    }
}

/// 买入申报单位的一句话描述。
fn lot_rule(market: Market) -> &'static str {
    match market {
        Market::Cn => "A 股按手交易，股数需要向下取整到 100 股的整数倍。",
        Market::Us => "美股按整股交易，股数向下取整到整股。",
    }
}

/// 四步文案。
pub fn steps(market: Market) -> Vec<StepCopy> {
    vec![
        StepCopy {
            index: 1,
            title: "强势股筛选".to_string(),
            goal: "先找出近期走得强、现在正在休息的股票".to_string(),
            one_line: format!(
                "从{}里筛出「涨过一波、现在横着」的股票，得到一张观察名单。",
                universe(market)
            ),
            detail: "筛选器只看四件事：近 6 个月有没有涨够；近 1 个月的涨幅排名落不落得进前 20%~30% 这一段；回调的低点是不是一次比一次高、近 20 日的波动是不是在收窄；收盘价有没有站在 9 日均线上、并且 EMA9 与 EMA21 走平或向上。四条同时满足才会进观察名单；如果高点与低点均下降且收盘价 < 向下的 EMA21，这只股票会被排除。这一步不判断这只股票值不值得买，只判断它是不是符合回调蓄势的形态。"
                .to_string(),
            term_keys: vec!["pullback".to_string(), "ema".to_string()],
            blocked_hint: String::new(),
        },
        StepCopy {
            index: 2,
            title: "标注支撑区".to_string(),
            goal: "在图上画出跌不动的那条线，看它有多可靠".to_string(),
            one_line: "在 K 线图上拖出水平线和支撑区，App 会算出这块区域的置信度。".to_string(),
            detail: "支撑区就是股价跌到这个位置附近就容易跌不动的地方。图上可以放水平线（前期高点或低点）、框出支撑区、标出跳空缺口，也可以指定 AVWAP 的起点。当 AVWAP、21 日均线、前期高点这三个价格落在 2% 以内时，这块支撑区会被标成高置信。"
                .to_string(),
            term_keys: vec![
                "support".to_string(),
                "ema".to_string(),
                "avwap".to_string(),
                "gap".to_string(),
            ],
            blocked_hint: "先在第 1 步把股票加进观察名单，再来标注支撑区。".to_string(),
        },
        StepCopy {
            index: 3,
            title: "等待买入信号".to_string(),
            goal: "盯着它回到支撑区之后有没有重新走强".to_string(),
            one_line: "价格站上高置信支撑区后，分钟图上出现「下探回升 + 突破前一根高点」才算触发。"
                .to_string(),
            detail: "这一步分两级。日线上，价格必须先站上高置信支撑区；分钟图上，要先出现一次下探后回升，随后某一根 K 线的最高价超过前一根 K 线的最高价。两个条件缺一不可。如果价格直接跌穿支撑区没有回升，App 保持等待，不发出任何提示。"
                .to_string(),
            term_keys: vec!["firstRetest".to_string(), "support".to_string()],
            blocked_hint: "先在第 2 步完成支撑区标注，并让支撑区达到高置信。".to_string(),
        },
        StepCopy {
            index: 4,
            title: "算仓位与目标位".to_string(),
            goal: "把「最多亏多少」换算成「买多少股」".to_string(),
            one_line:
                "输入买入价、止损价、账户金额和单笔风险比例，算出股数、投入金额与 3R/5R 目标位。"
                    .to_string(),
            detail: format!(
                "每股风险 = 买入价 − 止损价。本单最大可亏金额 = 账户金额 × 单笔风险比例。可买股数 = 最大可亏金额 ÷ 每股风险，向下取整。3R 与 5R 目标位分别是买入价加上 3 倍和 5 倍的每股风险。{}到达目标位后分批减仓：3R 卖三分之一、5R 再卖三分之一、剩余三分之一跟随 9 日均线。",
                lot_rule(market)
            ),
            term_keys: vec![
                "r".to_string(),
                "target".to_string(),
                "trialCost".to_string(),
                "slippage".to_string(),
            ],
            blocked_hint: "先在第 3 步等到买入信号，再来算仓位。".to_string(),
        },
    ]
}

/// 组装 `glossary` 命令的完整返回。
pub fn glossary(market: Market) -> Glossary {
    Glossary {
        terms: terms(),
        steps: steps(market),
    }
}

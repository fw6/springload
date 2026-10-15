//! `glossary` 模块：十个术语与四步文案的定稿内容。

use springload_core::market::Market;
use springload_core::glossary;
use std::collections::HashSet;

#[test]
fn ten_terms_in_requirement_order() {
    let terms = glossary::terms();
    let keys: Vec<&str> = terms.iter().map(|t| t.key.as_str()).collect();
    assert_eq!(
        keys,
        vec![
            "pullback",
            "ema",
            "support",
            "gap",
            "avwap",
            "firstRetest",
            "r",
            "target",
            "slippage",
            "trialCost",
        ]
    );
    assert_eq!(terms[0].label, "回调买入法");
    assert!(terms[0].plain.contains("休息"));
    assert_eq!(terms[6].label, "R（风险单位）");
}

#[test]
fn four_steps_with_titles_and_goals() {
    let steps = glossary::steps(Market::Cn);
    assert_eq!(steps.len(), 4);
    let titles: Vec<&str> = steps.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, vec!["强势股筛选", "标注支撑区", "等待买入信号", "算仓位与目标位"]);
    assert_eq!(steps[0].index, 1);
    assert_eq!(steps[3].index, 4);
    assert!(steps[0].blocked_hint.is_empty());
    assert!(steps[1].blocked_hint.contains("观察名单"));
    assert!(steps[2].blocked_hint.contains("高置信"));
    assert!(steps[3].blocked_hint.contains("买入信号"));
}

#[test]
fn step_one_detail_states_the_rank_band() {
    let detail = &glossary::steps(Market::Cn)[0].detail;
    assert!(detail.contains("前 20%~30%"), "第 1 步要明说排名区间: {detail}");
}

#[test]
fn step_one_detail_covers_c3_and_c4() {
    let detail = &glossary::steps(Market::Cn)[0].detail;
    assert!(detail.contains("EMA9 与 EMA21 走平或向上"), "第 1 步要补上 C3 后半段: {detail}");
    assert!(
        detail.contains("高点与低点均下降且收盘价 < 向下的 EMA21"),
        "第 1 步要补上 C4 排除条件: {detail}"
    );
}

#[test]
fn step_four_detail_states_the_trim_ratios() {
    let detail = &glossary::steps(Market::Cn)[3].detail;
    assert!(detail.contains("3R 卖三分之一"), "第 4 步要补上减仓比例: {detail}");
    assert!(detail.contains("5R 再卖三分之一"));
    assert!(detail.contains("剩余三分之一跟随 9 日均线"));
}

#[test]
fn every_referenced_term_key_exists() {
    let terms = glossary::terms();
    let known: HashSet<&str> = terms.iter().map(|t| t.key.as_str()).collect();
    for step in glossary::steps(Market::Cn) {
        for key in &step.term_keys {
            assert!(known.contains(key.as_str()), "第 {} 步引用了未定义的术语 {key}", step.index);
        }
    }
}

#[test]
fn glossary_bundles_terms_and_steps() {
    let g = glossary::glossary(Market::Cn);
    assert_eq!(g.terms.len(), 10);
    assert_eq!(g.steps.len(), 4);
}

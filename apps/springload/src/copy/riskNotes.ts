/**
 * 需求规格说明书第 8 章的风险提示定稿文案，以及按市场取的那几句交易制度说明。
 * 首次启动确认弹窗、报尾、设置页与持仓页都从这里取，不各自另写一份。
 */

import type { Market } from "../api/app";

export const riskNotes = {
  highReturn:
    "原文所述 969.8% 为 2025 年单场赛事的 12 个月战绩，不是年化收益率，不代表可持续收益。",
  market: "策略收益依赖市场环境，在趋势走坏年份会连续触发止损。",
  /** A 股的交易制度提示，首次启动弹窗用。 */
  tPlusOne:
    "A 股存在涨跌停与 T+1 制度，当日买入无法当日卖出，止损单可能无法成交导致实际亏损远超预设。",
  /** 美股的交易制度提示，首次启动弹窗用。 */
  usSession:
    "美股当日买入当日即可卖出，但盘前盘后与熔断期间的成交价可能远离计划，跳空同样会让止损失效。",
  slippage: "跳空或滑点会使实际成交价劣于预期，亏损可能超过预设的账户风险。",
  costFree: "统计未计入交易成本与滑点。",
};

/** 报尾与设置页那一行：一句话说清这个市场的交易制度。 */
export function tradingRule(market: Market): string {
  return market === "us"
    ? "美股 T+0，当日可卖；盘前盘后与熔断期间的成交价可能远离计划。"
    : "A 股 T+1，当日买入次日才能卖出，涨跌停可能让止损单无法成交。";
}

/** 持仓页「可执行日」后面的括注。 */
export function sellTiming(market: Market): string {
  return market === "us" ? "T+0，当天可卖" : "T+1，当天买入的次日才能卖";
}

/** 费用模型那一句里，这个市场逐笔扣减哪些费用。 */
export function feeItems(market: Market): string {
  return market === "us" ? "佣金" : "印花税、过户费、经手费、证管费与佣金";
}

/** 首次启动确认弹窗逐条列出的五条风险提示。 */
export function firstRunNotes(market: Market): string[] {
  return [
    riskNotes.highReturn,
    riskNotes.market,
    market === "us" ? riskNotes.usSession : riskNotes.tPlusOne,
    riskNotes.slippage,
    riskNotes.costFree,
  ];
}

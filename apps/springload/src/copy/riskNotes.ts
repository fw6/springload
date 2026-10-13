/**
 * 需求规格说明书第 8 章的风险提示定稿文案。
 * 首次启动确认弹窗与设置页共用。
 */

export const riskNotes = {
  highReturn:
    "原文所述 969.8% 为 2025 年单场赛事的 12 个月战绩，不是年化收益率，不代表可持续收益。",
  market: "策略收益依赖市场环境，在趋势走坏年份会连续触发止损。",
  tPlus1:
    "A 股存在涨跌停与 T+1 制度，当日买入无法当日卖出，止损单可能无法成交导致实际亏损远超预设。",
  slippage: "跳空或滑点会使实际成交价劣于预期，亏损可能超过预设的账户风险。",
  costFree: "统计未计入交易成本与滑点。",
};

/** 首次启动确认弹窗逐条列出的五条风险提示。 */
export const firstRunNotes: string[] = [
  riskNotes.highReturn,
  riskNotes.market,
  riskNotes.tPlus1,
  riskNotes.slippage,
  riskNotes.costFree,
];

/**
 * 图版用色。
 *
 * K 线图把颜色作为数值传给 klinecharts，读不到 Tailwind 的工具类，所以这一组
 * 颜色单独放在这里，作为唯一来源。红涨绿跌遵循 A 股习惯，不随版式切换。
 * 网格与坐标线用油墨的低透明度，让图版像是印在纸上。
 */
export const CHART_COLORS = {
  /** 上涨：红。 */
  up: "#b8232a",
  /** 下跌：绿。 */
  down: "#1c6b4a",
  /** 网格线。 */
  grid: "rgba(22, 22, 26, 0.08)",
  /** 坐标轴文字。 */
  axisText: "#56544c",
  /** 坐标轴线。 */
  axisLine: "rgba(22, 22, 26, 0.35)",
  /** 均线 EMA 9/21/50。 */
  ema: "#2f5fa8",
  /** AVWAP 与起点竖线。 */
  avwap: "#6b3fa0",
  /** 支撑区上沿参考（前期高点）。 */
  reference: "#8a5a12",
  /** 水平线。 */
  level: "#0f766e",
  /** 支撑区填充。 */
  levelFill: "rgba(15, 118, 110, 0.16)",
  /** 缺口填充。 */
  referenceFill: "rgba(138, 90, 18, 0.14)",
} as const;

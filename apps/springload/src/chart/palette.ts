/**
 * 图版用色。
 *
 * K 线图把颜色作为数值传给 klinecharts，读不到 Tailwind 的工具类，所以这一组
 * 颜色单独放在这里，作为唯一来源。网格与坐标线用油墨的低透明度，让图版像是印在
 * 纸上。
 *
 * 涨跌两色按市场取：A 股红涨绿跌，美股绿涨红跌，与各自市场的习惯一致。
 */
import type { Market } from "../api/app";

/** 红与绿。 */
const RED = "#b8232a";
const GREEN = "#1c6b4a";

export const CHART_COLORS = {
  /** 上涨。 */
  up: RED,
  /** 下跌。 */
  down: GREEN,
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

/** 图版用色的形状。 */
export type ChartColors = { [K in keyof typeof CHART_COLORS]: string };

/** 某个市场下的图版用色。 */
export function chartColors(market: Market): ChartColors {
  if (market !== "us") return CHART_COLORS;
  return { ...CHART_COLORS, up: GREEN, down: RED };
}

/** 涨跌约定的说法，报尾那一行用。与上面的配色同一处定义，不会各说各的。 */
export function upDownLabel(market: Market): string {
  return market === "us" ? "绿涨红跌。" : "红涨绿跌。";
}

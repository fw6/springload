import { describe, expect, it } from "vitest";
import { CHART_COLORS, chartColors, upDownLabel } from "./palette";

describe("图版配色按市场分流", () => {
  it("A 股红涨绿跌，美股绿涨红跌", () => {
    const cn = chartColors("cn");
    const us = chartColors("us");
    expect(cn.up).toBe("#b8232a");
    expect(cn.down).toBe("#1c6b4a");
    expect(us.up).toBe("#1c6b4a");
    expect(us.down).toBe("#b8232a");
    // 其余颜色两个市场一致。
    expect(us.ema).toBe(cn.ema);
    expect(us.grid).toBe(cn.grid);
  });

  it("涨跌约定的说法与配色同一处定义", () => {
    expect(upDownLabel("cn")).toBe("红涨绿跌。");
    expect(upDownLabel("us")).toBe("绿涨红跌。");
    // A 股那一份就是导出的常量本身，没有被复制。
    expect(chartColors("cn")).toBe(CHART_COLORS);
  });
});

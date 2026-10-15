import { describe, expect, it } from "vitest";
import { feeItems, firstRunNotes, riskNotes, sellTiming, tradingRule } from "./riskNotes";

describe("按市场取的风险与制度文案", () => {
  it("首次启动弹窗列出五条，交易制度那一条按市场换", () => {
    const cn = firstRunNotes("cn");
    const us = firstRunNotes("us");
    expect(cn).toHaveLength(5);
    expect(us).toHaveLength(5);
    expect(cn[2]).toBe(riskNotes.tPlusOne);
    expect(us[2]).toBe(riskNotes.usSession);
    // 其余四条两个市场共用。
    expect(cn.filter((_, index) => index !== 2)).toEqual(us.filter((_, index) => index !== 2));
  });

  it("报尾那一行按市场换交易制度", () => {
    expect(tradingRule("cn")).toContain("T+1");
    expect(tradingRule("us")).toContain("T+0");
  });

  it("可执行日的括注按市场换", () => {
    expect(sellTiming("cn")).toBe("T+1，当天买入的次日才能卖");
    expect(sellTiming("us")).toBe("T+0，当天可卖");
  });

  it("费用项目按市场换：美股只有佣金", () => {
    expect(feeItems("cn")).toBe("印花税、过户费、经手费、证管费与佣金");
    expect(feeItems("us")).toBe("佣金");
  });
});

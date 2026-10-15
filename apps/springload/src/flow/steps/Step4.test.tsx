import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import Step4, { reduceShares } from "./Step4";
import { GlossaryProvider } from "../../copy/GlossaryContext";
import type { SizingInput } from "../../api/types";
import { glossaryStub, mockedInvoke, renderAt, settingsStub, usAppInfoStub } from "../../test-utils";
import type { Flow } from "../useFlow";

/** 规格里的示例输入：账户 100,000 元，买入价 20.00、止损价 19.50。 */
const EXAMPLE: SizingInput = { entry: 20.0, stop: 19.5, account: 100_000, riskPct: 0.005 };

/** 与 `springload-core::sizing` 一致的纯计算桩，只用来喂给界面。 */
function calc(input: SizingInput) {
  if (!(input.entry > input.stop)) throw new Error("买入价必须高于止损价，请检查输入");
  const perR = input.entry - input.stop;
  const maxLoss = input.account * input.riskPct;
  const shares = Math.floor(Math.floor(maxLoss / perR) / 100) * 100;
  const capital = shares * input.entry;
  return {
    perR,
    maxLoss,
    shares,
    capital,
    posPct: input.account === 0 ? 0 : capital / input.account,
    stopDistance: perR / input.entry,
    t3: input.entry + 3 * perR,
    t5: input.entry + 5 * perR,
    unusedRisk: maxLoss - shares * perR,
    lotSize: 100,
  };
}

/** 只带第 4 步需要的字段的 flow 桩。 */
function flowStub(sizing: SizingInput): Flow {
  return {
    step: 4,
    setStep: vi.fn(),
    goStep: vi.fn(),
    unlock: { unlocked: [true, true, true, true], reasons: ["", "", "", ""] },
    canEnter: () => true,
    selected: "600036",
    selectStock: vi.fn(),
    watchlist: [],
    reloadWatchlist: vi.fn(async () => {}),
    loadError: null,
    period: "day",
    setPeriod: vi.fn(),
    tool: "none",
    setTool: vi.fn(),
    avwapStartDate: null,
    setAvwapStartDate: vi.fn(),
    signal: null,
    setSignal: vi.fn(),
    sizing,
    setSizing: vi.fn(),
    collapsed: false,
    setCollapsed: vi.fn(),
  };
}

let sizing: SizingInput;

beforeEach(() => {
  sizing = { ...EXAMPLE };
  mockedInvoke.mockImplementation((async (command: string) => {
    if (command === "glossary") return glossaryStub;
    if (command === "settings_get") return settingsStub;
    if (command === "plan_calc") return calc(sizing);
    if (command === "explain") {
      return { text: "这单的风险结构是：每股风险 0.50 元。", generatedAt: "x", model: "m", cached: true };
    }
    throw new Error(`测试桩没有登记命令 ${command}`);
  }) as never);
});

function renderStep4(input: SizingInput = sizing) {
  sizing = input;
  return renderAt(
    "/",
    <GlossaryProvider>
      <Step4 flow={flowStub(input)} />
    </GlossaryProvider>,
  );
}

/** 按美股渲染第 4 步：货币换成美元，股数按整股取整。 */
function renderStep4Us(input: SizingInput = sizing) {
  sizing = input;
  mockedInvoke.mockImplementation((async (command: string) => {
    if (command === "app_info") return usAppInfoStub;
    if (command === "glossary") return glossaryStub;
    if (command === "settings_get") return settingsStub;
    if (command === "plan_calc") {
      // 与 springload-core::sizing 一致：美股按 1 股取整。
      const perR = input.entry - input.stop;
      const maxLoss = input.account * input.riskPct;
      const shares = Math.floor(maxLoss / perR);
      const capital = shares * input.entry;
      return {
        perR,
        maxLoss,
        shares,
        capital,
        posPct: capital / input.account,
        stopDistance: perR / input.entry,
        t3: input.entry + 3 * perR,
        t5: input.entry + 5 * perR,
        unusedRisk: maxLoss - shares * perR,
        lotSize: 1,
      };
    }
    if (command === "explain") {
      return { text: "这单的风险结构是：每股风险 0.50 美元。", generatedAt: "x", model: "m", cached: true };
    }
    throw new Error(`测试桩没有登记命令 ${command}`);
  }) as never);
  return renderAt(
    "/",
    <GlossaryProvider>
      <Step4 flow={flowStub(input)} />
    </GlossaryProvider>,
  );
}

describe("第 4 步：仓位计算", () => {
  it("示例输入算出规格里那组数字", async () => {
    const { container } = renderStep4();

    expect(await screen.findByText("可买股数")).toBeTruthy();
    const grid = container.querySelector('[data-slot="result-grid"]')!;
    const cells = Array.from(grid.querySelectorAll('[data-slot="result-cell"]')).map((cell) => ({
      label: cell.querySelector("dt")!.textContent ?? "",
      value: cell.querySelector("dd")!.textContent ?? "",
    }));
    expect(cells).toEqual([
      { label: "可买股数", value: "1000" },
      { label: "投入金额", value: "20000.00" },
      { label: "仓位占比", value: "20.00%" },
      { label: "止损距离", value: "2.50%" },
      { label: "3R 目标位", value: "21.50" },
      { label: "5R 目标位", value: "22.50" },
    ]);
  });

  it("六个公式逐项展开给用户看", async () => {
    const { container } = renderStep4();
    await screen.findByText("可买股数");

    const formula = container.querySelector('[data-slot="formula"]')!.textContent ?? "";
    expect(formula).toContain("每股风险 = 20.00 − 19.50 = 0.50");
    expect(formula).toContain("最大可亏金额 = 100000.00 × 0.50% = 500.00");
    expect(formula).toContain("可买股数 = 500.00 ÷ 0.50 后向下取整到 100 股的整数倍 = 1000 股");
    expect(formula).toContain("投入金额 = 1000 × 20.00 = 20000.00");
    expect(formula).toContain("仓位占比 = 20000.00 ÷ 100000.00 = 20.00%");
    expect(formula).toContain("止损距离 = 0.50 ÷ 20.00 = 2.50%");
  });

  it("示例面板写死的数字可用来对照", async () => {
    renderStep4();
    const panel = await screen.findByText(/示例面板（写死数字/);
    const text = panel.nextElementSibling!.textContent ?? "";
    expect(text).toContain("账户 100000 元");
    expect(text).toContain("每股风险 0.50");
    expect(text).toContain("可买 1000 股");
    expect(text).toContain("3R 21.50");
    expect(text).toContain("5R 22.50");
  });

  it("买入价不高于止损价时报错，不算出结果", async () => {
    renderStep4({ entry: 19.5, stop: 19.5, account: 100_000, riskPct: 0.005 });

    expect(await screen.findByText("买入价必须高于止损价，请检查输入")).toBeTruthy();
    expect(screen.queryByText("可买股数")).toBeNull();
  });

  it("止损距离超过阈值时给出显著警告，但仍算出结果", async () => {
    // per_r = 1.10，止损距离 = 1.10 ÷ 20 = 5.5% > 5%。
    renderStep4({ entry: 20.0, stop: 18.9, account: 100_000, riskPct: 0.005 });

    const warn = await screen.findByText(/超出这套策略建议的 3%~5%/);
    expect(warn.textContent).toContain("5.50%");
    expect(screen.getByText("可买股数")).toBeTruthy();
  });

  it("股数为 0 时给出提示", async () => {
    // 最大可亏 5 元，每股风险 0.50 元 → 原始 10 股，取整到 100 股后为 0。
    renderStep4({ entry: 20.0, stop: 19.5, account: 1_000, riskPct: 0.005 });

    expect(await screen.findByText(/股数算出来是 0/)).toBeTruthy();
    expect(screen.getByText("未用满的风险金额 5.00 元")).toBeTruthy();
  });

  it("显示因 100 股取整而没用满的风险金额与减仓安排", async () => {
    // 最大可亏 375 元，每股风险 0.50 元 → 原始 750 股，取整到 700 股。
    renderStep4({ entry: 20.0, stop: 19.5, account: 75_000, riskPct: 0.005 });

    expect(await screen.findByText("未用满的风险金额 25.00 元")).toBeTruthy();
    const reduce = screen.getByText(/减仓安排/).textContent ?? "";
    expect(reduce).toContain("3R 卖 233 股");
    expect(reduce).toContain("5R 再卖 233 股");
    expect(reduce).toContain("剩余 234 股跟随 9 日均线");
  });

  it("改输入立刻重算", async () => {
    renderStep4();
    await screen.findByText("可买股数");

    sizing = { ...EXAMPLE, stop: 19.0 };
    fireEvent.change(screen.getByLabelText("止损价（元/股）"), { target: { value: "19" } });

    await waitFor(() => {
      expect(screen.getByText("1000")).toBeTruthy();
    });
  });
});

describe("减仓股数", () => {
  it("按整股取整，不按手取整", () => {
    expect(reduceShares(1000)).toEqual({ shares3r: 333, shares5r: 333, remaining: 334 });
    expect(reduceShares(100)).toEqual({ shares3r: 33, shares5r: 33, remaining: 34 });
    expect(reduceShares(0)).toEqual({ shares3r: 0, shares5r: 0, remaining: 0 });
  });
});

describe("第 4 步：美股", () => {
  it("货币换成美元，股数按整股取整", async () => {
    renderStep4Us({ entry: 20.0, stop: 19.5, account: 1_000, riskPct: 0.005 });

    expect(await screen.findByLabelText("买入价（美元/股）")).toBeTruthy();
    expect(screen.getByLabelText("止损价（美元/股）")).toBeTruthy();
    expect(screen.getByLabelText("账户金额（美元）")).toBeTruthy();
    // 最大可亏 5 美元、每股风险 0.50 美元 → 10 股；A 股同一笔输入会取整到 0。
    expect(await screen.findByText("可买股数")).toBeTruthy();
    const formula = document.querySelector('[data-slot="formula"]')!.textContent ?? "";
    expect(formula).toContain("后向下取整到 整股 = 10 股");
    // 减仓安排按整股算，两个市场一致。
    expect(screen.getByText(/3R 卖 3 股/)).toBeTruthy();
  });
});

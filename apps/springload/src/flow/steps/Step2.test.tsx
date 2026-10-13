import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import type { SupportZoneView } from "../../api/types";
import { mockedInvoke, stubInvoke } from "../../test-utils";
import type { Flow } from "../useFlow";
import Step2 from "./Step2";

/** explain 命令的桩返回值。 */
function explanation(text: string) {
  return { text, generatedAt: "2026-10-13T08:00:00Z", model: "deepseek-chat", cached: false };
}

function flowStub(overrides: Partial<Flow> = {}): Flow {
  return {
    step: 2,
    setStep: vi.fn(),
    goStep: vi.fn(),
    unlock: { unlocked: [true, true, true, true], reasons: ["", "", "", ""] },
    canEnter: () => true,
    selected: "600000",
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
    sizing: { entry: 0, stop: 0, account: 100000, riskPct: 0.005 },
    setSizing: vi.fn(),
    collapsed: false,
    setCollapsed: vi.fn(),
    ...overrides,
  };
}

function zone(partial: Partial<SupportZoneView> & { id: string }): SupportZoneView {
  return {
    code: "600000",
    kind: "support",
    low: 9.5,
    high: 10.5,
    confidence: "高",
    spread: 0.01,
    ...partial,
  };
}

beforeEach(() => {
  mockedInvoke.mockReset();
});

describe("第 2 步面板", () => {
  it("列出每条标注的下沿、上沿、置信度与参考价命中数", async () => {
    const code = "600100";
    const zones = [
      zone({ id: "b1", code }),
      zone({ id: "g1", code, kind: "gap", low: 11.0, high: 11.5, confidence: "低", spread: 0.08 }),
      zone({ id: "l1", code, kind: "hline", low: 9.8, high: 9.8, confidence: "低", spread: null }),
    ];
    stubInvoke({
      support_zones_get: zones,
      support_refs: { ema21: 9.9, avwap: 10.0, prevHigh: 10.1 },
      explain: explanation("这只股票正在 10 元附近横盘，三个参考价挤在 2% 以内。"),
      explain_invalidate: undefined,
      explain_retry: explanation("重算后的解释"),
    });

    render(<Step2 flow={flowStub({ selected: code })} />);

    expect(await screen.findByText("支撑区")).toBeTruthy();
    expect(screen.getByText("9.50")).toBeTruthy();
    expect(screen.getByText("10.50")).toBeTruthy();
    expect(screen.getByText("高置信")).toBeTruthy();
    expect(screen.getByText("参考价命中 3/3")).toBeTruthy();

    expect(screen.getByText("缺口")).toBeTruthy();
    expect(screen.getByText("11.00")).toBeTruthy();
    expect(screen.getByText("11.50")).toBeTruthy();
    expect(screen.getByText("低置信")).toBeTruthy();
    expect(screen.getByText("参考价命中 0/3")).toBeTruthy();

    expect(screen.getByText("水平线")).toBeTruthy();
    expect(screen.getByText("9.80")).toBeTruthy();
    expect(screen.getByText("拖动可改价格")).toBeTruthy();

    // 三个参考价各自的落框数：三个都在支撑区框内。
    expect(screen.getByText("EMA21")).toBeTruthy();
    expect(screen.getByText("AVWAP")).toBeTruthy();
    expect(screen.getByText("前期高点")).toBeTruthy();
    expect(screen.getByText("10.10")).toBeTruthy();
    expect(screen.getAllByText("落在 1 个框内")).toHaveLength(3);

    expect(await screen.findByText("这只股票正在 10 元附近横盘，三个参考价挤在 2% 以内。")).toBeTruthy();
  });

  it("有高置信支撑区时允许进入第 3 步", async () => {
    const code = "600200";
    stubInvoke({
      support_zones_get: [zone({ id: "b1", code })],
      support_refs: { ema21: 9.9, avwap: 10.0, prevHigh: 10.1 },
      explain: explanation("解释"),
      explain_invalidate: undefined,
      explain_retry: explanation("重算后的解释"),
    });
    const flow = flowStub({ selected: code });

    render(<Step2 flow={flow} />);
    const button = (await screen.findByRole("button", {
      name: "完成标注，进入第 3 步",
    })) as HTMLButtonElement;
    expect(button.disabled).toBe(false);

    fireEvent.click(button);
    expect(flow.goStep).toHaveBeenCalledWith(3);
  });

  it("没有高置信支撑区时挡住第 3 步并给出提示", async () => {
    const code = "600300";
    stubInvoke({
      support_zones_get: [
        zone({ id: "b1", code, confidence: "低", low: 11.0, high: 11.5, spread: 0.08 }),
      ],
      support_refs: { ema21: 9.9, avwap: 10.0, prevHigh: 10.1 },
      explain: explanation("解释"),
      explain_invalidate: undefined,
      explain_retry: explanation("重算后的解释"),
    });

    render(<Step2 flow={flowStub({ selected: code })} />);
    const button = (await screen.findByRole("button", {
      name: "完成标注，进入第 3 步",
    })) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    expect(screen.getByText("至少需要一个高置信支撑区")).toBeTruthy();
  });

  it("没有标注时给出提示", async () => {
    const code = "600400";
    stubInvoke({
      support_zones_get: [],
      support_refs: { ema21: null, avwap: null, prevHigh: null },
      explain: explanation("解释"),
      explain_invalidate: undefined,
      explain_retry: explanation("重算后的解释"),
    });

    render(<Step2 flow={flowStub({ selected: code })} />);

    expect(await screen.findByText("还没有标注。点上面的「支撑区」在图上拖一个框出来。")).toBeTruthy();
    expect(screen.getAllByText("数据不足").length).toBeGreaterThan(0);
  });

  it("删除标注会调用命令并从列表里移除", async () => {
    const code = "600500";
    stubInvoke({
      support_zones_get: [zone({ id: "b1", code })],
      support_refs: { ema21: 9.9, avwap: 10.0, prevHigh: 10.1 },
      support_zone_delete: undefined,
      explain: explanation("解释"),
      explain_invalidate: undefined,
      explain_retry: explanation("重算后的解释"),
    });

    render(<Step2 flow={flowStub({ selected: code })} />);
    fireEvent.click(await screen.findByRole("button", { name: "删除" }));

    await waitFor(() => {
      expect(screen.queryByText("支撑区")).toBeNull();
    });
    expect(mockedInvoke).toHaveBeenCalledWith("support_zone_delete", { id: "b1" });
  });
});

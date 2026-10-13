import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

import Step3 from "./Step3";
import { GlossaryProvider } from "../../copy/GlossaryContext";
import type { SignalStatus } from "../../api/types";
import { glossaryStub, mockedInvoke, renderAt, settingsStub } from "../../test-utils";
import type { Flow } from "../useFlow";

/** 只带第 3 步需要的字段的 flow 桩。 */
function flowStub(): Flow {
  return {
    step: 3,
    setStep: vi.fn(),
    goStep: vi.fn(),
    unlock: { unlocked: [true, true, true, false], reasons: ["", "", "", "先等到买入信号"] },
    canEnter: () => true,
    selected: "600036",
    selectStock: vi.fn(),
    watchlist: [],
    reloadWatchlist: vi.fn(async () => {}),
    loadError: null,
    period: "minute",
    setPeriod: vi.fn(),
    tool: "none",
    setTool: vi.fn(),
    avwapStartDate: null,
    setAvwapStartDate: vi.fn(),
    signal: null,
    setSignal: vi.fn(),
    sizing: { entry: 0, stop: 0, account: 100_000, riskPct: 0.005 },
    setSizing: vi.fn(),
    collapsed: false,
    setCollapsed: vi.fn(),
  };
}

let status: SignalStatus;
let evaluateFails: boolean;

beforeEach(() => {
  status = {
    stage: "waiting",
    zoneId: null,
    zone: null,
    detail: "前置条件未满足：还没有高置信支撑区。",
    trigger: null,
  };
  evaluateFails = false;
  mockedInvoke.mockImplementation((async (command: string) => {
    if (command === "glossary") return glossaryStub;
    if (command === "settings_get") return settingsStub;
    if (command === "signal_status") return status;
    if (command === "signal_evaluate") {
      if (evaluateFails) throw new Error("请求分钟线失败");
      return status;
    }
    if (command === "explain") {
      return { text: "当前卡在第二级。", generatedAt: "x", model: "m", cached: true };
    }
    throw new Error(`测试桩没有登记命令 ${command}`);
  }) as never);
});

function renderStep3() {
  return renderAt(
    "/",
    <GlossaryProvider>
      <Step3 flow={flowStub()} />
    </GlossaryProvider>,
  );
}

describe("第 3 步：等待买入信号", () => {
  it("显示当前卡在哪一级", async () => {
    status = {
      stage: "waiting",
      zoneId: "zone-1",
      zone: { id: "zone-1", code: "600036", kind: "support", low: 9.9, high: 10.1, confidence: "高", spread: 0.01 },
      detail: "前置条件已满足：价格站上高置信支撑区，等待分钟图出现下探回升后突破前一根高点。",
      trigger: null,
    };
    renderStep3();

    expect(await screen.findByText(/前置条件已满足/)).toBeTruthy();
    expect(screen.getByText("已满足")).toBeTruthy();
    expect(screen.getByText("未触发")).toBeTruthy();
  });

  it("没有高置信支撑区时第一级显示未满足", async () => {
    renderStep3();

    expect(await screen.findByText(/前置条件未满足/)).toBeTruthy();
    expect(screen.getByText("未满足")).toBeTruthy();
  });

  it("触发时两级都显示已满足并给出提示", async () => {
    status = {
      stage: "triggered",
      zoneId: "zone-1",
      zone: { id: "zone-1", code: "600036", kind: "support", low: 9.9, high: 10.1, confidence: "高", spread: 0.01 },
      detail: "已触发：2026-10-13 09:34 · 1 分钟 · 本根最高 10.06 超过前一根最高 10.04。",
      trigger: { ts: "2026-10-13 09:34", brokenHigh: 10.06, preHigh: 10.04 },
    };
    renderStep3();

    expect(await screen.findByText(/已触发：2026-10-13 09:34/)).toBeTruthy();
    expect(screen.getByText("已触发")).toBeTruthy();
    expect(screen.getByText("已满足")).toBeTruthy();
    expect(screen.getByText("已触发，可以进第 4 步算仓位。")).toBeTruthy();
  });

  it("跌穿未回升时保持等待，不出现任何触发字样", async () => {
    status = {
      stage: "failed",
      zoneId: "zone-1",
      zone: { id: "zone-1", code: "600036", kind: "support", low: 9.9, high: 10.1, confidence: "高", spread: 0.01 },
      detail: "价格 9.5 已跌穿高置信支撑区下沿 9.9 且没有回升，保持等待，不发出提示。",
      trigger: null,
    };
    renderStep3();

    expect(await screen.findByText(/已跌穿高置信支撑区下沿/)).toBeTruthy();
    expect(screen.getByText("未触发")).toBeTruthy();
    expect(screen.queryByText("已触发")).toBeNull();
  });

  it("抓取失败时给出错误文案", async () => {
    evaluateFails = true;
    renderStep3();
    await screen.findByText(/前置条件未满足/);

    fireEvent.click(screen.getByRole("button", { name: "抓一次分钟线并判定" }));

    expect(await screen.findByText("请求分钟线失败")).toBeTruthy();
  });

  it("没选股票时提示先加进观察名单", async () => {
    const flow = flowStub();
    flow.selected = null;
    renderAt(
      "/",
      <GlossaryProvider>
        <Step3 flow={flow} />
      </GlossaryProvider>,
    );

    await waitFor(() => {
      expect(screen.getByText("先在第 1 步把股票加进观察名单。")).toBeTruthy();
    });
  });
});

import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import PositionsPage, { REFRESH_INTERVAL_MS } from "./PositionsPage";
import { GlossaryProvider } from "../copy/GlossaryContext";
import type { PositionView } from "../api/positions";
import type { TradePlan } from "../api/plan";
import { glossaryStub, mockedInvoke, renderAt, settingsStub } from "../test-utils";

/** 一条持有中的仓位，价格 20.10 落在止损之上、3R 之下。 */
const holding: PositionView = {
  id: "position-1",
  planId: "plan-1",
  code: "600036",
  name: "招商银行",
  entryDate: "2026-10-13",
  entryPrice: 20.0,
  stop: 19.5,
  quantity: 1000,
  last: 20.1,
  ema9: 20.05,
  perR: 0.5,
  t3: 21.5,
  t5: 22.5,
  followEma: true,
  alerts: [],
};

/** 跌破止损的那条。 */
const stopped: PositionView = {
  ...holding,
  last: 19.4,
  alerts: [
    {
      kind: "stopLoss",
      message: "触发止损，按计划离场",
      signalDate: "2026-10-13",
      executableDate: "2026-10-14",
    },
  ],
};

const draft: TradePlan = {
  id: "plan-2",
  code: "601899",
  entry: 16.4,
  stop: 15.9,
  account: 200_000,
  riskPct: 0.005,
  perR: 0.5,
  shares: 2000,
  capital: 32_800,
  posPct: 0.164,
  stopDistance: 0.0305,
  t3: 17.9,
  t5: 18.9,
  status: "draft",
  zoneId: null,
  signalId: null,
  createdAt: "2026-10-13T09:00:00Z",
  updatedAt: "2026-10-13T09:00:00Z",
};

let views: PositionView[];
let drafts: TradePlan[];

beforeEach(() => {
  views = [holding];
  drafts = [];
  mockedInvoke.mockImplementation((async (command: string, args?: Record<string, unknown>) => {
    switch (command) {
      case "glossary":
        return glossaryStub;
      case "settings_get":
        return settingsStub;
      case "positions_get":
      case "positions_refresh_quotes":
        return views;
      case "plan_list":
        return drafts;
      case "position_exit":
        return {
          id: "log-plan-1",
          positionId: "position-1",
          code: "600036",
          name: "招商银行",
          entryDate: "2026-10-13",
          entryPrice: 20.0,
          stopPrice: 19.5,
          exitDate: String(args?.exitDate),
          exitPrice: Number(args?.exitPrice),
          quantity: 1000,
          perR: 0.5,
          rMultiple: (Number(args?.exitPrice) - 20.0) / 0.5,
          pnl: (Number(args?.exitPrice) - 20.0) * 1000,
          fees: 0,
          note: null,
          source: "plan",
          createdAt: "2026-10-14T01:00:00Z",
        };
      default:
        throw new Error(`测试桩没有登记命令 ${command}`);
    }
  }) as never);
});

function renderPositions() {
  return renderAt(
    "/positions",
    <GlossaryProvider>
      <PositionsPage />
    </GlossaryProvider>,
  );
}

describe("持仓跟踪", () => {
  it("列出代码、名称、入场日、买入价、止损价、数量、最新价、9 日 EMA 与 3R/5R", async () => {
    renderPositions();

    expect(await screen.findByText(/入场 2026-10-13/)).toBeTruthy();
    expect(screen.getByText("招商银行")).toBeTruthy();
    expect(screen.getByText("600036")).toBeTruthy();
    const summary = screen.getByText(/买入 20.00/).textContent ?? "";
    expect(summary).toContain("买入 20.00");
    expect(summary).toContain("止损 19.50");
    expect(summary).toContain("最新 20.10");
    expect(summary).toContain("9 日 EMA 20.05");
    expect(summary).toContain("3R 21.50");
    expect(summary).toContain("5R 22.50");
    expect(screen.getByText("持有")).toBeTruthy();
  });

  it("跌破止损时给出提示，并区分信号日与可执行日", async () => {
    views = [stopped];
    renderPositions();

    expect(await screen.findByText("触发止损，按计划离场")).toBeTruthy();
    const detail = screen.getByText(/信号日 2026-10-13/).textContent ?? "";
    expect(detail).toContain("可执行日 2026-10-14");
    expect(detail).toContain("T+1");
  });

  it("到 3R 与 5R 时给出减仓提示", async () => {
    views = [
      {
        ...holding,
        last: 21.6,
        alerts: [
          {
            kind: "reduce3R",
            message: "到达 3R 减仓目标位",
            signalDate: "2026-10-13",
            executableDate: "2026-10-14",
          },
        ],
      },
    ];
    renderPositions();

    expect(await screen.findByText("到达 3R 减仓目标位")).toBeTruthy();
  });

  it("跌破 9 日 EMA 时给出趋势提示", async () => {
    views = [
      {
        ...holding,
        last: 20.0,
        ema9: 20.05,
        alerts: [
          {
            kind: "belowEma9",
            message: "趋势可能走坏，建议评估离场",
            signalDate: "2026-10-13",
            executableDate: "2026-10-14",
          },
        ],
      },
    ];
    renderPositions();

    expect(await screen.findByText("趋势可能走坏，建议评估离场")).toBeTruthy();
  });

  it("填入实际成交价后写出场记录", async () => {
    renderPositions();
    await screen.findByText(/入场 2026-10-13/);

    fireEvent.change(screen.getByLabelText("实际成交价（元/股）"), { target: { value: "21.5" } });
    fireEvent.click(screen.getByRole("button", { name: "填入成交价并离场" }));

    expect(await screen.findByText(/已离场：出场价 21.50/)).toBeTruthy();
    expect(screen.getByText(/R 倍数 3.00/)).toBeTruthy();
    expect(screen.getByText(/盈亏 1500.00 元/)).toBeTruthy();
  });

  it("每分钟刷新一次最新价", async () => {
    vi.useFakeTimers();
    try {
      renderPositions();
      await vi.waitFor(() => {
        expect(mockedInvoke).toHaveBeenCalledWith("positions_get", undefined);
      });
      const before = mockedInvoke.mock.calls.filter(
        ([command]) => command === "positions_refresh_quotes",
      ).length;

      vi.advanceTimersByTime(REFRESH_INTERVAL_MS);

      await vi.waitFor(() => {
        const after = mockedInvoke.mock.calls.filter(
          ([command]) => command === "positions_refresh_quotes",
        ).length;
        expect(after).toBe(before + 1);
      });
    } finally {
      vi.useRealTimers();
    }
  });

  it("草稿计划在持仓页可以加载", async () => {
    drafts = [draft];
    renderPositions();

    expect(await screen.findByText("计划草稿（存为草稿之后在这里加载）")).toBeTruthy();
    expect(screen.getByText("601899")).toBeTruthy();
    expect(screen.getByRole("link", { name: "在计划卡打开" })).toBeTruthy();
  });

  it("没有持仓时给出说明", async () => {
    views = [];
    renderPositions();

    await waitFor(() => {
      expect(screen.getByText(/还没有持仓/)).toBeTruthy();
    });
  });
});

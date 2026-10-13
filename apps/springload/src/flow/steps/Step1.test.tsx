import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import FlowPage from "../FlowPage";
import { GlossaryProvider } from "../../copy/GlossaryContext";
import type { ScreenReport } from "../../api/screen";
import type { WatchlistRow } from "../../api/types";
import {
  appInfoStub,
  glossaryStub,
  mockedInvoke,
  renderAt,
  settingsStub,
} from "../../test-utils";

/** 筛选报告：入选两只、未入选两只，各差在不同的地方。 */
const report: ScreenReport = {
  ranAt: "2026-10-13T09:00:00Z",
  universe: 23,
  elapsedMs: 12,
  picked: [
    {
      code: "600001",
      name: "甲股",
      sector: "银行",
      price: 10.5,
      chg1m: 0.1,
      chg3m: 0.2,
      chg6m: 0.4,
      rankPct: 0.25,
      avgAmount: 2e8,
      convergence: 0.1,
      c1: true,
      c2: true,
      c3: true,
      c4: false,
      liquid: true,
    },
    {
      code: "600002",
      name: "乙股",
      sector: "证券",
      price: 20.5,
      chg1m: 0.2,
      chg3m: 0.3,
      chg6m: 0.5,
      rankPct: 0.3,
      avgAmount: 3e8,
      convergence: 0.08,
      c1: true,
      c2: true,
      c3: true,
      c4: false,
      liquid: true,
    },
  ],
  rejected: [
    {
      code: "600003",
      name: "丙股",
      sector: "医药",
      price: 8.0,
      chg1m: 0.4,
      chg3m: 0.2,
      chg6m: 0.1,
      rankPct: 0.05,
      avgAmount: 2e8,
      convergence: 0.12,
      c1: false,
      c2: true,
      c3: true,
      c4: false,
      liquid: true,
    },
    {
      code: "600004",
      name: "丁股",
      sector: null,
      price: 30.0,
      chg1m: -0.1,
      chg3m: -0.2,
      chg6m: 0.35,
      rankPct: 0.6,
      avgAmount: 5e7,
      convergence: 0.3,
      c1: false,
      c2: false,
      c3: true,
      c4: true,
      liquid: false,
    },
  ],
};

let watchlist: WatchlistRow[] = [];

/** 把 invoke 换成桩：筛选取固定报告，观察名单用一个可变数组模拟。 */
function stub() {
  watchlist = [];
  mockedInvoke.mockImplementation((async (command: string, args?: Record<string, unknown>) => {
    switch (command) {
      case "app_info":
        return appInfoStub;
      case "glossary":
        return glossaryStub;
      case "settings_get":
        return settingsStub;
      case "settings_update":
        return settingsStub;
      case "screen_last":
        return report;
      case "screen_stocks":
        return { ...report, universe: 30, elapsedMs: 20 };
      case "watchlist_get":
        return watchlist;
      case "watchlist_add": {
        const code = String(args?.code);
        const row = [...report.picked, ...report.rejected].find((item) => item.code === code)!;
        watchlist = [
          ...watchlist,
          {
            code: row.code,
            name: row.name,
            addedAt: report.ranAt,
            c1: row.c1,
            c2: row.c2,
            c3: row.c3,
            c4: row.c4,
            chg1m: row.chg1m,
            chg3m: row.chg3m,
            chg6m: row.chg6m,
            rankPct: row.rankPct,
            avgAmount: row.avgAmount,
            convergence: row.convergence,
          },
        ];
        return undefined;
      }
      case "watchlist_remove":
        return undefined;
      case "step_unlock":
        return { unlocked: [true, true, false, false], reasons: ["", "", "先标注支撑区", ""] };
      case "explain":
        return { text: "这只股票近 6 个月上涨 40%。", generatedAt: "x", model: "m", cached: true };
      default:
        throw new Error(`测试桩没有登记命令 ${command}`);
    }
  }) as never);
}

function renderFlow() {
  return renderAt(
    "/",
    <GlossaryProvider>
      <FlowPage />
    </GlossaryProvider>,
  );
}

/** 一张表里的代码列，按显示顺序。 */
function codesOf(container: HTMLElement, tableIndex: number): string[] {
  const table = container.querySelectorAll("table.grid")[tableIndex];
  return Array.from(table.querySelectorAll("tbody tr")).map(
    (row) => row.querySelector("td")!.textContent ?? "",
  );
}

function rowOf(name: string): HTMLElement {
  return screen.getByText(name).closest("tr") as HTMLElement;
}

beforeEach(() => {
  stub();
});

describe("第 1 步：筛选表", () => {
  it("进面板就显示上次筛选的两张表、跑过多少只股票与用了多久", async () => {
    const { container } = renderFlow();

    expect(await screen.findByText("入选（2 只）")).toBeTruthy();
    expect(screen.getByText("未入选（2 只）")).toBeTruthy();
    expect(screen.getByText("跑在 23 只股票上，用了 12 毫秒。")).toBeTruthy();

    await waitFor(() => {
      expect(container.querySelectorAll("table.grid")).toHaveLength(2);
    });
    expect(codesOf(container, 0)).toEqual(["600001", "600002"]);
    expect(codesOf(container, 1)).toEqual(["600003", "600004"]);
  });

  it("入选表逐条显示 C1~C4 的满足情况，未入选表写出差哪一条", async () => {
    renderFlow();
    await screen.findByText("入选（2 只）");

    // 入选表有四条判定的列头。
    expect(screen.getByText("C1")).toBeTruthy();
    expect(screen.getByText("C2")).toBeTruthy();
    expect(screen.getByText("C3")).toBeTruthy();
    expect(screen.getByText("C4")).toBeTruthy();

    // 未入选的差哪一条逐条写出来。
    expect(screen.getByText("C1 涨幅")).toBeTruthy();
    expect(screen.getByText("C1 涨幅 · C2 结构 · C4 排除命中 · 成交额不足")).toBeTruthy();
  });

  it("列头可以按涨幅与排名排序", async () => {
    const { container } = renderFlow();
    await screen.findByText("入选（2 只）");

    // 默认按排名升序：分位小的在前。
    expect(codesOf(container, 0)).toEqual(["600001", "600002"]);

    fireEvent.click(screen.getByRole("button", { name: "近 6 月排序" }));
    await waitFor(() => {
      expect(codesOf(container, 0)).toEqual(["600002", "600001"]);
    });

    // 再点一下反过来。
    fireEvent.click(screen.getByRole("button", { name: "近 6 月排序" }));
    await waitFor(() => {
      expect(codesOf(container, 0)).toEqual(["600001", "600002"]);
    });
  });

  it("点「开始筛选」重新跑一遍并刷新进度", async () => {
    renderFlow();
    await screen.findByText("跑在 23 只股票上，用了 12 毫秒。");

    fireEvent.click(screen.getByRole("button", { name: "开始筛选" }));

    expect(await screen.findByText("跑在 30 只股票上，用了 20 毫秒。")).toBeTruthy();
  });

  it("选中未入选的一行也能加进观察名单", async () => {
    renderFlow();
    await screen.findByText("未入选（2 只）");

    const add = screen.getByRole("button", { name: "加入观察名单" }) as HTMLButtonElement;
    expect(add.disabled).toBe(true);

    fireEvent.click(rowOf("丙股"));
    await waitFor(() => {
      expect((screen.getByRole("button", { name: "加入观察名单" }) as HTMLButtonElement).disabled).toBe(
        false,
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "加入观察名单" }));

    expect(await screen.findByText("已加入 1 只")).toBeTruthy();
    expect(mockedInvoke).toHaveBeenCalledWith("watchlist_add", { code: "600003" });
    // 已加进名单的那只，按钮再点没有意义。
    await waitFor(() => {
      expect((screen.getByRole("button", { name: "加入观察名单" }) as HTMLButtonElement).disabled).toBe(
        true,
      );
    });
  });
});

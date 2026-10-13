import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { AdjustmentPanel } from "./AdjustmentPanel";
import { mockedInvoke, stubInvoke } from "../test-utils";

beforeEach(() => {
  stubInvoke({ corporate_actions_stale: [] });
});

describe("AdjustmentPanel", () => {
  it("没有需要重算的股票时显示提示", async () => {
    render(<AdjustmentPanel />);

    await waitFor(() => expect(screen.getByText(/没有需要重算的股票/)).toBeTruthy());
  });

  it("显示需要重算的股票数并列出股票", async () => {
    stubInvoke({
      corporate_actions_stale: [
        { code: "001309", name: "德明利", exDate: "2026-10-13" },
        { code: "001965", name: "招商公路", exDate: null },
      ],
    });
    render(<AdjustmentPanel />);

    await waitFor(() => expect(screen.getByText(/需要重算的股票：2 只/)).toBeTruthy());
    expect(screen.getByText(/001309 德明利 · 除权除息日 2026-10-13/)).toBeTruthy();
    expect(screen.getByText(/001965 招商公路/)).toBeTruthy();
  });

  it("点重新同步会重算并刷新列表", async () => {
    let checks = 0;
    mockedInvoke.mockImplementation((async (command: string, args?: unknown) => {
      if (command === "corporate_actions_stale") {
        checks += 1;
        return checks === 1
          ? [{ code: "001309", name: "德明利", exDate: "2026-10-13" }]
          : [];
      }
      if (command === "resync_adjusted") {
        expect((args as { codes: string[] }).codes).toEqual(["001309"]);
        return { stocks: 1, bars: 137, elapsedMs: 12, errors: [] };
      }
      throw new Error(`测试桩没有登记命令 ${command}`);
    }) as never);

    render(<AdjustmentPanel />);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "重新同步" })).toBeTruthy(),
    );

    act(() => {
      screen.getByRole("button", { name: "重新同步" }).click();
    });

    await waitFor(() => expect(screen.getByText(/没有需要重算的股票/)).toBeTruthy());
    expect(screen.getByText(/本次重算 1 只股票、137 根日线/)).toBeTruthy();
  });

  it("检查失败时显示错误", async () => {
    mockedInvoke.mockImplementation((async () => {
      throw new Error("网络不可达");
    }) as never);

    render(<AdjustmentPanel />);

    await waitFor(() => expect(screen.getByText(/网络不可达/)).toBeTruthy());
  });
});

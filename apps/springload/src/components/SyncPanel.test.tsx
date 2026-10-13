import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

/** 捕获 listen 注册的回调，测试里手工投递进度事件。 */
const captured = vi.hoisted(() => ({
  handler: undefined as ((event: { payload: unknown }) => void) | undefined,
  name: undefined as string | undefined,
  unlisten: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, handler: (event: { payload: unknown }) => void) => {
    captured.name = name;
    captured.handler = handler;
    return captured.unlisten;
  }),
}));

import SyncPanel from "./SyncPanel";
import { mockedInvoke, stubInvoke } from "../test-utils";

const baseStatus = {
  running: false,
  done: 0,
  total: 0,
  lastSyncAt: "2026-10-13T07:00:00Z",
  message: "同步完成",
};

beforeEach(() => {
  captured.handler = undefined;
  captured.name = undefined;
  captured.unlisten.mockClear();
  stubInvoke({ sync_status: baseStatus });
});

describe("SyncPanel", () => {
  it("显示最近同步时间与初始进度", async () => {
    render(<SyncPanel />);

    await waitFor(() => expect(screen.getByText(/最近同步/)).toBeTruthy());
    expect(screen.getByText(/同步完成/)).toBeTruthy();
    expect(captured.name).toBe("sync://progress");

    const bar = screen.getByRole("progressbar");
    expect(bar.getAttribute("aria-valuenow")).toBe("0");
  });

  it("订阅 sync://progress 事件并更新进度", async () => {
    render(<SyncPanel />);
    await waitFor(() => expect(captured.handler).toBeDefined());

    act(() => {
      captured.handler!({
        payload: {
          running: true,
          done: 17,
          total: 35,
          lastSyncAt: null,
          message: "回补日线 17/35",
        },
      });
    });

    const bar = screen.getByRole("progressbar");
    expect(bar.getAttribute("aria-valuenow")).toBe("49");
    expect(screen.getByText(/17 \/ 35/)).toBeTruthy();
    expect(screen.getByText(/回补日线 17\/35/)).toBeTruthy();
  });

  it("手动同步后显示本次结果", async () => {
    stubInvoke({
      sync_status: baseStatus,
      sync_market: { stocks: 3488, bars: 120000, elapsedMs: 9000, errors: [] },
    });
    render(<SyncPanel />);
    await waitFor(() => expect(screen.getByText(/最近同步/)).toBeTruthy());

    act(() => {
      screen.getByRole("button", { name: "立即同步" }).click();
    });

    await waitFor(() => expect(screen.getByText(/本次写入 3488 只股票/)).toBeTruthy());
  });

  it("同步失败时显示错误", async () => {
    stubInvoke({ sync_status: baseStatus });
    mockedInvoke.mockImplementation((async (command: string) => {
      if (command === "sync_status") return baseStatus;
      if (command === "sync_market") throw new Error("网络不可达");
      throw new Error(`测试桩没有登记命令 ${command}`);
    }) as never);
    render(<SyncPanel />);
    await waitFor(() => expect(screen.getByText(/最近同步/)).toBeTruthy());

    act(() => {
      screen.getByRole("button", { name: "立即同步" }).click();
    });

    await waitFor(() => expect(screen.getByText(/网络不可达/)).toBeTruthy());
  });
});

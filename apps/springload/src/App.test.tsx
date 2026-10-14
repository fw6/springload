import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { AppRoutes, ROUTES } from "./App";
import { renderAt, stubInvoke } from "./test-utils";

beforeEach(() => {
  stubInvoke();
});

describe("五个路由", () => {
  it("导航列出五个路由", async () => {
    renderAt("/", <AppRoutes />);
    await waitFor(() => {
      expect(screen.getAllByRole("link")).toHaveLength(ROUTES.length);
    });
  });

  it.each([
    ["/plan", "交易计划卡"],
    ["/positions", "持仓跟踪"],
    ["/journal", "交易日志"],
    ["/settings", "设置"],
  ])("%s 渲染 %s 页面", async (path, heading) => {
    renderAt(path, <AppRoutes />);
    expect(await screen.findByRole("heading", { name: heading })).toBeTruthy();
  });

  it("/ 渲染主流程的四步骨架", async () => {
    const { container } = renderAt("/", <AppRoutes />);

    // 顶部四步进度：四个步骤按钮。
    await waitFor(() => {
      expect(container.querySelectorAll('[data-slot="step-chip"]')).toHaveLength(4);
    });
    // 左侧图表区、右侧面板都在。
    expect(container.querySelector('[data-slot="chart-card"]')).not.toBeNull();
    expect(container.querySelector('[data-slot="flow-panel"]')).not.toBeNull();
    // 默认停在第 1 步。
    expect(screen.getByRole("heading", { name: "强势股筛选" })).toBeTruthy();
  });

  it("点导航可以在路由之间跳转", async () => {
    renderAt("/", <AppRoutes />);
    await screen.findByRole("heading", { name: "强势股筛选" });

    fireEvent.click(screen.getByRole("link", { name: "计划卡" }));

    expect(await screen.findByRole("heading", { name: "交易计划卡" })).toBeTruthy();
  });

  it("顶栏渲染版本号、数据源与最近同步时间", async () => {
    renderAt("/", <AppRoutes />);
    expect(await screen.findByText("版本 0.1.0")).toBeTruthy();
    expect(screen.getByText("东方财富 · 前复权")).toBeTruthy();
    expect(screen.getByText("最近同步 尚未同步")).toBeTruthy();
  });
});

describe("报眉与隐藏标题栏", () => {
  const original = navigator.userAgent;

  afterEach(() => {
    Object.defineProperty(navigator, "userAgent", {
      value: original,
      configurable: true,
    });
  });

  function setUserAgent(value: string) {
    Object.defineProperty(navigator, "userAgent", { value, configurable: true });
  }

  it("刊期与运行信息排在报眉上，报头行只留刊名", async () => {
    renderAt("/", <AppRoutes />);
    await screen.findByText("版本 0.1.0");

    const line = document.querySelector('[data-slot="masthead-line"]')!;
    expect(line.textContent).toContain("日刊 ·");
    expect(line.textContent).toContain("版本 0.1.0");
    expect(line.textContent).toContain("东方财富 · 前复权");
    expect(line.textContent).toContain("最近同步 尚未同步");
    // 报头行里不再重复这些。
    expect(line.querySelector("h2, h1")).toBeNull();
  });

  it("整条报头是拖拽区，链接会被 Tauri 自动排除", async () => {
    setUserAgent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15");
    const { container } = renderAt("/", <AppRoutes />);

    const header = container.querySelector("header")!;
    expect(header.getAttribute("data-tauri-drag-region")).toBe("deep");
    // 版次条是链接，Tauri 的拖拽脚本遇到 <a> 会放行，不影响点击。
    expect(header.querySelectorAll("nav a")).toHaveLength(ROUTES.length);
  });
});

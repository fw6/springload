import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import FlowPage from "./FlowPage";
import { GlossaryProvider } from "../copy/GlossaryContext";
import { renderAt, stubInvoke } from "../test-utils";

beforeEach(() => {
  stubInvoke();
});

function renderFlow() {
  return renderAt(
    "/",
    <GlossaryProvider>
      <FlowPage />
    </GlossaryProvider>,
  );
}

describe("主流程骨架", () => {
  it("顶部四步进度、左侧图表区、右侧面板都可见", async () => {
    const { container } = renderFlow();
    await waitFor(() => {
      expect(container.querySelectorAll(".step-chip")).toHaveLength(4);
    });
    expect(container.querySelector(".chart-card")).not.toBeNull();
    expect(container.querySelector(".panel")).not.toBeNull();
  });

  it("收起按钮在四步进度条右侧，点一下把面板收起来", async () => {
    const { container } = renderFlow();
    await waitFor(() => {
      expect(container.querySelectorAll(".step-chip")).toHaveLength(4);
    });

    const steps = container.querySelector(".steps")!;
    const collapse = screen.getByRole("button", { name: "收起面板" });
    // 收起按钮是步骤条里的最后一个控件，位置在四步进度右侧。
    expect(steps.lastElementChild).toBe(collapse);
    expect(container.querySelector(".flow")!.getAttribute("data-collapsed")).toBe("false");
    expect(container.querySelector(".panel")).not.toBeNull();

    fireEvent.click(collapse);

    expect(container.querySelector(".panel")).toBeNull();
    expect(container.querySelector(".flow")!.getAttribute("data-collapsed")).toBe("true");
    expect(screen.getByRole("button", { name: "展开面板" })).toBeTruthy();
  });

  it("「这一步在干什么」弹出当前步的完整说明", async () => {
    renderFlow();
    await screen.findByRole("heading", { name: "强势股筛选" });

    fireEvent.click(screen.getByRole("button", { name: "这一步在干什么" }));

    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain("第 1 步 · 强势股筛选");
    expect(dialog.textContent).toContain("从沪深主板里筛出");
    expect(dialog.textContent).toContain("筛选器只看四件事");

    fireEvent.click(screen.getByRole("button", { name: "知道了" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
  });

  it("切到第 3 步自动换成分钟图", async () => {
    const { container } = renderFlow();
    await waitFor(() => {
      expect(container.querySelectorAll(".step-chip")).toHaveLength(4);
    });

    fireEvent.click(screen.getByRole("button", { name: /等待买入信号/ }));

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "等待买入信号" })).toBeTruthy();
    });
    expect(container.querySelector(".chart-placeholder")!.textContent).toContain("1 分钟");
  });
});

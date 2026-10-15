import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { Dialog } from "./dialog";

/** 关掉面板要等收起动画放完，260 毫秒。 */
const IMPRESS_MS = 260;

function Host() {
  const [open, setOpen] = useState(true);
  return (
    <Dialog open={open} ariaLabel="测试弹窗" onOpenChange={setOpen}>
      <p>正文</p>
    </Dialog>
  );
}

function panel() {
  return document.querySelector('[data-slot="dialog"] [data-state]');
}

describe("Dialog", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("打开时面板挂上，状态为打开", () => {
    render(<Host />);
    expect(panel()?.getAttribute("data-state")).toBe("open");
    expect(panel()?.classList.contains("dialog-in")).toBe(true);
    expect(panel()?.textContent).toContain("正文");
  });

  it("关闭时面板留着放完收起动画，动画走完才摘掉", () => {
    render(<Host />);

    fireEvent.click(screen.getByRole("button", { name: "关闭" }));

    expect(panel()?.getAttribute("data-state")).toBe("closed");
    expect(panel()?.classList.contains("dialog-out")).toBe(true);
    act(() => vi.advanceTimersByTime(IMPRESS_MS - 1));
    expect(panel()).not.toBeNull();

    act(() => vi.advanceTimersByTime(1));
    expect(panel()).toBeNull();
  });
});

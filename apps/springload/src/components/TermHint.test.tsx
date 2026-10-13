import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { render } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { GlossaryProvider } from "../copy/GlossaryContext";
import { TermHint } from "./TermHint";
import { stubInvoke } from "../test-utils";

beforeEach(() => {
  stubInvoke();
});

function renderHint(termKey: string) {
  return render(
    <GlossaryProvider>
      <TermHint termKey={termKey} />
    </GlossaryProvider>,
  );
}

describe("TermHint", () => {
  it("术语以虚线胶囊出现，点开后显示白话解释", async () => {
    renderHint("pullback");

    const chip = await screen.findByRole("button", { name: "回调买入法" });
    expect(chip.className).toContain("term-chip");
    expect(screen.queryByRole("dialog")).toBeNull();

    fireEvent.click(chip);

    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain("回调买入法");
    expect(dialog.textContent).toContain("股票涨过一波后休息");
  });

  it("点「知道了」关闭弹窗", async () => {
    renderHint("support");
    fireEvent.click(await screen.findByRole("button", { name: "支撑区" }));
    await screen.findByRole("dialog");

    fireEvent.click(screen.getByRole("button", { name: "知道了" }));

    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
  });

  it("未收录的术语 key 直接渲染 children", async () => {
    render(
      <GlossaryProvider>
        <TermHint termKey="不存在的术语">自定义标签</TermHint>
      </GlossaryProvider>,
    );
    expect(await screen.findByText("自定义标签")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "自定义标签" })).toBeNull();
  });
});

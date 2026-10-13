import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { Tooltip } from "../components/Tooltip";
import { TermHint } from "../components/TermHint";
import { mockedInvoke, settingsStub, stubInvoke } from "../test-utils";
import { GlossaryProvider } from "./GlossaryContext";
import { BeginnerModeProvider } from "./BeginnerModeContext";

const confirmed = { ...settingsStub, firstRunConfirmed: true };

describe("首次启动确认", () => {
  it("未确认时弹出五条风险提示，点「我已阅读并理解」后消失", async () => {
    stubInvoke({ confirm_first_run: { ...settingsStub, firstRunConfirmed: true } });
    render(
      <BeginnerModeProvider>
        <div>内容</div>
      </BeginnerModeProvider>,
    );

    const dialog = await screen.findByRole("dialog");
    expect(dialog.querySelectorAll("li")).toHaveLength(5);
    expect(dialog.textContent).toContain("969.8%");
    expect(dialog.textContent).toContain("T+1");

    fireEvent.click(screen.getByRole("button", { name: "我已阅读并理解" }));

    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(mockedInvoke).toHaveBeenCalledWith("confirm_first_run", undefined);
  });

  it("已经确认过就不再弹出", async () => {
    stubInvoke({ settings_get: confirmed });
    render(
      <BeginnerModeProvider>
        <div>内容</div>
      </BeginnerModeProvider>,
    );

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("settings_get", undefined);
    });
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

describe("小白模式", () => {
  it("关闭后术语标签与悬停气泡不再显示解释", async () => {
    stubInvoke({ settings_get: { ...settingsStub, beginnerMode: false, firstRunConfirmed: true } });
    render(
      <BeginnerModeProvider>
        <GlossaryProvider>
          <TermHint termKey="pullback" />
          <Tooltip text="近 1 个月涨幅排名落在前 20%~30%">
            <span>排名</span>
          </Tooltip>
        </GlossaryProvider>
      </BeginnerModeProvider>,
    );

    // 术语退化成纯文本，没有虚线胶囊按钮。
    expect(await screen.findByText("回调买入法")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "回调买入法" })).toBeNull();
    // 悬停气泡的内容不再渲染。
    expect(screen.queryByRole("tooltip")).toBeNull();
    expect(screen.getByText("排名")).toBeTruthy();
  });
});

import { describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import SettingsPage from "./SettingsPage";
import { BeginnerModeProvider } from "../copy/BeginnerModeContext";
import { appInfoStub, glossaryStub, mockedInvoke, renderAt, settingsStub, stubInvoke } from "../test-utils";

const confirmed = { ...settingsStub, firstRunConfirmed: true };

function renderSettings(overrides: Record<string, unknown> = {}) {
  stubInvoke({ settings_get: confirmed, ...overrides });
  return renderAt(
    "/settings",
    <BeginnerModeProvider>
      <SettingsPage />
    </BeginnerModeProvider>,
  );
}

describe("设置页", () => {
  it("显示数据源、最近同步时间与筛选参数默认值", async () => {
    renderSettings();

    expect(await screen.findByDisplayValue("东方财富 · 前复权")).toBeTruthy();
    expect(screen.getByDisplayValue("尚未同步")).toBeTruthy();

    expect((screen.getByLabelText("单笔风险比例（%）") as HTMLInputElement).value).toBe("0.5");
    expect((screen.getByLabelText("止损距离阈值（%）") as HTMLInputElement).value).toBe("5");
    expect((screen.getByLabelText("流动性阈值（亿元）") as HTMLInputElement).value).toBe("1");
    expect((screen.getByLabelText("近 6 个月涨幅阈值（%）") as HTMLInputElement).value).toBe("30");
    expect((screen.getByLabelText("排名分位下限（%）") as HTMLInputElement).value).toBe("20");
    expect((screen.getByLabelText("排名分位上限（%）") as HTMLInputElement).value).toBe("30");
    expect((screen.getByLabelText("近 20 日极差阈值（%）") as HTMLInputElement).value).toBe("25");
  });

  it("改单笔风险比例后保存，写回的是小数比例", async () => {
    renderSettings();

    const input = await screen.findByLabelText("单笔风险比例（%）");
    fireEvent.change(input, { target: { value: "0.8" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith(
        "settings_update",
        expect.objectContaining({ patch: expect.objectContaining({ riskPct: 0.008 }) }),
      );
    });
  });

  it("关闭小白模式立即写回配置", async () => {
    renderSettings({ settings_update: { ...confirmed, beginnerMode: false } });

    const select = await screen.findByLabelText("小白模式（术语弹窗解释）");
    fireEvent.change(select, { target: { value: "off" } });

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith(
        "settings_update",
        expect.objectContaining({ patch: expect.objectContaining({ beginnerMode: false }) }),
      );
    });
  });

  it("CSV 导入成功后显示写入与跳过行数", async () => {
    renderSettings({
      import_bars_csv: { rows: 2, skipped: 1, errors: ["第 3 行第 3 列「open」不是数字：abc"] },
    });

    const input = await screen.findByLabelText("CSV 文件路径");
    fireEvent.change(input, { target: { value: "/tmp/bars.csv" } });
    fireEvent.click(screen.getByRole("button", { name: "导入" }));

    expect(await screen.findByText(/写入 2 行/)).toBeTruthy();
    expect(screen.getByText(/跳过 1 行/)).toBeTruthy();
    expect(screen.getByText(/第 3 行第 3 列「open」不是数字：abc/)).toBeTruthy();
    expect(mockedInvoke).toHaveBeenCalledWith("import_bars_csv", { path: "/tmp/bars.csv" });
  });

  it("CSV 导入失败时显示明确错误", async () => {
    stubInvoke({
      settings_get: confirmed,
      app_info: appInfoStub,
      glossary: glossaryStub,
      settings_update: confirmed,
    });
    mockedInvoke.mockImplementation((async (command: string) => {
      if (command === "import_bars_csv") {
        throw new Error("CSV 表头缺少必需列「amount」");
      }
      return { settings_get: confirmed, app_info: appInfoStub }[command];
    }) as typeof invoke);

    renderAt(
      "/settings",
      <BeginnerModeProvider>
        <SettingsPage />
      </BeginnerModeProvider>,
    );

    const input = await screen.findByLabelText("CSV 文件路径");
    fireEvent.change(input, { target: { value: "/tmp/broken.csv" } });
    fireEvent.click(screen.getByRole("button", { name: "导入" }));

    expect(await screen.findByText(/CSV 表头缺少必需列「amount」/)).toBeTruthy();
  });

  it("设置页保留风险提示入口，点开可重看五条", async () => {
    renderSettings();

    fireEvent.click(await screen.findByRole("button", { name: "查看风险提示" }));

    const dialog = await screen.findByRole("dialog");
    expect(dialog.querySelectorAll("li")).toHaveLength(5);
  });
});

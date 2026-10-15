import { describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

import SettingsPage from "./SettingsPage";
import { BeginnerModeProvider } from "../copy/BeginnerModeContext";
import {
  appInfoStub,
  glossaryStub,
  mockedInvoke,
  renderAt,
  settingsStub,
  stubInvoke,
  usAppInfoStub,
} from "../test-utils";

const confirmed = { ...settingsStub, firstRunConfirmed: true };

/** `sync_status` 命令的桩数据，设置页内嵌的 SyncPanel 会读它。 */
const syncStatusStub = {
  running: false,
  done: 0,
  total: 0,
  lastSyncAt: null,
  message: "尚未同步",
};

function renderSettings(overrides: Record<string, unknown> = {}) {
  stubInvoke({ settings_get: confirmed, sync_status: syncStatusStub, ...overrides });
  return renderAt(
    "/settings",
    <BeginnerModeProvider>
      <SettingsPage />
    </BeginnerModeProvider>,
  );
}

describe("设置页", () => {
  it("显示数据源、同步面板与筛选参数默认值", async () => {
    renderSettings();

    expect(await screen.findByDisplayValue("通达信 · 前复权")).toBeTruthy();
    expect(await screen.findByText(/最近同步：尚未同步/)).toBeTruthy();

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

    const toggle = await screen.findByLabelText("小白模式（术语弹窗解释）");
    expect(toggle.getAttribute("aria-checked")).toBe("true");
    fireEvent.click(toggle);

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith(
        "settings_update",
        expect.objectContaining({ patch: expect.objectContaining({ beginnerMode: false }) }),
      );
    });
  });

  it("费用模型默认关闭，开启并改佣金费率后一起写回", async () => {
    renderSettings();

    const feeToggle = await screen.findByLabelText("费用模型（交易日志逐笔扣减）");
    expect(feeToggle.getAttribute("aria-checked")).toBe("false");
    expect((screen.getByLabelText("佣金费率（%，含单笔最低 5 元）") as HTMLInputElement).value).toBe(
      "0.025",
    );

    fireEvent.click(feeToggle);
    fireEvent.change(screen.getByLabelText("佣金费率（%，含单笔最低 5 元）"), {
      target: { value: "0.03" },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith(
        "settings_update",
        expect.objectContaining({
          patch: expect.objectContaining({
            feeModelEnabled: true,
            cn: expect.objectContaining({ commissionRate: 0.0003 }),
          }),
        }),
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
      return { settings_get: confirmed, app_info: appInfoStub, sync_status: syncStatusStub }[command];
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

describe("设置页：美股", () => {
  it("按美股口径显示成交额单位、佣金与数据源", async () => {
    stubInvoke({
      app_info: usAppInfoStub,
      settings_get: { ...confirmed, market: "us" },
      sync_status: syncStatusStub,
    });
    renderAt(
      "/settings",
      <BeginnerModeProvider>
        <SettingsPage />
      </BeginnerModeProvider>,
    );

    // 数据源与成交额单位都换成美股那一套。
    expect(await screen.findByDisplayValue("Yahoo Finance · 前复权")).toBeTruthy();
    expect((screen.getByLabelText("流动性阈值（百万美元）") as HTMLInputElement).value).toBe("20");
    // 美股没有最低佣金，标签不再提 5 元。
    expect((screen.getByLabelText("佣金费率（%）") as HTMLInputElement).value).toBe("0");
    expect(screen.getByText(/开启后按佣金逐笔扣减，没有印花税与各项规费/)).toBeTruthy();
  });
});

import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { CommandError, invoke, toMessage } from "./invoke";
import { mockedInvoke } from "../test-utils";

beforeEach(() => {
  mockedInvoke.mockReset();
});

describe("invoke 封装", () => {
  it("把命令名与入参透传给 Tauri 的 invoke", async () => {
    mockedInvoke.mockResolvedValue({ ok: true } as never);

    const result = await invoke<{ ok: boolean }>("settings_update", { patch: { autostart: true } });

    expect(mockedInvoke).toHaveBeenCalledWith("settings_update", { patch: { autostart: true } });
    expect(result).toEqual({ ok: true });
  });

  it("命令失败时抛 CommandError，带上命令名与可展示文案", async () => {
    mockedInvoke.mockRejectedValue("数据库操作失败: 锁超时");

    await expect(invoke("settings_get")).rejects.toBeInstanceOf(CommandError);
    await expect(invoke("settings_get")).rejects.toThrow("数据库操作失败: 锁超时");
  });

  it("toMessage 覆盖字符串、Error 与其他值", () => {
    expect(toMessage("直接文案")).toBe("直接文案");
    expect(toMessage(new Error("错误对象"))).toBe("错误对象");
    expect(toMessage({ code: 1 })).toBe('{"code":1}');
  });
});

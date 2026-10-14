import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { AiCard } from "./AiCard";
import { mockedInvoke, stubInvoke } from "../test-utils";

/** explain / explain_retry 命令的桩返回值。 */
function ready(text: string, cached = false) {
  return { text, generatedAt: "2025-03-20T08:00:00Z", model: "deepseek-chat", cached };
}

beforeEach(() => {
  mockedInvoke.mockReset();
});

describe("AiCard", () => {
  it("渲染解释正文、AI 生成标签与免责声明", async () => {
    stubInvoke({ explain: ready("这只股票近 6 个月上涨 63.8%。") });

    render(<AiCard code="600000" scope={1} />);

    expect(await screen.findByText("这只股票近 6 个月上涨 63.8%。")).toBeTruthy();
    expect(screen.getByText("AI 生成")).toBeTruthy();
    expect(
      screen.getByText("以上只是对图上形态与数据的描述，不构成买卖建议。"),
    ).toBeTruthy();
  });

  it("code 为 null 时不渲染卡片", () => {
    stubInvoke({ explain: ready("不该出现") });

    const { container } = render(<AiCard code={null} scope={1} />);

    expect(container.querySelector('[data-slot="ai-card"]')).toBeNull();
    expect(mockedInvoke).not.toHaveBeenCalled();
  });

  it("生成失败时显示「解释生成失败」与重试按钮，点重试走 explain_retry", async () => {
    let explainCalls = 0;
    mockedInvoke.mockImplementation((async (command: string) => {
      if (command === "explain") {
        explainCalls += 1;
        throw "模型调用失败: 模型返回了空 content";
      }
      if (command === "explain_retry") return ready("重试后的解释");
      throw new Error(`测试桩没有登记命令 ${command}`);
    }) as typeof invoke);

    render(<AiCard code="600000" scope={1} />);

    expect(await screen.findByText("解释生成失败")).toBeTruthy();
    expect(screen.queryByText("重试后的解释")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "重试" }));

    expect(await screen.findByText("重试后的解释")).toBeTruthy();
    expect(screen.queryByText("解释生成失败")).toBeNull();
    expect(explainCalls).toBe(1);
  });

  it("invalidateToken 变化时先让缓存失效再重新生成", async () => {
    const calls: string[] = [];
    mockedInvoke.mockImplementation((async (command: string) => {
      calls.push(command);
      if (command === "explain_invalidate") return undefined;
      return ready(`第 ${calls.filter((name) => name === "explain").length} 版`);
    }) as typeof invoke);

    const { rerender } = render(<AiCard code="600000" scope={2} invalidateToken="a" />);
    expect(await screen.findByText("第 1 版")).toBeTruthy();

    rerender(<AiCard code="600000" scope={2} invalidateToken="b" />);

    expect(await screen.findByText("第 2 版")).toBeTruthy();
    expect(calls).toEqual(["explain", "explain_invalidate", "explain"]);
  });

  it("切换股票时重新取解释", async () => {
    mockedInvoke.mockImplementation((async (_command: string, args?: Record<string, unknown>) =>
      ready(args?.code === "000001" ? "平安银行的解释" : "浦发银行的解释")) as typeof invoke);

    const { rerender } = render(<AiCard code="600000" scope={1} />);
    expect(await screen.findByText("浦发银行的解释")).toBeTruthy();

    rerender(<AiCard code="000001" scope={1} />);

    expect(await screen.findByText("平安银行的解释")).toBeTruthy();
  });

  it("交易日志页用 step 0 调命令", async () => {
    stubInvoke({ explain: ready("本笔按计划在 21.50 元触及 3R 减仓三分之一。") });

    render(<AiCard code="600000" scope="journal" />);

    await screen.findByText("本笔按计划在 21.50 元触及 3R 减仓三分之一。");
    expect(mockedInvoke).toHaveBeenCalledWith("explain", { code: "600000", step: 0 });
  });
});

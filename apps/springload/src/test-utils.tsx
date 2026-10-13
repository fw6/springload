import { render } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import type { ReactNode } from "react";
import { vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import type { Glossary } from "./copy/GlossaryContext";
import type { Settings } from "./api/settings";

/** 被替换成桩的 invoke。测试文件里先 `vi.mock("@tauri-apps/api/core", ...)`。 */
export const mockedInvoke = vi.mocked(invoke);

/** glossary 命令的桩数据。 */
export const glossaryStub: Glossary = {
  terms: [
    {
      key: "pullback",
      label: "回调买入法",
      plain: "股票涨过一波后休息（横盘），等它再次走强时上车，而不是追着已经涨高的价格买。",
    },
    {
      key: "ema",
      label: "EMA 9/21/50",
      plain: "最近 9/21/50 天股价的平均成本线。9 日看短线、21 日看中短线、50 日看中期。",
    },
    {
      key: "support",
      label: "支撑区",
      plain: "股价跌到这个价格附近就容易跌不动的区域，通常叠在均线、前期高点、AVWAP 附近。",
    },
    {
      key: "r",
      label: "R（风险单位）",
      plain: "这一单预设的最大每股亏损额（买入价 − 止损价）。全程用它衡量盈亏，不用百分比。",
    },
  ],
  steps: [
    {
      index: 1,
      title: "强势股筛选",
      goal: "先找出近期走得强、现在正在休息的股票",
      oneLine: "从沪深主板里筛出「涨过一波、现在横着」的股票，得到一张观察名单。",
      detail: "筛选器只看四件事，四条同时满足才会进观察名单。",
      termKeys: ["pullback", "ema"],
      blockedHint: "",
    },
    {
      index: 2,
      title: "标注支撑区",
      goal: "在图上画出跌不动的那条线，看它有多可靠",
      oneLine: "在 K 线图上拖出水平线和支撑区，App 会算出这块区域的置信度。",
      detail: "支撑区就是股价跌到这个位置附近就容易跌不动的地方。",
      termKeys: ["support"],
      blockedHint: "先在第 1 步把股票加进观察名单，再来标注支撑区。",
    },
    {
      index: 3,
      title: "等待买入信号",
      goal: "盯着它回到支撑区之后有没有重新走强",
      oneLine: "价格站上高置信支撑区后，分钟图上出现「下探回升 + 突破前一根高点」才算触发。",
      detail: "这一步分两级，两个条件缺一不可。",
      termKeys: ["support"],
      blockedHint: "先在第 2 步完成支撑区标注，并让支撑区达到高置信。",
    },
    {
      index: 4,
      title: "算仓位与目标位",
      goal: "把「最多亏多少」换算成「买多少股」",
      oneLine: "输入买入价、止损价、账户金额和单笔风险比例，算出股数、投入金额与 3R/5R 目标位。",
      detail: "每股风险 = 买入价 − 止损价。",
      termKeys: ["r"],
      blockedHint: "先在第 3 步等到买入信号，再来算仓位。",
    },
  ],
};

/** app_info 命令的桩数据。 */
export const appInfoStub = {
  version: "0.1.0",
  dataSource: "东方财富 · 前复权",
  lastSyncAt: null,
};

/** settings_get 命令的桩数据。 */
export const settingsStub: Settings = {
  riskPct: 0.005,
  stopDistanceWarn: 0.05,
  liquidityThreshold: 1e8,
  chg6mThreshold: 0.3,
  rankBand: [0.2, 0.3],
  convergenceThreshold: 0.25,
  beginnerMode: true,
  autostart: false,
  feeModelEnabled: false,
  commissionRate: 0.00025,
  firstRunConfirmed: false,
};

/** 给 invoke 装上默认实现；未登记的命令直接失败，避免静默通过。 */
export function stubInvoke(overrides: Record<string, unknown> = {}) {
  const table: Record<string, unknown> = {
    app_info: appInfoStub,
    glossary: glossaryStub,
    settings_get: settingsStub,
    settings_update: settingsStub,
    watchlist_get: [],
    step_unlock: { unlocked: [true, true, true, true], reasons: ["", "", "", ""] },
    corporate_actions_stale: [],
    ...overrides,
  };
  mockedInvoke.mockImplementation((async (command: string) => {
    if (command in table) return table[command];
    throw new Error(`测试桩没有登记命令 ${command}`);
  }) as typeof invoke);
}

/** 用 memory router 在指定路由渲染节点。 */
export function renderAt(path: string, node: ReactNode) {
  return render(<MemoryRouter initialEntries={[path]}>{node}</MemoryRouter>);
}

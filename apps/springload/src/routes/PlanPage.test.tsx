import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import PlanPage, { cardToText, STATUS_LABELS } from "./PlanPage";
import { GlossaryProvider } from "../copy/GlossaryContext";
import type { PlanCard, TradePlan } from "../api/plan";
import { glossaryStub, mockedInvoke, renderAt, settingsStub } from "../test-utils";

/** 一条草稿状态的计划，数值用规格里的示例。 */
const draft: TradePlan = {
  id: "plan-600036-1",
  code: "600036",
  entry: 20.0,
  stop: 19.5,
  account: 100_000,
  riskPct: 0.005,
  perR: 0.5,
  shares: 1000,
  capital: 20_000,
  posPct: 0.2,
  stopDistance: 0.025,
  t3: 21.5,
  t5: 22.5,
  status: "draft",
  zoneId: "zone-1",
  signalId: null,
  createdAt: "2026-10-13T09:00:00Z",
  updatedAt: "2026-10-13T09:00:00Z",
};

const card: PlanCard = {
  plan: draft,
  name: "招商银行",
  board: "main",
  sector: "银行",
  c1: true,
  c2: true,
  c3: true,
  c4: false,
  reduce: { shares3r: 333, shares5r: 333, remaining: 334 },
  actualRiskPct: 0.005,
};

let plans: TradePlan[];
let current: PlanCard;
let confirmed: string[];

beforeEach(() => {
  plans = [draft];
  current = card;
  confirmed = [];
  Object.assign(navigator, {
    clipboard: { writeText: vi.fn(async () => {}) },
  });
  mockedInvoke.mockImplementation((async (command: string, args?: Record<string, unknown>) => {
    switch (command) {
      case "glossary":
        return glossaryStub;
      case "settings_get":
        return settingsStub;
      case "plan_list":
        return plans;
      case "plan_card":
        return current;
      case "support_zones_get":
        return [
          {
            id: "zone-1",
            code: "600036",
            kind: "support",
            low: 19.9,
            high: 20.1,
            confidence: "高",
            spread: 0.01,
          },
        ];
      case "support_refs":
        return { ema21: 20.05, avwap: 20.0, prevHigh: 20.1 };
      case "plan_save":
        return { ...draft, id: String(args?.plan && (args.plan as TradePlan).id) };
      case "plan_set_status":
        return { ...draft, status: String(args?.status) };
      case "plan_confirm_executed":
        confirmed.push(String(args?.id));
        return {
          id: `log-${draft.id}`,
          positionId: `position-${draft.id}`,
          code: "600036",
          name: "招商银行",
          entryDate: "2026-10-14",
          entryPrice: 20.0,
          stopPrice: 19.5,
          exitDate: null,
          exitPrice: null,
          quantity: 1000,
          perR: 0.5,
          rMultiple: null,
          pnl: null,
          fees: 0,
          note: null,
          source: "plan",
          createdAt: "2026-10-14T01:00:00Z",
        };
      default:
        throw new Error(`测试桩没有登记命令 ${command}`);
    }
  }) as never);
});

function renderPlan() {
  return renderAt(
    "/plan",
    <GlossaryProvider>
      <PlanPage />
    </GlossaryProvider>,
  );
}

/** 表里的「标签 → 值」。 */
function fields(container: HTMLElement): Record<string, string> {
  const out: Record<string, string> = {};
  container.querySelectorAll("table tbody tr").forEach((row) => {
    const cells = row.querySelectorAll("td");
    out[cells[0].textContent ?? ""] = cells[1].textContent ?? "";
  });
  return out;
}

describe("交易计划卡", () => {
  it("汇总筛选结论、支撑区、三个参考价与下单字段", async () => {
    const { container } = renderPlan();
    await screen.findByText("交易计划卡");

    const values = await waitFor(() => {
      const map = fields(container);
      expect(Object.keys(map)).toContain("计划风险（占账户）");
      return map;
    });

    expect(values["代码与名称"]).toBe("600036 招商银行");
    expect(values["所属板块"]).toBe("main · 银行");
    expect(values["日线筛选结论"]).toBe("C1 中 · C2 中 · C3 中 · C4 未中");
    expect(values["支撑区价格"]).toBe("19.90 ~ 20.10");
    expect(values["AVWAP / EMA21 / 前期高点"]).toBe("20.00 / 20.05 / 20.10");
    expect(values["买入价"]).toBe("20.00");
    expect(values["止损价"]).toBe("19.50");
    expect(values["3R / 5R 目标位"]).toBe("21.50 / 22.50");
    expect(values["仓位股数"]).toBe("1000 股");
    expect(values["投入金额"]).toBe("20000.00 元");
    expect(values["止损距离"]).toBe("2.50%");
    expect(values["计划风险（占账户）"]).toBe("0.50%");
  });

  it("显示状态与减仓安排", async () => {
    renderPlan();
    await screen.findByText("交易计划卡");

    expect(screen.getByText(STATUS_LABELS.draft)).toBeTruthy();
    const reduce = screen.getByText(/减仓安排/).textContent ?? "";
    expect(reduce).toContain("3R 卖 333 股");
    expect(reduce).toContain("5R 再卖 333 股");
    expect(reduce).toContain("剩余 334 股跟随 9 日均线");
  });

  it("一键复制为文本", async () => {
    renderPlan();
    await screen.findByText("交易计划卡");

    fireEvent.click(screen.getByRole("button", { name: "复制为文本" }));

    expect(await screen.findByText("已复制为文本。")).toBeTruthy();
    const copied = vi.mocked(navigator.clipboard.writeText).mock.calls[0][0] as string;
    expect(copied).toContain("交易计划卡 · 600036 招商银行");
    expect(copied).toContain("仓位股数：1000 股");
    expect(copied).toContain("3R / 5R 目标位：21.50 / 22.50");
  });

  it("存为草稿后提示可以在持仓跟踪页加载", async () => {
    renderPlan();
    await screen.findByText("交易计划卡");

    fireEvent.click(screen.getByRole("button", { name: "存为草稿" }));

    expect(await screen.findByText(/已存为草稿/)).toBeTruthy();
    expect(screen.getByText(/可以在持仓跟踪页加载/)).toBeTruthy();
  });

  it("确认已执行之后写入场记录", async () => {
    renderPlan();
    await screen.findByText("交易计划卡");

    fireEvent.click(screen.getByRole("button", { name: "确认已执行" }));

    expect(await screen.findByText(/交易日志写入入场记录 log-plan-600036-1/)).toBeTruthy();
    expect(confirmed).toEqual([draft.id]);
  });

  it("已执行的计划不再让点确认", async () => {
    current = { ...card, plan: { ...draft, status: "executed" } };
    plans = [current.plan];
    renderPlan();

    const button = (await screen.findByRole("button", { name: "已执行" })) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
  });

  it("带 ?id= 时打开指定的那条计划，不取最近一条", async () => {
    // 前面几条用例的调用记录还留着，这里只看本次渲染发了哪些命令。
    mockedInvoke.mockClear();
    renderAt(
      "/plan?id=plan-600036-9",
      <GlossaryProvider>
        <PlanPage />
      </GlossaryProvider>,
    );

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("plan_card", { id: "plan-600036-9" });
    });
    expect(mockedInvoke).not.toHaveBeenCalledWith("plan_list", expect.anything());
  });

  it("没有计划时给出回到主流程的入口", async () => {
    plans = [];
    renderPlan();

    expect(await screen.findByText(/还没有计划/)).toBeTruthy();
    expect(screen.getByRole("link", { name: "回到主流程" })).toBeTruthy();
  });
});

describe("计划卡文本", () => {
  it("带上三个参考价、减仓安排与统计口径", () => {
    const text = cardToText(card, { ema21: 20.05, avwap: 20.0, prevHigh: 20.1 }, "19.90 ~ 20.10");
    expect(text).toContain("支撑区价格：19.90 ~ 20.10");
    expect(text).toContain("AVWAP / EMA21 / 前期高点：20.00 / 20.05 / 20.10");
    expect(text).toContain("止损距离：2.50%");
    expect(text).toContain("剩余 334 股跟随 9 日均线");
    expect(text).toContain("统计未计入交易成本与滑点。");
  });

  it("没有参考价时用破折号占位", () => {
    expect(cardToText(card, null, "未标注")).toContain("AVWAP / EMA21 / 前期高点：—");
  });
});

import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import JournalPage, { STATS_NOTE } from "./JournalPage";
import { BeginnerModeProvider } from "../copy/BeginnerModeContext";
import { GlossaryProvider } from "../copy/GlossaryContext";
import type { JournalView, ManualEntry } from "../api/journal";
import type { TradeLogEntry } from "../api/plan";
import { glossaryStub, mockedInvoke, renderAt, settingsStub } from "../test-utils";

/** 一笔已出场的交易。 */
const closed: TradeLogEntry = {
  id: "log-1",
  positionId: null,
  code: "600036",
  name: "招商银行",
  entryDate: "2026-10-01",
  entryPrice: 20.0,
  stopPrice: 19.5,
  exitDate: "2026-10-10",
  exitPrice: 21.5,
  quantity: 1000,
  perR: 0.5,
  rMultiple: 3.0,
  pnl: 1500.0,
  fees: 0.0,
  note: "到 3R 减三分之一",
  source: "manual",
  createdAt: "2026-10-10T00:00:00Z",
};

/** 一笔还没出场的。 */
const open: TradeLogEntry = {
  ...closed,
  id: "log-2",
  exitDate: null,
  exitPrice: null,
  rMultiple: null,
  pnl: null,
  note: null,
};

const view: JournalView = {
  entries: [closed, open],
  stats: {
    trades: 1,
    wins: 1,
    winRate: 1.0,
    avgR: 3.0,
    totalPnl: 1500.0,
    totalFees: 0.0,
    note: STATS_NOTE,
  },
  feeModelEnabled: false,
  commissionRate: 0.00025,
};

let current: JournalView;
let added: ManualEntry[];
let exported: string[];
let reviewFails: boolean;

beforeEach(() => {
  current = view;
  added = [];
  exported = [];
  reviewFails = false;
  mockedInvoke.mockImplementation((async (command: string, args?: Record<string, unknown>) => {
    switch (command) {
      case "glossary":
        return glossaryStub;
      case "settings_get":
        return settingsStub;
      case "journal_get":
        return current;
      case "journal_add_manual":
        added.push(args?.entry as ManualEntry);
        return closed;
      case "journal_export_csv":
        exported.push(String(args?.path));
        return undefined;
      case "journal_review":
        if (reviewFails) throw new Error("解释生成失败");
        return { text: "本段时间共 1 笔，平均 R 3.00。", generatedAt: "x", model: "m", cached: true };
      default:
        throw new Error(`测试桩没有登记命令 ${command}`);
    }
  }) as never);
});

function renderJournal() {
  return renderAt(
    "/journal",
    <BeginnerModeProvider>
      <GlossaryProvider>
        <JournalPage />
      </GlossaryProvider>
    </BeginnerModeProvider>,
  );
}

describe("交易日志", () => {
  it("列出每笔交易的字段与统计摘要", async () => {
    renderJournal();

    expect(await screen.findByText("交易笔数")).toBeTruthy();
    const cells = Array.from(document.querySelectorAll(".result-cell")).map((cell) => ({
      label: cell.querySelector("dt")!.textContent ?? "",
      value: cell.querySelector("dd")!.textContent ?? "",
    }));
    expect(cells[0]).toEqual({ label: "交易笔数", value: "1" });
    expect(cells[1]).toEqual({ label: "胜率", value: "100.0%" });
    expect(cells[2].label).toContain("平均 R 倍数");
    expect(cells[2].value).toBe("3.00");
    expect(cells[3]).toEqual({ label: "总盈亏", value: "1500.00" });

    const row = screen.getAllByText(/600036 招商银行/)[0].closest("tr")!;
    const values = Array.from(row.querySelectorAll("td")).map((cell) => cell.textContent ?? "");
    expect(values).toEqual([
      "600036 招商银行",
      "2026-10-01",
      "20.00",
      "19.50",
      "2026-10-10",
      "21.50",
      "3.00",
      "1500.00",
      "0.00",
      "到 3R 减三分之一",
    ]);
  });

  it("未出场的记录显示持有中", async () => {
    renderJournal();
    await screen.findByText("交易笔数");

    expect(screen.getByText("持有中")).toBeTruthy();
  });

  it("统计口径固定标注未计入交易成本与滑点", async () => {
    renderJournal();

    const notice = await screen.findByText(new RegExp(`统计口径：${STATS_NOTE}`));
    expect(notice.textContent).toContain("可在设置页打开费用模型");
  });

  it("费用模型开启时显示扣减后的费用合计", async () => {
    current = {
      ...view,
      feeModelEnabled: true,
      stats: { ...view.stats, totalFees: 23.78515, totalPnl: 1476.21485 },
    };
    renderJournal();

    const notice = await screen.findByText(new RegExp(`统计口径：${STATS_NOTE}`));
    expect(notice.textContent).toContain("已逐笔扣减费用合计 23.79 元");
    expect(screen.getByText(/费用模型已开启/)).toBeTruthy();
  });

  it("手动补录历史交易", async () => {
    renderJournal();
    await screen.findByText("交易笔数");

    fireEvent.click(screen.getByRole("button", { name: "开始补录" }));
    fireEvent.change(screen.getByLabelText("标的"), { target: { value: "600036" } });
    fireEvent.change(screen.getByLabelText("入场日"), { target: { value: "2026-10-01" } });
    fireEvent.change(screen.getByLabelText("买入价"), { target: { value: "20" } });
    fireEvent.change(screen.getByLabelText("止损价"), { target: { value: "19.5" } });
    fireEvent.change(screen.getByLabelText("出场日"), { target: { value: "2026-10-10" } });
    fireEvent.change(screen.getByLabelText("出场价"), { target: { value: "21.5" } });
    fireEvent.click(screen.getByRole("button", { name: "补录" }));

    expect(await screen.findByText("已补录一笔历史交易。")).toBeTruthy();
    expect(added).toHaveLength(1);
    expect(added[0].code).toBe("600036");
    expect(added[0].entryPrice).toBe(20);
    expect(added[0].stopPrice).toBe(19.5);
    expect(added[0].exitPrice).toBe(21.5);
    expect(added[0].exitDate).toBe("2026-10-10");
  });

  it("导出 CSV", async () => {
    renderJournal();
    await screen.findByText("交易笔数");

    fireEvent.click(screen.getByRole("button", { name: "导出 CSV" }));

    expect(await screen.findByText(/已导出 CSV/)).toBeTruthy();
    expect(exported).toHaveLength(1);
    expect(exported[0]).toMatch(/^springload-journal-\d{4}-\d{2}-\d{2}\.csv$/);
  });

  it("AI 复盘总结固定带 AI 生成标签与免责声明", async () => {
    renderJournal();

    expect(await screen.findByText("本段时间共 1 笔，平均 R 3.00。")).toBeTruthy();
    expect(screen.getByText("AI 生成")).toBeTruthy();
    expect(screen.getByText(/不构成买卖建议/)).toBeTruthy();
  });

  it("复盘生成失败时显示解释生成失败与重试", async () => {
    reviewFails = true;
    renderJournal();

    expect(await screen.findByText("解释生成失败")).toBeTruthy();
    expect(screen.getByRole("button", { name: "重试" })).toBeTruthy();
  });
});

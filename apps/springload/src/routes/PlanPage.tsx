import { useCallback, useEffect, useRef, useState } from "react";
import { NavLink, useSearchParams } from "react-router-dom";
import { toMessage } from "../api/invoke";
import {
  planCard,
  planConfirmExecuted,
  planList,
  planSave,
  planSetStatus,
  type PlanCard,
  type PlanStatus,
} from "../api/plan";
import { supportRefs, supportZonesGet, type ZoneRefs } from "../api/zone";
import { TermHint } from "../components/TermHint";
import { riskNotes } from "../copy/riskNotes";

/** 状态的中文名。 */
export const STATUS_LABELS: Record<PlanStatus, string> = {
  draft: "草稿",
  triggered: "已触发",
  executed: "已执行",
  abandoned: "已放弃",
};

/** 价格保留两位小数。 */
function price(value: number): string {
  return value.toFixed(2);
}

/** 金额按元展示。 */
function money(value: number): string {
  return value.toFixed(2);
}

/** 比例按百分数展示，两位小数。 */
function pct(value: number): string {
  return `${(value * 100).toFixed(2)}%`;
}

/** 三条判定的显示文案。 */
function hit(value: boolean | null): string {
  return value === null ? "—" : value ? "中" : "未中";
}

/** 三个参考价的显示文案。 */
function refsText(refs: ZoneRefs | null): string {
  if (refs === null) return "—";
  return `${price(refs.avwap ?? 0)} / ${price(refs.ema21 ?? 0)} / ${price(refs.prevHigh ?? 0)}`;
}

/** 把计划卡拼成可以带走的纯文本。 */
export function cardToText(card: PlanCard, refs: ZoneRefs | null, zone: string): string {
  const plan = card.plan;
  return [
    `交易计划卡 · ${plan.code} ${card.name ?? ""}`.trim(),
    `所属板块：${card.board ?? "—"} · ${card.sector ?? "—"}`,
    `日线筛选结论：C1 ${hit(card.c1)} · C2 ${hit(card.c2)} · C3 ${hit(card.c3)} · C4 ${hit(card.c4)}`,
    `支撑区价格：${zone}`,
    `AVWAP / EMA21 / 前期高点：${refsText(refs)}`,
    `买入价：${price(plan.entry)}`,
    `止损价：${price(plan.stop)}`,
    `3R / 5R 目标位：${price(plan.t3)} / ${price(plan.t5)}`,
    `仓位股数：${plan.shares} 股`,
    `投入金额：${money(plan.capital)} 元`,
    `止损距离：${pct(plan.stopDistance)}`,
    `计划风险（占账户）：${pct(card.actualRiskPct)}`,
    `减仓安排：3R 卖 ${card.reduce.shares3r} 股、5R 再卖 ${card.reduce.shares5r} 股，剩余 ${card.reduce.remaining} 股跟随 9 日均线`,
    riskNotes.costFree,
  ].join("\n");
}

/** 计划卡：把第 4 步的结果打包成一页，可以复制、导出、存草稿、确认执行。 */
export default function PlanPage() {
  const [card, setCard] = useState<PlanCard | null>(null);
  const [refs, setRefs] = useState<ZoneRefs | null>(null);
  const [zone, setZone] = useState<string>("未标注");
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const sheet = useRef<HTMLDivElement>(null);
  const [searchParams] = useSearchParams();
  const requestedId = searchParams.get("id");

  const load = useCallback(async () => {
    try {
      // 持仓页的草稿列表带 `?id=` 指到具体一条；没有就取最近建的那条。
      let id = requestedId;
      if (id === null) {
        const plans = await planList();
        if (plans.length === 0) {
          setCard(null);
          setError(null);
          return;
        }
        id = [...plans].sort((a, b) => b.createdAt.localeCompare(a.createdAt))[0].id;
      }
      const next = await planCard(id);
      setCard(next);
      setError(null);

      const zones = await supportZonesGet(next.plan.code);
      const picked = zones.find((item) => item.id === next.plan.zoneId) ?? zones[0];
      setZone(picked === undefined ? "未标注" : `${price(picked.low)} ~ ${price(picked.high)}`);
      setRefs(await supportRefs(next.plan.code, null));
    } catch (cause: unknown) {
      setError(toMessage(cause));
    }
  }, [requestedId]);

  useEffect(() => {
    void load();
  }, [load]);

  const run = useCallback(async (action: () => Promise<string>) => {
    setBusy(true);
    try {
      setStatus(await action());
      setError(null);
    } catch (cause: unknown) {
      setError(toMessage(cause));
    } finally {
      setBusy(false);
    }
  }, []);

  const copyText = useCallback(async () => {
    if (card === null) return;
    await run(async () => {
      await navigator.clipboard.writeText(cardToText(card, refs, zone));
      return "已复制为文本。";
    });
  }, [card, refs, run, zone]);

  const exportAs = useCallback(
    async (kind: "png" | "pdf") => {
      const node = sheet.current;
      if (card === null || node === null) return;
      await run(async () => {
        const { toPng } = await import("html-to-image");
        const dataUrl = await toPng(node, { backgroundColor: "#ffffff" });
        const name = `springload-plan-${card.plan.code}`;
        if (kind === "png") {
          const link = document.createElement("a");
          link.href = dataUrl;
          link.download = `${name}.png`;
          link.click();
          return "已导出 PNG。";
        }
        const { jsPDF } = await import("jspdf");
        const pdf = new jsPDF({
          orientation: node.offsetWidth >= node.offsetHeight ? "landscape" : "portrait",
          unit: "px",
          format: [node.offsetWidth, node.offsetHeight],
        });
        pdf.addImage(dataUrl, "PNG", 0, 0, node.offsetWidth, node.offsetHeight);
        pdf.save(`${name}.pdf`);
        return "已导出 PDF。";
      });
    },
    [card, run],
  );

  const saveDraft = useCallback(async () => {
    if (card === null) return;
    await run(async () => {
      const saved = await planSave({
        id: card.plan.id,
        code: card.plan.code,
        entry: card.plan.entry,
        stop: card.plan.stop,
        account: card.plan.account,
        riskPct: card.plan.riskPct,
        zoneId: card.plan.zoneId,
        signalId: card.plan.signalId,
      });
      await load();
      return `已存为草稿（${saved.id}）。可以在持仓跟踪页加载。`;
    });
  }, [card, load, run]);

  const changeStatus = useCallback(
    async (next: PlanStatus, message: string) => {
      if (card === null) return;
      await run(async () => {
        await planSetStatus(card.plan.id, next);
        await load();
        return message;
      });
    },
    [card, load, run],
  );

  const confirm = useCallback(async () => {
    if (card === null) return;
    await run(async () => {
      const entry = await planConfirmExecuted(card.plan.id, new Date().toISOString().slice(0, 10));
      await load();
      return `已执行，交易日志写入入场记录 ${entry.id}。`;
    });
  }, [card, load, run]);

  if (card === null) {
    return (
      <div className="page">
        <div className="chart-card" style={{ maxWidth: 760 }}>
          <div className="panel-head">
            <h2>交易计划卡</h2>
          </div>
          <div className="panel-body">
            <div className="notice" data-tone="info">
              还没有计划。先走完主流程四步，在第 4 步点「生成计划卡」。
            </div>
            {error !== null ? (
              <div className="notice" data-tone="risk">
                {error}
              </div>
            ) : null}
            <div className="panel-foot">
              <NavLink className="primary" to="/">
                回到主流程
              </NavLink>
            </div>
          </div>
        </div>
      </div>
    );
  }

  const plan = card.plan;
  const groups: Array<{ title: string; fields: Array<[string, string]> }> = [
    {
      title: "标的",
      fields: [
        ["代码与名称", `${plan.code} ${card.name ?? ""}`.trim()],
        ["所属板块", `${card.board ?? "—"} · ${card.sector ?? "—"}`],
        [
          "日线筛选结论",
          `C1 ${hit(card.c1)} · C2 ${hit(card.c2)} · C3 ${hit(card.c3)} · C4 ${hit(card.c4)}`,
        ],
      ],
    },
    {
      title: "支撑区",
      fields: [
        ["支撑区价格", zone],
        ["AVWAP / EMA21 / 前期高点", refsText(refs)],
      ],
    },
    {
      title: "下单",
      fields: [
        ["买入价", price(plan.entry)],
        ["止损价", price(plan.stop)],
        ["3R / 5R 目标位", `${price(plan.t3)} / ${price(plan.t5)}`],
        ["仓位股数", `${plan.shares} 股`],
        ["投入金额", `${money(plan.capital)} 元`],
        ["止损距离", pct(plan.stopDistance)],
        ["计划风险（占账户）", pct(card.actualRiskPct)],
      ],
    },
  ];

  return (
    <div className="page">
      <div className="chart-card" style={{ maxWidth: 760 }}>
        <div className="panel-head">
          <h2>交易计划卡</h2>
          <span className="toolbar-spacer" />
          <span className="pill" data-tone="info">
            {STATUS_LABELS[plan.status]}
          </span>
        </div>
        <div className="panel-body">
          <div ref={sheet}>
            {groups.map((group) => (
              <div key={group.title}>
                <h3 className="block-title">{group.title}</h3>
                <table className="grid">
                  <tbody>
                    {group.fields.map(([label, value]) => (
                      <tr key={label}>
                        <td className="left" style={{ color: "#64748b", width: 220 }}>
                          {label}
                        </td>
                        <td className="left mono">{value}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ))}
            <div className="notice" data-tone="info">
              减仓安排：3R 卖 {card.reduce.shares3r} 股、5R 再卖 {card.reduce.shares5r} 股，剩余{" "}
              {card.reduce.remaining} 股跟随 9 日均线。
              <br />
              {riskNotes.costFree}
              {riskNotes.slippage}
            </div>
          </div>

          {error !== null ? (
            <div className="notice" data-tone="risk">
              {error}
            </div>
          ) : null}
          {status !== null ? (
            <div className="notice" data-tone="info">
              {status}
            </div>
          ) : null}

          <p className="hint">
            计划风险是理论值，<TermHint termKey="slippage" />
            会让实际亏损超过预设的账户风险。
          </p>

          <div className="panel-foot" style={{ borderTop: "none", padding: 0 }}>
            <button type="button" className="primary" onClick={() => void copyText()} disabled={busy}>
              复制为文本
            </button>
            <button
              type="button"
              className="ghost"
              onClick={() => void exportAs("png")}
              disabled={busy}
            >
              导出 PNG
            </button>
            <button
              type="button"
              className="ghost"
              onClick={() => void exportAs("pdf")}
              disabled={busy}
            >
              导出 PDF
            </button>
            <button type="button" className="ghost" onClick={() => void saveDraft()} disabled={busy}>
              存为草稿
            </button>
            <button
              type="button"
              className="ghost"
              onClick={() => void changeStatus("abandoned", "已标记为放弃。")}
              disabled={busy}
            >
              放弃
            </button>
            <button
              type="button"
              className="primary"
              onClick={() => void confirm()}
              disabled={busy || plan.status === "executed"}
            >
              {plan.status === "executed" ? "已执行" : "确认已执行"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

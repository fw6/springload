import { useCallback, useEffect, useState } from "react";
import { NavLink } from "react-router-dom";
import { toMessage } from "../api/invoke";
import { planList, type TradePlan } from "../api/plan";
import {
  positionExit,
  positionsGet,
  positionsRefreshQuotes,
  type Alert,
  type PositionView,
} from "../api/positions";
import { TermHint } from "../components/TermHint";
import { riskNotes } from "../copy/riskNotes";

/** 刷新间隔：每分钟一次。 */
export const REFRESH_INTERVAL_MS = 60_000;

/** 价格保留两位小数；没有数据时显示破折号。 */
function price(value: number | null): string {
  return value === null ? "—" : value.toFixed(2);
}

/** 提示的胶囊等级：止损与 5R 减仓最显眼。 */
function levelOf(kind: Alert["kind"]): "高" | "中" | "低" {
  if (kind === "stopLoss" || kind === "reduce5R") return "高";
  if (kind === "belowEma9" || kind === "reduce3R") return "中";
  return "低";
}

/** 一行持仓。 */
function PositionRow({
  view,
  onExit,
  busy,
}: {
  view: PositionView;
  onExit: (id: string, price: number, date: string) => void;
  busy: boolean;
}) {
  const [exitPrice, setExitPrice] = useState<string>(view.last === null ? "" : String(view.last));
  const [exitDate, setExitDate] = useState<string>(new Date().toISOString().slice(0, 10));

  return (
    <div className="result-cell" style={{ display: "block", marginBottom: 12 }}>
      <div style={{ display: "flex", gap: 16, alignItems: "center" }}>
        <div style={{ minWidth: 180 }}>
          <div style={{ fontWeight: 600 }}>
            {view.name ?? "—"}{" "}
            <span className="mono" style={{ color: "#64748b" }}>
              {view.code}
            </span>
          </div>
          <div className="hint">
            入场 {view.entryDate} · {view.quantity} 股
          </div>
        </div>
        <div className="hint">
          买入 {price(view.entryPrice)} ｜ 止损 {price(view.stop)} ｜ 最新 {price(view.last)} ｜ 9
          日 EMA {price(view.ema9)} ｜ 3R {price(view.t3)} ｜ 5R {price(view.t5)}
        </div>
        <span className="toolbar-spacer" />
        {view.alerts.length === 0 ? (
          <span className="pill" data-level="低">
            持有
          </span>
        ) : (
          view.alerts.map((alert) => (
            <span key={alert.kind} className="pill" data-level={levelOf(alert.kind)}>
              {alert.message}
            </span>
          ))
        )}
      </div>

      {view.alerts.map((alert) => (
        <div key={alert.kind} className="hint" style={{ marginTop: 4 }}>
          {alert.message}：信号日 {alert.signalDate}，可执行日 {alert.executableDate}
          （T+1，当天买入的次日才能卖）
        </div>
      ))}

      <div className="field-row" style={{ marginTop: 8 }}>
        <div className="field">
          <label htmlFor={`exit-price-${view.id}`}>实际成交价（元/股）</label>
          <input
            id={`exit-price-${view.id}`}
            type="number"
            step="0.01"
            value={exitPrice}
            onChange={(event) => setExitPrice(event.target.value)}
          />
        </div>
        <div className="field">
          <label htmlFor={`exit-date-${view.id}`}>出场日</label>
          <input
            id={`exit-date-${view.id}`}
            type="date"
            value={exitDate}
            onChange={(event) => setExitDate(event.target.value)}
          />
        </div>
        <div className="field">
          <button
            type="button"
            className="primary"
            disabled={busy || exitPrice.trim() === ""}
            onClick={() => onExit(view.id, Number(exitPrice), exitDate)}
          >
            填入成交价并离场
          </button>
        </div>
      </div>
    </div>
  );
}

/** 持仓跟踪：已执行的仓位、最新价、离场提示与填成交价。 */
export default function PositionsPage() {
  const [views, setViews] = useState<PositionView[]>([]);
  const [drafts, setDrafts] = useState<TradePlan[]>([]);
  const [refreshedAt, setRefreshedAt] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setViews(await positionsGet());
      setDrafts(await planList("draft"));
      setError(null);
    } catch (cause: unknown) {
      setError(toMessage(cause));
    }
  }, []);

  const refresh = useCallback(async () => {
    try {
      const next = await positionsRefreshQuotes();
      setViews(next);
      setRefreshedAt(new Date().toLocaleTimeString("zh-CN", { hour12: false }));
      setError(null);
    } catch (cause: unknown) {
      setError(toMessage(cause));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  // 每分钟刷新一次最新价。
  useEffect(() => {
    const timer = window.setInterval(() => {
      void refresh();
    }, REFRESH_INTERVAL_MS);
    return () => {
      window.clearInterval(timer);
    };
  }, [refresh]);

  const exit = useCallback(
    async (id: string, exitPrice: number, exitDate: string) => {
      setBusy(true);
      try {
        const entry = await positionExit(id, exitPrice, exitDate);
        setStatus(
          `已离场：出场价 ${price(entry.exitPrice)}，R 倍数 ${
            entry.rMultiple === null ? "—" : entry.rMultiple.toFixed(2)
          }，盈亏 ${entry.pnl === null ? "—" : entry.pnl.toFixed(2)} 元。`,
        );
        setError(null);
        await load();
      } catch (cause: unknown) {
        setError(toMessage(cause));
      } finally {
        setBusy(false);
      }
    },
    [load],
  );

  return (
    <div className="page">
      <div className="chart-card">
        <div className="panel-head">
          <h2>持仓跟踪</h2>
          <span className="toolbar-spacer" />
          <span className="hint">
            每 60 秒刷新一次{refreshedAt === null ? "" : ` · 最近刷新 ${refreshedAt}`}
          </span>
          <button type="button" className="ghost" onClick={() => void refresh()} disabled={busy}>
            立即刷新
          </button>
        </div>
        <div className="panel-body">
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

          {views.length === 0 ? (
            <div className="notice" data-tone="info">
              还没有持仓。在计划卡上确认「已执行」之后，这里会出现这条仓位。
            </div>
          ) : (
            views.map((view) => (
              <PositionRow key={view.id} view={view} onExit={exit} busy={busy} />
            ))
          )}

          <div className="notice" data-tone="info">
            {riskNotes.tPlus1}
          </div>

          <div className="block-title">计划草稿（存为草稿之后在这里加载）</div>
          {drafts.length === 0 ? (
            <div className="hint">还没有草稿。</div>
          ) : (
            <table className="grid">
              <thead>
                <tr>
                  <th className="left">代码</th>
                  <th>买入价</th>
                  <th>止损价</th>
                  <th>股数</th>
                  <th>3R / 5R</th>
                  <th className="left">操作</th>
                </tr>
              </thead>
              <tbody>
                {drafts.map((plan) => (
                  <tr key={plan.id}>
                    <td className="left mono">{plan.code}</td>
                    <td className="mono">{price(plan.entry)}</td>
                    <td className="mono">{price(plan.stop)}</td>
                    <td className="mono">{plan.shares}</td>
                    <td className="mono">
                      {price(plan.t3)} / {price(plan.t5)}
                    </td>
                    <td className="left">
                      <NavLink className="ghost" to={`/plan?id=${plan.id}`}>
                        在计划卡打开
                      </NavLink>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}

          <p className="hint">
            <TermHint termKey="ema" />
            被跌破只说明趋势可能走坏，不是必须卖出的信号。
          </p>
        </div>
      </div>
    </div>
  );
}

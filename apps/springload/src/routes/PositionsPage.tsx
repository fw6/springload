import { useCallback, useEffect, useState, useTransition } from "react";
import { Link } from "react-router-dom";
import { toMessage } from "../api/invoke";
import { planList, type TradePlan } from "../api/plan";
import {
  positionExit,
  positionsGet,
  positionsRefreshQuotes,
  type Alert,
  type PositionView,
} from "../api/positions";
import { DataTable, Td, Th, Tr } from "../components/DataTable";
import { Panel, SectionHead } from "../components/Panel";
import { TermHint } from "../components/TermHint";
import { Alert as Note, AlertContent, AlertDescription, AlertIcon } from "../components/ui/alert";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { Stamp, type StampStatus } from "../components/ui/stamp";
import { useCurrency, useMarket } from "../copy/MarketContext";
import { riskNotes, sellTiming } from "../copy/riskNotes";

/** 刷新间隔：每分钟一次。 */
export const REFRESH_INTERVAL_MS = 60_000;

/** 价格保留两位小数；没有数据时显示破折号。 */
function price(value: number | null): string {
  return value === null ? "—" : value.toFixed(2);
}

/** 提示的戳记等级：止损与 5R 减仓最显眼。 */
function levelOf(kind: Alert["kind"]): StampStatus {
  if (kind === "stopLoss" || kind === "reduce5R") return "danger";
  if (kind === "belowEma9" || kind === "reduce3R") return "warning";
  return "neutral";
}

/** 一条持仓：标的一行、关键价位一行、提示一行，下面是离场填写。 */
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
  const currency = useCurrency();
  const { market } = useMarket();

  return (
    <article className="flex flex-col gap-2.5 border border-rule bg-slip p-3">
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
        <div className="min-w-[11rem]">
          <div className="font-display text-lede font-bold text-ink">
            {view.name ?? "—"} <span className="font-mono text-body text-ink-soft">{view.code}</span>
          </div>
          <div className="text-note text-ink-soft">
            入场 {view.entryDate} · {view.quantity} 股
          </div>
        </div>
        <p className="font-mono text-note text-ink-soft">
          买入 {price(view.entryPrice)} ｜ 止损 {price(view.stop)} ｜ 最新 {price(view.last)} ｜ 9
          日 EMA {price(view.ema9)} ｜ 3R {price(view.t3)} ｜ 5R {price(view.t5)}
        </p>
        <span className="ml-auto flex flex-wrap items-center gap-1.5">
          {view.alerts.length === 0 ? (
            <Stamp status="neutral" size="sm">
              持有
            </Stamp>
          ) : (
            view.alerts.map((alert) => (
              <Stamp key={alert.kind} status={levelOf(alert.kind)} size="sm">
                {alert.message}
              </Stamp>
            ))
          )}
        </span>
      </div>

      {view.alerts.map((alert) => (
        <p key={alert.kind} className="text-note text-ink-soft">
          {alert.message}：信号日 {alert.signalDate}，可执行日 {alert.executableDate}
          （{sellTiming(market)}）
        </p>
      ))}

      <div className="grid grid-cols-2 gap-3 lg:grid-cols-3">
        <Input
          label={`实际成交价（${currency}/股）`}
          type="number"
          step="0.01"
          value={exitPrice}
          onChange={setExitPrice}
        />
        <Input label="出场日" type="date" value={exitDate} onChange={setExitDate} />
        <div className="flex items-end">
          <Button
            size="sm"
            disabled={busy || exitPrice.trim() === ""}
            onClick={() => onExit(view.id, Number(exitPrice), exitDate)}
          >
            填入成交价并离场
          </Button>
        </div>
      </div>
    </article>
  );
}

/** 持仓跟踪：已执行的仓位、最新价、离场提示与填成交价。 */
export default function PositionsPage() {
  const currency = useCurrency();
  const { market } = useMarket();
  const [views, setViews] = useState<PositionView[]>([]);
  const [drafts, setDrafts] = useState<TradePlan[]>([]);
  const [refreshedAt, setRefreshedAt] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [busy, startBusy] = useTransition();

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
    (id: string, exitPrice: number, exitDate: string) => {
      startBusy(async () => {
        try {
          const entry = await positionExit(id, exitPrice, exitDate);
          setStatus(
            `已离场：出场价 ${price(entry.exitPrice)}，R 倍数 ${
              entry.rMultiple === null ? "—" : entry.rMultiple.toFixed(2)
            }，盈亏 ${entry.pnl === null ? "—" : entry.pnl.toFixed(2)} ${currency}。`,
          );
          setError(null);
          await load();
        } catch (cause: unknown) {
          setError(toMessage(cause));
        }
      });
    },
    [load],
  );

  return (
    <div className="h-full overflow-auto p-4">
      <Panel
        title="持仓跟踪"
        action={
          <>
            <span className="text-note text-ink-soft">
              每 60 秒刷新一次{refreshedAt === null ? "" : ` · 最近刷新 ${refreshedAt}`}
            </span>
            <Button size="sm" variant="secondary" onClick={() => refresh()} disabled={busy}>
              立即刷新
            </Button>
          </>
        }
      >
        {error !== null ? (
          <Note variant="destructive">
            <AlertIcon />
            <AlertContent>
              <AlertDescription>{error}</AlertDescription>
            </AlertContent>
          </Note>
        ) : null}
        {status !== null ? (
          <Note variant="info">
            <AlertContent>
              <AlertDescription>{status}</AlertDescription>
            </AlertContent>
          </Note>
        ) : null}

        {views.length === 0 ? (
          <Note variant="info">
            <AlertContent>
              <AlertDescription>
                还没有持仓。在计划卡上确认「已执行」之后，这里会出现这条仓位。
              </AlertDescription>
            </AlertContent>
          </Note>
        ) : (
          views.map((view) => (
            <PositionRow key={view.id} view={view} onExit={exit} busy={busy} />
          ))
        )}

        <Note variant="info">
          <AlertIcon />
          <AlertContent>
            <AlertDescription>
              {market === "us" ? riskNotes.usSession : riskNotes.tPlusOne}
            </AlertDescription>
          </AlertContent>
        </Note>

        <SectionHead>计划草稿（存为草稿之后在这里加载）</SectionHead>
        {drafts.length === 0 ? (
          <p className="text-note text-ink-soft">还没有草稿。</p>
        ) : (
          <DataTable>
            <thead>
              <tr>
                <Th>代码</Th>
                <Th align="right">买入价</Th>
                <Th align="right">止损价</Th>
                <Th align="right">股数</Th>
                <Th align="right">3R / 5R</Th>
                <Th>操作</Th>
              </tr>
            </thead>
            <tbody>
              {drafts.map((plan) => (
                <Tr key={plan.id}>
                  <Td>{plan.code}</Td>
                  <Td align="right">{price(plan.entry)}</Td>
                  <Td align="right">{price(plan.stop)}</Td>
                  <Td align="right">{plan.shares}</Td>
                  <Td align="right">
                    {price(plan.t3)} / {price(plan.t5)}
                  </Td>
                  <Td>
                    <Link
                      to={`/plan?id=${plan.id}`}
                      className="inline-flex h-7 items-center border border-ink bg-slip px-2.5 text-note font-medium text-ink transition-colors hover:bg-ink hover:text-paper"
                    >
                      在计划卡打开
                    </Link>
                  </Td>
                </Tr>
              ))}
            </tbody>
          </DataTable>
        )}

        <p className="text-note text-ink-soft">
          <TermHint termKey="ema" />
          被跌破只说明趋势可能走坏；要不要离场由自己判断。
        </p>
      </Panel>
    </div>
  );
}

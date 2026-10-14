import { useCallback, useEffect, useState, useTransition } from "react";
import {
  corporateActionsStale,
  resyncAdjusted,
  type ResyncReport,
  type StaleStock,
} from "../api/adjustment";
import { toMessage } from "../api/invoke";
import { Button, StatefulButton } from "./ui/button";

/**
 * 复权维护栏：显示有多少只股票的前复权历史需要重算，并提供「重新同步」入口。
 *
 * 除权除息之后，除权日之前的历史会被整体改写；这一栏调用 `corporate_actions_stale`
 * 拿到需要重算的股票，点「重新同步」调用 `resync_adjusted` 重算并刷新列表。
 */
export function AdjustmentPanel() {
  const [stale, setStale] = useState<StaleStock[] | null>(null);
  const [report, setReport] = useState<ResyncReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, startBusy] = useTransition();

  const check = useCallback(() => {
    startBusy(async () => {
      try {
        setStale(await corporateActionsStale());
        setError(null);
      } catch (cause: unknown) {
        setError(toMessage(cause));
      }
    });
  }, []);

  useEffect(() => {
    check();
  }, [check]);

  const resync = () => {
    if (stale === null || stale.length === 0) return;
    startBusy(async () => {
      try {
        const result = await resyncAdjusted(stale.map((item) => item.code));
        setReport(result);
        setStale(await corporateActionsStale());
        setError(null);
      } catch (cause: unknown) {
        setError(toMessage(cause));
      }
    });
  };

  const count = stale?.length ?? 0;

  return (
    <section aria-label="复权维护" className="flex flex-col gap-2.5 border-t border-rule pt-3">
      <div className="flex items-center gap-3">
        <h3 className="font-display text-body font-bold text-ink">复权维护</h3>
        <Button
          className="ml-auto"
          size="sm"
          variant="ghost"
          onClick={() => check()}
          disabled={busy}
        >
          {busy ? "检查中…" : "检查"}
        </Button>
      </div>

      {stale === null ? (
        <p className="text-note text-ink-soft">正在检查复权…</p>
      ) : count === 0 ? (
        <p className="text-note text-ink-soft">没有需要重算的股票。</p>
      ) : (
        <>
          <p className="text-body text-ink">需要重算的股票：{count} 只</p>
          <ul className="flex flex-col gap-0.5 text-note text-ink-soft">
            {stale.map((item) => (
              <li key={item.code} className="border-l border-rule pl-2">
                {item.code} {item.name}
                {item.exDate !== null ? ` · 除权除息日 ${item.exDate}` : ""}
              </li>
            ))}
          </ul>
          <div>
            <StatefulButton
              size="sm"
              state={busy ? "loading" : "idle"}
              loadingText="重算中…"
              onClick={() => resync()}
            >
              重新同步
            </StatefulButton>
          </div>
        </>
      )}

      {report !== null ? (
        <p className="text-note text-ink-soft">
          本次重算 {report.stocks} 只股票、{report.bars} 根日线，用时 {report.elapsedMs} 毫秒
          {report.errors.length > 0 ? `，${report.errors.length} 只失败` : ""}
        </p>
      ) : null}

      {error !== null ? <p className="text-note text-alert">复权维护失败：{error}</p> : null}
    </section>
  );
}

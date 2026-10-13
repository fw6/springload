import { useCallback, useEffect, useState } from "react";
import {
  corporateActionsStale,
  resyncAdjusted,
  type ResyncReport,
  type StaleStock,
} from "../api/adjustment";

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * 复权维护面板：显示有多少只股票的前复权历史需要重算，并提供「重新同步」入口。
 *
 * 除权除息之后，除权日之前的历史会被整体改写；面板调用 `corporate_actions_stale`
 * 拿到需要重算的股票，点「重新同步」调用 `resync_adjusted` 重算并刷新列表。
 */
export function AdjustmentPanel() {
  const [stale, setStale] = useState<StaleStock[] | null>(null);
  const [report, setReport] = useState<ResyncReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const check = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      setStale(await corporateActionsStale());
    } catch (err: unknown) {
      setError(message(err));
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    void check();
  }, [check]);

  const resync = async () => {
    if (stale === null || stale.length === 0) return;
    setBusy(true);
    setError(null);
    setReport(null);
    try {
      const result = await resyncAdjusted(stale.map((item) => item.code));
      setReport(result);
      setStale(await corporateActionsStale());
    } catch (err: unknown) {
      setError(message(err));
    } finally {
      setBusy(false);
    }
  };

  const count = stale?.length ?? 0;

  return (
    <section
      className="panel adjustment-panel"
      aria-label="复权维护"
      style={{ maxWidth: 680, marginTop: 16 }}
    >
      <div className="panel-head">
        <h2>复权维护</h2>
        <span className="toolbar-spacer" />
        <button type="button" className="ghost" onClick={check} disabled={busy}>
          {busy ? "检查中…" : "检查"}
        </button>
      </div>
      <div className="panel-body">
        {stale === null ? (
          <p className="hint">正在检查复权…</p>
        ) : count === 0 ? (
          <p className="hint">没有需要重算的股票。</p>
        ) : (
          <>
            <p>需要重算的股票：{count} 只</p>
            <ul className="adjustment-list">
              {stale.map((item) => (
                <li key={item.code}>
                  {item.code} {item.name}
                  {item.exDate !== null ? ` · 除权除息日 ${item.exDate}` : ""}
                </li>
              ))}
            </ul>
            <div>
              <button type="button" className="primary" onClick={resync} disabled={busy}>
                {busy ? "重算中…" : "重新同步"}
              </button>
            </div>
          </>
        )}

        {report !== null ? (
          <p className="hint">
            本次重算 {report.stocks} 只股票、{report.bars} 根日线，用时 {report.elapsedMs} 毫秒
            {report.errors.length > 0 ? `，${report.errors.length} 只失败` : ""}
          </p>
        ) : null}

        {error !== null ? (
          <p className="notice" data-tone="risk">
            复权维护失败：{error}
          </p>
        ) : null}
      </div>
    </section>
  );
}

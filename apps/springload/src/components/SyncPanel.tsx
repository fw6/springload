import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  SYNC_PROGRESS_EVENT,
  syncMarket,
  syncStatus,
  type SyncReport,
  type SyncStatus,
} from "../api/market";

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** 把 ISO 时间显示成本地时间；无法解析时原样显示。 */
function displayTime(value: string | null): string {
  if (value === null) return "尚未同步";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

/** 进度百分比，`total` 为 0 时按 0 处理。 */
function percent(status: SyncStatus): number {
  if (status.total <= 0) return 0;
  return Math.min(100, Math.round((status.done / status.total) * 100));
}

/**
 * 同步面板：显示同步进度与最近同步时间，并可以手动触发一次回补。
 *
 * 进度来自 `sync://progress` 事件，初始状态与最近同步时间来自 `sync_status` 命令。
 */
export function SyncPanel() {
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [report, setReport] = useState<SyncReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    listen<SyncStatus>(SYNC_PROGRESS_EVENT, (event) => {
      if (active) setStatus(event.payload);
    })
      .then((stop) => {
        if (active) {
          unlisten = stop;
        } else {
          stop();
        }
      })
      .catch((err: unknown) => {
        if (active) setError(message(err));
      });
    syncStatus()
      .then((next) => {
        if (active) setStatus(next);
      })
      .catch((err: unknown) => {
        if (active) setError(message(err));
      });
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  const start = async () => {
    setBusy(true);
    setError(null);
    try {
      const result = await syncMarket(false);
      setReport(result);
      setStatus(await syncStatus());
    } catch (err: unknown) {
      setError(message(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="panel sync-panel" aria-label="行情同步">
      <div className="sync-panel-head">
        <h3>行情同步</h3>
        <button type="button" onClick={start} disabled={busy || (status?.running ?? false)}>
          {status?.running ? "同步中…" : "立即同步"}
        </button>
      </div>

      {status !== null ? (
        <>
          <p className="sync-panel-time">最近同步：{displayTime(status.lastSyncAt)}</p>
          <div
            className="sync-progress"
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={percent(status)}
            aria-label="同步进度"
            style={{ height: 8, borderRadius: 4, background: "#e5e7eb", overflow: "hidden" }}
          >
            <div
              className="sync-progress-fill"
              style={{ width: `${percent(status)}%`, height: "100%", background: "#2563eb" }}
            />
          </div>
          <p className="sync-panel-message">
            {status.total > 0 ? `${status.done} / ${status.total} · ` : ""}
            {status.message}
          </p>
        </>
      ) : (
        <p className="hint">正在读取同步状态…</p>
      )}

      {report !== null ? (
        <p className="sync-panel-report">
          本次写入 {report.stocks} 只股票、{report.bars} 根日线，用时 {report.elapsedMs} 毫秒
          {report.errors.length > 0 ? `，${report.errors.length} 只失败` : ""}
        </p>
      ) : null}

      {error !== null ? (
        <p className="notice" data-tone="risk">
          同步失败：{error}
        </p>
      ) : null}
    </section>
  );
}

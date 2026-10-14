import { useEffect, useState, useTransition } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  SYNC_PROGRESS_EVENT,
  syncMarket,
  syncStatus,
  type SyncReport,
  type SyncStatus,
} from "../api/market";
import { toMessage } from "../api/invoke";
import { StatefulButton } from "./ui/button";

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
 * 同步栏：显示同步进度与最近同步时间，并可以手动触发一次回补。
 *
 * 进度来自 `sync://progress` 事件，初始状态与最近同步时间来自 `sync_status` 命令。
 */
export function SyncPanel() {
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [report, setReport] = useState<SyncReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, startBusy] = useTransition();

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
      .catch((cause: unknown) => {
        if (active) setError(toMessage(cause));
      });
    syncStatus()
      .then((next) => {
        if (active) setStatus(next);
      })
      .catch((cause: unknown) => {
        if (active) setError(toMessage(cause));
      });
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  const start = () => {
    startBusy(async () => {
      try {
        const result = await syncMarket(false);
        setReport(result);
        setStatus(await syncStatus());
        setError(null);
      } catch (cause: unknown) {
        setError(toMessage(cause));
      }
    });
  };

  const running = status?.running ?? false;

  return (
    <section aria-label="行情同步" className="flex flex-col gap-2.5 border-t border-rule pt-3">
      <div className="flex items-center gap-3">
        <h3 className="font-display text-body font-bold text-ink">行情同步</h3>
        <StatefulButton
          className="ml-auto"
          size="sm"
          variant="secondary"
          state={running || busy ? "loading" : "idle"}
          loadingText={running ? "同步中…" : "启动中…"}
          disabled={running}
          onClick={() => start()}
        >
          立即同步
        </StatefulButton>
      </div>

      {status !== null ? (
        <>
          <p className="text-note text-ink-soft">最近同步：{displayTime(status.lastSyncAt)}</p>
          <div
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={percent(status)}
            aria-label="同步进度"
            className="h-1.5 border border-rule-strong bg-slip"
          >
            <div
              className="h-full bg-ink transition-[width] duration-300"
              style={{ width: `${percent(status)}%` }}
            />
          </div>
          <p className="text-note text-ink-soft">
            {status.total > 0 ? `${status.done} / ${status.total} · ` : ""}
            {status.message}
          </p>
        </>
      ) : (
        <p className="text-note text-ink-soft">正在读取同步状态…</p>
      )}

      {report !== null ? (
        <p className="text-note text-ink-soft">
          本次写入 {report.stocks} 只股票、{report.bars} 根日线，用时 {report.elapsedMs} 毫秒
          {report.errors.length > 0 ? `，${report.errors.length} 只失败` : ""}
        </p>
      ) : null}

      {error !== null ? <p className="text-note text-alert">同步失败：{error}</p> : null}
    </section>
  );
}

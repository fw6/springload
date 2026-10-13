import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { toMessage } from "../../api/invoke";
import { POLL_TICK_EVENT, signalEvaluate, signalStatus } from "../../api/signal";
import { AiCard } from "../../components/AiCard";
import { TermHint } from "../../components/TermHint";
import type { SignalStatus } from "../../api/types";
import type { StepProps } from "../useFlow";

/** 当前卡在哪一级的进度条：两级条件各自的状态。 */
function Stages({ status }: { status: SignalStatus | null }) {
  const stage = status?.stage ?? "waiting";
  const preMet = stage === "triggered" || (status?.zoneId ?? null) !== null;
  return (
    <div className="formula">
      第一级（日线）价格站上高置信支撑区：<b>{preMet ? "已满足" : "未满足"}</b>
      <br />
      第二级（分钟线）下探回升后突破前一根高点：<b>{stage === "triggered" ? "已触发" : "未触发"}</b>
    </div>
  );
}

/** 第 3 步：价格站上高置信支撑区后盯分钟图，出现两级触发才算数。 */
export default function Step3({ flow }: StepProps) {
  const code = flow.selected;
  const [status, setStatus] = useState<SignalStatus | null>(flow.signal);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    if (code === null) {
      setStatus(null);
      return;
    }
    try {
      const next = await signalStatus(code);
      setStatus(next);
      flow.setSignal(next);
      setError(null);
    } catch (cause: unknown) {
      setError(toMessage(cause));
    }
  }, [code, flow]);

  useEffect(() => {
    void load();
  }, [load]);

  // 每分钟一轮轮询结束之后刷新当前这只股票的状态。
  useEffect(() => {
    let stop: (() => void) | null = null;
    void listen(POLL_TICK_EVENT, () => {
      void load();
    }).then((unlisten) => {
      stop = unlisten;
    });
    return () => {
      stop?.();
    };
  }, [load]);

  const evaluate = useCallback(async () => {
    if (code === null) return;
    setBusy(true);
    try {
      const next = await signalEvaluate(code);
      setStatus(next);
      flow.setSignal(next);
      setError(null);
    } catch (cause: unknown) {
      setError(toMessage(cause));
    } finally {
      setBusy(false);
    }
  }, [code, flow]);

  if (code === null) {
    return (
      <div className="notice" data-tone="info">
        先在第 1 步把股票加进观察名单。
      </div>
    );
  }

  return (
    <>
      <p className="hint">
        <TermHint termKey="firstRetest" />：价格先站上高置信
        <TermHint termKey="support" />
        ，分钟图上再出现「下探回升 + 突破前一根高点」才算触发。
      </p>

      <Stages status={status} />

      {status !== null ? (
        <div className="notice" data-tone={status.stage === "triggered" ? "ok" : "info"}>
          {status.detail}
        </div>
      ) : null}

      {error !== null ? (
        <div className="notice" data-tone="risk">
          {error}
        </div>
      ) : null}

      <div className="panel-foot">
        <button type="button" className="primary" onClick={() => void evaluate()} disabled={busy}>
          {busy ? "正在抓分钟线…" : "抓一次分钟线并判定"}
        </button>
        <span className="hint">
          {status?.stage === "triggered"
            ? "已触发，可以进第 4 步算仓位。"
            : "价格直接跌穿支撑区没有回升时，这里不会有任何提示。"}
        </span>
      </div>

      <AiCard code={code} scope={3} />
    </>
  );
}

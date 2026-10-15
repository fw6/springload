import { useCallback, useEffect, useLayoutEffect, useRef, useState, useTransition } from "react";
import { listen } from "@tauri-apps/api/event";
import { toMessage } from "../../api/invoke";
import { POLL_TICK_EVENT, signalEvaluate, signalStatus } from "../../api/signal";
import { AiCard } from "../../components/AiCard";
import { TermHint } from "../../components/TermHint";
import { Alert, AlertContent, AlertDescription, AlertIcon } from "../../components/ui/alert";
import { StatefulButton } from "../../components/ui/button";
import type { SignalStatus } from "../../api/types";
import { cn } from "../../lib/utils";
import type { Flow, StepProps } from "../useFlow";

/** 当前卡在哪一级：两级条件各自的状态。 */
function Stages({ status }: { status: SignalStatus | null }) {
  const stage = status?.stage ?? "waiting";
  const preMet = stage === "triggered" || (status?.zoneId ?? null) !== null;
  return (
    <div className="flex flex-col gap-px border border-rule bg-rule text-body">
      <p className="flex flex-wrap items-baseline gap-x-2 bg-paper px-3 py-2">
        <span className="text-micro text-ink-soft">第一级 · 日线</span>
        <span className="text-ink-soft">价格站上高置信支撑区</span>
        <b className={cn("ml-auto", preMet ? "text-ok" : "text-ink")}>
          {preMet ? "已满足" : "未满足"}
        </b>
      </p>
      <p className="flex flex-wrap items-baseline gap-x-2 bg-paper px-3 py-2">
        <span className="text-micro text-ink-soft">第二级 · 分钟线</span>
        <span className="text-ink-soft">下探回升后突破前一根高点</span>
        <b className={cn("ml-auto", stage === "triggered" ? "text-proof" : "text-ink")}>
          {stage === "triggered" ? "已触发" : "未触发"}
        </b>
      </p>
    </div>
  );
}

/** 第 3 版：价格站上高置信支撑区后盯分钟图，出现两级触发才算数。 */
export default function Step3({ flow }: StepProps) {
  const code = flow.selected;
  const [status, setStatus] = useState<SignalStatus | null>(flow.signal);
  const [error, setError] = useState<string | null>(null);
  const [busy, startBusy] = useTransition();

  // flow 每次渲染都是新对象；把它放进依赖里会让下面两个 effect 每渲染一次就跑一轮，
  // 形成读状态与写状态互相触发的死循环。回调只认 code，flow 走 ref 取最新的。
  const flowRef = useRef<Flow>(flow);
  useLayoutEffect(() => {
    flowRef.current = flow;
  }, [flow]);

  const load = useCallback(async () => {
    if (code === null) {
      setStatus(null);
      return;
    }
    try {
      const next = await signalStatus(code);
      setStatus(next);
      flowRef.current.setSignal(next);
      setError(null);
    } catch (cause: unknown) {
      setError(toMessage(cause));
    }
  }, [code]);

  useEffect(() => {
    void load();
  }, [load]);

  // 每分钟一轮轮询结束之后刷新当前这只股票的状态。
  useEffect(() => {
    let stop: (() => void) | null = null;
    listen(POLL_TICK_EVENT, () => {
      void load();
    })
      .then((unlisten) => {
        stop = unlisten;
      })
      .catch((cause: unknown) => {
        setError(toMessage(cause));
      });
    return () => {
      stop?.();
    };
  }, [load]);

  const evaluate = useCallback(() => {
    if (code === null) return;
    startBusy(async () => {
      try {
        const next = await signalEvaluate(code);
        setStatus(next);
        flowRef.current.setSignal(next);
        setError(null);
      } catch (cause: unknown) {
        setError(toMessage(cause));
      }
    });
  }, [code]);

  if (code === null) {
    return (
      <Alert variant="info">
        <AlertIcon />
        <AlertContent>
          <AlertDescription>先在第 1 步把股票加进观察名单。</AlertDescription>
        </AlertContent>
      </Alert>
    );
  }

  return (
    <>
      <p className="text-body leading-relaxed text-ink-soft">
        <TermHint termKey="firstRetest" />：价格先站上高置信
        <TermHint termKey="support" />
        ，分钟图上再出现「下探回升 + 突破前一根高点」才算触发。
      </p>

      <Stages status={status} />

      {status !== null ? (
        <Alert variant={status.stage === "triggered" ? "proof" : "info"}>
          <AlertIcon />
          <AlertContent>
            <AlertDescription>{status.detail}</AlertDescription>
          </AlertContent>
        </Alert>
      ) : null}

      {error !== null ? (
        <Alert variant="destructive">
          <AlertIcon />
          <AlertContent>
            <AlertDescription>{error}</AlertDescription>
          </AlertContent>
        </Alert>
      ) : null}

      <div className="flex flex-wrap items-center gap-3">
        <StatefulButton
          size="sm"
          state={busy ? "loading" : "idle"}
          loadingText="正在抓分钟线…"
          onClick={() => evaluate()}
        >
          抓一次分钟线并判定
        </StatefulButton>
        <span className="text-note text-ink-soft">
          {status?.stage === "triggered"
            ? "已触发，可以进第 4 步算仓位。"
            : "价格直接跌穿支撑区没有回升时，这里不会有任何提示。"}
        </span>
      </div>

      <AiCard code={code} scope={3} invalidateToken={status?.stage ?? "waiting"} />
    </>
  );
}

import { useCallback, useEffect, useState } from "react";
import { invoke, toMessage } from "../api/invoke";
import type {
  Period,
  SignalStatus,
  SizingInput,
  StepIndex,
  StepUnlock,
  ToolId,
  WatchlistRow,
} from "../api/types";

export type { Period, SignalStatus, SizingInput, StepIndex, StepUnlock, ToolId, WatchlistRow };

/**
 * 主流程的共享状态。对外形状由架构契约冻结：
 * 各步自己的数据（筛选结果、标注列表、信号明细、计算结果）由该步自己的 hook 取；
 * 需要让别的步骤看到的状态通过这里的 setter 写回。
 */
export interface Flow {
  step: StepIndex;
  setStep: (s: StepIndex) => void;
  goStep: (s: StepIndex) => void;
  unlock: StepUnlock;
  canEnter: (s: StepIndex) => boolean;
  selected: string | null;
  selectStock: (code: string) => void;
  /** 读共享状态（观察名单、解锁状态）失败的原因；取到了就是 null。 */
  loadError: string | null;
  watchlist: WatchlistRow[];
  reloadWatchlist: () => Promise<void>;
  period: Period;
  setPeriod: (p: Period) => void;
  tool: ToolId;
  setTool: (t: ToolId) => void;
  avwapStartDate: string | null;
  setAvwapStartDate: (d: string | null) => void;
  signal: SignalStatus | null;
  setSignal: (s: SignalStatus | null) => void;
  sizing: SizingInput;
  setSizing: (s: SizingInput) => void;
  collapsed: boolean;
  setCollapsed: (v: boolean) => void;
}

/** 步骤面板的统一入参。 */
export interface StepProps {
  flow: Flow;
}

/** 还没取到 `step_unlock` 结果时的初始解锁状态：四步都可见。 */
const INITIAL_UNLOCK: StepUnlock = {
  unlocked: [true, true, true, true],
  reasons: ["", "", "", ""],
};

const INITIAL_SIZING: SizingInput = { entry: 0, stop: 0, account: 100000, riskPct: 0.005 };

/** 主流程状态机。 */
export function useFlow(): Flow {
  const [step, setStep] = useState<StepIndex>(1);
  const [unlock, setUnlock] = useState<StepUnlock>(INITIAL_UNLOCK);
  const [selected, setSelected] = useState<string | null>(null);
  const [watchlist, setWatchlist] = useState<WatchlistRow[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [period, setPeriod] = useState<Period>("day");
  const [tool, setTool] = useState<ToolId>("none");
  const [avwapStartDate, setAvwapStartDate] = useState<string | null>(null);
  const [signal, setSignal] = useState<SignalStatus | null>(null);
  const [sizing, setSizing] = useState<SizingInput>(INITIAL_SIZING);
  const [collapsed, setCollapsed] = useState(false);

  const reloadWatchlist = useCallback(async () => {
    setWatchlist(await invoke<WatchlistRow[]>("watchlist_get"));
    setLoadError(null);
  }, []);

  useEffect(() => {
    reloadWatchlist().catch((failure: unknown) => {
      setLoadError(toMessage(failure));
    });
  }, [reloadWatchlist]);

  const reloadUnlock = useCallback(async (code: string) => {
    setUnlock(await invoke<StepUnlock>("step_unlock", { code }));
    setLoadError(null);
  }, []);

  const canEnter = useCallback((target: StepIndex) => unlock.unlocked[target - 1], [unlock]);

  const goStep = useCallback(
    (target: StepIndex) => {
      if (!canEnter(target)) return;
      setStep(target);
      // 第 3 步看分钟图，其余步骤看日线。
      setPeriod(target === 3 ? "minute" : "day");
      if (target !== 2) setTool("none");
    },
    [canEnter],
  );

  const selectStock = useCallback(
    (code: string) => {
      setSelected(code);
      setSignal(null);
      setAvwapStartDate(null);
      // 读不到解锁状态时保留上一次的结果，把原因交给页面显示。
      reloadUnlock(code).catch((failure: unknown) => {
        setLoadError(toMessage(failure));
      });
      setStep(2);
    },
    [reloadUnlock],
  );

  return {
    step,
    setStep,
    goStep,
    unlock,
    canEnter,
    selected,
    selectStock,
    loadError,
    watchlist,
    reloadWatchlist,
    period,
    setPeriod,
    tool,
    setTool,
    avwapStartDate,
    setAvwapStartDate,
    signal,
    setSignal,
    sizing,
    setSizing,
    collapsed,
    setCollapsed,
  };
}

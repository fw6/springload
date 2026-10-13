/**
 * 跨 ticket 的 DTO 形状。
 *
 * 命令的实现分属各 ticket，前端用到的类型在这里一次冻结，主流程状态机
 * （`flow/useFlow.ts`）与各步骤面板只依赖这里，不依赖别的 api 模块的实现进度。
 */

/** 主流程的四个步骤序号。 */
export type StepIndex = 1 | 2 | 3 | 4;

/** 图表上的标注工具。 */
export type ToolId = "none" | "horizontal" | "zone" | "gap" | "avwap";

/** 图表周期。 */
export type Period = "day" | "minute";

/** 支撑区置信度。 */
export type Confidence = "高" | "中" | "低";

/** `step_unlock` 命令的返回：四步是否解锁，以及各自的阻止原因。 */
export interface StepUnlock {
  unlocked: [boolean, boolean, boolean, boolean];
  reasons: [string, string, string, string];
}

/** `watchlist_get` 命令返回的一行。 */
export interface WatchlistRow {
  code: string;
  name: string;
  addedAt: string;
  c1: boolean;
  c2: boolean;
  c3: boolean;
  c4: boolean;
  chg1m: number | null;
  chg3m: number | null;
  chg6m: number | null;
  rankPct: number | null;
  avgAmount: number | null;
  convergence: number | null;
}

/** 支撑区的对外形状。 */
export interface SupportZoneView {
  id: string;
  code: string;
  kind: "hline" | "support" | "gap" | "avwap_start";
  low: number;
  high: number;
  confidence: Confidence;
  spread: number | null;
}

/** 触发点在分钟图上的位置。 */
export interface SignalTrigger {
  /** 触发那根分钟线的时间，`YYYY-MM-DD HH:MM`。 */
  ts: string;
  /** 触发那根分钟线的最高价。 */
  brokenHigh: number;
  /** 前一根分钟线的最高价。 */
  preHigh: number;
}

/** `signal_status` 命令的返回。 */
export interface SignalStatus {
  stage: "waiting" | "triggered" | "failed";
  zoneId: string | null;
  zone: SupportZoneView | null;
  detail: string;
  /** 已落库的触发点；还没触发过时为 null。 */
  trigger: SignalTrigger | null;
}

/** `plan_calc` 的入参。 */
export interface SizingInput {
  entry: number;
  stop: number;
  account: number;
  riskPct: number;
}

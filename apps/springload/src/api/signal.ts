import { invoke } from "./invoke";
import type { SignalStatus } from "./types";

/** `poll_tick` 的返回：本轮刷新的对象。 */
export interface PollReport {
  waiting: string[];
  positions: string[];
  triggered: string[];
}

/** 看某只股票当前卡在哪一级。只读已入库的分钟线，不抓网络。 */
export function signalStatus(code: string): Promise<SignalStatus> {
  return invoke<SignalStatus>("signal_status", { code });
}

/** 抓一次这只股票的当日 1 分钟线，再判定触发。 */
export function signalEvaluate(code: string): Promise<SignalStatus> {
  return invoke<SignalStatus>("signal_evaluate", { code });
}

/** 手动跑一轮轮询。 */
export function pollTick(): Promise<PollReport> {
  return invoke<PollReport>("poll_tick");
}

/** 每分钟一轮轮询结束的事件名。 */
export const POLL_TICK_EVENT = "poll://tick";

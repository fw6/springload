import { invoke } from "./invoke";

/** `corporate_actions_stale` 命令返回的一行：前复权历史过期、需要重算的股票。 */
export interface StaleStock {
  code: string;
  name: string;
  /** 触发过期的最近一次除权除息日；仅由尾部数值比对发现时为 null。 */
  exDate: string | null;
}

/** `resync_adjusted` 命令的返回。 */
export interface ResyncReport {
  stocks: number;
  bars: number;
  elapsedMs: number;
  errors: string[];
}

/** 检查已回补股票的前复权历史是否过期。 */
export function corporateActionsStale(): Promise<StaleStock[]> {
  return invoke<StaleStock[]>("corporate_actions_stale");
}

/** 重算给定股票的前复权历史。 */
export function resyncAdjusted(codes: string[]): Promise<ResyncReport> {
  return invoke<ResyncReport>("resync_adjusted", { codes });
}

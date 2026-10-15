import { invoke } from "./invoke";
import type { StepUnlock, WatchlistRow } from "./types";

/** 筛选结果里的一行。 */
export interface ScreenRow {
  code: string;
  name: string;
  /** 所属板块；股票表里没有时是 null。 */
  sector: string | null;
  /** 最新收盘价。 */
  price: number;
  chg1m: number | null;
  chg3m: number | null;
  chg6m: number | null;
  /** 近 1 个月涨幅的全市场排名分位。 */
  rankPct: number | null;
  /** 近 20 日日均成交额，单位是当前市场的货币。 */
  avgAmount: number | null;
  /** 近 20 日极差。 */
  convergence: number | null;
  c1: boolean;
  c2: boolean;
  c3: boolean;
  c4: boolean;
  /** 日均成交额是否达到阈值。 */
  liquid: boolean;
}

/** `screen_stocks` / `screen_last` 的返回。 */
export interface ScreenReport {
  ranAt: string;
  /** 参与筛选的股票数。 */
  universe: number;
  elapsedMs: number;
  picked: ScreenRow[];
  rejected: ScreenRow[];
}

/** 跑一遍筛选，把入选写进观察名单。 */
export function screenStocks(): Promise<ScreenReport> {
  return invoke<ScreenReport>("screen_stocks");
}

/** 读最近一次筛选报告；还没跑过时返回 null。 */
export function screenLast(): Promise<ScreenReport | null> {
  return invoke<ScreenReport | null>("screen_last");
}

/** 观察名单列表。 */
export function watchlistGet(): Promise<WatchlistRow[]> {
  return invoke<WatchlistRow[]>("watchlist_get");
}

/** 手工把一只股票加进观察名单。 */
export function watchlistAdd(code: string): Promise<void> {
  return invoke<void>("watchlist_add", { code });
}

/** 从观察名单里移除一只股票。 */
export function watchlistRemove(code: string): Promise<void> {
  return invoke<void>("watchlist_remove", { code });
}

/** 某只股票当前已解锁的步骤与阻止原因。 */
export function stepUnlock(code: string): Promise<StepUnlock> {
  return invoke<StepUnlock>("step_unlock", { code });
}

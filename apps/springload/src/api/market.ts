import { invoke } from "./invoke";

/** `sync_market` 命令的返回。 */
export interface SyncReport {
  stocks: number;
  bars: number;
  elapsedMs: number;
  errors: string[];
}

/** `sync_status` 命令的返回，也是 `sync://progress` 事件的载荷。 */
export interface SyncStatus {
  running: boolean;
  done: number;
  total: number;
  lastSyncAt: string | null;
  message: string;
}

/** `stock_list` 命令返回的一行。 */
export interface StockRow {
  code: string;
  name: string;
  market: string;
  board: string;
  sector: string | null;
  listedAt: string | null;
  isSt: boolean;
  updatedAt: string;
}

/** 一根 K 线。日线的 `date` 是 YYYY-MM-DD，分钟线是 YYYY-MM-DD HH:MM。 */
export interface Bar {
  code: string;
  date: string;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
  amount: number;
}

/** 同步进度事件名。 */
export const SYNC_PROGRESS_EVENT = "sync://progress";

/** 抓股票池与日线。`full` 为 true 时忽略已有进度、重新抓取整段历史。 */
export function syncMarket(full: boolean): Promise<SyncReport> {
  return invoke<SyncReport>("sync_market", { full });
}

/** 当前同步状态与最近同步时间。 */
export function syncStatus(): Promise<SyncStatus> {
  return invoke<SyncStatus>("sync_status");
}

/** 股票池列表。 */
export function stockList(): Promise<StockRow[]> {
  return invoke<StockRow[]>("stock_list");
}

/** 读一只股票的 K 线。`period` 取 `day` 或分钟周期字符串。 */
export function barsGet(code: string, period: string): Promise<Bar[]> {
  return invoke<Bar[]>("bars_get", { code, period });
}

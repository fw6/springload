import { invoke } from "./invoke";
import type { TradeLogEntry } from "./plan";

/** 提示的种类。 */
export type AlertKind = "stopLoss" | "belowEma9" | "reduce3R" | "reduce5R";

/** 一条离场提示。 */
export interface Alert {
  kind: AlertKind;
  message: string;
  /** 观察到条件的那一天。 */
  signalDate: string;
  /** 最早可以卖出的那一天，不早于买入日的次一交易日。 */
  executableDate: string;
}

/** 持仓列表里的一行。 */
export interface PositionView {
  id: string;
  planId: string;
  code: string;
  name: string | null;
  entryDate: string;
  entryPrice: number;
  stop: number;
  quantity: number;
  last: number | null;
  ema9: number | null;
  perR: number;
  t3: number;
  t5: number;
  followEma: boolean;
  alerts: Alert[];
}

/** 读全部未平仓持仓。 */
export function positionsGet(): Promise<PositionView[]> {
  return invoke<PositionView[]>("positions_get");
}

/** 抓一次实时快照刷新最新价，并按提示发系统通知。 */
export function positionsRefreshQuotes(): Promise<PositionView[]> {
  return invoke<PositionView[]>("positions_refresh_quotes");
}

/** 填实际成交价，平掉一条持仓。 */
export function positionExit(
  id: string,
  exitPrice: number,
  exitDate: string,
): Promise<TradeLogEntry> {
  return invoke<TradeLogEntry>("position_exit", { id, exitPrice, exitDate });
}

import { invoke } from "./invoke";

/** 计划卡的状态取值。 */
export type PlanStatus = "draft" | "triggered" | "executed" | "abandoned";

/** 一条交易计划。股数、投入金额与目标位由服务端算出。 */
export interface TradePlan {
  id: string;
  code: string;
  entry: number;
  stop: number;
  account: number;
  riskPct: number;
  perR: number;
  shares: number;
  capital: number;
  posPct: number;
  stopDistance: number;
  t3: number;
  t5: number;
  status: PlanStatus;
  zoneId: string | null;
  signalId: string | null;
  createdAt: string;
  updatedAt: string;
}

/** 减仓安排。 */
export interface ReducePlan {
  shares3r: number;
  shares5r: number;
  remaining: number;
}

/** 计划卡上要展示的全部字段。 */
export interface PlanCard {
  plan: TradePlan;
  name: string | null;
  board: string | null;
  sector: string | null;
  c1: boolean | null;
  c2: boolean | null;
  c3: boolean | null;
  c4: boolean | null;
  reduce: ReducePlan;
  actualRiskPct: number;
}

/** `plan_save` 的入参。`id` 为空时新建。 */
export interface PlanInput {
  id?: string | null;
  code: string;
  entry: number;
  stop: number;
  account: number;
  riskPct: number;
  zoneId?: string | null;
  signalId?: string | null;
}

/** 一条交易日志记录。 */
export interface TradeLogEntry {
  id: string;
  positionId: string | null;
  code: string;
  name: string | null;
  entryDate: string;
  entryPrice: number;
  stopPrice: number;
  exitDate: string | null;
  exitPrice: number | null;
  quantity: number;
  perR: number | null;
  rMultiple: number | null;
  pnl: number | null;
  fees: number;
  note: string | null;
  source: string;
  createdAt: string;
}

/** 算仓位并存一条计划。 */
export function planSave(plan: PlanInput): Promise<TradePlan> {
  return invoke<TradePlan>("plan_save", { plan });
}

/** 读一条计划。 */
export function planGet(id: string): Promise<TradePlan> {
  return invoke<TradePlan>("plan_get", { id });
}

/** 读一条计划的计划卡。 */
export function planCard(id: string): Promise<PlanCard> {
  return invoke<PlanCard>("plan_card", { id });
}

/** 按状态列出计划；不给状态时列全部。 */
export function planList(status?: PlanStatus): Promise<TradePlan[]> {
  return invoke<TradePlan[]>("plan_list", { status: status ?? null });
}

/** 确认计划已执行，返回写入的入场记录。 */
export function planConfirmExecuted(id: string, entryDate: string): Promise<TradeLogEntry> {
  return invoke<TradeLogEntry>("plan_confirm_executed", { id, entryDate });
}

/** 改一条计划的状态。 */
export function planSetStatus(id: string, status: PlanStatus): Promise<TradePlan> {
  return invoke<TradePlan>("plan_set_status", { id, status });
}

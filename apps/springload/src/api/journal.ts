import { invoke } from "./invoke";
import type { TradeLogEntry } from "./plan";

/** 统计摘要。 */
export interface JournalStats {
  trades: number;
  wins: number;
  winRate: number;
  avgR: number;
  totalPnl: number;
  totalFees: number;
  /** 统计口径的固定标注。 */
  note: string;
}

/** 交易日志页需要的全部数据。 */
export interface JournalView {
  entries: TradeLogEntry[];
  stats: JournalStats;
  feeModelEnabled: boolean;
  commissionRate: number;
}

/** 手动补录一笔历史交易。 */
export interface ManualEntry {
  code: string;
  entryDate: string;
  entryPrice: number;
  stopPrice: number;
  exitPrice?: number | null;
  exitDate?: string | null;
  quantity: number;
  note?: string | null;
}

/** AI 解释。 */
export interface Explanation {
  text: string;
  generatedAt: string;
  model: string;
  cached: boolean;
}

/** 读记录表与统计摘要。 */
export function journalGet(): Promise<JournalView> {
  return invoke<JournalView>("journal_get");
}

/** 手动补录一笔历史交易。 */
export function journalAddManual(entry: ManualEntry): Promise<TradeLogEntry> {
  return invoke<TradeLogEntry>("journal_add_manual", { entry });
}

/** 把全部记录导出成 CSV。 */
export function journalExportCsv(path: string): Promise<void> {
  return invoke<void>("journal_export_csv", { path });
}

/** AI 复盘总结。 */
export function journalReview(): Promise<Explanation> {
  return invoke<Explanation>("journal_review");
}

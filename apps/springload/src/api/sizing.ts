import { invoke } from "./invoke";
import type { SizingInput } from "./types";

/** `plan_calc` 的返回。 */
export interface SizingResult {
  perR: number;
  maxLoss: number;
  shares: number;
  capital: number;
  posPct: number;
  stopDistance: number;
  t3: number;
  t5: number;
  unusedRisk: number;
}

/** 算仓位与目标位。买入价不高于止损价时命令返回错误。 */
export function planCalc(input: SizingInput): Promise<SizingResult> {
  return invoke<SizingResult>("plan_calc", { input });
}

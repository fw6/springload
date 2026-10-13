import { invoke } from "./invoke";
import type { StepIndex } from "./types";

/** `explain` / `explain_retry` 命令的返回。 */
export interface Explanation {
  text: string;
  generatedAt: string;
  model: string;
  /** 这次结果是读缓存来的还是刚生成的。 */
  cached: boolean;
}

/** 解释的作用范围：主流程四步与交易日志页。 */
export type ExplainScope = StepIndex | "journal";

/**
 * 作用范围映射到命令的 `step` 参数：1~4 对应主流程四步，交易日志页用 0。
 */
export function scopeToStep(scope: ExplainScope): number {
  return scope === "journal" ? 0 : scope;
}

/** 读缓存，没有缓存就在数据齐备时生成一次。 */
export function explain(code: string, scope: ExplainScope): Promise<Explanation> {
  return invoke<Explanation>("explain", { code, step: scopeToStep(scope) });
}

/** 跳过缓存重新生成一次。 */
export function explainRetry(code: string, scope: ExplainScope): Promise<Explanation> {
  return invoke<Explanation>("explain_retry", { code, step: scopeToStep(scope) });
}

/** 让某只股票某一步的解释缓存失效。 */
export function explainInvalidate(code: string, scope: ExplainScope): Promise<void> {
  return invoke<void>("explain_invalidate", { code, step: scopeToStep(scope) });
}

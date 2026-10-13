import type { StepProps } from "../useFlow";

/** 第 1 步面板占位。本步骤由 ticket #14 重写。 */
export default function Step1({ flow }: StepProps) {
  return (
    <div className="notice" data-tone="info">
      第 1 步「强势股筛选」尚未实现。当前观察名单 {flow.watchlist.length} 只。
    </div>
  );
}

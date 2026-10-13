import type { StepProps } from "../useFlow";

/** 第 4 步面板占位。本步骤由 ticket #18 重写。 */
export default function Step4({ flow }: StepProps) {
  return (
    <div className="notice" data-tone="info">
      第 4 步「算仓位与目标位」尚未实现。当前输入：买入价 {flow.sizing.entry}、止损价{" "}
      {flow.sizing.stop}、账户 {flow.sizing.account}、单笔风险比例{" "}
      {(flow.sizing.riskPct * 100).toFixed(2)}%。
    </div>
  );
}

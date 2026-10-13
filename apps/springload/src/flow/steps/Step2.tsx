import type { StepProps } from "../useFlow";

/** 第 2 步面板占位。本步骤由 ticket #16 重写。 */
export default function Step2({ flow }: StepProps) {
  return (
    <div className="notice" data-tone="info">
      第 2 步「标注支撑区」尚未实现。当前选中 {flow.selected ?? "未选股票"}。
    </div>
  );
}

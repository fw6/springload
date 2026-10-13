import type { StepProps } from "../useFlow";

/** 第 3 步面板占位。本步骤由 ticket #17 重写。 */
export default function Step3({ flow }: StepProps) {
  return (
    <div className="notice" data-tone="info">
      第 3 步「等待买入信号」尚未实现。当前信号状态{" "}
      {flow.signal === null ? "未出现" : flow.signal.stage}。
    </div>
  );
}

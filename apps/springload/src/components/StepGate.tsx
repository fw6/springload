import type { ReactNode } from "react";
import type { StepIndex } from "../api/types";
import { useSteps } from "../copy/GlossaryContext";
import type { Flow } from "../flow/useFlow";

/** 步骤门：未解锁时显示该步的阻止提示，替代正文。 */
export function StepGate({
  flow,
  target,
  children,
}: {
  flow: Flow;
  target: StepIndex;
  children: ReactNode;
}) {
  const steps = useSteps();
  if (flow.canEnter(target)) return <>{children}</>;
  const copy = steps[target - 1];
  return (
    <div className="notice" data-tone="warn">
      {copy?.blockedHint ?? "这一步还没有解锁。"}
    </div>
  );
}

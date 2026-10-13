import type { StepIndex } from "../api/types";
import { useSteps } from "../copy/GlossaryContext";
import type { Flow } from "./useFlow";

/**
 * 顶部四步进度条。
 * 最右侧依次是「这一步在干什么」与面板收起按钮。
 * 还没解锁的步骤是灰的，点一下由 `onBlocked` 给出原因。
 */
export function Steps({
  flow,
  onExplain,
  onBlocked,
}: {
  flow: Flow;
  onExplain: () => void;
  onBlocked: (target: StepIndex) => void;
}) {
  const steps = useSteps();

  return (
    <div className="steps">
      {steps.map((step) => {
        const index = step.index as StepIndex;
        const locked = !flow.canEnter(index);
        return (
          <button
            key={step.index}
            type="button"
            className="step-chip"
            aria-current={flow.step === index ? "step" : undefined}
            aria-disabled={locked}
            data-done={flow.step > index}
            onClick={() => (locked ? onBlocked(index) : flow.goStep(index))}
          >
            <span className="num">{step.index}</span>
            <span>
              <span style={{ display: "block" }}>{step.title}</span>
              <span style={{ display: "block", fontSize: 11, opacity: 0.7 }}>{step.goal}</span>
            </span>
          </button>
        );
      })}
      <span className="toolbar-spacer" />
      <button type="button" className="tool" onClick={onExplain}>
        这一步在干什么
      </button>
      <button
        type="button"
        className="collapse-btn"
        onClick={() => flow.setCollapsed(!flow.collapsed)}
      >
        {flow.collapsed ? "展开面板" : "收起面板"}
      </button>
    </div>
  );
}

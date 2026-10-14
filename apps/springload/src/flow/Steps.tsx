import type { StepIndex } from "../api/types";
import { useSteps } from "../copy/GlossaryContext";
import { cn } from "../lib/utils";
import { Button } from "../components/ui/button";
import { Tabs, TabsList, TabsTrigger } from "../components/ui/tabs";
import type { Flow } from "./useFlow";

/**
 * 稿件目录条：四则稿件并排，每则给出版次号、标题与一行当前状态。
 * 最右侧依次是「这一步在干什么」与面板收起按钮。
 * 还没解锁的稿件是灰的，点一下由 `onBlocked` 给出原因。
 */
export function Steps({
  flow,
  onExplain,
  onBlocked,
  statusOf,
}: {
  flow: Flow;
  onExplain: () => void;
  onBlocked: (target: StepIndex) => void;
  /** 每一步入口上显示的那行状态。 */
  statusOf: (target: StepIndex) => string;
}) {
  const steps = useSteps();

  return (
    <div
      data-slot="steps"
      className="flex flex-none flex-wrap items-stretch gap-px border border-rule-strong bg-rule-strong"
    >
      <Tabs
        value={String(flow.step)}
        onValueChange={(value) => flow.goStep(Number(value) as StepIndex)}
        className="min-w-0 flex-1"
      >
        <TabsList className="flex items-stretch gap-px">
          {steps.map((step) => {
            const index = step.index as StepIndex;
            const locked = !flow.canEnter(index);
            const active = flow.step === index;
            return (
              <TabsTrigger
                key={step.index}
                value={String(index)}
                id={`step-tab-${index}`}
                data-slot="step-chip"
                locked={locked}
                onClick={(event) => {
                  if (!locked) return;
                  // 锁着的稿件不切换，只说明原因。
                  event.preventDefault();
                  onBlocked(index);
                }}
                className={cn(
                  "flex min-w-0 flex-1 flex-col gap-0.5 border-t-2 border-t-transparent bg-paper px-3 py-2",
                  active && "border-t-proof bg-slip",
                  locked && "opacity-45",
                )}
              >
                <span className="flex items-baseline gap-1.5">
                  <span
                    className={cn(
                      "font-display text-head font-bold tabular",
                      active ? "text-proof" : "text-ink",
                    )}
                  >
                    {step.index}
                  </span>
                  <span
                    className={cn(
                      "font-display text-body font-bold",
                      active ? "text-proof" : "text-ink",
                    )}
                  >
                    {step.title}
                  </span>
                </span>
                <span className="truncate text-micro text-ink-soft">{statusOf(index)}</span>
              </TabsTrigger>
            );
          })}
        </TabsList>
      </Tabs>
      <Button size="sm" variant="ghost" className="flex-none bg-paper px-3" onClick={onExplain}>
        这一步在干什么
      </Button>
      <Button
        size="sm"
        variant="secondary"
        className="flex-none"
        onClick={() => flow.setCollapsed(!flow.collapsed)}
      >
        {flow.collapsed ? "展开面板" : "收起面板"}
      </Button>
    </div>
  );
}

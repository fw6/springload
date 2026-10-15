import { useMemo, useState, useSyncExternalStore } from "react";
import type { StepIndex, ToolId } from "../api/types";
import KLineChart from "../chart/KLineChart";
import { getZones, subscribeZones } from "../chart/zoneStore";
import { Modal } from "../components/Modal";
import { StepGate } from "../components/StepGate";
import { Alert, AlertContent, AlertDescription, AlertIcon } from "../components/ui/alert";
import { Button } from "../components/ui/button";
import { useSteps } from "../copy/GlossaryContext";
import { cn } from "../lib/utils";
import Step1 from "./steps/Step1";
import Step2 from "./steps/Step2";
import Step3 from "./steps/Step3";
import Step4 from "./steps/Step4";
import { Steps } from "./Steps";
import { useFlow, type Flow } from "./useFlow";

const TOOLS: Array<{ id: ToolId; label: string }> = [
  { id: "horizontal", label: "水平线" },
  { id: "zone", label: "支撑区" },
  { id: "gap", label: "跳空缺口" },
  { id: "avwap", label: "AVWAP 起点" },
];

/**
 * 图版宽度占比：按当前是哪一版排。
 * 第 1 步要看两张表，第 4 步要看公式，图版让位；第 3 步盯分钟线，图版最大。
 */
const CHART_SHARE: Record<StepIndex, number> = { 1: 30, 2: 44, 3: 54, 4: 32 };

/** 图版工具条：四个标注工具 + 退出标注 + 周期切换。 */
function FlowToolbar({ flow }: { flow: Flow }) {
  return (
    <>
      {TOOLS.map((tool) => {
        const active = flow.tool === tool.id;
        return (
          <Button
            key={tool.id}
            size="sm"
            variant={active ? "primary" : "outline"}
            aria-pressed={active}
            onClick={() => flow.setTool(active ? "none" : tool.id)}
          >
            {tool.label}
          </Button>
        );
      })}
      <Button
        size="sm"
        variant="ghost"
        onClick={() => flow.setTool("none")}
        disabled={flow.tool === "none"}
      >
        退出标注
      </Button>
      <span className="ml-auto flex items-center gap-1.5">
        {(["day", "minute"] as const).map((period) => {
          const active = flow.period === period;
          return (
            <Button
              key={period}
              size="sm"
              variant={active ? "primary" : "outline"}
              aria-pressed={active}
              onClick={() => flow.setPeriod(period)}
            >
              {period === "day" ? "日线" : "1 分钟"}
            </Button>
          );
        })}
      </span>
    </>
  );
}

/** 主流程：一条稿件目录加一块版心，版心左边是图版、右边是当前这一版的正文。 */
export default function FlowPage() {
  const flow = useFlow();
  const steps = useSteps();
  const [explainOpen, setExplainOpen] = useState(false);
  const [blocked, setBlocked] = useState<StepIndex | null>(null);
  const current = steps[flow.step - 1];
  const blockedCopy = blocked === null ? undefined : steps[blocked - 1];

  const zoneSnapshot = useSyncExternalStore(subscribeZones, getZones, getZones);
  const zoneCount =
    zoneSnapshot.code === flow.selected
      ? zoneSnapshot.zones.filter((zone) => zone.kind !== "avwap_start").length
      : 0;

  // 每一步入口上的那行状态：让目录条自己说明走到哪了。
  const statusOf = useMemo(
    () => (target: StepIndex): string => {
      if (target === 1) {
        return flow.watchlist.length > 0 ? `观察名单 ${flow.watchlist.length} 只` : "尚未筛选";
      }
      if (target === 2) {
        return zoneCount > 0 ? `已标注 ${zoneCount} 条` : "尚未标注";
      }
      if (target === 3) {
        if (flow.signal?.stage === "triggered") return "已触发";
        if (flow.signal?.stage === "failed") return "已跌穿";
        return "等待中";
      }
      return flow.sizing.entry > 0 && flow.sizing.stop > 0 ? "已算仓位" : "尚未计算";
    },
    [flow.signal, flow.sizing.entry, flow.sizing.stop, flow.watchlist.length, zoneCount],
  );

  const triggered = flow.step === 3 && flow.signal?.stage === "triggered";
  const chartShare = flow.collapsed ? 100 : CHART_SHARE[flow.step];

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 p-4">
      <Steps
        flow={flow}
        onExplain={() => setExplainOpen(true)}
        onBlocked={(target) => setBlocked(target)}
        statusOf={statusOf}
      />

      {flow.loadError !== null ? (
        <Alert variant="destructive">
          <AlertIcon />
          <AlertContent>
            <AlertDescription>读取观察名单与解锁状态失败：{flow.loadError}</AlertDescription>
          </AlertContent>
        </Alert>
      ) : null}

      {triggered ? (
        <div
          data-slot="extra"
          className="extra-in flex flex-none origin-top flex-wrap items-center gap-x-3 gap-y-1 border-2 border-proof bg-proof px-3 py-1.5 text-paper"
        >
          <span className="font-display text-head font-bold tracking-[0.3em]">号外</span>
          <span className="text-body">
            {flow.signal?.trigger ? `分钟线触发于 ${flow.signal.trigger.ts.slice(11)}` : "分钟线已触发"}
          </span>
          <span className="ml-auto text-micro">可以进第 4 步算仓位</span>
        </div>
      ) : null}

      <div
        data-slot="flow"
        data-collapsed={flow.collapsed}
        className="@container min-h-0 flex-1"
      >
        <div className="flex h-full min-h-0 flex-col @4xl:flex-row">
          <figure
            data-slot="chart-card"
            style={{ flexGrow: chartShare }}
            className="flex min-h-0 min-w-0 basis-0 flex-col overflow-hidden border border-rule bg-paper transition-[flex-grow] duration-300"
          >
            <div className="flex flex-none flex-wrap items-center gap-1.5 border-b border-rule px-2.5 py-2">
              <span className="mr-1 text-micro tracking-[0.08em] text-ink-soft">图版</span>
              <FlowToolbar flow={flow} />
            </div>
            <div className="flex min-h-0 flex-1 flex-col p-2.5">
              <KLineChart flow={flow} />
            </div>
            <figcaption className="flex flex-none flex-wrap items-center gap-x-3 gap-y-0.5 border-t border-rule px-2.5 py-1.5 text-micro text-ink-soft">
              <span className="tabular">{flow.selected ?? "未选标的"}</span>
              <span>{flow.period === "day" ? "日线" : "1 分钟"}</span>
              <span className="ml-auto">拖矩形改支撑区，拖水平线改价格</span>
            </figcaption>
          </figure>

          {flow.collapsed ? null : (
            <section
              data-slot="flow-panel"
              role="tabpanel"
              id="flow-step-panel"
              aria-labelledby={`step-tab-${flow.step}`}
              style={{ flexGrow: 100 - chartShare }}
              className="flex min-h-0 min-w-0 basis-0 flex-col overflow-auto border-2 border-proof bg-paper transition-[flex-grow] duration-300"
            >
              <div className="flex flex-none flex-wrap items-baseline gap-x-3 gap-y-1 border-b-2 border-ink px-3.5 py-2.5">
                <h2 className="font-display text-title font-bold text-ink">
                  {current?.title ?? `第 ${flow.step} 步`}
                </h2>
                <span
                  className={cn(
                    "ml-auto text-micro tabular text-ink-soft",
                    flow.selected !== null && "text-ink",
                  )}
                >
                  {flow.selected ?? "未选标的"}
                </span>
                {current !== undefined ? (
                  <p className="w-full font-display text-note text-ink-soft">{current.goal}</p>
                ) : null}
              </div>
              <div className="flex flex-col gap-3.5 p-3.5">
                {flow.step === 1 ? <Step1 flow={flow} /> : null}
                {flow.step === 2 ? (
                  <StepGate flow={flow} target={2}>
                    <Step2 flow={flow} />
                  </StepGate>
                ) : null}
                {flow.step === 3 ? (
                  <StepGate flow={flow} target={3}>
                    <Step3 flow={flow} />
                  </StepGate>
                ) : null}
                {flow.step === 4 ? (
                  <StepGate flow={flow} target={4}>
                    <Step4 flow={flow} />
                  </StepGate>
                ) : null}
              </div>
            </section>
          )}
        </div>
      </div>

      <Modal
        open={explainOpen && current !== undefined}
        title={
          current === undefined ? "这一步在干什么" : `第 ${current.index} 步 · ${current.title}`
        }
        onClose={() => setExplainOpen(false)}
      >
        {current !== undefined ? (
          <>
            <p>{current.oneLine}</p>
            <p>{current.detail}</p>
          </>
        ) : null}
      </Modal>

      <Modal
        open={blocked !== null && blockedCopy !== undefined}
        title={
          blockedCopy === undefined
            ? "这一步还没解锁"
            : `第 ${blockedCopy.index} 步 · ${blockedCopy.title}`
        }
        onClose={() => setBlocked(null)}
      >
        {blockedCopy !== undefined ? <p>{blockedCopy.blockedHint}</p> : null}
      </Modal>
    </div>
  );
}

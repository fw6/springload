import { useState } from "react";
import type { StepIndex, ToolId } from "../api/types";
import KLineChart from "../chart/KLineChart";
import { Modal } from "../components/Modal";
import { StepGate } from "../components/StepGate";
import { useSteps } from "../copy/GlossaryContext";
import Step1 from "./steps/Step1";
import Step2 from "./steps/Step2";
import Step3 from "./steps/Step3";
import Step4 from "./steps/Step4";
import { Steps } from "./Steps";
import { useFlow, type Flow } from "./useFlow";

/** 图表工具栏：四个标注工具 + 退出标注 + 周期切换。 */
function FlowToolbar({ flow }: { flow: Flow }) {
  const tools: Array<{ id: ToolId; label: string }> = [
    { id: "horizontal", label: "水平线" },
    { id: "zone", label: "支撑区" },
    { id: "gap", label: "跳空缺口" },
    { id: "avwap", label: "AVWAP 起点" },
  ];

  return (
    <div className="chart-toolbar">
      {tools.map((tool) => (
        <button
          key={tool.id}
          type="button"
          className="tool"
          aria-pressed={flow.tool === tool.id}
          onClick={() => flow.setTool(flow.tool === tool.id ? "none" : tool.id)}
        >
          {tool.label}
        </button>
      ))}
      <button
        type="button"
        className="tool"
        onClick={() => flow.setTool("none")}
        disabled={flow.tool === "none"}
      >
        退出标注
      </button>
      <span className="toolbar-spacer" />
      <button
        type="button"
        className="tool"
        aria-pressed={flow.period === "day"}
        onClick={() => flow.setPeriod("day")}
      >
        日线
      </button>
      <button
        type="button"
        className="tool"
        aria-pressed={flow.period === "minute"}
        onClick={() => flow.setPeriod("minute")}
      >
        1 分钟
      </button>
    </div>
  );
}

/** 主流程一页四步：顶部四步进度，左侧图表区，右侧当前步骤面板。 */
export default function FlowPage() {
  const flow = useFlow();
  const steps = useSteps();
  const [explainOpen, setExplainOpen] = useState(false);
  const [blocked, setBlocked] = useState<StepIndex | null>(null);
  const current = steps[flow.step - 1];
  const blockedCopy = blocked === null ? null : steps[blocked - 1];

  return (
    <div className="flow-wrap">
      <Steps
        flow={flow}
        onExplain={() => setExplainOpen(true)}
        onBlocked={(target) => setBlocked(target)}
      />
      {flow.loadError !== null ? (
        <div className="notice" data-tone="risk">
          读取观察名单与解锁状态失败：{flow.loadError}
        </div>
      ) : null}
      <div className="flow" data-collapsed={flow.collapsed}>
        <div className="chart-card">
          <FlowToolbar flow={flow} />
          <KLineChart flow={flow} />
        </div>
        {flow.collapsed ? null : (
          <div className="panel">
            <div className="panel-head">
              <h2>{current?.title ?? `第 ${flow.step} 步`}</h2>
              <span className="toolbar-spacer" />
              <span className="hint">{flow.selected ?? "未选股票"}</span>
            </div>
            <div className="panel-body">
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
          </div>
        )}
      </div>
      {explainOpen && current ? (
        <Modal
          title={`第 ${current.index} 步 · ${current.title}`}
          onClose={() => setExplainOpen(false)}
        >
          <p>{current.oneLine}</p>
          <p>{current.detail}</p>
        </Modal>
      ) : null}
      {blockedCopy !== null ? (
        <Modal
          title={`第 ${blockedCopy.index} 步 · ${blockedCopy.title}`}
          onClose={() => setBlocked(null)}
        >
          <p>{blockedCopy.blockedHint}</p>
        </Modal>
      ) : null}
    </div>
  );
}

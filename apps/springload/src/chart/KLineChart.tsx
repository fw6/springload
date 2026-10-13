import type { Flow } from "../flow/useFlow";

/**
 * K 线图占位。
 *
 * 图表由后续 ticket（#16）在 klinecharts 上实现，这里只占住左侧图表区，
 * 并把主流程状态（周期、选中股票、当前标注工具）透出去。对外形状冻结为
 * `{ flow }`，实现方从 flow 里取需要的字段。
 */
export default function KLineChart({ flow }: { flow: Flow }) {
  return (
    <div className="kline chart-placeholder">
      <p className="hint">K 线图区。图表由后续 ticket 接入 klinecharts，当前是占位。</p>
      <p className="hint mono">
        周期 {flow.period === "day" ? "日线" : "1 分钟"} · 选中 {flow.selected ?? "未选股票"} ·
        工具 {flow.tool}
      </p>
    </div>
  );
}

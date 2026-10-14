import { useEffect, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { dispose, init, type Chart, type KLineData, type OverlayCreate } from "klinecharts";
import { toMessage } from "../api/invoke";
import { barsGet, type Bar } from "../api/market";
import type { SupportZoneView } from "../api/types";
import type { Flow } from "../flow/useFlow";
import { type ZoneKind } from "./overlays";
import { CHART_COLORS } from "./palette";
import { getZones, loadZones, resaveZones, saveZone, subscribeZones } from "./zoneStore";
import "./overlays";

/** AVWAP 未指定起点时从末尾往前取的交易日数，与 Rust 侧默认值一致。 */
const AVWAP_DEFAULT_BARS_BACK = 60;

/** 稳定的空列表：切换股票的瞬间用它，避免每次渲染都产生新的数组。 */
const NO_ZONES: SupportZoneView[] = [];

const PERIOD = { day: { type: "day", span: 1 }, minute: { type: "minute", span: 1 } } as const;

/** 把 `YYYY-MM-DD` 或 `YYYY-MM-DD HH:MM` 按本地时间转成毫秒时间戳。 */
function toTimestamp(date: string): number {
  const [day, time] = date.split(" ");
  const [year, month, dayOfMonth] = day.split("-").map(Number);
  if (time === undefined) return new Date(year, month - 1, dayOfMonth).getTime();
  const [hour, minute] = time.split(":").map(Number);
  return new Date(year, month - 1, dayOfMonth, hour, minute).getTime();
}

const toKLine = (bar: Bar): KLineData => ({
  timestamp: toTimestamp(bar.date),
  open: bar.open,
  high: bar.high,
  low: bar.low,
  close: bar.close,
  volume: bar.volume,
});

/** 当前环境是否提供 canvas 2d 上下文。klinecharts 没有它无法初始化。 */
function supportsCanvas(): boolean {
  return document.createElement("canvas").getContext("2d") !== null;
}

/**
 * K 线图：渲染日线与分钟线，叠加 EMA 9/21/50 与 AVWAP，并承载支撑区标注。
 *
 * 标注的增删改都写回 `support_zone` 表并触发服务端重算置信度；图表与第 2 步面板
 * 通过 `chart/zoneStore` 看到同一份标注列表。
 */
export default function KLineChart({ flow }: { flow: Flow }) {
  const { selected, period, tool, avwapStartDate, signal } = flow;
  const holderRef = useRef<HTMLDivElement | null>(null);
  const chartRef = useRef<Chart | null>(null);
  /** 正在画的 overlay 的 id，对账时要跳过它。 */
  const pendingRef = useRef<string | null>(null);
  const barsRef = useRef<Bar[]>([]);
  const flowRef = useRef(flow);

  const [bars, setBars] = useState<Bar[]>([]);
  const [barsError, setBarsError] = useState<string | null>(null);
  const [loadingBars, setLoadingBars] = useState(false);
  const [chartFailed, setChartFailed] = useState(false);

  const zoneSnapshot = useSyncExternalStore(subscribeZones, getZones, getZones);
  // 只画当前这只股票的标注；切换股票的瞬间不会串到上一只的标注。
  const zones = zoneSnapshot.code === selected ? zoneSnapshot.zones : NO_ZONES;

  // 最新一次的 flow 与 bars 供事件回调读取，写入放在 effect 里，渲染期间不改 ref。
  useLayoutEffect(() => {
    flowRef.current = flow;
    barsRef.current = bars;
  }, [flow, bars]);

  // 日线走 `day`，分钟线走 1 分钟周期。
  useEffect(() => {
    if (selected === null) {
      setBars([]);
      setBarsError(null);
      return;
    }
    let active = true;
    setLoadingBars(true);
    barsGet(selected, period === "day" ? "day" : "1")
      .then((rows) => {
        if (!active) return;
        setBars(rows);
        setBarsError(null);
      })
      .catch((error) => {
        if (!active) return;
        setBars([]);
        setBarsError(toMessage(error));
      })
      .finally(() => {
        if (active) setLoadingBars(false);
      });
    return () => {
      active = false;
    };
  }, [selected, period]);

  useEffect(() => {
    if (selected !== null) void loadZones(selected);
  }, [selected]);

  /** AVWAP 起点的 bar 下标。未指定时取末尾往前 60 根。 */
  const avwapStartIndex = useMemo(() => {
    if (bars.length === 0) return 0;
    if (avwapStartDate !== null) {
      const index = bars.findIndex((bar) => bar.date >= avwapStartDate);
      if (index >= 0) return index;
    }
    return Math.max(0, bars.length - AVWAP_DEFAULT_BARS_BACK);
  }, [bars, avwapStartDate]);

  // 初始化：环境没有 canvas（例如测试用的 jsdom）时图表不可用。
  useEffect(() => {
    const holder = holderRef.current;
    if (holder === null) return;
    if (!supportsCanvas()) {
      setChartFailed(true);
      return;
    }
    let chart: Chart | null = null;
    try {
      chart = init(holder, { locale: "zh-CN" });
    } catch {
      setChartFailed(true);
      return;
    }
    if (chart === null) {
      setChartFailed(true);
      return;
    }
    chartRef.current = chart;
    chart.setStyles({
      grid: {
        horizontal: { color: CHART_COLORS.grid },
        vertical: { color: CHART_COLORS.grid },
      },
      candle: {
        bar: {
          upColor: CHART_COLORS.up,
          downColor: CHART_COLORS.down,
          upBorderColor: CHART_COLORS.up,
          downBorderColor: CHART_COLORS.down,
          upWickColor: CHART_COLORS.up,
          downWickColor: CHART_COLORS.down,
        },
      },
      indicator: {
        lines: [
          { color: CHART_COLORS.ema },
          { color: CHART_COLORS.reference },
          { color: CHART_COLORS.avwap },
        ],
        ohlc: { upColor: CHART_COLORS.up, downColor: CHART_COLORS.down },
      },
      xAxis: {
        axisLine: { color: CHART_COLORS.axisLine },
        tickLine: { color: CHART_COLORS.axisLine },
        tickText: { color: CHART_COLORS.axisText, family: "SF Mono, Menlo, monospace" },
      },
      yAxis: {
        axisLine: { color: CHART_COLORS.axisLine },
        tickLine: { color: CHART_COLORS.axisLine },
        tickText: { color: CHART_COLORS.axisText, family: "SF Mono, Menlo, monospace" },
      },
      separator: { color: CHART_COLORS.axisLine },
      crosshair: {
        horizontal: {
          line: { color: CHART_COLORS.axisText },
          text: { color: CHART_COLORS.axisText, borderColor: CHART_COLORS.axisLine },
        },
        vertical: {
          line: { color: CHART_COLORS.axisText },
          text: { color: CHART_COLORS.axisText, borderColor: CHART_COLORS.axisLine },
        },
      },
    });
    return () => {
      dispose(holder);
      chartRef.current = null;
    };
  }, []);

  // 标的与周期：换股票或换周期都重新载入数据。
  useEffect(() => {
    const chart = chartRef.current;
    if (chart === null || selected === null) return;
    chart.setSymbol({ ticker: selected, pricePrecision: 2, volumePrecision: 0 });
    chart.setPeriod(PERIOD[period]);
    chart.setDataLoader({
      getBars: ({ type, callback }) => {
        if (type === "init") callback(barsRef.current.map(toKLine));
        else callback([], false);
      },
    });
    chart.resetData();
    chart.setOffsetRightDistance(24);
  }, [selected, period, bars]);

  // EMA 9/21/50 始终叠在主图上。
  useEffect(() => {
    const chart = chartRef.current;
    if (chart === null) return;
    chart.removeIndicator({ name: "EMA", paneId: "candle_pane" });
    if (bars.length === 0) return;
    chart.createIndicator({ name: "EMA", calcParams: [9, 21, 50], paneId: "candle_pane" }, false);
  }, [selected, period, bars]);

  // AVWAP 线：从起点 bar 起算。
  useEffect(() => {
    const chart = chartRef.current;
    if (chart === null) return;
    chart.removeIndicator({ name: "AVWAP", paneId: "candle_pane" });
    if (bars.length === 0) return;
    chart.createIndicator(
      { name: "AVWAP", calcParams: [avwapStartIndex], paneId: "candle_pane" },
      false,
    );
  }, [selected, period, bars, avwapStartIndex]);

  // AVWAP 起点竖线。
  useEffect(() => {
    const chart = chartRef.current;
    if (chart === null) return;
    chart.removeOverlay({ name: "verticalStraightLine" });
    const bar = bars[avwapStartIndex];
    if (bar === undefined) return;
    chart.createOverlay({
      name: "verticalStraightLine",
      lock: true,
      points: [{ timestamp: toTimestamp(bar.date), value: bar.close }],
      styles: { line: { color: CHART_COLORS.avwap, style: "dashed" } },
    });
  }, [avwapStartIndex, selected, period, bars]);

  // 触发信号：只在分钟图上画，锚点是触发那根 K 线的最高价。
  useEffect(() => {
    const chart = chartRef.current;
    if (chart === null) return;
    chart.removeOverlay({ name: "signalMarker" });
    if (period !== "minute" || signal?.trigger == null) return;
    const trigger = signal.trigger;
    const bar = bars.find((item) => item.date === trigger.ts);
    if (bar === undefined) return;
    chart.createOverlay({
      name: "signalMarker",
      lock: true,
      extendData: { label: `触发 ${trigger.ts.slice(11)}` },
      points: [{ timestamp: toTimestamp(bar.date), value: bar.high }],
    });
  }, [bars, period, selected, signal]);

  // 已画的矩形与水平线：按 id 对账，多的删掉、少的补上；拖动结束后写回数据库。
  useEffect(() => {
    const chart = chartRef.current;
    if (chart === null) return;
    const managed = new Set(["zoneBox", "priceLine"]);
    const existing = chart.getOverlays().filter((overlay) => managed.has(overlay.name));
    const wanted = new Set(zones.map((zone) => zone.id));
    existing.forEach((overlay) => {
      if (overlay.id === pendingRef.current) return;
      if (!wanted.has(overlay.id)) chart.removeOverlay({ id: overlay.id });
    });
    const have = new Set(existing.map((overlay) => overlay.id));
    const first = bars[0];
    const last = bars[bars.length - 1];
    if (first === undefined || last === undefined) return;

    zones.forEach((zone) => {
      if (have.has(zone.id)) return;
      if (zone.kind === "hline") {
        chart.createOverlay({
          id: zone.id,
          name: "priceLine",
          points: [{ timestamp: toTimestamp(first.date), value: zone.low }],
          styles: { line: { color: CHART_COLORS.level, style: "dashed" } },
          onPressedMoveEnd: (event) => {
            const value = event.overlay.points[0]?.value;
            if (typeof value === "number") {
              moveZone(zone, value, value, flowRef.current.avwapStartDate);
            }
          },
        });
        return;
      }
      if (zone.kind === "avwap_start") return;
      const kind: ZoneKind = zone.kind === "gap" ? "gap" : "support";
      chart.createOverlay({
        id: zone.id,
        name: "zoneBox",
        extendData: { kind, label: kind === "gap" ? "缺口" : "支撑区" },
        points: [
          { timestamp: toTimestamp(first.date), value: zone.high },
          { timestamp: toTimestamp(last.date), value: zone.low },
        ],
        onPressedMoveEnd: (event) => {
          const values = event.overlay.points
            .map((point) => point.value)
            .filter((value): value is number => typeof value === "number");
          if (values.length === 2) {
            moveZone(zone, Math.min(...values), Math.max(...values), flowRef.current.avwapStartDate);
          }
        },
      });
    });
  }, [zones, bars, selected, period]);

  // 工具：选中之后在图上画，画完把几何形状写回。
  useEffect(() => {
    const chart = chartRef.current;
    if (chart === null || tool === "none" || selected === null) return;
    if (bars[0] === undefined) return;
    let created: string | null = null;
    const shared: Partial<OverlayCreate> = {
      onDrawEnd: (event) => {
        const overlay = event.overlay;
        created = overlay.id;
        pendingRef.current = null;
        const values = overlay.points
          .map((point) => point.value)
          .filter((value): value is number => typeof value === "number");
        const current = flowRef.current;
        if (tool === "horizontal") {
          if (values.length > 0) {
            void saveZone({
              id: overlay.id,
              code: current.selected ?? "",
              kind: "hline",
              low: values[0],
              high: values[0],
              avwapStartDate: current.avwapStartDate,
            });
          }
        } else if (tool === "avwap") {
          const timestamp = overlay.points[0]?.timestamp;
          const bar = barsRef.current.find((item) => toTimestamp(item.date) === timestamp);
          if (bar !== undefined) {
            current.setAvwapStartDate(bar.date);
            // 起点变了，参考价跟着变，所有标注的置信度重算。
            void resaveZones(bar.date);
          }
        } else if (values.length === 2) {
          void saveZone({
            id: overlay.id,
            code: current.selected ?? "",
            kind: tool === "gap" ? "gap" : "support",
            low: Math.min(...values),
            high: Math.max(...values),
            avwapStartDate: current.avwapStartDate,
          });
        }
        current.setTool("none");
      },
    };
    const drawing =
      tool === "horizontal"
        ? chart.createOverlay({ name: "priceLine", ...shared })
        : tool === "avwap"
          ? chart.createOverlay({ name: "verticalStraightLine", ...shared })
          : chart.createOverlay({
              name: "zoneBox",
              extendData: { kind: tool === "gap" ? "gap" : "support", label: "" },
              ...shared,
            });
    pendingRef.current = typeof drawing === "string" ? drawing : null;
    return () => {
      // 画到一半就换了工具：只删掉这一条正在画的，别动已保存的标注。
      if (created !== null) return;
      const pending = pendingRef.current;
      pendingRef.current = null;
      if (pending !== null) chart.removeOverlay({ id: pending });
    };
  }, [tool, selected, bars]);

  const message = useMemo(() => {
    if (chartFailed) return "图表不可用";
    if (selected === null) return "先选一只股票";
    if (barsError !== null) return barsError;
    if (bars.length === 0 && !loadingBars) return "数据不足";
    return null;
  }, [chartFailed, selected, barsError, bars, loadingBars]);

  return (
    <div className="relative flex min-h-0 flex-1">
      <div className="h-full min-h-0 w-full flex-1" ref={holderRef} />
      {message === null ? null : (
        <div className="absolute inset-0 flex flex-col items-center justify-center gap-1 border border-dashed border-rule">
          <p className="font-display text-lede text-ink">{message}</p>
          <p className="text-micro text-ink-soft">图版留白</p>
        </div>
      )}
    </div>
  );
}

/** 拖动结束或水平线移动后，写回新的价格并让服务端重算置信度。 */
function moveZone(
  zone: SupportZoneView,
  low: number,
  high: number,
  avwapStartDate: string | null,
): void {
  if (zone.kind !== "hline" && zone.kind !== "support" && zone.kind !== "gap") return;
  void saveZone({
    id: zone.id,
    code: zone.code,
    kind: zone.kind,
    low,
    high,
    avwapStartDate,
  });
}

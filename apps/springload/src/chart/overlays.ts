import { registerIndicator, registerOverlay } from "klinecharts";
import { CHART_COLORS } from "./palette";

export type ZoneKind = "support" | "gap";

interface ZoneExtend {
  kind?: ZoneKind;
  label?: string;
}

const palette: Record<ZoneKind, { fill: string; border: string; text: string }> = {
  support: {
    fill: CHART_COLORS.levelFill,
    border: CHART_COLORS.level,
    text: CHART_COLORS.level,
  },
  gap: {
    fill: CHART_COLORS.referenceFill,
    border: CHART_COLORS.reference,
    text: CHART_COLORS.reference,
  },
};

/**
 * klinecharts 内置 overlay 里没有矩形，支撑区与缺口都是矩形，所以自己注册一个。
 * 两点（左上、右下）拖出矩形，颜色由 extendData.kind 决定。
 */
registerOverlay<ZoneExtend>({
  name: "zoneBox",
  totalStep: 3,
  needDefaultPointFigure: true,
  needDefaultXAxisFigure: true,
  needDefaultYAxisFigure: true,
  createPointFigures: ({ coordinates, overlay }) => {
    if (coordinates.length < 2) return [];
    const [a, b] = coordinates;
    const x = Math.min(a.x, b.x);
    const y = Math.min(a.y, b.y);
    const width = Math.abs(a.x - b.x);
    const height = Math.abs(a.y - b.y);
    if (width < 2 || height < 2) return [];
    const kind = (overlay.extendData as ZoneExtend | undefined)?.kind ?? "support";
    const color = palette[kind];
    return [
      {
        key: "zone",
        type: "rect",
        attrs: { x, y, width, height },
        styles: { style: "stroke_fill", color: color.fill, borderColor: color.border, borderSize: 1 },
      },
      {
        key: "label",
        type: "text",
        attrs: {
          x: x + 6,
          y: y + 12,
          text: (overlay.extendData as ZoneExtend | undefined)?.label ?? "",
          align: "left",
          baseline: "middle",
        },
        styles: { color: color.text, size: 11, weight: "500", backgroundColor: "transparent" },
        ignoreEvent: true,
      },
    ];
  },
});

interface AvwapPoint {
  avwap?: number;
}

interface SignalExtend {
  label?: string;
}

/**
 * 触发信号标记：在触发那根分钟线的最高价上方画一个向下指的三角与一行文字。
 * 只在分钟图上用，锚点是触发那根 K 线。
 */
registerOverlay<SignalExtend>({
  name: "signalMarker",
  totalStep: 1,
  createPointFigures: ({ coordinates, overlay }) => {
    const point = coordinates[0];
    if (point === undefined) return [];
    const half = 7;
    const height = 11;
    const color = CHART_COLORS.reference;
    return [
      {
        key: "marker",
        type: "polygon",
        attrs: {
          coordinates: [
            { x: point.x, y: point.y },
            { x: point.x - half, y: point.y - height },
            { x: point.x + half, y: point.y - height },
          ],
        },
        styles: { style: "fill", color, borderColor: color, borderSize: 1 },
        ignoreEvent: true,
      },
      {
        key: "label",
        type: "text",
        attrs: {
          x: point.x,
          y: point.y - height - 4,
          text: (overlay.extendData as SignalExtend | undefined)?.label ?? "",
          align: "center",
          baseline: "bottom",
        },
        styles: { color, size: 11, weight: "600", backgroundColor: "transparent" },
        ignoreEvent: true,
      },
    ];
  },
});

/**
 * klinecharts 内置指标里没有 AVWAP，自己注册一个：从 `calcParams[0]` 指定的 bar
 * 起，按典型价 `(high + low + close) / 3` 与成交量累计平均成本价。
 */
registerIndicator<AvwapPoint, number>({
  name: "AVWAP",
  shortName: "AVWAP",
  series: "price",
  calcParams: [0],
  precision: 2,
  styles: { lines: [{ color: CHART_COLORS.avwap, style: "dashed" }] },
  figures: [{ key: "avwap", title: "AVWAP: ", type: "line" }],
  calc: (dataList, indicator) => {
    const start = Math.max(0, Math.floor(Number(indicator.calcParams[0]) || 0));
    let priceVolume = 0;
    let volume = 0;
    return dataList.map((bar, index) => {
      if (index < start) return {};
      const typical = (bar.high + bar.low + bar.close) / 3;
      const barVolume = bar.volume ?? 0;
      priceVolume += typical * barVolume;
      volume += barVolume;
      return volume > 0 ? { avwap: priceVolume / volume } : {};
    });
  },
});

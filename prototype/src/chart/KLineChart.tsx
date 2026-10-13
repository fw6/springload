import { useEffect, useRef } from 'react'
import { dispose, init, type Chart, type KLineData, type OverlayCreate } from 'klinecharts'
import type { Bar } from '../data/market'
import type { ZoneKind } from './overlays'
import './overlays'

export type ToolId = 'none' | 'horizontal' | 'zone' | 'gap' | 'avwap'

export interface BoxRecord {
  id: string
  kind: ZoneKind
  low: number
  high: number
  label: string
}

export interface LineRecord {
  id: string
  price: number
}

interface Props {
  code: string
  bars: Bar[]
  period: 'day' | 'minute'
  showEma: boolean
  avwapStart: number | null
  boxes: BoxRecord[]
  lines: LineRecord[]
  tool: ToolId
  onToolDone: () => void
  onBoxDrawn: (box: { id: string; kind: ZoneKind; low: number; high: number }) => void
  onBoxMoved: (id: string, low: number, high: number) => void
  onLineDrawn: (line: { id: string; price: number }) => void
  onLineMoved: (id: string, price: number) => void
  onAvwapPicked: (dataIndex: number) => void
}

const PERIOD = { day: { type: 'day', span: 1 }, minute: { type: 'minute', span: 1 } } as const

const toKLine = (bar: Bar): KLineData => ({
  timestamp: bar.timestamp,
  open: bar.open,
  high: bar.high,
  low: bar.low,
  close: bar.close,
  volume: bar.volume,
})

export function KLineChart(props: Props) {
  const { code, bars, period, showEma, avwapStart, boxes, lines, tool } = props
  const holderRef = useRef<HTMLDivElement | null>(null)
  const chartRef = useRef<Chart | null>(null)
  /** 正在画的 overlay 的 id，对账时要跳过它。 */
  const pendingRef = useRef<string | null>(null)
  const barsRef = useRef<Bar[]>(bars)
  const periodRef = useRef(period)
  const propsRef = useRef(props)
  barsRef.current = bars
  periodRef.current = period
  propsRef.current = props

  useEffect(() => {
    const holder = holderRef.current
    if (holder === null) return
    const chart = init(holder, { locale: 'zh-CN' })
    if (chart === null) return
    chartRef.current = chart
    chart.setStyles({
      grid: { horizontal: { color: '#eef1f4' }, vertical: { color: '#eef1f4' } },
      candle: { bar: { upColor: '#c0392b', downColor: '#1e8e5a', upBorderColor: '#c0392b', downBorderColor: '#1e8e5a' } },
      indicator: { lines: [{ color: '#2563eb' }, { color: '#b45309' }, { color: '#7c3aed' }] },
    })
    return () => {
      dispose(holder)
      chartRef.current = null
    }
  }, [])

  // 标的与周期：换股票或换周期都重新载入数据。
  useEffect(() => {
    const chart = chartRef.current
    if (chart === null) return
    chart.setSymbol({ ticker: code, pricePrecision: 2, volumePrecision: 0 })
    chart.setPeriod(PERIOD[period])
    chart.setDataLoader({
      getBars: ({ type, callback }) => {
        if (type === 'init') callback(barsRef.current.map(toKLine))
        else callback([], false)
      },
    })
    chart.resetData()
    chart.setOffsetRightDistance(24)
  }, [code, period])

  useEffect(() => {
    const chart = chartRef.current
    if (chart === null) return
    if (showEma) {
      chart.createIndicator({ name: 'EMA', calcParams: [9, 21, 50], paneId: 'candle_pane' }, false)
    } else {
      chart.removeIndicator({ name: 'EMA', paneId: 'candle_pane' })
    }
  }, [showEma, code, period])

  // AVWAP 起点：图上画一条竖线标出来。
  useEffect(() => {
    const chart = chartRef.current
    if (chart === null) return
    chart.removeOverlay({ name: 'verticalStraightLine' })
    if (avwapStart === null) return
    const bar = bars[avwapStart]
    if (bar === undefined) return
    chart.createOverlay({
      name: 'verticalStraightLine',
      lock: true,
      points: [{ timestamp: bar.timestamp, value: bar.close }],
      styles: { line: { color: '#7c3aed', style: 'dashed' } },
    })
  }, [avwapStart, code, period, bars])

  // 已画的矩形与水平线：按 id 对账，多的删掉、少的补上。拖动由 klinecharts 自己处理。
  useEffect(() => {
    const chart = chartRef.current
    if (chart === null) return
    const managed = new Set(['zoneBox', 'priceLine'])
    const existing = chart.getOverlays().filter((o) => managed.has(o.name))
    const wanted = new Set<string>([...boxes.map((b) => b.id), ...lines.map((l) => l.id)])
    existing.forEach((o) => {
      // 正在画的这一条还没画完，不能在这里被当成多余的删掉。
      if (o.id === pendingRef.current) return
      if (!wanted.has(o.id)) chart.removeOverlay({ id: o.id })
    })
    const have = new Set(existing.map((o) => o.id))
    boxes.forEach((box) => {
      if (have.has(box.id)) return
      const first = bars[0]
      const last = bars[bars.length - 1]
      if (first === undefined || last === undefined) return
      chart.createOverlay({
        id: box.id,
        name: 'zoneBox',
        extendData: { kind: box.kind, label: box.label },
        points: [
          { timestamp: first.timestamp, value: box.high },
          { timestamp: last.timestamp, value: box.low },
        ],
        onPressedMoveEnd: (event) => {
          const values = event.overlay.points.map((p) => p.value).filter((v): v is number => typeof v === 'number')
          propsRef.current.onBoxMoved(box.id, Math.min(...values), Math.max(...values))
        },
      })
    })
    lines.forEach((line) => {
      if (have.has(line.id)) return
      const first = bars[0]
      if (first === undefined) return
      chart.createOverlay({
        id: line.id,
        name: 'priceLine',
        points: [{ timestamp: first.timestamp, value: line.price }],
        styles: { line: { color: '#0f766e', style: 'dashed' } },
        onPressedMoveEnd: (event) => {
          const value = event.overlay.points[0]?.value
          if (typeof value === 'number') propsRef.current.onLineMoved(line.id, value)
        },
      })
    })
  }, [boxes, lines, code, period, bars])

  // 工具：选中之后在图上画，画完把几何形状回报给外面。
  useEffect(() => {
    const chart = chartRef.current
    if (chart === null || tool === 'none') return
    const first = bars[0]
    if (first === undefined) return
    let created: string | null = null
    const shared: Partial<OverlayCreate> = {
      onDrawEnd: (event) => {
        const overlay = event.overlay
        created = overlay.id
        pendingRef.current = null
        const values = overlay.points.map((p) => p.value).filter((v): v is number => typeof v === 'number')
        if (tool === 'horizontal') {
          if (values.length > 0) propsRef.current.onLineDrawn({ id: overlay.id, price: values[0] })
        } else if (tool === 'avwap') {
          const timestamp = overlay.points[0]?.timestamp
          const index = barsRef.current.findIndex((b) => b.timestamp === timestamp)
          if (index >= 0) propsRef.current.onAvwapPicked(index)
        } else {
          if (values.length === 2) {
            propsRef.current.onBoxDrawn({
              id: overlay.id,
              kind: tool === 'gap' ? 'gap' : 'support',
              low: Math.min(...values),
              high: Math.max(...values),
            })
          }
        }
        propsRef.current.onToolDone()
      },
    }
    const drawing =
      tool === 'horizontal'
        ? chart.createOverlay({ name: 'priceLine', ...shared })
        : tool === 'avwap'
          ? chart.createOverlay({ name: 'verticalStraightLine', ...shared })
          : chart.createOverlay({ name: 'zoneBox', extendData: { kind: tool === 'gap' ? 'gap' : 'support', label: '' }, ...shared })
    pendingRef.current = typeof drawing === 'string' ? drawing : null
    return () => {
      if (created !== null) return
      pendingRef.current = null
      chart.removeOverlay({ name: 'zoneBox' })
      chart.removeOverlay({ name: 'priceLine' })
      chart.removeOverlay({ name: 'verticalStraightLine' })
    }
  }, [tool])

  return <div className="kline" ref={holderRef} />
}

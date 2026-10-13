import { useCallback, useMemo, useState } from 'react'
import type { BoxRecord, LineRecord, ToolId } from '../chart/KLineChart'
import { avwap, evaluateZone, minuteBars, type Confidence } from '../data/market'
import { byCode, featured } from '../data/stocks'
import { planFixture, size, type Sizing } from '../data/fixtures'

export type StepIndex = 1 | 2 | 3 | 4

export interface Signal {
  triggeredAt: string
  price: number
  timeframe: string
  brokenHigh: number
}

export interface BoxView extends BoxRecord {
  /** 三个参考价里有几个落在框内 */
  hits: number
  confidence: Confidence
}

const day = featured.bars[featured.bars.length - 1].timestamp

/** 换股票时把标注重置成这只股票的初始支撑区：刚好裹住三个参考价。 */
function seedFor(detail: typeof featured): { boxes: BoxRecord[]; lines: LineRecord[] } {
  const values = [detail.refs.ema21, detail.refs.avwap, detail.refs.prevHigh].filter((v) => Number.isFinite(v))
  const low = Math.min(...values) * 0.995
  const high = Math.max(...values) * 1.005
  return {
    boxes: [
      {
        id: 'seed-zone',
        kind: 'support',
        low: Math.round(low * 100) / 100,
        high: Math.round(high * 100) / 100,
        label: '支撑区',
      },
    ],
    lines: [{ id: 'seed-line', price: Math.round(detail.row.prevLow * 100) / 100 }],
  }
}

export function useFlow() {
  const [step, setStep] = useState<StepIndex>(1)
  const [watchlist, setWatchlist] = useState<string[]>([featured.row.code, '601899'])
  const [selected, setSelected] = useState(featured.row.code)
  const [period, setPeriod] = useState<'day' | 'minute'>('day')
  const [showEma, setShowEma] = useState(true)
  const [tool, setTool] = useState<ToolId>('none')
  const [avwapStart, setAvwapStart] = useState<number | null>(60)
  const [sizing, setSizing] = useState<Sizing>(planFixture)
  const [signal, setSignal] = useState<Signal | null>(null)

  const detail = byCode.get(selected) ?? featured

  const refs = useMemo(() => {
    const bars = detail.bars
    const ema21 = detail.refs.ema21
    const start = avwapStart ?? 60
    return {
      ema21,
      avwap: avwap(bars, start),
      prevHigh: detail.refs.prevHigh,
    }
  }, [detail, avwapStart])

  const refLevel = useMemo(() => evaluateZone(refs), [refs])

  const [boxes, setBoxes] = useState<BoxRecord[]>(() => seedFor(featured).boxes)
  const [lines, setLines] = useState<LineRecord[]>(() => seedFor(featured).lines)

  const boxViews: BoxView[] = useMemo(
    () =>
      boxes.map((box) => {
        const values = [refs.ema21, refs.avwap, refs.prevHigh]
        const hits = values.filter((v) => Number.isFinite(v) && v >= box.low && v <= box.high).length
        const confidence: Confidence = hits >= 2 && refLevel.confidence === '高' ? '高' : hits >= 2 ? refLevel.confidence : '低'
        return { ...box, hits, confidence }
      }),
    [boxes, refs, refLevel],
  )

  const hasHighConfidence = boxViews.some((b) => b.confidence === '高')

  const bars = period === 'day' ? detail.bars : minuteBars(detail.row.code, detail.row.price, day)

  const result = useMemo(() => size(sizing), [sizing])

  const selectStock = useCallback((code: string) => {
    const next = byCode.get(code)
    if (next === undefined) return
    const seed = seedFor(next)
    setSelected(code)
    setBoxes(seed.boxes)
    setLines(seed.lines)
    setSignal(null)
    setAvwapStart(60)
    setStep(2)
  }, [])

  const resetTool = useCallback(() => setTool('none'), [])

  const triggerSignal = useCallback(() => {
    const last = detail.bars[detail.bars.length - 1]
    const prev = detail.bars[detail.bars.length - 2]
    const entry = Math.round(last.close * 100) / 100
    // 止损放在支撑区下沿，没有支撑区就退回买入价下方 2%。
    const floor = boxes.length > 0 ? Math.min(...boxes.map((b) => b.low)) : entry * 0.98
    setSignal({
      triggeredAt: '2026-10-09 14:32',
      price: entry,
      timeframe: '1 分钟',
      brokenHigh: prev.high,
    })
    setSizing((current) => ({ ...current, entry, stop: Math.round(floor * 100) / 100 }))
  }, [detail, boxes])

  const canEnter = (target: StepIndex) => {
    if (target <= 1) return true
    if (target === 2) return watchlist.length > 0
    if (target === 3) return watchlist.length > 0 && hasHighConfidence
    return watchlist.length > 0 && hasHighConfidence && signal !== null
  }

  const goStep = useCallback(
    (target: StepIndex) => {
      if (!canEnter(target)) return
      setStep(target)
      // 第 3 步看分钟图，其余步骤看日线。
      setPeriod(target === 3 ? 'minute' : 'day')
      if (target !== 2) setTool('none')
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [watchlist.length, hasHighConfidence, signal],
  )

  return {
    step,
    setStep,
    goStep,
    canEnter,
    watchlist,
    setWatchlist,
    selected,
    selectStock,
    detail,
    bars,
    period,
    setPeriod,
    showEma,
    setShowEma,
    tool,
    setTool,
    resetTool,
    avwapStart,
    setAvwapStart,
    refs,
    refLevel,
    boxes,
    setBoxes,
    lines,
    setLines,
    boxViews,
    hasHighConfidence,
    sizing,
    setSizing,
    result,
    signal,
    setSignal,
    triggerSignal,
  }
}

export type Flow = ReturnType<typeof useFlow>

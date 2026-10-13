// 一次性原型的模拟行情。不联网，全部由种子确定性生成，形状符合「回调蓄势」：
// 主升 → 第一次回踩 → 反弹 → 第二次回踩（低点抬高）→ 收敛整理。

export interface Bar {
  timestamp: number
  open: number
  high: number
  low: number
  close: number
  volume: number
}

export interface ScreenerRow {
  code: string
  name: string
  sector: string
  market: '沪市主板' | '深市主板'
  price: number
  chg1m: number
  chg3m: number
  chg6m: number
  rankPct: number
  avgVolumeWan: number
  /** 近 20 日（最高价 − 最低价）的均值 ÷ 20 日均价 */
  convergence: number
  c1: boolean
  c2: boolean
  c3: boolean
  c4: boolean
  prevHigh: number
  prevLow: number
  gapTop: number
  gapBottom: number
}

function mulberry32(seed: number) {
  let a = seed >>> 0
  return () => {
    a = (a + 0x6d2b79f5) >>> 0
    let t = a
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

const DAY = 24 * 60 * 60 * 1000

/** 把「最近的交易日」倒推出来，跳过周末，够原型用。 */
export function tradingDaysBack(count: number, endDate = new Date('2026-10-09T00:00:00+08:00')): number[] {
  const out: number[] = []
  const cursor = new Date(endDate.getTime())
  while (out.length < count) {
    const dow = cursor.getDay()
    if (dow !== 0 && dow !== 6) out.push(cursor.getTime())
    cursor.setTime(cursor.getTime() - DAY)
  }
  return out.reverse()
}

/** 每只股票的形状参数：主升幅度、整理区宽度、整理区收尾的漂移。 */
export interface Shape {
  /** 近 6 个月涨幅 */
  gain: number
  /** 整理区的宽度，占现价的比例 */
  tightness: number
  /** 整理区末端的漂移，负值表示收尾走弱 */
  endDrift: number
}

/** 收盘价的锚点曲线：120 根日线里的七个阶段。 */
function anchorCurve(price: number, shape: Shape): number[] {
  // 整理区紧贴反弹高点下方，宽度是 tightness；末端漂移由 endDrift 决定。
  const peak = price * 1.015
  const width = price * shape.tightness
  const seg = (from: number, to: number, bars: number) => {
    const out: number[] = []
    for (let i = 0; i < bars; i += 1) out.push(from + (to - from) * (i / (bars - 1)))
    return out
  }
  return [
    ...seg(price / (1 + shape.gain), price, 24), // 主升
    ...seg(price, price * 0.94, 24), // 第一次回踩
    ...seg(price * 0.94, peak, 24), // 反弹
    ...seg(peak, peak - width * 0.1, 12), // 回落进整理区
    ...seg(peak - width * 0.1, peak - width * 0.35, 16), // 区间内反抽
    ...seg(peak - width * 0.35, peak - width, 10), // 下探到整理区下沿
    ...seg(peak - width, peak - width * 0.35 + price * shape.endDrift, 10), // 收尾
  ]
}

export function dailyBars(code: string, price: number, shape: Shape): Bar[] {
  const seed = Number(code.slice(-3)) + 7
  const rand = mulberry32(seed)
  const days = tradingDaysBack(120)
  const curve = anchorCurve(price, shape)
  const bars: Bar[] = []
  let prevClose = curve[0]
  curve.forEach((target, index) => {
    const noise = (rand() - 0.5) * price * 0.012
    const close = target + noise
    const open = prevClose + (rand() - 0.5) * price * 0.006
    const high = Math.max(open, close) + rand() * price * 0.004
    const low = Math.min(open, close) - rand() * price * 0.004
    const base = 60000 + (index % 7) * 9000
    const volume = Math.round(base * (0.7 + rand() * 0.8) * (index > 95 ? 0.75 : 1))
    bars.push({ timestamp: days[index], open: r2(open), high: r2(high), low: r2(low), close: r2(close), volume })
    prevClose = close
  })
  return bars
}

/** 触发阶段的 1 分钟线：下探后回升，最后一根突破前一根高点。 */
export function minuteBars(code: string, price: number, day: number): Bar[] {
  const rand = mulberry32(Number(code.slice(-3)) + 991)
  const bars: Bar[] = []
  const start = day + 9.5 * 60 * 60 * 1000
  let cursor = price * 0.998
  for (let i = 0; i < 240; i += 1) {
    // 上午下探，午后回升
    const drift = i < 90 ? -price * 0.00035 : price * 0.00042
    const open = cursor
    const close = open + drift + (rand() - 0.5) * price * 0.0016
    const high = Math.max(open, close) + rand() * price * 0.0009
    const low = Math.min(open, close) - rand() * price * 0.0009
    bars.push({
      timestamp: start + i * 60 * 1000,
      open: r2(open),
      high: r2(high),
      low: r2(low),
      close: r2(close),
      volume: Math.round(400 + rand() * 900),
    })
    cursor = close
  }
  return bars
}

function r2(value: number) {
  return Math.round(value * 100) / 100
}

export function ema(values: number[], period: number): number[] {
  if (values.length === 0) return []
  const alpha = 2 / (period + 1)
  const out: number[] = []
  let prev = values.slice(0, period).reduce((sum, v) => sum + v, 0) / Math.min(period, values.length)
  values.forEach((value, index) => {
    if (index < period - 1) {
      out.push(Number.NaN)
      return
    }
    if (index === period - 1) {
      out.push(prev)
      return
    }
    prev = value * alpha + prev * (1 - alpha)
    out.push(prev)
  })
  return out
}

/** 从 startIndex 起按成交量加权的平均成本价。 */
export function avwap(bars: Bar[], startIndex: number): number {
  let pv = 0
  let vol = 0
  for (let i = startIndex; i < bars.length; i += 1) {
    const typical = (bars[i].high + bars[i].low + bars[i].close) / 3
    pv += typical * bars[i].volume
    vol += bars[i].volume
  }
  return vol === 0 ? Number.NaN : pv / vol
}

export interface ZoneRefs {
  ema21: number
  avwap: number
  prevHigh: number
}

export type Confidence = '高' | '中' | '低'

/** 三处参考价的离散程度：±2% 以内为高置信。 */
export function evaluateZone(refs: ZoneRefs): { confidence: Confidence; spread: number } {
  const values = [refs.ema21, refs.avwap, refs.prevHigh].filter((v) => Number.isFinite(v))
  if (values.length < 2) return { confidence: '低', spread: Number.NaN }
  const max = Math.max(...values)
  const min = Math.min(...values)
  const mean = values.reduce((sum, v) => sum + v, 0) / values.length
  const spread = (max - min) / mean
  if (spread <= 0.02) return { confidence: '高', spread }
  if (spread <= 0.05) return { confidence: '中', spread }
  return { confidence: '低', spread }
}

import { avwap, dailyBars, ema, evaluateZone, type Bar, type ScreenerRow, type Shape } from './market'

interface Seed extends Shape {
  code: string
  name: string
  sector: string
  price: number
}

// 沪深主板（沪市 600/601/603/605、深市 000/001/002/003），价格用人民币。
// gain 决定近 6 个月涨幅，tightness 决定整理区宽度，endDrift 决定收尾强弱。
// 三只涨幅最高的是 600585 / 601899 / 002415，只有排名前 30% 才可能中 C1。
const seeds: Seed[] = [
  { code: '600036', name: '招商银行', sector: '银行', price: 38.62, gain: 0.62, tightness: 0.02, endDrift: 0.005 },
  { code: '601899', name: '紫金矿业', sector: '有色金属', price: 17.44, gain: 0.81, tightness: 0.015, endDrift: 0.004 },
  { code: '600030', name: '中信证券', sector: '证券', price: 26.81, gain: 0.44, tightness: 0.022, endDrift: 0.005 },
  { code: '000001', name: '平安银行', sector: '银行', price: 11.23, gain: 0.18, tightness: 0.028, endDrift: -0.01 },
  { code: '002415', name: '海康威视', sector: '电子', price: 31.56, gain: 0.72, tightness: 0.105, endDrift: 0.03 },
  { code: '600276', name: '恒瑞医药', sector: '医药生物', price: 45.9, gain: 0.35, tightness: 0.025, endDrift: 0.006 },
  { code: '000651', name: '格力电器', sector: '家用电器', price: 42.35, gain: 0.27, tightness: 0.02, endDrift: -0.025 },
  { code: '601012', name: '隆基绿能', sector: '电力设备', price: 19.72, gain: 0.09, tightness: 0.11, endDrift: -0.05 },
  { code: '600887', name: '伊利股份', sector: '食品饮料', price: 28.41, gain: 0.52, tightness: 0.018, endDrift: 0.004 },
  { code: '002594', name: '比亚迪', sector: '汽车', price: 245.3, gain: 0.68, tightness: 0.085, endDrift: 0.025 },
  { code: '600585', name: '海螺水泥', sector: '建筑材料', price: 24.16, gain: 0.96, tightness: 0.014, endDrift: 0.003 },
  { code: '000333', name: '美的集团', sector: '家用电器', price: 63.85, gain: 0.4, tightness: 0.02, endDrift: -0.02 },
]

export interface StockDetail {
  row: ScreenerRow
  bars: Bar[]
  refs: { ema21: number; avwap: number; prevHigh: number }
  confidence: ReturnType<typeof evaluateZone>
}

function buildDetail(seed: Seed): StockDetail {
  const bars = dailyBars(seed.code, seed.price, seed)
  const closes = bars.map((b) => b.close)
  const n = bars.length
  const chg = (back: number) => (closes[n - 1] - closes[n - 1 - back]) / closes[n - 1 - back]
  const window20 = bars.slice(-20)
  const hi = Math.max(...window20.map((b) => b.high))
  const lo = Math.min(...window20.map((b) => b.low))
  const mean = window20.reduce((sum, b) => sum + b.close, 0) / 20
  const convergence = (hi - lo) / mean
  const ema9 = ema(closes, 9)
  const ema21 = ema(closes, 21)
  // 前期高点取反弹段的高点，前期低点取第一次回踩的低点。
  const prevHigh = Math.max(...bars.slice(64, 71).map((b) => b.high))
  const prevLow = Math.min(...bars.slice(40, 47).map((b) => b.low))
  const gapBar = bars[n - 12]
  const refs = {
    ema21: ema21[n - 1],
    avwap: avwap(bars, 60),
    prevHigh,
  }
  const row: ScreenerRow = {
    code: seed.code,
    name: seed.name,
    sector: seed.sector,
    market: seed.code.startsWith('6') ? '沪市主板' : '深市主板',
    price: closes[n - 1],
    chg1m: chg(21),
    chg3m: chg(63),
    chg6m: chg(119),
    rankPct: 0,
    avgVolumeWan: Math.round(bars.slice(-20).reduce((sum, b) => sum + b.volume, 0) / 20 / 100) / 100,
    convergence,
    c1: chg(119) > 0.3,
    c2: (hi - lo) / mean < 0.08,
    c3: closes[n - 1] > ema9[n - 1],
    c4: false,
    prevHigh,
    prevLow,
    gapTop: gapBar.open,
    gapBottom: bars[n - 13].close,
  }
  return { row, bars, refs, confidence: evaluateZone(refs) }
}

export const stocks: StockDetail[] = seeds.map(buildDetail)

// 按近 6 个月涨幅排名，赋排名百分位，再把 C1 按前 20%~30% 重新判定。
{
  const sorted = [...stocks].sort((a, b) => b.row.chg6m - a.row.chg6m)
  sorted.forEach((item, index) => {
    item.row.rankPct = (index + 1) / sorted.length
    item.row.c1 = item.row.chg6m > 0.3 && item.row.rankPct <= 0.3
    // C4 是排除条件：高点低点都在往下、且跌破向下的 EMA21。
    item.row.c4 = item.row.rankPct > 0.6 && item.row.convergence > 0.1
  })
}

export const byCode = new Map(stocks.map((s) => [s.row.code, s]))

const isPicked = (row: ScreenerRow) => row.c1 && row.c2 && row.c3 && !row.c4

/** 默认选中的股票取第一只四条全中的。 */
export const featured = stocks.find((s) => isPicked(s.row)) ?? stocks[0]

import { featured } from './stocks'

export interface Sizing {
  entry: number
  stop: number
  account: number
  riskPct: number
}

export interface SizingResult {
  perR: number
  maxLoss: number
  shares: number
  capital: number
  posPct: number
  stopDistance: number
  t3: number
  t5: number
  /** 因 100 股一手取整而未能用满的风险金额 */
  unusedRisk: number
}

export function size(input: Sizing): SizingResult {
  const perR = input.entry - input.stop
  const maxLoss = input.account * input.riskPct
  const rawShares = perR > 0 ? Math.floor(maxLoss / perR) : 0
  const shares = Math.floor(rawShares / 100) * 100
  const capital = shares * input.entry
  return {
    perR,
    maxLoss,
    shares,
    capital,
    posPct: input.account === 0 ? 0 : capital / input.account,
    stopDistance: input.entry === 0 ? 0 : perR / input.entry,
    t3: input.entry + 3 * perR,
    t5: input.entry + 5 * perR,
    unusedRisk: maxLoss - shares * perR,
  }
}

export const planFixture: Sizing = {
  entry: 38.6,
  stop: 37.6,
  account: 200000,
  riskPct: 0.005,
}

export interface PositionFixture {
  code: string
  name: string
  entryDate: string
  entry: number
  stop: number
  quantity: number
  last: number
  ema9: number
  t3: number
  t5: number
  status: '持有' | '已到 3R' | '跌破 9 日 EMA'
}

export const positions: PositionFixture[] = [
  {
    code: '600036',
    name: '招商银行',
    entryDate: '2026-09-18',
    entry: 38.6,
    stop: 37.6,
    quantity: 2600,
    last: 39.24,
    ema9: 38.9,
    t3: 41.6,
    t5: 43.6,
    status: '持有',
  },
  {
    code: '601899',
    name: '紫金矿业',
    entryDate: '2026-09-02',
    entry: 16.4,
    stop: 15.9,
    quantity: 20000,
    last: 18.1,
    ema9: 17.5,
    t3: 17.9,
    t5: 18.9,
    status: '已到 3R',
  },
  {
    code: '002415',
    name: '海康威视',
    entryDate: '2026-08-21',
    entry: 33.2,
    stop: 32.2,
    quantity: 1000,
    last: 30.9,
    ema9: 31.4,
    t3: 36.2,
    t5: 38.2,
    status: '跌破 9 日 EMA',
  },
]

export interface LogFixture {
  code: string
  name: string
  entryDate: string
  exitDate: string
  entry: number
  exit: number
  perR: number
  rMultiple: number
  pnl: number
  note: string
}

export const logs: LogFixture[] = [
  {
    code: '600585',
    name: '海螺水泥',
    entryDate: '2026-07-14',
    exitDate: '2026-08-06',
    entry: 22.8,
    exit: 26.9,
    perR: 1.1,
    rMultiple: 3.7,
    pnl: 8200,
    note: '到 3R 减三分之一，剩余跟随 9 日 EMA',
  },
  {
    code: '000651',
    name: '格力电器',
    entryDate: '2026-06-30',
    exitDate: '2026-07-11',
    entry: 41.2,
    exit: 40.0,
    perR: 1.2,
    rMultiple: -1,
    pnl: -2400,
    note: '跌破止损离场',
  },
  {
    code: '600887',
    name: '伊利股份',
    entryDate: '2026-06-09',
    exitDate: '2026-07-21',
    entry: 26.5,
    exit: 32.4,
    perR: 1.1,
    rMultiple: 5.4,
    pnl: 11800,
    note: '5R 再减三分之一，余量跟随 EMA',
  },
  {
    code: '601012',
    name: '隆基绿能',
    entryDate: '2026-05-19',
    exitDate: '2026-05-28',
    entry: 18.4,
    exit: 17.6,
    perR: 0.8,
    rMultiple: -1,
    pnl: -1600,
    note: '触发止损',
  },
  {
    code: '600030',
    name: '中信证券',
    entryDate: '2026-04-27',
    exitDate: '2026-06-03',
    entry: 24.1,
    exit: 27.3,
    perR: 1.0,
    rMultiple: 3.2,
    pnl: 6400,
    note: '3R 减仓后剩余被 EMA 打出',
  },
]

export const journalStats = {
  trades: logs.length,
  wins: logs.filter((l) => l.rMultiple > 0).length,
  winRate: logs.filter((l) => l.rMultiple > 0).length / logs.length,
  avgR: logs.reduce((sum, l) => sum + l.rMultiple, 0) / logs.length,
  pnl: logs.reduce((sum, l) => sum + l.pnl, 0),
}

export const account = {
  currency: '人民币',
  equity: 200000,
  defaultRiskPct: 0.005,
  defaultStopPct: 0.03,
  dataSource: '东方财富（日线 klt=101，前复权 fqt=1）',
  beginnerMode: true,
}

export const watchlist = [featured.row.code, '601899', '002415', '000651']

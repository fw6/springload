export const pct = (value: number) => `${(value * 100).toFixed(1)}%`

export const money = (value: number) => `¥${value.toLocaleString('zh-CN', { maximumFractionDigits: 0 })}`

export const price = (value: number) => value.toFixed(2)

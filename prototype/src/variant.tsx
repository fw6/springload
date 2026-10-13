import { useEffect } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'

export type VariantKey = 'A' | 'B' | 'C'

export interface VariantMeta {
  key: VariantKey
  name: string
  density: string
  ratio: string
  explain: string
  blurb: string
}

export const variants: Record<VariantKey, VariantMeta> = {
  A: {
    key: 'A',
    name: '紧凑 · 图表主导 · 悬停解释',
    density: '行高 28px，正文 12.5px，面板内边距 10px',
    ratio: '图表 : 面板 = 62 : 38',
    explain: '悬停',
    blurb: '信息塞得最满，图表占大头。所有术语与解释都收在悬停提示里，不占版面。适合已经熟悉这套流程、只想看图的人。',
  },
  B: {
    key: 'B',
    name: '宽松 · 面板主导 · 弹窗解释（已选定）',
    density: '行高 44px，正文 15px，面板内边距 20px',
    ratio: '图表 : 面板 = 38 : 62，面板可收起后图表占满',
    explain: '弹窗',
    blurb: '版面露白多，右侧面板是主角。解释默认不出现，点术语标签或「这一步在干什么」才弹出一整屏说明。面板可以收起来把图表铺满。',
  },
  C: {
    key: 'C',
    name: '中密度 · 面板可收起 · 内联解释',
    density: '行高 36px，正文 14px，面板内边距 14px',
    ratio: '图表 : 面板 = 55 : 45，面板可收起后图表占满',
    explain: '内联',
    blurb: '每个控件下面常驻一行白话说明，「展开」看完整解释。面板可以收起来把图表铺满。适合边看边学、看熟之后收起说明的人。',
  },
}

export const variantKeys: VariantKey[] = ['A', 'B', 'C']

export function useVariant(): VariantKey {
  const [params] = useSearchParams()
  const raw = params.get('variant')
  return raw === 'B' || raw === 'C' ? raw : 'A'
}

export function PrototypeSwitcher() {
  const current = useVariant()
  const navigate = useNavigate()
  const [params] = useSearchParams()

  const cycle = (delta: number) => {
    const index = variantKeys.indexOf(current)
    const next = variantKeys[(index + delta + variantKeys.length) % variantKeys.length]
    const search = new URLSearchParams(params)
    search.set('variant', next)
    navigate({ search: search.toString() }, { replace: true })
  }

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null
      const tag = target?.tagName
      if (tag === 'INPUT' || tag === 'TEXTAREA' || target?.isContentEditable === true) return
      if (event.key === 'ArrowLeft') cycle(-1)
      if (event.key === 'ArrowRight') cycle(1)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })

  if (import.meta.env.PROD) return null

  const meta = variants[current]
  return (
    <div className="switcher" data-variant={current}>
      <button type="button" className="switcher-arrow" onClick={() => cycle(-1)} aria-label="上一个变体">
        ←
      </button>
      <div className="switcher-body">
        <div className="switcher-title">
          变体 {meta.key} · {meta.name}
        </div>
        <div className="switcher-meta">
          {meta.density} ｜ {meta.ratio} ｜ 解释方式：{meta.explain}
        </div>
      </div>
      <button type="button" className="switcher-arrow" onClick={() => cycle(1)} aria-label="下一个变体">
        →
      </button>
    </div>
  )
}

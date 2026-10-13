import type { ReactNode } from 'react'
import type { Flow } from '../flow/useFlow'
import { steps, terms } from '../data/glossary'
import type { Confidence } from '../data/market'

export function ConfidencePill({ level }: { level: Confidence }) {
  return (
    <span className="pill" data-level={level}>
      {level}置信
    </span>
  )
}

/** 判定结果：满足显示对勾，不满足显示破折号。 */
export function Flag({ ok }: { ok: boolean }) {
  return (
    <span className="flag" data-ok={ok}>
      {ok ? '✓' : '—'}
    </span>
  )
}

export function AiCard({ text, disclaimer = true }: { text: string; disclaimer?: boolean }) {
  return (
    <div className="ai-card">
      <span className="ai-tag">AI 生成</span>
      <div>{text}</div>
      {disclaimer ? <span className="disclaimer">以上只是对图上形态与数据的描述，不构成买卖建议。</span> : null}
    </div>
  )
}

export function TermHint({ termKey, children }: { termKey: string; children?: ReactNode }) {
  const term = terms[termKey]
  if (term === undefined) return <>{children}</>
  return (
    <span className="tip">
      {children ?? term.label}
      <span className="tip-body">
        <b>{term.label}</b>
        <br />
        {term.plain}
      </span>
    </span>
  )
}

export function ChartToolbar({ flow }: { flow: Flow }) {
  const tools: Array<{ id: Flow['tool']; label: string }> = [
    { id: 'horizontal', label: '水平线' },
    { id: 'zone', label: '支撑区' },
    { id: 'gap', label: '跳空缺口' },
    { id: 'avwap', label: 'AVWAP 起点' },
  ]
  return (
    <div className="chart-toolbar">
      {tools.map((tool) => (
        <button
          key={tool.id}
          type="button"
          className="tool"
          aria-pressed={flow.tool === tool.id}
          onClick={() => flow.setTool(flow.tool === tool.id ? 'none' : tool.id)}
        >
          {tool.label}
        </button>
      ))}
      <button type="button" className="tool" onClick={() => flow.setTool('none')} disabled={flow.tool === 'none'}>
        退出标注
      </button>
      <span className="toolbar-spacer" />
      <button type="button" className="tool" aria-pressed={flow.period === 'day'} onClick={() => flow.setPeriod('day')}>
        日线
      </button>
      <button type="button" className="tool" aria-pressed={flow.period === 'minute'} onClick={() => flow.setPeriod('minute')}>
        1 分钟
      </button>
      <button type="button" className="tool" aria-pressed={flow.showEma} onClick={() => flow.setShowEma(!flow.showEma)}>
        EMA 9/21/50
      </button>
    </div>
  )
}

export function RefTable({ flow }: { flow: Flow }) {
  const rows: Array<{ label: string; value: number }> = [
    { label: 'EMA21', value: flow.refs.ema21 },
    { label: 'AVWAP', value: flow.refs.avwap },
    { label: '前期高点', value: flow.refs.prevHigh },
  ]
  return (
    <table className="grid">
      <tbody>
        {rows.map((row) => {
          const inside = flow.boxViews.filter((b) => row.value >= b.low && row.value <= b.high)
          return (
            <tr key={row.label}>
              <td className="left">{row.label}</td>
              <td className="mono">{row.value.toFixed(2)}</td>
              <td className="left" style={{ color: '#64748b' }}>
                {inside.length === 0 ? '不在任何框内' : `落在 ${inside.length} 个框内`}
              </td>
            </tr>
          )
        })}
        <tr>
          <td className="left">三者离散度</td>
          <td className="mono">{(flow.refLevel.spread * 100).toFixed(2)}%</td>
          <td className="left">
            <ConfidencePill level={flow.refLevel.confidence} />
            <span style={{ color: '#64748b', marginLeft: 8 }}>±2% 以内算高置信</span>
          </td>
        </tr>
      </tbody>
    </table>
  )
}

export function ZoneList({ flow }: { flow: Flow }) {
  const remove = (id: string) => {
    flow.setBoxes(flow.boxes.filter((b) => b.id !== id))
    flow.setLines(flow.lines.filter((l) => l.id !== id))
  }
  if (flow.boxViews.length === 0 && flow.lines.length === 0) {
    return <p className="hint">还没有标注。点上面的「支撑区」在图上拖一个框出来。</p>
  }
  return (
    <table className="grid">
      <thead>
        <tr>
          <th className="left">标注</th>
          <th>下沿</th>
          <th>上沿</th>
          <th className="left">置信度</th>
          <th />
        </tr>
      </thead>
      <tbody>
        {flow.boxViews.map((box) => (
          <tr key={box.id}>
            <td className="left">
              <span className="pill" data-tone={box.kind}>
                {box.kind === 'support' ? '支撑区' : '缺口'}
              </span>
            </td>
            <td className="mono">{box.low.toFixed(2)}</td>
            <td className="mono">{box.high.toFixed(2)}</td>
            <td className="left">
              <ConfidencePill level={box.confidence} />
              <span style={{ color: '#64748b', marginLeft: 8 }}>参考价命中 {box.hits}/3</span>
            </td>
            <td>
              <button type="button" className="tool" onClick={() => remove(box.id)}>
                删除
              </button>
            </td>
          </tr>
        ))}
        {flow.lines.map((line) => (
          <tr key={line.id}>
            <td className="left">
              <span className="pill">水平线</span>
            </td>
            <td className="mono" colSpan={2}>
              {line.price.toFixed(2)}
            </td>
            <td className="left" style={{ color: '#64748b' }}>
              拖动可改价格
            </td>
            <td>
              <button type="button" className="tool" onClick={() => remove(line.id)}>
                删除
              </button>
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}

export function StepGate({ flow, target, children }: { flow: Flow; target: 2 | 3 | 4; children: ReactNode }) {
  if (flow.canEnter(target)) return <>{children}</>
  const copy = steps[target - 1]
  return <div className="notice" data-tone="warn">{copy.blockedHint}</div>
}

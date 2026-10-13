import { useState, type ReactNode } from 'react'
import { NavLink } from 'react-router-dom'
import { KLineChart } from '../chart/KLineChart'
import type { Flow } from '../flow/useFlow'
import { aiExplain, riskNotes, steps, terms } from '../data/glossary'
import { account, journalStats, logs, positions } from '../data/fixtures'
import { stocks } from '../data/stocks'
import { money, pct, price } from '../format'
import { AiCard, ChartToolbar, RefTable, ZoneList } from './common'
import type { VariantModule } from './types'

const routes = [
  { to: '/', label: '主流程' },
  { to: '/plan', label: '计划卡' },
  { to: '/positions', label: '持仓跟踪' },
  { to: '/journal', label: '交易日志' },
  { to: '/settings', label: '设置' },
]

interface ModalState {
  title: string
  body: ReactNode
}

export function ShellB({ children }: { flow: Flow; children: ReactNode }) {
  return (
    <div className="app" data-variant="B">
      <header className="topbar">
        <div className="brand">
          蓄势 Springload<small>回调买入法工作台</small>
        </div>
        <div className="topbar-right">
          <span className="mono">账户 {money(account.equity)}</span>
          <span>东方财富 · 前复权</span>
        </div>
      </header>
      <nav className="tabs">
        {routes.map((route) => (
          <NavLink key={route.to} to={route.to} end className="tab">
            {route.label}
          </NavLink>
        ))}
      </nav>
      {children}
    </div>
  )
}

/** 变体 B 的解释一律走弹窗：点术语标签或「这一步在干什么」。 */
function TermChip({ termKey, onOpen, children }: { termKey: string; onOpen: (state: ModalState) => void; children?: ReactNode }) {
  const term = terms[termKey]
  if (term === undefined) return <>{children}</>
  return (
    <button
      type="button"
      className="term-chip"
      onClick={() => onOpen({ title: term.label, body: <p>{term.plain}</p> })}
    >
      {children ?? term.label}
    </button>
  )
}

function Modal({ state, onClose }: { state: ModalState | null; onClose: () => void }) {
  if (state === null) return null
  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h3>{state.title}</h3>
        {state.body}
        <button type="button" className="primary modal-close" onClick={onClose}>
          知道了
        </button>
      </div>
    </div>
  )
}

function StepsB({
  flow,
  onOpen,
  collapsed,
  onToggle,
}: {
  flow: Flow
  onOpen: (state: ModalState) => void
  collapsed: boolean
  onToggle: () => void
}) {
  return (
    <div className="steps">
      {steps.map((step) => (
        <button
          key={step.index}
          type="button"
          className="step-chip"
          aria-current={flow.step === step.index ? 'step' : undefined}
          data-done={flow.step > step.index}
          disabled={!flow.canEnter(step.index as 1 | 2 | 3 | 4)}
          onClick={() => flow.goStep(step.index as 1 | 2 | 3 | 4)}
        >
          <span className="num">{step.index}</span>
          <span>
            <span style={{ display: 'block' }}>{step.title}</span>
            <span style={{ display: 'block', fontSize: 11, opacity: 0.7 }}>{step.goal}</span>
          </span>
        </button>
      ))}
      <span className="toolbar-spacer" />
      <button
        type="button"
        className="tool"
        onClick={() =>
          onOpen({
            title: `第 ${flow.step} 步 · ${steps[flow.step - 1].title}`,
            body: (
              <>
                <p>{steps[flow.step - 1].oneLine}</p>
                <p>{steps[flow.step - 1].detail}</p>
              </>
            ),
          })
        }
      >
        这一步在干什么
      </button>
      <button type="button" className="collapse-btn" onClick={onToggle}>
        {collapsed ? '展开面板' : '收起面板'}
      </button>
    </div>
  )
}

export function FlowB({ flow }: { flow: Flow }) {
  const [modal, setModal] = useState<ModalState | null>(null)
  const [collapsed, setCollapsed] = useState(false)
  return (
    <div className="flow-wrap">
      <StepsB flow={flow} onOpen={setModal} collapsed={collapsed} onToggle={() => setCollapsed(!collapsed)} />
      <div className="flow" data-collapsed={collapsed}>
        <div className="chart-card">
          <ChartToolbar flow={flow} />
          <KLineChart
            code={flow.detail.row.code}
            bars={flow.bars}
            period={flow.period}
            showEma={flow.showEma}
            avwapStart={flow.avwapStart}
            boxes={flow.boxes}
            lines={flow.lines}
            tool={flow.tool}
            onToolDone={flow.resetTool}
            onBoxDrawn={(box) => flow.setBoxes([...flow.boxes, { ...box, label: box.kind === 'gap' ? '缺口' : '支撑区' }])}
            onBoxMoved={(id, low, high) => flow.setBoxes(flow.boxes.map((b) => (b.id === id ? { ...b, low, high } : b)))}
            onLineDrawn={(line) => flow.setLines([...flow.lines, line])}
            onLineMoved={(id, value) => flow.setLines(flow.lines.map((l) => (l.id === id ? { ...l, price: value } : l)))}
            onAvwapPicked={(index) => flow.setAvwapStart(index)}
          />
        </div>
        {collapsed ? null : (
          <div className="panel">
            <div className="panel-head">
              <h2>{steps[flow.step - 1].title}</h2>
              <span className="toolbar-spacer" />
              <span className="hint">
                {flow.detail.row.code} {flow.detail.row.name}
              </span>
            </div>
            <div className="panel-body">
              {flow.step === 1 ? <Step1B flow={flow} onOpen={setModal} /> : null}
              {flow.step === 2 ? <Step2B flow={flow} onOpen={setModal} /> : null}
              {flow.step === 3 ? <Step3B flow={flow} onOpen={setModal} /> : null}
              {flow.step === 4 ? <Step4B flow={flow} onOpen={setModal} /> : null}
            </div>
          </div>
        )}
      </div>
      <Modal state={modal} onClose={() => setModal(null)} />
    </div>
  )
}

function Step1B({ flow, onOpen }: { flow: Flow; onOpen: (s: ModalState) => void }) {
  const picked = [...stocks].filter((s) => s.row.c1 && s.row.c2 && s.row.c3 && !s.row.c4)
  const rest = [...stocks].filter((s) => !picked.includes(s))
  return (
    <>
      <p className="hint">
        这一步只做一件事：找出正在休息的强势股。四条判定全中才进观察名单 ——{' '}
        <TermChip termKey="pullback" onOpen={onOpen} />。
      </p>
      <div className="field-row">
        <div className="field">
          <label>近 6 个月涨幅阈值</label>
          <input defaultValue="30" />
        </div>
        <div className="field">
          <label>涨幅排名区间（前 %）</label>
          <input defaultValue="30" />
        </div>
        <div className="field">
          <label>日均成交量下限（万股）</label>
          <input defaultValue="3000" />
        </div>
        <div className="field">
          <label>市场</label>
          <select defaultValue="main">
            <option value="main">沪深主板</option>
          </select>
        </div>
      </div>

      <h3 className="block-title">入选（{picked.length} 只）</h3>
      <table className="grid">
        <thead>
          <tr>
            <th className="left">代码</th>
            <th className="left">名称</th>
            <th className="left">板块</th>
            <th>最新价</th>
            <th>近 1 月</th>
            <th>近 6 月</th>
            <th>排名</th>
          </tr>
        </thead>
        <tbody>
          {picked.map((item) => (
            <tr
              key={item.row.code}
              data-selected={flow.selected === item.row.code}
              onClick={() => flow.selectStock(item.row.code)}
              style={{ cursor: 'pointer' }}
            >
              <td className="left mono">{item.row.code}</td>
              <td className="left">{item.row.name}</td>
              <td className="left" style={{ color: '#64748b' }}>{item.row.sector}</td>
              <td className="mono">{price(item.row.price)}</td>
              <td className="mono">{pct(item.row.chg1m)}</td>
              <td className="mono">{pct(item.row.chg6m)}</td>
              <td className="mono">{pct(item.row.rankPct)}</td>
            </tr>
          ))}
        </tbody>
      </table>

      <h3 className="block-title">未入选（{rest.length} 只）</h3>
      <table className="grid">
        <thead>
          <tr>
            <th className="left">代码</th>
            <th className="left">名称</th>
            <th>近 6 月</th>
            <th>排名</th>
            <th className="left">未中的条件</th>
          </tr>
        </thead>
        <tbody>
          {rest.map((item) => {
            const r = item.row
            const missed = [
              r.c1 ? null : 'C1 涨幅',
              r.c2 ? null : 'C2 结构',
              r.c3 ? null : 'C3 均线',
              r.c4 ? 'C4 排除命中' : null,
            ].filter((v): v is string => v !== null)
            return (
              <tr key={r.code} onClick={() => flow.selectStock(r.code)} style={{ cursor: 'pointer' }}>
                <td className="left mono">{r.code}</td>
                <td className="left">{r.name}</td>
                <td className="mono">{pct(r.chg6m)}</td>
                <td className="mono">{pct(r.rankPct)}</td>
                <td className="left" style={{ color: '#b45309' }}>{missed.join(' · ')}</td>
              </tr>
            )
          })}
        </tbody>
      </table>
      <div className="panel-foot">
        <button
          type="button"
          className="primary"
          disabled={flow.watchlist.includes(flow.selected)}
          onClick={() => flow.setWatchlist([...flow.watchlist, flow.selected])}
        >
          加入观察名单
        </button>
        <span className="hint">已加入 {flow.watchlist.length} 只</span>
      </div>
    </>
  )
}

function Step2B({ flow, onOpen }: { flow: Flow; onOpen: (s: ModalState) => void }) {
  return (
    <>
      <p className="hint">
        在左边图上用 <TermChip termKey="support" onOpen={onOpen} /> 工具拖出矩形。
        <TermChip termKey="avwap" onOpen={onOpen} /> 的起点也可以改。拖动矩形会实时重算置信度。
      </p>
      <h3 className="block-title">已标注</h3>
      <ZoneList flow={flow} />
      <h3 className="block-title">三个参考价</h3>
      <RefTable flow={flow} />
      <AiCard text={aiExplain.whyPicked} />
      <div className="panel-foot">
        <button type="button" className="primary" disabled={!flow.hasHighConfidence} onClick={() => flow.goStep(3)}>
          完成标注，进入第 3 步
        </button>
        {flow.hasHighConfidence ? null : <span className="hint">至少需要一个高置信支撑区</span>}
      </div>
    </>
  )
}

function Step3B({ flow, onOpen }: { flow: Flow; onOpen: (s: ModalState) => void }) {
  return (
    <>
      <p className="hint">
        日线先站上高置信支撑区，分钟图再出现 <TermChip termKey="firstRetest" onOpen={onOpen} /> 才算触发。
      </p>
      <div className="result-grid">
        <dl className="result-cell">
          <dt>前置条件 · 站上高置信支撑区</dt>
          <dd>{flow.hasHighConfidence ? '满足' : '未满足'}</dd>
        </dl>
        <dl className="result-cell">
          <dt>触发条件 · 下探回升 + 突破前高</dt>
          <dd>{flow.signal === null ? '未出现' : '已出现'}</dd>
        </dl>
      </div>
      {flow.signal === null ? (
        <div className="notice" data-tone="info">
          当前状态：等待。价格直接跌穿支撑区且没有回升时不触发，App 不发出任何提示。
        </div>
      ) : (
        <div className="notice" data-tone="info">
          {flow.signal.triggeredAt} · {flow.signal.timeframe} · 突破前一根高点 {price(flow.signal.brokenHigh)} ·
          建议买入价 {price(flow.signal.price)}
        </div>
      )}
      <AiCard text={aiExplain.pattern} />
      <div className="panel-foot">
        <button type="button" className="ghost" disabled={flow.signal !== null} onClick={flow.triggerSignal}>
          模拟触发信号
        </button>
        <button type="button" className="primary" disabled={flow.signal === null} onClick={() => flow.goStep(4)}>
          进入第 4 步
        </button>
      </div>
    </>
  )
}

function Step4B({ flow, onOpen }: { flow: Flow; onOpen: (s: ModalState) => void }) {
  const r = flow.result
  return (
    <>
      <p className="hint">
        <TermChip termKey="r" onOpen={onOpen} />：买入价减止损价。股数按 A 股一手 100 股向下取整。
      </p>
      <div className="field-row">
        <div className="field">
          <label>买入价（元/股）</label>
          <input
            type="number"
            step="0.01"
            value={flow.sizing.entry}
            onChange={(e) => flow.setSizing({ ...flow.sizing, entry: Number(e.target.value) })}
          />
        </div>
        <div className="field">
          <label>止损价（元/股）</label>
          <input
            type="number"
            step="0.01"
            value={flow.sizing.stop}
            onChange={(e) => flow.setSizing({ ...flow.sizing, stop: Number(e.target.value) })}
          />
        </div>
        <div className="field">
          <label>账户金额（元）</label>
          <input
            type="number"
            value={flow.sizing.account}
            onChange={(e) => flow.setSizing({ ...flow.sizing, account: Number(e.target.value) })}
          />
        </div>
        <div className="field">
          <label>单笔风险比例（%）</label>
          <input
            type="number"
            step="0.1"
            value={flow.sizing.riskPct * 100}
            onChange={(e) => flow.setSizing({ ...flow.sizing, riskPct: Number(e.target.value) / 100 })}
          />
        </div>
      </div>
      <div className="result-grid">
        <dl className="result-cell">
          <dt>可买股数</dt>
          <dd className="mono">{r.shares}</dd>
        </dl>
        <dl className="result-cell">
          <dt>投入金额</dt>
          <dd className="mono">{money(r.capital)}</dd>
        </dl>
        <dl className="result-cell">
          <dt>仓位占比</dt>
          <dd className="mono">{pct(r.posPct)}</dd>
        </dl>
        <dl className="result-cell">
          <dt>止损距离</dt>
          <dd className="mono">{pct(r.stopDistance)}</dd>
        </dl>
        <dl className="result-cell">
          <dt>3R 目标位</dt>
          <dd className="mono">{price(r.t3)}</dd>
        </dl>
        <dl className="result-cell">
          <dt>5R 目标位</dt>
          <dd className="mono">{price(r.t5)}</dd>
        </dl>
      </div>
      <div className="formula">
        每股风险 = {price(flow.sizing.entry)} − {price(flow.sizing.stop)} = <b>{price(r.perR)}</b>
        <br />
        最大可亏 = {money(flow.sizing.account)} × {pct(flow.sizing.riskPct)} = <b>{money(r.maxLoss)}</b>
        <br />
        可买股数 = {money(r.maxLoss)} ÷ {price(r.perR)} = <b>{r.shares} 股</b>
        <br />
        投入金额 = {r.shares} × {price(flow.sizing.entry)} = <b>{money(r.capital)}</b>
      </div>
      {flow.sizing.entry <= flow.sizing.stop ? (
        <div className="notice" data-tone="risk">买入价必须高于止损价，请检查输入。</div>
      ) : null}
      {r.stopDistance > 0.03 ? (
        <div className="notice" data-tone="warn">
          止损过远，超出该策略建议的 2.5%~3%，请重新选择止损位或放弃该标的。
        </div>
      ) : null}
      {r.shares === 0 ? <div className="notice" data-tone="warn">每股风险过小，请检查止损位或降低风险比例。</div> : null}
      <div className="notice" data-tone="risk">
        {riskNotes.tPlus1} 仓位计算结果为理论值，{riskNotes.slippage}
      </div>
      <div className="panel-foot">
        <NavLink className="primary" to="/plan">
          生成计划卡
        </NavLink>
        <span className="hint">未用满的风险金额 {money(r.unusedRisk)}</span>
      </div>
    </>
  )
}

export function PlanB({ flow }: { flow: Flow }) {
  const r = flow.result
  const groups: Array<{ title: string; fields: Array<[string, string]> }> = [
    {
      title: '标的',
      fields: [
        ['代码与名称', `${flow.detail.row.code} ${flow.detail.row.name}`],
        ['所属板块', `${flow.detail.row.market} · ${flow.detail.row.sector}`],
        [
          '日线筛选结论',
          `C1 ${flow.detail.row.c1 ? '中' : '未中'} · C2 ${flow.detail.row.c2 ? '中' : '未中'} · C3 ${flow.detail.row.c3 ? '中' : '未中'}`,
        ],
      ],
    },
    {
      title: '支撑区',
      fields: [
        ['支撑区价格', flow.boxViews.length > 0 ? `${price(flow.boxViews[0].low)} ~ ${price(flow.boxViews[0].high)}` : '未标注'],
        ['AVWAP / EMA21 / 前期高点', `${price(flow.refs.avwap)} / ${price(flow.refs.ema21)} / ${price(flow.refs.prevHigh)}`],
      ],
    },
    {
      title: '下单',
      fields: [
        ['买入价', price(flow.sizing.entry)],
        ['止损价', price(flow.sizing.stop)],
        ['3R / 5R 目标位', `${price(r.t3)} / ${price(r.t5)}`],
        ['仓位股数', `${r.shares} 股`],
        ['投入金额', money(r.capital)],
        ['止损距离', pct(r.stopDistance)],
        ['计划风险（占账户）', pct(flow.sizing.riskPct)],
      ],
    },
  ]
  return (
    <div className="page">
      <div className="chart-card" style={{ maxWidth: 760 }}>
        <div className="panel-head">
          <h2>交易计划卡</h2>
        </div>
        <div className="panel-body">
          {groups.map((group) => (
            <div key={group.title}>
              <h3 className="block-title">{group.title}</h3>
              <table className="grid">
                <tbody>
                  {group.fields.map(([label, value]) => (
                    <tr key={label}>
                      <td className="left" style={{ color: '#64748b', width: 220 }}>{label}</td>
                      <td className="left mono">{value}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ))}
          <div className="notice" data-tone="info">{riskNotes.costFree} {riskNotes.slippage}</div>
          <div className="panel-foot" style={{ borderTop: 'none', padding: 0 }}>
            <button type="button" className="primary">复制为文本</button>
            <button type="button" className="ghost">导出 PNG</button>
            <button type="button" className="ghost">存为草稿</button>
          </div>
        </div>
      </div>
    </div>
  )
}

export function PositionsB() {
  return (
    <div className="page">
      <div className="chart-card">
        <div className="panel-head">
          <h2>持仓跟踪</h2>
          <span className="toolbar-spacer" />
          <span className="hint">每 60 秒刷新一次 · 最近刷新 14:32:10</span>
        </div>
        <div className="panel-body">
          {positions.map((p) => (
            <div key={p.code} className="result-cell" style={{ display: 'flex', gap: 16, alignItems: 'center' }}>
              <div style={{ minWidth: 160 }}>
                <div style={{ fontWeight: 600 }}>
                  {p.name} <span className="mono" style={{ color: '#64748b' }}>{p.code}</span>
                </div>
                <div className="hint">入场 {p.entryDate} · {p.quantity} 股</div>
              </div>
              <div className="hint">
                买入 {price(p.entry)} ｜ 止损 {price(p.stop)} ｜ 最新 {price(p.last)} ｜ 9 日 EMA {price(p.ema9)}
              </div>
              <span className="toolbar-spacer" />
              {p.status === '持有' ? <span className="pill" data-level="低">持有</span> : null}
              {p.status === '已到 3R' ? <span className="pill" data-level="高">到达 3R，减仓三分之一</span> : null}
              {p.status === '跌破 9 日 EMA' ? <span className="pill" data-level="中">跌破 9 日 EMA，评估离场</span> : null}
            </div>
          ))}
          <div className="notice" data-tone="risk">{riskNotes.tPlus1}</div>
        </div>
      </div>
    </div>
  )
}

export function JournalB() {
  return (
    <div className="page">
      <div className="chart-card">
        <div className="panel-head">
          <h2>交易日志</h2>
          <span className="toolbar-spacer" />
          <button type="button" className="ghost">导出 CSV</button>
          <button type="button" className="ghost">手动补录</button>
        </div>
        <div className="panel-body">
          <div className="result-grid" style={{ gridTemplateColumns: 'repeat(4, minmax(0,1fr))' }}>
            <dl className="result-cell">
              <dt>交易笔数</dt>
              <dd className="mono">{journalStats.trades}</dd>
            </dl>
            <dl className="result-cell">
              <dt>胜率</dt>
              <dd className="mono">{pct(journalStats.winRate)}</dd>
            </dl>
            <dl className="result-cell">
              <dt>平均 R 倍数</dt>
              <dd className="mono">{journalStats.avgR.toFixed(2)}</dd>
            </dl>
            <dl className="result-cell">
              <dt>总盈亏</dt>
              <dd className="mono">{money(journalStats.pnl)}</dd>
            </dl>
          </div>
          <table className="grid">
            <thead>
              <tr>
                <th className="left">代码</th>
                <th className="left">名称</th>
                <th>入场日</th>
                <th>出场日</th>
                <th>R 倍数</th>
                <th>盈亏</th>
                <th className="left">备注</th>
              </tr>
            </thead>
            <tbody>
              {logs.map((l) => (
                <tr key={`${l.code}-${l.entryDate}`}>
                  <td className="left mono">{l.code}</td>
                  <td className="left">{l.name}</td>
                  <td className="mono">{l.entryDate}</td>
                  <td className="mono">{l.exitDate}</td>
                  <td className="mono">{l.rMultiple.toFixed(1)}</td>
                  <td className="mono">{money(l.pnl)}</td>
                  <td className="left" style={{ color: '#64748b' }}>{l.note}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <div className="notice" data-tone="info">{riskNotes.costFree}</div>
          <AiCard text={aiExplain.review} />
        </div>
      </div>
    </div>
  )
}

export function SettingsB() {
  return (
    <div className="page">
      <div className="chart-card" style={{ maxWidth: 680 }}>
        <div className="panel-head">
          <h2>设置</h2>
        </div>
        <div className="panel-body">
          <div className="field-row">
            <div className="field">
              <label>小白模式（术语弹窗解释）</label>
              <select defaultValue="on">
                <option value="on">开启</option>
                <option value="off">关闭</option>
              </select>
            </div>
            <div className="field">
              <label>单笔风险比例上限（%）</label>
              <input defaultValue="1" />
            </div>
            <div className="field">
              <label>止损距离阈值（%）</label>
              <input defaultValue="3" />
            </div>
            <div className="field">
              <label>账户金额（元）</label>
              <input defaultValue={String(account.equity)} />
            </div>
          </div>
          <div className="field">
            <label>数据源</label>
            <input defaultValue={account.dataSource} readOnly />
          </div>
          <div className="field">
            <label>K 线数据导入（CSV）</label>
            <input type="file" />
          </div>
          <div className="notice" data-tone="risk">
            {riskNotes.highReturn}
            <br />
            {riskNotes.market}
            <br />
            {riskNotes.tPlus1}
          </div>
        </div>
      </div>
    </div>
  )
}

export const variantB: VariantModule = {
  Shell: ShellB,
  Flow: FlowB,
  Plan: PlanB,
  Positions: PositionsB,
  Journal: JournalB,
  Settings: SettingsB,
}

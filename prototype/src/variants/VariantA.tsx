import { useState, type ReactNode } from 'react'
import { NavLink } from 'react-router-dom'
import { KLineChart } from '../chart/KLineChart'
import type { Flow } from '../flow/useFlow'
import { aiExplain, riskNotes, steps, terms } from '../data/glossary'
import { account, journalStats, logs, positions } from '../data/fixtures'
import { stocks } from '../data/stocks'
import { money, pct, price } from '../format'
import { AiCard, ChartToolbar, Flag, RefTable, TermHint, ZoneList } from './common'
import type { VariantModule } from './types'

const routes = [
  { to: '/', label: '主流程' },
  { to: '/plan', label: '计划卡' },
  { to: '/positions', label: '持仓跟踪' },
  { to: '/journal', label: '交易日志' },
  { to: '/settings', label: '设置' },
]

export function ShellA({ flow, children }: { flow: Flow; children: ReactNode }) {
  return (
    <div className="app" data-variant="A">
      <header className="topbar">
        <div className="brand">
          蓄势 Springload<small>回调买入法工作台</small>
        </div>
        <div className="topbar-right">
          <span className="mono">账户 {money(account.equity)}</span>
          <span>数据源：东方财富 · 前复权</span>
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
      <div style={{ display: 'none' }}>{flow.step}</div>
    </div>
  )
}

function StepsA({ flow }: { flow: Flow }) {
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
          <TermHint termKey={step.termKeys[0]}>{step.title}</TermHint>
        </button>
      ))}
    </div>
  )
}

export function FlowA({ flow }: { flow: Flow }) {
  return (
    <div className="flow-wrap">
      <StepsA flow={flow} />
      <div className="flow">
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
        <div className="panel">
          <div className="panel-head">
            <h2>
              {flow.step}. {steps[flow.step - 1].title}
            </h2>
            <TermHint termKey={steps[flow.step - 1].termKeys[0]}>?</TermHint>
            <span className="toolbar-spacer" />
            <span className="hint">{flow.detail.row.code} {flow.detail.row.name}</span>
          </div>
          <div className="panel-body">
            {flow.step === 1 ? <Step1A flow={flow} /> : null}
            {flow.step === 2 ? <Step2A flow={flow} /> : null}
            {flow.step === 3 ? <Step3A flow={flow} /> : null}
            {flow.step === 4 ? <Step4A flow={flow} /> : null}
          </div>
        </div>
      </div>
    </div>
  )
}

function Step1A({ flow }: { flow: Flow }) {
  const [sort, setSort] = useState<'chg6m' | 'rankPct'>('chg6m')
  const rows = [...stocks].sort((a, b) => (sort === 'chg6m' ? b.row.chg6m - a.row.chg6m : a.row.rankPct - b.row.rankPct))
  return (
    <>
      <p className="hint">
        <TermHint termKey="pullback" />：涨过一波之后横盘休息的股票。四条判定同时满足才进观察名单。
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
      <table className="grid">
        <thead>
          <tr>
            <th className="left">代码</th>
            <th className="left">名称</th>
            <th className="left">板块</th>
            <th>最新价</th>
            <th onClick={() => setSort('chg6m')}>近 6 月</th>
            <th onClick={() => setSort('rankPct')}>排名</th>
            <th>C1</th>
            <th>C2</th>
            <th>C3</th>
            <th>C4</th>
            <th className="left">结论</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((item) => {
            const r = item.row
            const picked = r.c1 && r.c2 && r.c3 && !r.c4
            return (
              <tr
                key={r.code}
                data-selected={flow.watchlist.includes(r.code)}
                onClick={() => flow.selectStock(r.code)}
                style={{ cursor: 'pointer' }}
              >
                <td className="left mono">{r.code}</td>
                <td className="left">{r.name}</td>
                <td className="left" style={{ color: '#64748b' }}>{r.sector}</td>
                <td className="mono">{price(r.price)}</td>
                <td className="mono">{pct(r.chg6m)}</td>
                <td className="mono">{pct(r.rankPct)}</td>
                <td><Flag ok={r.c1} /></td>
                <td><Flag ok={r.c2} /></td>
                <td><Flag ok={r.c3} /></td>
                <td><Flag ok={r.c4} /></td>
                <td className="left">
                  {picked ? <span className="pill" data-level="高">入选</span> : <span className="pill" data-level="低">不入选</span>}
                </td>
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

function Step2A({ flow }: { flow: Flow }) {
  return (
    <>
      <p className="hint">
        在图上用 <TermHint termKey="support">支撑区</TermHint> 工具拖出矩形，
        <TermHint termKey="avwap" /> 的起点也可以改。拖动矩形或水平线会实时重算置信度。
      </p>
      <ZoneList flow={flow} />
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

function Step3A({ flow }: { flow: Flow }) {
  return (
    <>
      <p className="hint">
        日线先站上高置信支撑区，分钟图再出现 <TermHint termKey="firstRetest">第一次回测</TermHint> 才算触发。
      </p>
      <table className="grid">
        <tbody>
          <tr>
            <td className="left">前置条件：价格站上高置信支撑区</td>
            <td><Flag ok={flow.hasHighConfidence} /></td>
          </tr>
          <tr>
            <td className="left">触发条件：下探回升 + 突破前一根高点</td>
            <td><Flag ok={flow.signal !== null} /></td>
          </tr>
          <tr>
            <td className="left">当前状态</td>
            <td className="left">
              <span className="pill" data-level={flow.signal === null ? '中' : '高'}>
                {flow.signal === null ? '等待' : '已触发'}
              </span>
            </td>
          </tr>
        </tbody>
      </table>
      {flow.signal === null ? (
        <p className="hint">价格直接跌穿支撑区且没有回升时不触发，App 不发出任何提示。</p>
      ) : (
        <div className="notice" data-tone="info">
          {flow.signal.triggeredAt} · {flow.signal.timeframe} · 突破前一根高点 {price(flow.signal.brokenHigh)} ·
          建议买入价 {price(flow.signal.price)}
        </div>
      )}
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

function Step4A({ flow }: { flow: Flow }) {
  const r = flow.result
  const tooFar = r.stopDistance > 0.03
  return (
    <>
      <p className="hint">
        <TermHint termKey="r" />：买入价减止损价，这一单每股最多亏多少。股数按 A 股一手 100 股向下取整。
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
      <div className="formula">
        每股风险 = {price(flow.sizing.entry)} − {price(flow.sizing.stop)} = <b>{price(r.perR)}</b>
        <br />
        最大可亏 = {money(flow.sizing.account)} × {pct(flow.sizing.riskPct)} = <b>{money(r.maxLoss)}</b>
        <br />
        可买股数 = {money(r.maxLoss)} ÷ {price(r.perR)} = <b>{r.shares} 股</b>（一手 100 股取整）
        <br />
        投入金额 = {r.shares} × {price(flow.sizing.entry)} = <b>{money(r.capital)}</b>
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
      {flow.sizing.entry <= flow.sizing.stop ? (
        <div className="notice" data-tone="risk">买入价必须高于止损价，请检查输入。</div>
      ) : null}
      {tooFar ? (
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

export function PlanA({ flow }: { flow: Flow }) {
  const r = flow.result
  const fields: Array<[string, string]> = [
    ['标的代码', `${flow.detail.row.code} ${flow.detail.row.name}`],
    ['所属板块', `${flow.detail.row.market} · ${flow.detail.row.sector}`],
    ['日线筛选结论', `C1 ${flow.detail.row.c1 ? '中' : '未中'} · C2 ${flow.detail.row.c2 ? '中' : '未中'} · C3 ${flow.detail.row.c3 ? '中' : '未中'}`],
    ['支撑区价格', flow.boxViews.length > 0 ? `${price(flow.boxViews[0].low)} ~ ${price(flow.boxViews[0].high)}` : '未标注'],
    ['AVWAP / EMA21 / 前期高点', `${price(flow.refs.avwap)} / ${price(flow.refs.ema21)} / ${price(flow.refs.prevHigh)}`],
    ['买入价', price(flow.sizing.entry)],
    ['止损价', price(flow.sizing.stop)],
    ['3R / 5R 目标位', `${price(r.t3)} / ${price(r.t5)}`],
    ['仓位股数', `${r.shares} 股`],
    ['投入金额', money(r.capital)],
    ['止损距离', pct(r.stopDistance)],
    ['计划风险（占账户）', pct(flow.sizing.riskPct)],
  ]
  return (
    <div className="page">
      <div className="chart-card" style={{ maxWidth: 720 }}>
        <div className="panel-head">
          <h2>交易计划卡</h2>
          <span className="toolbar-spacer" />
          <TermHint termKey="target">3R / 5R</TermHint>
        </div>
        <div className="panel-body">
          <table className="grid">
            <tbody>
              {fields.map(([label, value]) => (
                <tr key={label}>
                  <td className="left" style={{ color: '#64748b' }}>{label}</td>
                  <td className="left mono">{value}</td>
                </tr>
              ))}
            </tbody>
          </table>
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

export function PositionsA() {
  return (
    <div className="page">
      <div className="chart-card">
        <div className="panel-head">
          <h2>持仓跟踪</h2>
          <span className="toolbar-spacer" />
          <span className="hint">每 60 秒刷新一次 · 最近刷新 14:32:10</span>
        </div>
        <div className="panel-body">
          <table className="grid">
            <thead>
              <tr>
                <th className="left">代码</th>
                <th className="left">名称</th>
                <th>入场日</th>
                <th>买入价</th>
                <th>止损价</th>
                <th>股数</th>
                <th>最新价</th>
                <th>9 日 EMA</th>
                <th>3R</th>
                <th className="left">提示</th>
              </tr>
            </thead>
            <tbody>
              {positions.map((p) => (
                <tr key={p.code}>
                  <td className="left mono">{p.code}</td>
                  <td className="left">{p.name}</td>
                  <td className="mono">{p.entryDate}</td>
                  <td className="mono">{price(p.entry)}</td>
                  <td className="mono">{price(p.stop)}</td>
                  <td className="mono">{p.quantity}</td>
                  <td className="mono">{price(p.last)}</td>
                  <td className="mono">{price(p.ema9)}</td>
                  <td className="mono">{price(p.t3)}</td>
                  <td className="left">
                    {p.status === '持有' ? <span className="pill" data-level="低">持有</span> : null}
                    {p.status === '已到 3R' ? <span className="pill" data-level="高">到达 3R，减仓三分之一</span> : null}
                    {p.status === '跌破 9 日 EMA' ? (
                      <span className="pill" data-level="中">跌破 9 日 EMA，评估离场</span>
                    ) : null}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <div className="notice" data-tone="risk">{riskNotes.tPlus1}</div>
        </div>
      </div>
    </div>
  )
}

export function JournalA() {
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
                <th>买入价</th>
                <th>出场价</th>
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
                  <td className="mono">{price(l.entry)}</td>
                  <td className="mono">{price(l.exit)}</td>
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

export function SettingsA() {
  return (
    <div className="page">
      <div className="chart-card" style={{ maxWidth: 640 }}>
        <div className="panel-head">
          <h2>设置</h2>
        </div>
        <div className="panel-body">
          <div className="field-row">
            <div className="field">
              <label><TermHint termKey="pullback">小白模式</TermHint>（术语悬停解释）</label>
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
          <table className="grid">
            <tbody>
              {Object.values(terms).map((term) => (
                <tr key={term.key}>
                  <td className="left" style={{ width: 120 }}>{term.label}</td>
                  <td className="left" style={{ color: '#64748b', whiteSpace: 'normal' }}>{term.plain}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  )
}

export const variantA: VariantModule = {
  Shell: ShellA,
  Flow: FlowA,
  Plan: PlanA,
  Positions: PositionsA,
  Journal: JournalA,
  Settings: SettingsA,
}

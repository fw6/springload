// 术语白话解释取自需求规格说明书第 2 章，逐字沿用；AI 解释段是占位示例。

export interface Term {
  key: string
  label: string
  plain: string
}

export const terms: Record<string, Term> = {
  pullback: {
    key: 'pullback',
    label: '回调买入法',
    plain: '股票涨过一波后休息（横盘），等它再次走强时上车，而不是追着已经涨高的价格买。',
  },
  ema: {
    key: 'ema',
    label: 'EMA 9/21/50',
    plain: '最近 9/21/50 天股价的平均成本线。9 日看短线、21 日看中短线、50 日看中期。',
  },
  support: {
    key: 'support',
    label: '支撑区',
    plain: '股价跌到这个价格附近就容易跌不动的区域，通常叠在均线、前期高点、AVWAP 附近。',
  },
  gap: {
    key: 'gap',
    label: '跳空缺口',
    plain: '昨天收盘价与今天开盘价之间那段没有成交的价格区间，常成为支撑/压力。',
  },
  avwap: {
    key: 'avwap',
    label: 'AVWAP',
    plain: '从某个指定起点开始，按成交量加权算出的平均成本价，约等于那段时间参与者的平均成本。价格回到这里附近常获得支撑。',
  },
  firstRetest: {
    key: 'firstRetest',
    label: '第一次回测',
    plain: '股价刚站上某个关键位，回头又踩它一脚。踩住 = 支撑有效，是这套策略最看重的触发场景。',
  },
  r: {
    key: 'r',
    label: 'R（风险单位）',
    plain: '这一单预设的最大每股亏损额（买入价 − 止损价）。全程用它衡量盈亏，不用百分比。',
  },
  target: {
    key: 'target',
    label: '3R / 5R 目标位',
    plain: '盈利达到 3 倍或 5 倍初始风险的价位。3R 卖出一部分，5R 再卖一部分。',
  },
  slippage: {
    key: 'slippage',
    label: '滑点',
    plain: '按下买入那一刻，实际成交价比看到的价格差了一点。会导致实际亏损大于预设值。',
  },
  trialCost: {
    key: 'trialCost',
    label: '试错成本',
    plain: '这单如果错了，会亏掉的钱。策略的核心是用极小的试错成本换大段上涨。',
  },
}

export interface StepCopy {
  index: number
  title: string
  goal: string
  /** 一句话说明这一步在干什么 */
  oneLine: string
  /** 展开后的完整说明 */
  detail: string
  /** 本步的关键术语 */
  termKeys: string[]
  /** 前序未完成时的阻止提示 */
  blockedHint: string
}

export const steps: StepCopy[] = [
  {
    index: 1,
    title: '强势股筛选',
    goal: '先找出近期走得强、现在正在休息的股票',
    oneLine: '从沪深主板里筛出「涨过一波、现在横着」的股票，得到一张观察名单。',
    detail:
      '筛选器只看四件事：近 6 个月有没有涨够、近 1 个月的涨幅排不排得进前列、回调的低点是不是一次比一次高、收盘价有没有站在 9 日均线上。四条同时满足才会进观察名单。这一步不判断这只股票值不值得买，只判断它是不是符合回调蓄势的形态。',
    termKeys: ['pullback', 'ema'],
    blockedHint: '',
  },
  {
    index: 2,
    title: '标注支撑区',
    goal: '在图上画出跌不动的那条线，看它有多可靠',
    oneLine: '在 K 线图上拖出水平线和支撑区，App 会算出这块区域的置信度。',
    detail:
      '支撑区就是股价跌到这个位置附近就容易跌不动的地方。图上可以放水平线（前期高点或低点）、框出支撑区、标出跳空缺口，也可以指定 AVWAP 的起点。当 AVWAP、21 日均线、前期高点这三个价格落在 2% 以内时，这块支撑区会被标成高置信。',
    termKeys: ['support', 'ema', 'avwap', 'gap'],
    blockedHint: '先在第 1 步把股票加进观察名单，再来标注支撑区。',
  },
  {
    index: 3,
    title: '等待买入信号',
    goal: '盯着它回到支撑区之后有没有重新走强',
    oneLine: '价格站上高置信支撑区后，分钟图上出现「下探回升 + 突破前一根高点」才算触发。',
    detail:
      '这一步分两级。日线上，价格必须先站上高置信支撑区；分钟图上，要先出现一次下探后回升，随后某一根 K 线的最高价超过前一根 K 线的最高价。两个条件缺一不可。如果价格直接跌穿支撑区没有回升，App 保持等待，不发出任何提示。',
    termKeys: ['firstRetest', 'support'],
    blockedHint: '先在第 2 步完成支撑区标注，并让支撑区达到高置信。',
  },
  {
    index: 4,
    title: '算仓位与目标位',
    goal: '把「最多亏多少」换算成「买多少股」',
    oneLine: '输入买入价、止损价、账户金额和单笔风险比例，算出股数、投入金额与 3R/5R 目标位。',
    detail:
      '每股风险 = 买入价 − 止损价。本单最大可亏金额 = 账户金额 × 单笔风险比例。可买股数 = 最大可亏金额 ÷ 每股风险，向下取整。3R 与 5R 目标位分别是买入价加上 3 倍和 5 倍的每股风险。A 股按手交易，股数需要向下取整到 100 股的整数倍。',
    termKeys: ['r', 'target', 'trialCost', 'slippage'],
    blockedHint: '先在第 3 步等到买入信号，再来算仓位。',
  },
]

/** AI 生成的上下文解释示例。措辞只描述形态与位置，不给买卖结论。 */
export const aiExplain = {
  whyPicked:
    '这只股票近 6 个月上涨 63.8%，在观察名单里排第 2 位。最近 20 个交易日的最高价与最低价之差只有 6.9%，区间在收窄。本次回调的低点 38.10 元高于上一次的 36.40 元，低点在抬高。收盘价 38.62 元站在 9 日均线 38.05 元之上。',
  pattern:
    '图上能看到两段回踩：第一次回踩到 36.40 元后反弹，第二次回踩到 38.10 元止住。当前价格贴着 21 日均线与 AVWAP 运行，两者相差 0.8%。',
  review: '本笔按计划在 21.50 元触及 3R 减仓三分之一，剩余仓位跟随 9 日均线。',
  disclaimer: '以上只是对图上形态与数据的描述，不构成买卖建议。',
}

export const riskNotes = {
  highReturn: '原文所述 969.8% 为 2025 年单场赛事的 12 个月战绩，不是年化收益率，不代表可持续收益。',
  market: '策略收益依赖市场环境，在趋势走坏年份会连续触发止损。',
  tPlus1: 'A 股存在涨跌停与 T+1 制度，当日买入无法当日卖出，止损单可能无法成交导致实际亏损远超预设。',
  slippage: '跳空或滑点会使实际成交价劣于预期，亏损可能超过预设的账户风险。',
  costFree: '统计未计入交易成本与滑点。',
}

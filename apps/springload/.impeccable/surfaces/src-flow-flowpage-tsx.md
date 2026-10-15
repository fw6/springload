---
version: 1
slug: "src-flow-flowpage-tsx"
primary_target: "src/flow/FlowPage.tsx"
related_targets: ["src/App.tsx","src/routes/PlanPage.tsx","src/routes/PositionsPage.tsx","src/routes/JournalPage.tsx","src/routes/SettingsPage.tsx"]
---

# 主流程版面（报刊行情版）

## 范围与模式

- 目标：`src/flow/FlowPage.tsx` —— 主流程四步的版面。
- 关联：`src/App.tsx`（报头带与版次条）、`src/routes/PlanPage.tsx`、`src/routes/PositionsPage.tsx`、`src/routes/JournalPage.tsx`、`src/routes/SettingsPage.tsx`。
- 访问者模式：Operate。盘中盯盘与收盘复盘都要经得起长时间注视。

## 受众与任务

- 受众：一个用「回调买入法」的个人投资者。看得懂 K 线与均线，没受过金融工程训练。
- 任务：按顺序走完四步（筛选强势股 → 标注支撑区 → 等买入信号 → 算仓位与目标位）。每一步都要看到白话说明与可对账的数字。
- 关键状态：还没跑筛选、还没有标注、没有高置信支撑区、还没触发、股数为 0、止损距离超过阈值、数据源同步失败。
- 约束：A 股规则（一手 100 股向下取整、T+1、涨跌停）；红涨绿跌；统计口径固定标注「未计入交易成本与滑点」；AI 只做解释、不给买卖建议，卡片固定带免责声明。

## 选定方向

- 报刊行情版：铅字排版的证券报版面。用户从掷签结果中选定了它。
- 记得住的时刻：第 3 步触发时，图版上落下一张「号外」条。
- 用户明确选择「全部重做」，因此旧版的纸白底、单一深蓝强调色与圆角卡片一律不作保留；红涨绿跌属于 A 股规则，继续保留。

## 未决

- 无。

## Direction contract

THESIS：这一页是一份每天出的行情纸，头条就是今天该做的那一步。它拒绝品类默认的「侧边栏 + 圆角卡片 + 大号指标块」。

OWN-WORLD：新闻纸灰 `#e4e3de` 铺满整页，油墨黑 `#16161a` 做正文与栏线，套红 `#b8232a` 只给当前步骤、主操作与「涨」。没有圆角、没有投影，版面靠 1px 栏线与加框稿件分区。报头与标题用宋体，正文用系统黑体，数字与代码用等宽。

STORY：打开就知道今天该做什么；每个数字都沿同一条栏线排齐，可以一路对下去；走完四步，签出一张计划卡。

FIRST VIEWPORT：报头横贯整宽，左边是「蓄势」两个宋体大字加副题，右边是期号、日期、版本、数据源与最近同步时间，排成等宽小字两行；报头下压一条粗加细的双线；再下一条版次条，五个版次里当前那一版套红；版面主体左栏 38% 是加框的行情图版，右栏 62% 是当前稿件的加框正文，当前步骤整栏套红加框，主操作落在稿件栏底部。

FORM：铅字排版的证券报版面。它在我自排的七条候选里排第 7 位（末位）。种子键 `db24ced2`。

FINISH：unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance.

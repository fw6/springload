---
name: 蓄势 Springload
description: A 股回调买入法工作台：铅字排版的证券报版面，新闻纸灰铺底，套红只给当前版次与主操作。
colors:
  paper: "#e4e3de"
  slip: "#efeee9"
  ink: "#16161a"
  ink-soft: "#56544c"
  rule: "#a09c8c"
  rule-soft: "#c4c1b4"
  rule-strong: "#84806f"
  proof: "#b8232a"
  up: "#b8232a"
  down: "#1c6b4a"
  warn: "#7d4e0c"
  warn-soft: "#e9dcc3"
  alert: "#b8232a"
  alert-soft: "#ecd7d4"
  ok: "#1c6b4a"
typography:
  masthead:
    fontFamily: '"Songti SC", "STSong", "SimSun", "Source Han Serif SC", "Noto Serif CJK SC", "Noto Serif SC", "AR PL UMing CN", "Times New Roman", serif'
    fontSize: "30px"
    fontWeight: 700
    lineHeight: 1.1
    letterSpacing: "-0.025em"
  title:
    fontFamily: '"Songti SC", "STSong", "SimSun", "Source Han Serif SC", "Noto Serif CJK SC", "Noto Serif SC", "AR PL UMing CN", "Times New Roman", serif'
    fontSize: "20px"
    fontWeight: 700
    lineHeight: 1.3
    letterSpacing: "normal"
  head:
    fontFamily: '"Songti SC", "STSong", "SimSun", "Source Han Serif SC", "Noto Serif CJK SC", "Noto Serif SC", "AR PL UMing CN", "Times New Roman", serif'
    fontSize: "16px"
    fontWeight: 700
    lineHeight: 1.35
    letterSpacing: "normal"
  lede:
    fontFamily: '-apple-system, BlinkMacSystemFont, "PingFang SC", "Helvetica Neue", "Microsoft YaHei", sans-serif'
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.7
    letterSpacing: "normal"
  body:
    fontFamily: '-apple-system, BlinkMacSystemFont, "PingFang SC", "Helvetica Neue", "Microsoft YaHei", sans-serif'
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.6
    letterSpacing: "normal"
  note:
    fontFamily: '-apple-system, BlinkMacSystemFont, "PingFang SC", "Helvetica Neue", "Microsoft YaHei", sans-serif'
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: "normal"
  micro:
    fontFamily: '-apple-system, BlinkMacSystemFont, "PingFang SC", "Helvetica Neue", "Microsoft YaHei", sans-serif'
    fontSize: "11px"
    fontWeight: 400
    lineHeight: 1.45
    letterSpacing: "normal"
  mono:
    fontFamily: '"SF Mono", ui-monospace, Menlo, monospace'
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.6
    letterSpacing: "normal"
spacing:
  page: "16px"
  block: "12px"
  panel: "14px"
  band: "10px"
  row: "6px"
  cell: "12px"
  hairline: "1px"
components:
  button-primary:
    backgroundColor: "{colors.proof}"
    textColor: "{colors.paper}"
    typography: "{typography.body}"
    padding: "0 14px"
    height: "34px"
  button-primary-sm:
    backgroundColor: "{colors.proof}"
    textColor: "{colors.paper}"
    typography: "{typography.note}"
    padding: "0 10px"
    height: "28px"
  button-primary-lg:
    backgroundColor: "{colors.proof}"
    textColor: "{colors.paper}"
    typography: "{typography.lede}"
    padding: "0 20px"
    height: "40px"
  button-secondary:
    backgroundColor: "{colors.slip}"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    padding: "0 14px"
    height: "34px"
  button-outline:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    padding: "0 14px"
    height: "34px"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.ink-soft}"
    typography: "{typography.body}"
    padding: "0 14px"
    height: "34px"
  input:
    backgroundColor: "{colors.slip}"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    padding: "0 10px"
    height: "34px"
    width: "100%"
  stamp-neutral:
    backgroundColor: "{colors.slip}"
    textColor: "{colors.ink-soft}"
    typography: "{typography.micro}"
    padding: "0 6px"
    height: "20px"
  stamp-info:
    backgroundColor: "{colors.slip}"
    textColor: "{colors.ink}"
    typography: "{typography.micro}"
    padding: "0 6px"
    height: "20px"
  stamp-success:
    backgroundColor: "{colors.slip}"
    textColor: "{colors.ok}"
    typography: "{typography.micro}"
    padding: "0 6px"
    height: "20px"
  stamp-warning:
    backgroundColor: "{colors.warn-soft}"
    textColor: "{colors.warn}"
    typography: "{typography.micro}"
    padding: "0 6px"
    height: "20px"
  stamp-danger:
    backgroundColor: "{colors.alert}"
    textColor: "{colors.paper}"
    typography: "{typography.micro}"
    padding: "0 6px"
    height: "20px"
  panel:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink}"
    padding: "14px"
  panel-header:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink}"
    typography: "{typography.head}"
    padding: "10px 14px"
  nav-item-active:
    backgroundColor: "{colors.proof}"
    textColor: "{colors.paper}"
    typography: "{typography.body}"
    padding: "6px 14px"
  nav-item:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink-soft}"
    typography: "{typography.body}"
    padding: "6px 14px"
  step-chip-active:
    backgroundColor: "{colors.slip}"
    textColor: "{colors.proof}"
    padding: "8px 12px"
  step-chip:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink}"
    padding: "8px 12px"
  table-header:
    backgroundColor: "{colors.slip}"
    textColor: "{colors.ink-soft}"
    typography: "{typography.note}"
    padding: "6px 12px"
  table-cell:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    padding: "6px 12px"
---

# Design System: 蓄势 Springload

## Overview

**Creative North Star: 「铅字行情纸」（The Letterpress Market Sheet）**

整页是一份每天出的行情纸。新闻纸灰铺满，油墨黑写字与画线，套红只落在四处：当前版次、主操作、涨、号外条。版面里没有圆角，也没有投影；分区靠 1px 栏线，分块靠 2px 油墨线，报头下面压一条粗加细的双线。每一个边界都是直线，每一块内容都有一圈框。

字号拉开对比度，不靠留白：报头 30px，稿件标题 20px，小节标题 16px，正文 13px，注释 11–12px。宋体只出现在报头和标题上，正文走系统黑体，数字与代码走等宽并一律带 tabular，刷新与翻页时列不会左右跳动。结果行沿同一条左边缘排齐，一行读下来就是一组可以逐项对账的数。

版面密度高，一屏里能并排放下图版、稿件正文、表格与告警。动效只做一件事：说明状态变了。压印、反白、落下，时长都在 100–300 毫秒之间，`prefers-reduced-motion` 打开时全局收敛。

**Key Characteristics:**

- 三类颜色参与版面：新闻纸灰的底、油墨黑的字与线、套红的点睛。
- 零圆角、零投影，全部边界由 1px 与 2px 直线给出。
- 宋体标题、黑体正文、等宽数字的三字体分工。
- 版面按容器宽度分栏，图版占比随版次变化。
- 数字沿同一条左边缘排齐，标签占固定的 8.5rem 窄列。

## Colors

一套纸与油墨的颜色：两档纸色、五档墨与线的深浅、一档套红、涨跌两色、三组状态色。

### Primary

- **套红 (Proof Red)（#b8232a）**：唯一强调色。用在当前版次、主操作按钮、涨、号外条。它的稀罕是重点，一屏里出现三处以上就该怀疑。

### Secondary

- **警示褐 (Warning Ochre)（#7d4e0c）**，配浅底 **警示褐底（#e9dcc3）**：风险提示与「中置信」「已放弃」这类需要用户留意但不阻断的状态。
- **告警红 (Alert Red)（#b8232a）**，配浅底 **告警红底（#ecd7d4）**：读取失败、导入失败、输入非法。值与套红相同，语义不同：套红说「这里是重点」，告警红说「这里出错了」。

### Tertiary

- **苔绿 (Moss Green)（#1c6b4a）**：通过、已执行、高置信、跌。与告警红成对出现。

### Neutral

- **新闻纸灰 (Newsprint)（#e4e3de）**：整页底色，也用于稿件框、表格、弹窗的面。
- **浅纸灰 (Slip)（#efeee9）**：下凹一档的底。表头、告示、输入框、结果栏、AI 解释边栏、当前版次格。
- **油墨黑 (Ink)（#16161a）**：正文、标题、以及分块用的 2px 重线。
- **淡墨 (Ink Soft)（#56544c）**：说明文字、标签、图注、运行信息。
- **栏线灰 (Rule)（#a09c8c）**：稿件框外框、图版题头与图注的分隔线、小节标题下的短线。
- **细线灰 (Rule Soft)（#c4c1b4）**：表格行与行之间、结果行与行之间的分隔。
- **控件边界灰 (Rule Strong)（#84806f）**：输入框、开关、关闭按钮、版次条与稿件目录条的格子缝。比栏线深一档，对纸张 3.08:1、对浅纸灰 3.41:1。

### Named Rules

**The 套红 Only Rule.** 套红只给四处：当前版次、主操作、涨、号外条。别处一律用墨色、栏线或状态三色。

**The Red-Up Lock Rule.** 涨是红 #b8232a，跌是绿 #1c6b4a，任何版式与主题下都不交换。图版把颜色作为数值交给 klinecharts，读不到工具类，因此图版用色单独放在 `src/chart/palette.ts`。

**The Visible Boundary Rule.** 控件的边界用控件边界灰，让框看得见；栏线灰与细线灰只做分区与分组，不承担控件轮廓。

## Typography

**Display Font:** 宋体（Songti SC / SimSun / 思源宋体，末位落到通用衬线体）
**Body Font:** 系统黑体（PingFang SC / Microsoft YaHei）
**Label/Mono Font:** SF Mono（ui-monospace / Menlo）

**Character:** 报头与标题用宋体，笔画有横细竖粗的印刷感，读起来像报纸的题；正文换成系统黑体，小字号下笔画干净；数字与代码走等宽，位置固定。

### Hierarchy

- **Masthead**（700，30px，1.1，字距 −0.025em）：报头「蓄势」两个字，全篇只此一处。
- **Title**（700，20px，1.3）：当前稿件栏的标题。
- **Head**（700，16px，1.35）：稿件框标题、弹窗标题、号外条的「号外」、稿件目录条上的版次号与标题。
- **Lede**（400，14px，1.7）：导语式段落——每步开头那句白话说明、复盘总结的正文、持仓卡上的标的名。
- **Body**（400，13px，1.6）：正文、表格内容、按钮文字。
- **Note**（400，12px，1.5）：字段标签、表头、图注、告警文字、戳记的 md 尺寸。
- **Micro**（400，11px，1.45）：报眉那行的刊期与运行信息、刊名旁的 SPRINGLOAD、版次状态行、AI 免责声明、戳记的 sm 尺寸。
- **Mono**（400，13px）：价格、比例、代码、公式块、报眉那行的期号与版本。

### Named Rules

**The Two-Face Rule.** 宋体只给报头、稿件标题、小节标题与版次号；正文一律系统黑体；数字与代码一律等宽。三者的分工不越界。

**The Tabular Rule.** 所有数字、价格、比例、代码加 tabular。等宽字体本身承担公式块与运行信息。

**The Wide Tracking Rule.** 字距放宽只用在三处：报头刊名的负字距、刊名旁的 SPRINGLOAD 与号外条的 0.3em、图版标签与 AI 标签的 0.08em。

## Layout

外壳是「报头 + 版次条 / 内容 / 报尾」三段。报头横贯整宽，从上往下四层：报眉、报头行、双线、版次条。报眉是一条等宽小字的窄带，左边是「日刊 · 期号与日期」，右边是版本、数据源与最近同步时间；macOS 上红黄绿三个按钮浮在这一条里，所以它的高度取系统标题栏的高度（这台机器上是 32px，其余平台 28px），左侧再让开 80px 给按钮，整条报头同时是拖拽区。报头行左边是刊名与 SPRINGLOAD，右边压一句刊物定位。报头行下压 2px 油墨线加 1px 油墨线的双线，再下一条版次条，五个版次并排，当前那一版套红实底反白。报尾一行 micro，写全刊统一的口径与风险说明。

**报眉与系统按钮同高。** 窄带的高度不是随手定的：macOS 把三个按钮居中在窗口标题栏里，报眉取同样的高度并把文字居中，日期那行才会落在按钮的中线上。系统换了标题栏高度，这个数跟着改。

主流程（`src/flow/FlowPage.tsx`）在 16px 内边距里竖排三块，块间 12px：稿件目录条、号外条（第 3 步触发时才出现）、版心。稿件目录条是四个版次格加右侧两个按钮，格子之间用 1px 控件边界灰的缝分开，当前格压 2px 套红上边线、底色下凹一档，锁住的格半透明。版心是容器查询的容器：容器宽到 56rem 以上时图版与稿件栏左右并排，窄了上下叠放。

图版占比随版次变，因为它要跟着当前这一步读什么走：第 1 步 30、第 2 步 44、第 3 步 54、第 4 步 32，稿件栏拿剩下的份额；面板收起时图版占满整宽。占比变化带 300 毫秒的过渡。图版是 1px 栏线外框，题头一条 1px 栏线下边线，图注一条 1px 栏线上边线；稿件栏是 2px 套红外框，压在当前版次上。

其余四页都是 16px 内边距加一个稿件框：计划卡最宽 46rem，设置页最宽 48rem。表单在窄处两列、宽到 64rem 时三到四列，列间距 12px。

页面内边距 16px，块间距 12px，稿件框内边距 14px，表格与结果行的上下内边距 6px、左右 12px。

### Named Rules

**The Rule-Line Rule.** 分区用 1px 栏线，分块用 2px 油墨线。留白只调节奏，不承担分区。

**The Container-Split Rule.** 版心按容器宽度分栏，不用视口断点。图版与稿件栏的宽度由 flex 占比给出，两者都从 0 基础宽度长出来。

**The Share-By-Step Rule.** 图版占比是版次的一部分：30 / 44 / 54 / 32。改版次就改占比，收起面板时给 100。

## Elevation & Depth

没有投影。整篇 `src/` 里唯一的阴影是报头上「蓄势」两个字的铅字压痕（`0 1px 0 rgb(255 255 255 / 0.55)` 与 `0 -1px 0 rgb(0 0 0 / 0.14)`），它画的是油墨压进纸里的压痕，与高度无关。层次由三样东西给出：1px 栏线分区、2px 油墨线分块、浅纸灰的底色把一块内容压下去一档。弹窗与悬停提示走原生顶层（`<dialog>` 的 top layer 与 fixed 定位），不靠 z-index 堆叠，也不加阴影；弹窗遮罩是一层 25% 的油墨黑。

### Named Rules

**The No-Shadow Rule.** 任何面都不加投影。要分出一块内容，就用栏线、重线或浅纸灰的底。

**The Flat-Top-Layer Rule.** 弹窗与提示的层级交给浏览器顶层与 fixed 定位，不用阴影模拟高度。

## Shapes

全篇 0 圆角，没有一处例外。形状语言是矩形与直线：控件是方角小启事，稿件是一圈 1px 的框，分块是 2px 的粗线，报头下面是 2px 加 1px 的双线。三处例外形状各有用途：AI 解释边栏与图版空态用 1px 虚线框，表示这块是临时或占位的；术语用 1px 点线下划线，表示这个词可以点开；焦点圈是 2px 套红描边、外扩 2px。

报头外面还铺了一层新闻纸纤维：`body::before` 用固定定位的 SVG 噪声，55% 不透明度、multiply 混合，让底色不像屏幕。

### Named Rules

**The Square-Corner Rule.** 全篇 0 圆角。要区分形状，只用边线的粗细、虚实与颜色。

## Components

### Buttons

- **Shape:** 方角，1px 边框（0 圆角）。
- **Primary:** 套红实底、纸张色字（#b8232a / #e4e3de），一屏一个区域里只放一个。
- **Secondary:** 浅纸灰底、油墨字（#efeee9 / #16161a），用于并列的次要操作，例如「导出 PNG」「立即同步」。
- **Outline:** 透明底、1px 油墨边，用于工具条上未选中的工具与周期按钮；选中时换成 primary。
- **Ghost:** 无底无边、淡墨字，悬停时变油墨色并加一条点线下划线，用于「删除」「退出标注」「放弃」这类弱操作。
- **Sizes:** 28px（小）、34px（默认）、40px（大），左右内边距 10 / 14 / 20px。
- **Hover / Focus:** 悬停把底色与字色对调，这是印刷里的反白；按下向下压 1px；颜色过渡 100 毫秒。焦点圈由全局的 2px 套红描边给出。
- **Disabled:** 不透明度 40%，不响应指针。

异步操作走 `StatefulButton`：加载时左边滚一枚 6px 的方形墨点，文案换成进行时（「筛选进行中…」「重算中…」），完成后换成 13px 的对勾。

### Chips

戳记（`Stamp`）是版面上一枚加了框的状态小签。

- **Style:** 1px 边框、方角、micro 字号，高 20px（sm）或 24px（md），左右内边距 6px 或 8px。
- **Tones:** neutral（栏线灰边、淡墨字、浅纸灰底）、info（油墨边、油墨字）、success（苔绿边字）、warning（警示褐边字、警示褐浅底）、danger（告警红实底、纸张色字）。
- **Where:** 置信度（高 / 中 / 低）、计划状态（草稿 / 已触发 / 已执行 / 已放弃）、标注类型（支撑区 / 缺口 / 水平线）、持仓告警。

### Cards / Containers

- **Corner Style:** 方角（0 圆角）。
- **Background:** 纸张灰；需要下凹一档时用浅纸灰。
- **Shadow Strategy:** 见 Elevation & Depth——没有投影。
- **Border:** 稿件框 1px 栏线灰；当前版次的稿件栏 2px 套红；表格上下 2px 油墨线；AI 解释边栏 1px 油墨虚线。
- **Internal Padding:** 稿件框内边距 14px，内部块间距 14px。
- **Header:** 标题是 16px 宋体加粗的 `h2`，动作按钮是它的兄弟节点，靠 `ml-auto` 落到右端；标题下面一条 2px 油墨线把题头与正文分开。动作按钮不塞进标题元素里，标题的可访问名保持干净。
- **结果栏（StatGrid / StatTile）:** 上下 2px 油墨线夹住、浅纸灰底、左右 12px 内边距；每行一条 1px 细线灰分隔。标签占固定 8.5rem 窄列、12px 淡墨字；数值紧随其后，16px 等宽加粗带 tabular；补充说明用 11px 淡墨字跟在数值右边。
- **公式块:** 上下 2px 油墨线、浅纸灰底，13px 等宽、行高 6px 级，逐行列出代入值与结果，结果用油墨黑加粗。
- **图版框:** 1px 栏线灰外框；题头一行放「图版」micro 标签与工具条，图注一行放标的、周期与操作提示。

### Inputs / Fields

- **Style:** 方角、1px 控件边界灰、浅纸灰底、高 34px、左右 10px 内边距，13px 等宽带 tabular（数字与日期按位排列）。
- **Label:** 标签在框上方，12px 中等字重淡墨字。
- **Focus:** 边框换成套红，去掉浏览器默认描边；全局焦点圈仍由 2px 套红描边给出。
- **Error:** 边框与底色换成告警红与告警红浅底，框下方追加一行 12px 告警红说明。
- **Disabled / Read-only:** 不可用时不透明度 50% 且不响应指针；只读时字色转淡墨。
- **Switch:** 16px 方格，未选中是控件边界灰边加浅纸灰底，选中时填油墨黑并在里面打一枚 12px 纸张色的勾；标签在方格右边，状态由 `aria-checked` 暴露。

### Navigation

- **报头版次条:** 五个版次并排，格子之间留 1px 缝露出控件边界灰的底，形成细线分隔。当前版次套红实底、纸张色字、中等字重；其余是纸张灰底、淡墨字，悬停时底色下凹一档、字色转油墨黑。每格高约 33px。
- **主流程稿件目录条:** `role="tablist"`，四则稿件等分，每格两行——宋体加粗的版次号与标题，下面一行 11px 淡墨字的状态（「观察名单 12 只」「已标注 3 条」「已触发」「尚未计算」）。当前格压 2px 套红上边线、底色下凹一档、版次号与标题套红；锁住的格不透明度 45%，仍可聚焦并点出原因。左右方向键在格与格之间走。
- **工具条:** 图版题头里四个标注工具加「退出标注」，用 outline 与 primary 表示选中与否，`aria-pressed` 同步。

### 号外条 (signature)

第 3 步的分钟线触发时，版心上方落下一条套红实底的横条：2px 套红边、套红底、纸张色字，左边是宋体加粗、字距 0.3em 的「号外」，中间一句「分钟线触发于 09:41」，右端一行 11px 的「可以进第 4 步算仓位」。入场从上方 8px、纵向压到 0.9 再展开，240 毫秒。它是这个版面里唯一一处整条套红。

### 弹窗 (signature)

原生 `<dialog>` 加 `showModal()`：顶层堆叠、背景 inert、焦点圈闭、Esc 与点遮罩由浏览器负责。面板是纸张灰底、2px 油墨边框、最宽 30rem、最高 85vh，从正中以 0.965 缩放加 6px 下移压印上来，260 毫秒，收起时反向折回。关闭按钮是 28px 方格，1px 控件边界灰边，悬停时反白成油墨黑底纸张色字。滚动约束用 `overscroll-behavior: contain`（对话框与 `::backdrop` 都要），不动 `body` 的 overflow。

### 行情表

- **Frame:** 上下各一条 2px 油墨线，无外框、无圆角、无投影。
- **Header:** 浅纸灰底、12px 淡墨字、1px 油墨下边线、吸顶；数字列靠右，判定列居中。
- **Rows:** 行与行之间 1px 细线灰；悬停时底色下凹一档；选中的整行反白成油墨黑底纸张色字。
- **Cells:** 上下 6px、左右 12px 内边距，13px 正文，数字带 tabular。
- **判定与排序:** 满足条件的判定打一枚 14px 苔绿的勾，不满足留一道栏线灰的横线；可排序的列头是一个按钮，右边跟一枚 13px 的箭头，未激活时 40% 不透明度，升序时翻转。

### AI 解释边栏

虚线框的一则边栏，与正文分开：1px 油墨虚线边、浅纸灰底、13px 正文、行高放宽。顶上一行 micro 标签是「AI 解释」与一枚 1px 栏线框住的「AI 生成」；生成中显示「解释生成中…」加三枚来回走的 4px 墨块；失败时显示「解释生成失败」与一个 secondary 的重试按钮，不沿用上一次的解释；底部固定一行 11px 免责声明，不随模型输出变化。

## Do's and Don'ts

### Do:

- **Do** 用令牌，不写字面色值：`bg-paper`、`text-ink-soft`、`border-rule-strong`。
- **Do** 分区用 1px 栏线，分块用 2px 油墨线，把一块内容压下去就用浅纸灰的底。
- **Do** 每个数字、价格、比例、代码都加 tabular，并用等宽字体。
- **Do** 把套红留给当前版次、主操作、涨、号外条这四处。
- **Do** 给每个控件一条看得见的边界：控件边界灰对纸张 3.08:1、对浅纸灰 3.41:1。
- **Do** 标题用宋体、正文用系统黑体、数字用等宽，三者的分工保持住。
- **Do** 每个异步按钮都带进行时文案与一枚滚动的墨点，不要只把按钮变灰。
- **Do** 风险与口径提示就地写在使用它的地方，免责声明固定出现在每一张 AI 卡片上。

### Don't:

- **Don't** 加圆角或投影；要分层就用栏线、重线与浅纸灰的底。
- **Don't** 引入第二个强调色，也不要用颜色做装饰。
- **Don't** 交换涨跌颜色：涨永远是红，跌永远是绿。
- **Don't** 用视口断点切主流程的分栏，版心按容器宽度分栏。
- **Don't** 用宋体排正文，也不要用黑体排报头与标题。
- **Don't** 把动作按钮塞进标题元素里，标题的可访问名要保持干净。
- **Don't** 在弹窗打开时给 `body` 加 `overflow: hidden`。
- **Don't** 把图版占比写成一个固定值，它随版次变。

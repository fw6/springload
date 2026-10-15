# Product

蓄势 Springload —— 个人用的「回调买入法」工作台，覆盖 A 股与美股，桌面应用。

## Platform

macOS / Windows / Linux 桌面应用。外壳是 Tauri v2，前端是跑在 WebView 里的 React
单页应用，后端是 Rust。没有服务端，数据全部落在本机 SQLite。

**运行环境：** 只跑在 Tauri 自带 WebView 里（macOS 是 WKWebView，Windows 是
WebView2，Linux 是 WebKitGTK），不面向外部浏览器。因此新平台特性（原生 `<dialog>`、
容器查询、`overscroll-behavior`）直接用，不写降级分支；`closedby` 目前 Safari 系
不支持，单独留了一处点击遮罩的兜底。

**窗口：** macOS 上标题栏是 `Overlay` 样式，红黄绿三个按钮浮在网页内容上，报头顶部
留一条天条给它们。这条天条的高度取系统标题栏的高度（这台机器上是 32px）：按钮居中在
标题栏里，天条取同样的高度并把文字居中，左边的日期那行才会落在按钮的中线上，否则会
比按钮高出一点。天条左侧还要让开 80px 给按钮。整条报头同时是拖拽区
（`data-tauri-drag-region="deep"`，拖拽需要 `core:window:allow-start-dragging` 权限）。
Windows 与 Linux 保留系统标题栏，不留天条，这条窄带只用来放刊期与运行信息。

## Stack

- 前端：React 19.3 + Vite 7 + react-router-dom 7（HashRouter，桌面应用没有服务端路由）
- 样式：Tailwind CSS v4，令牌写在 `src/index.css` 的 `@theme`，组件源码在 `src/components/ui`
- 动效：motion，曲线统一走 `src/lib/ease.ts`
- 图表：klinecharts 10（自绘 overlay，承载支撑区标注与拖动）
- 后端：Rust + Tauri v2 命令，SQLite 存储，AI 走 rig-core

## Users

一个会用「回调买入法」的个人投资者。看得懂 K 线与均线，但没有受过金融工程训练，
不希望被术语挡住。默认开启「小白模式」：每个术语都能点开看一句白话解释。

## Product Purpose

把一次完整的交易决策固化成四步：筛选强势股 → 标注支撑区 → 等待买入信号 → 算仓位与
目标位。每一步都给出「这一步在干什么」的说明，算出来的每个数字都能看到公式。

## Positioning

不是行情软件，也不是策略回测平台。它是一张**检查清单**：在按下买入之前，把该确认的
四件事按顺序确认完，并把结论写成一张可以带走的计划卡。

## Operating Context

- 盘中打开，四步里第 3 步盯着分钟线等触发；收盘后翻交易日志与复盘。
- 单机、单人、无账号。A 股走通达信协议，美股走 Yahoo Finance，实时快照两家共用
  新浪与腾讯的未公开接口，需要时手动同步。
- 两个市场各用一套数据、参数与交易规则，在报头直接切换；切过去看到的是那个市场
  自己的观察名单、计划、持仓与日志。
- 屏幕以笔记本为主，窗口可缩放，图表与面板并排。

## Capabilities and Constraints

- 四步有先后依赖：没有观察名单进不了第 2 步，没有高置信支撑区进不了第 3 步，
  没有触发信号进不了第 4 步。未解锁的步骤在顶部是灰的，点一下说明原因。
- 交易规则按市场体现。A 股：一手 100 股向下取整，T+1 当日买入次日才能卖，涨跌停与
  停牌会让止损单无法成交，红涨绿跌。美股：按整股买入，T+0 当日可卖，没有涨跌停但
  盘前盘后与熔断期间成交价可能远离计划，绿涨红跌。
- 统计口径固定标注「未计入交易成本与滑点」；费用模型默认关闭，打开后逐笔扣减。
- 免责声明固定出现在每一张 AI 卡片上，不随模型输出变化。

## Brand Commitments

- 涨跌颜色按市场：A 股红涨绿跌，美股绿涨红跌。同一屏里只出现一种约定。
- 新闻纸灰底 + 油墨黑正文，套红是唯一强调色，只给当前版次、主操作、涨（A 股）与号外条。
- 中文界面，identifier 保持英文原名。

## Evidence on Hand

- 需求规格说明书：`docs/需求规格说明书.md`
- 领域词汇：仓库根目录 `GLOSSARY.md`
- 架构与接口契约：`docs/agents/architecture.md`
- 取舍记录：`docs/adr/`

## Product Principles

1. **先讲清楚再动手。** 每一步都有白话说明与「这一步在干什么」，术语默认可点开。
2. **算出来的每个数字都要能对账。** 仓位结果旁边逐项列出公式与代入值。
3. **锁住顺序。** 该确认的没确认，就不让往下走，并说明卡在哪。
4. **风险写在脸上。** 止损距离、T+1、滑点、统计口径，都在使用它们的地方就地提示。
5. **AI 只做解释。** 它描述图上的形态与数据，不给买卖建议，卡片上固定带免责声明。

## Accessibility & Inclusion

- 所有交互控件有可访问名；步骤条是 `role="tablist"`，面板是 `role="tabpanel"` 并由
  步骤标题标注。
- 弹窗走原生 `<dialog>` + `showModal()`：顶层堆叠、背景 inert、焦点圈闭由浏览器负责。
- 尊重 `prefers-reduced-motion`：全局收敛动画时长，beUI 组件内部按 `useReducedMotion`
  关闭位移与缩放。
- 表格表头用 `scope="col"`；数字列用等宽数字对齐。

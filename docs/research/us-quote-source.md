# 美股行情源

2026-10-15 实测。A 股那一套（通达信协议 + 腾讯/新浪快照）只覆盖沪深，美股另找来源。
结论：日线、分钟线与复权用 **Yahoo Finance 的 chart 接口**（走 `yahoo_finance_api`
crate），股票池用 **Nasdaq Trader 的证券目录**，实时快照继续用 **腾讯与新浪**
（两家都有美股频道，与 A 股同一批接口）。

## 1. 候选与取舍

免费能拿到美股日线的路子不多：Stooq 的批量 CSV 被浏览器校验挡住（返回一段 JS 校验
脚本，不是数据）；新浪的 `US_MinKService` 系列接口已经下线（返回
`{"__ERROR":3,"__ERRORMSG":"Service not valid"}`）；腾讯的美股日线接口
（`web.ifzq.gtimg.cn/appstock/app/usfqkline/get`）无论怎么调 `param` 都只回两根
（2011 年的首日和最新一日），拿不到历史。Alpha Vantage、Finnhub、Tiingo 都要 key。

Yahoo 的 chart 接口不需要 key，日线与分钟线都全，还带拆股与分红事件。它的响应是
JSON，用 serde 解析，不涉及手工切字节流。

## 1.1 为什么用 crate 而不是自己解析

第一版是自己用 `serde_json` 解析 chart 响应的。后来查 crates.io 发现
[`yahoo_finance_api`](https://crates.io/crates/yahoo_finance_api) 5.0.0 已经把这件事
做完了：MIT OR Apache-2.0、22.7 万次下载、2026-08-31 还在更新，带 `blocking` feature
给同步 API，正好对得上这个应用（Rust 侧全是同步调用）。

它给的东西比自己写的多一处关键的：**`Quote` 里的 `adjclose` 是必填字段**。第一版从
`serde_json::Value` 里 `and_then` 取它，取不到就 `_ => 1.0` 悄悄按未复权价写库——行情源
哪天不返回这个字段，图上会多出除息日的假缺口而没有任何一处报错。换成 crate 之后
字段缺失会让整个响应解析失败，逐只报进同步报告，不再是静默降级。

依赖代价实测：净新增 22 个包，其中 `aws-lc-sys`（rustls 的加密后端）要用 cmake 编译。
`time`、`serde_with`、`thiserror 2`、`reqwest 0.13` 本来就已在依赖图里（rig-core 与
tauri 拉的），所以没有多出第二套 HTTP 栈。构建耗时增量在几十秒量级。

**复权模型没有跟着改。** 原本设想是既然拿到了结构化的分红与拆股事件，美股可以改成
和 A 股同一条路：抓原始价 + 事件表，本地按 `adjustment.rs` 换算。实测之后不成立：
Yahoo 的 `quote` 序列**已经是拆股调整后的价**（NVDA 2024-06-10 的 10 拆 1 在日线上没有
跳空），所以本地再施加一次拆股就会重复调整。要在美股上走 A 股那条路，得把拆股记成
一条「只用来判过期、不参与换算」的空事件，或者另加一套比对新旧窗口价格的基准检查。
两条路都是为了合并而复权路径而引入新的不变量，代价大于收益，所以保留
`adjclose ÷ close` 这个系数口径：库里存的仍然是前复权日线，`adjustment.rs` 对空事件表
是恒等换算。

## 2. Yahoo chart 接口

```
https://query1.finance.yahoo.com/v8/finance/chart/{symbol}
  ?period1={epoch}&period2={epoch}&interval=1d      # 日线
  ?range=1d&interval=1m                             # 分钟线
```

请求要带 `User-Agent`，不带也能通，但带上更稳。实测 60 次并发 6 的请求全部 200，
平均每次 79 毫秒。整条链路跑一遍（证券目录 → 5449 只逐个取三年日线 → 换算 → 写库）
实测 1572 秒，约 26 分钟：取数是七分钟左右，剩下的是把一百多万根日线写进 SQLite。
后续的增量回补仍要逐个股票发一次请求，所以每天收盘后的自动回补也要几分钟。

响应结构（crate 把它映射成 `YResponse` / `YQuoteBlock` / `Quote`）：

```
chart.result[0].meta          # currency / symbol / exchangeTimezoneName / gmtoffset / firstTradeDate
chart.result[0].timestamp[]   # 秒级时间戳，UTC
chart.result[0].indicators.quote[0]     # open / high / low / close / volume，停牌日是 null
chart.result[0].indicators.adjclose[0].adjclose   # 复权收盘价
chart.result[0].events        # 分红与拆股，各带日期与金额/比例
chart.error                   # 查不到代码时 result 为 null，这里给 {code, description}
```

`YResponse::quotes()` 把前四项摊成 `Vec<Quote>`，收盘价缺失的 bar 直接跳过，所以停牌日
不用自己判。查不到的代码返回 HTTP 404，crate 转成 `YahooError::ApiError`，适配器把它
包成网络错误逐只上报。

适配器只调两个方法：`get_quote_history_interval(symbol, start, end, "1d")` 取日线，
`get_quote_range(symbol, interval, range)` 取分钟线。`start`/`end` 是
`yahoo_finance_api::time::OffsetDateTime`，crate 把 `time` 重新导出了，不用自己再依赖
一个可能版本对不上的 `time`。

### 价格口径（关键）

**`quote` 里的开高低收已经是拆股调整后的价，`adjclose` 再叠加分红调整。**
实测 NVDA 2024-06-10 的 10 拆 1：6 月 7 日收 120.888、6 月 10 日收 121.790，日线上
没有十倍跳空，说明拆股已经在源里做过。

所以适配器不去本地施加除权除息事件，而是把 `adjclose ÷ close` 当系数缩放四个价格，
得到前复权序列写进 `bar` 表，`corporate_actions` 返回空。这样 `bar` 表的语义
（前复权日线）两个市场一致，`adjustment.rs` 对空事件表是恒等换算。

对比一下两个口径在同一段历史上的差别：AAPL 2024-10-28 的原始收盘 233.40、
adjclose 231.46，比值 1.0084，差异来自这段时间之后的分红。Yahoo 用的是乘法因子，
A 股的除权参考价公式 `(P − D) / (1 + r)` 是减法。两者都是「把分红从历史价格里去掉」，
数值上略有差别；这里选与行情源一致的那个，界面上看到的价与 Yahoo 自己的一致。

### 成交额

接口不返回成交额，用当日收盘价乘成交量近似。这个数只用于筛股的日均成交额下限，
窗口是最近 20 个交易日，那段时间的收盘价不存在拆股问题。

### 分钟线

`interval` 与可回溯长度的关系：1 分钟最多 7 天，5/15/30/60 分钟最多 60 天。适配器
按这个取 `range`。时间戳换算成 `America/New_York` 的当地时刻写进 `ts`，与 A 股按
北京时间写 `ts` 的口径一致。收盘后 Yahoo 会补一根成交量为 0 的收尾线，它不代表任何
成交，不写库。

### 时区

分钟线的时间戳要按美东时间还原，夏令时切换不能错一小时，所以核心 crate 引入
`chrono-tz`。交易日历与收盘时间（16:00 美东）也用它算。crate 给的
`YMetaData.exchange_timezone_name` 与它一致，但适配器固定用 `America/New_York`，
不跟着响应里的字符串走。

## 3. 股票池

```
https://www.nasdaqtrader.com/dynamic/SymDir/nasdaqlisted.txt
https://www.nasdaqtrader.com/dynamic/SymDir/otherlisted.txt
```

两份竖线分隔的文本，各带一行 `File Creation Time: ...` 的表尾。纳斯达克那份的列是
`Symbol|Security Name|Market Category|Test Issue|Financial Status|Round Lot Size|ETF|NextShares`，
其余交易所那份是 `ACT Symbol|Security Name|Exchange|CQS Symbol|ETF|Round Lot Size|Test Issue|NASDAQ Symbol`。

两份合计 13289 行，按下面的规则筛出 5449 只：

- `Test Issue = N` 且 `ETF = N`；
- 交易所只留纳斯达克（Q）、纽交所（N）、美交所（A）；
- 代码不含 `$`（纳斯达克的优先股写法）、不以数字开头；
- 证券名里「 - 」之后的种类说明不含 warrant / right / unit / preferred / notes /
  when issued / trust preferred / closed-end / etf / fund / `%`；
- 种类说明里含 common / ordinary share / american depositary / depositary shares。

存托凭证（ADR）的说明是 `American Depositary Shares`，本身是普通股的凭证，保留；
优先股的存托份额说明里带 `Preferred`，被前一条挡掉。落选的 1999 条正是单位、权证、
权利与优先股。

## 4. 代码写法

目录里的代码原样存库（`BRK.B`）。调不同接口时的写法不一样：

| 接口 | 写法 | 实测 |
| --- | --- | --- |
| Yahoo chart | 点换成连字符 `BRK-B` | 有数据 |
| 腾讯快照 | `us` 前缀，点保留 `usBRK.B` | 有数据 |
| 新浪快照 | `gb_` 前缀 + 全小写 `gb_brk.b` | 空占位 |

新浪的美股代码区分大小写：`gb_aapl` 有行情，`gb_AAPL` 返回空占位。

## 5. 实时快照

两家都能用，字段与 A 股不是一套。

腾讯 `https://qt.gtimg.cn/q=usAAPL`（GBK 编码），73 项：

| 项 | 含义 |
| --- | --- |
| 1 | 名称 |
| 2 | 代码带交易所后缀（`AAPL.OQ`） |
| 3 / 4 / 5 | 最新价 / 昨收 / 今开 |
| 6 | 成交量，**单位是股**（A 股是手） |
| 30 | 时间戳 |
| 33 / 34 | 最高 / 最低 |
| 37 | 成交额（美元） |
| 40 | 交易状态，空为正常 |

新浪 `https://hq.sinajs.cn/list=gb_aapl`（GB18030，要带 `sina.com.cn` 的 Referer），
36 项：

| 项 | 含义 |
| --- | --- |
| 0 / 1 | 名称 / 最新价 |
| 3 | 时间戳 |
| 5 / 6 / 7 | 今开 / 最高 / 最低 |
| 10 | 成交量（股） |
| 26 | 昨收 |
| 30 | 成交额（美元） |

## 6. 交易制度

- 美股当日买入当日可卖（T+0），持仓提示的「可执行日」就是信号日本身。
- 买入按整股，没有一手 100 股的限制；`Round Lot Size` 那一列是 100 居多，但它是
  做市商的最小报价单位，不是交易的最小申报单位。
- 没有涨跌停，但有个股熔断（LULD）与盘前盘后时段，成交价可能远离计划。
- 费用只有佣金，没有印花税、过户费、经手费与证管费，也没有单笔最低佣金。
- 涨跌颜色与 A 股相反：绿涨红跌。

## 7. 筛选在美股量级上的开销

筛选要读全市场的日线再逐只判定。用合成的 5449 只 × 750 根日线量了一遍（造数据 88 秒
不计入）：`screening::run_screen` 用 10.1 秒，其中绝大部分是五千多次
`bar::list_for_code` 的查询与四百多万行的读取。`screen_stocks` 命令是 `async` 加
`spawn_blocking`，这十秒不会冻住窗口，界面上只看到按钮停在「筛选进行中…」。

## 8. 价格基准变了怎么办

`bar` 表存的是前复权日线，隐含一个前提：库里同一只股票的所有行都在同一个价格基准上。

- 全量回补满足这个前提：行情源一次给出整段窗口，基准统一。
- 增量回补从最后一条日线开始取，新取的这一段是**今天**的基准，旧的那一段是**当时**的
  基准。中间发生过改变基准的事（美股拆股、A 股除权除息，或者行情源改了复权口径），
  接缝处就会出现台阶。

判据放在 `backfill_one` 里：增量取到的那一段与库里的旧历史在**最后一条日线那天**重叠，
把这一天的换算结果与库里存的值比一下。两边走同一条换算与取整路径，所以基准没变时两个
数完全相同；出现差别就是基准变了，于是把整段历史按新基准重取一遍（`bar::delete_for_code`
之后整体重写，免得新窗口之外还留着旧基准的行）。

这一条对两个市场都生效，A 股原来只能靠设置页的「复权维护」手动发现与重算，现在收盘后
的自动回补会自己修好；那个面板留着做手工核查。

## 9. 已知限制

- 一次只取一只股票的日线，没有批量接口。首次全量回补约 26 分钟，增量约 7 分钟。
- Yahoo 是不公开接口，字段与限流策略都可能变；解析失败按网络错误上报，不静默跳过。
- 目录里的部分代码在 Yahoo 上没有数据（近期上市、场外转板等），这些股票记进同步
  报告的错误列表，不进股票池。

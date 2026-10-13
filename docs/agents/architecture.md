# 架构与接口契约

本文件是各 ticket 并行实现时的接口契约。凡是本文件冻结的名字、签名、表结构、文件归属，实现时必须照做；跨 ticket 的调用点只依赖本文件，不依赖对方当前的实现进度。

规格是 [issue #10](https://github.com/fw6/springload/issues/10)，ticket 是 #11~#22。调研记录在 `docs/research/`，原型在 `prototype/`（选定变体 B），需求底本在 `docs/需求规格说明书.md`。

## 1. 仓库骨架

参照 `/Users/fengwei/Projects/private/comic` 的布局：pnpm workspace 一次安装同时解析 npm 依赖图与 crate 依赖图，Rust 侧是仓库根的 Cargo workspace。

```
Cargo.toml                  # Cargo workspace
package.json                # 根 npm 包 springload-workspace（private）
pnpm-workspace.yaml         # packages: apps/*, crates/springload-core；cargo.enabled: true
.npmrc                      # registry=https://registry.npmmirror.com
.cargo/config.toml          # [registries.crates-io] protocol = "sparse" + pnpm 维护的 include 块
.gitignore
AGENTS.md
GLOSSARY.md
docs/agents/architecture.md # 本文件
docs/agents/issue-tracker.md
docs/agents/triage-labels.md
docs/agents/domain.md
docs/research/
docs/adr/
docs/需求规格说明书.md
prototype/                  # 页面原型（变体 B），只作参考，不参与构建
apps/springload/            # 前端 + Tauri 壳
crates/springload-core/     # 领域核心
```

`prototype/` 有独立的 `package-lock.json`，不加入 pnpm workspace；根 `Cargo.toml` 的 `exclude` 要把 `prototype` 排除，避免 pnpm 扫描时被它的依赖卡住。

## 2. 命名与标识符

按需求文档 0.3 表，逐项一致：

| 用途 | 取值 |
| --- | --- |
| 界面标题栏 | `蓄势 Springload` |
| bundle id | `com.springload.app` |
| 可执行文件 | `springload` |
| 用户数据目录 | `~/.springload` |
| 配置文件名 | `springload.config.json` |
| 根 npm 包名 | `springload-workspace` |
| 应用 npm 包名 | `springload` |
| core crate 名 | `springload-core`（lib name `springload_core`） |
| src-tauri lib name | `springload_lib`，bin name `springload` |

版本号的事实来源是 `apps/springload/src-tauri/tauri.conf.json` 的 `version`，根 `Cargo.toml` 的 `[workspace.package] version` 跟一份，并用一条测试守着两者一致。

## 3. Rust 侧模块归属

`crates/springload-core/src/` 下每个文件只有一个所有者 ticket。非所有者不得改动该文件；需要新增能力时向所有者 ticket 提出，或在自己的文件里实现。

| 文件 | 所有者 | 内容 |
| --- | --- | --- |
| `lib.rs` | #11 | 模块声明与 re-export |
| `error.rs` | #11 | `CoreError`、`Result` |
| `config.rs` | #11 | `~/.springload/` 目录、配置文件读写、`Settings` 结构 |
| `glossary.rs` | #11 | 十个术语与四步文案（定稿文案的唯一来源） |
| `storage/mod.rs` | #11 | `Database` 打开、迁移、连接池 |
| `storage/migrations.rs` | #11 | 全部建表 SQL |
| `storage/stock.rs`、`storage/bar.rs`、`storage/bar_sync_state.rs`、`storage/watchlist.rs`、`storage/support_zone.rs`、`storage/signal.rs`、`storage/trade_plan.rs`、`storage/position.rs`、`storage/trade_log.rs`、`storage/setting.rs` | #11 | 各表的读写 |
| `storage/corporate_action.rs` | #13 | 除权除息事件的读写 |
| `storage/bar_minute.rs` | #17 | 分钟线读写 |
| `market/mod.rs` | #12 | `Bar`、`Snapshot`、`QuoteSource` 与适配器选择 |
| `market/eastmoney.rs` | #12 | 东方财富适配器 |
| `market/tencent.rs` | #12 | 腾讯适配器 |
| `market/sina.rs` | #12 | 新浪适配器 |
| `market/url_budget.rs` | #12 | 批量请求的 URL 长度切分 |
| `backfill.rs` | #12 | 股票池与日线回补 |
| `adjustment.rs` | #13 | 前复权换算与过期判定 |
| `indicators.rs` | #14 | `ema`、`avwap`、`change`、`convergence`、`r_multiple` |
| `screening.rs` | #14 | C1~C4 与剔除条件 |
| `flow.rs` | #14 | 四步解锁判定（读 watchlist / support_zone / signal 三张表） |
| `ai/mod.rs`、`ai/prompts.rs`、`ai/cache.rs` | #15 | Rig 接 DeepSeek、五段解释、缓存 |
| `support.rs` | #16 | 支撑区置信度（与收盘后批处理共用） |
| `signal.rs` | #17 | 两级触发判定 |
| `sizing.rs` | #18 | 仓位计算与减仓股数 |
| `alerts.rs` | #20 | 持仓三类提示与 T+1 日期计算 |
| `fees.rs` | #21 | 费用模型 |
| `journal.rs` | #21 | 日志统计 |

`apps/springload/src-tauri/src/` 下的命令模块按域切分，每个 ticket 只动自己的那个：

| 文件 | 所有者 |
| --- | --- |
| `main.rs`、`lib.rs`、`app.rs`、`state.rs`、`commands/mod.rs` | #11 |
| `commands/app.rs` | #11 |
| `commands/market.rs` | #12 |
| `commands/adjustment.rs` | #13 |
| `commands/screen.rs` | #14 |
| `commands/ai.rs` | #15 |
| `commands/zone.rs` | #16 |
| `commands/signal.rs` | #17 |
| `commands/sizing.rs` | #18 |
| `commands/plan.rs` | #19 |
| `commands/positions.rs` | #20 |
| `commands/journal.rs` | #21 |
| `commands/settings.rs` | #22 |

`app.rs` 里的 `invoke_handler` 按上面的顺序排列，每个 ticket 只在自己的那一行块里加自己的命令，合并时冲突只出现在相邻行。

## 4. 核心类型（冻结）

```rust
// market/mod.rs
pub struct Bar {
    pub code: String,
    pub date: String,       // YYYY-MM-DD
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,        // 股
    pub amount: f64,        // 元
}

pub struct MinuteBar {
    pub code: String,
    pub ts: String,         // YYYY-MM-DD HH:MM
    pub period: u32,        // 1 / 5 / 15 / 30 / 60
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub amount: f64,
}

pub struct Snapshot {
    pub code: String,
    pub name: String,
    pub last: f64,
    pub prev_close: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub volume: f64,
    pub amount: f64,
    pub status: TradeStatus,   // Trading | Suspended | Delisted | Unknown
    pub captured_at: String,
}

pub enum QuoteSource { Eastmoney, Sina }
```

`Bar` 与 `MinuteBar` 里 `volume` 为 0 或日期缺口都表示当天没有交易；指标计算必须跳过这些 bar，不得当成 0 参与。

## 5. SQLite 表结构（冻结）

库文件 `~/.springload/springload.db`。迁移是 `storage/migrations.rs` 里的一组按序号排列的 SQL 字符串，从空库一次跑到最新版本，重复运行不报错。全部表在 #11 建好，后续 ticket 只读写，不新增迁移。

```sql
CREATE TABLE stock (
  code TEXT PRIMARY KEY,        -- 六位代码
  name TEXT NOT NULL,
  market TEXT NOT NULL,         -- 'SH' | 'SZ'
  board TEXT NOT NULL,          -- 'main' | 'star' | 'chinext' | 'bse'
  sector TEXT,
  listed_at TEXT,               -- 上市日 YYYY-MM-DD
  is_st INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL
);

CREATE TABLE bar (              -- 前复权日线
  code TEXT NOT NULL,
  date TEXT NOT NULL,
  open REAL NOT NULL, high REAL NOT NULL, low REAL NOT NULL, close REAL NOT NULL,
  volume REAL NOT NULL, amount REAL NOT NULL,
  PRIMARY KEY (code, date)
);
CREATE INDEX idx_bar_code_date ON bar(code, date);

CREATE TABLE bar_minute (
  code TEXT NOT NULL,
  ts TEXT NOT NULL,
  period INTEGER NOT NULL,
  open REAL NOT NULL, high REAL NOT NULL, low REAL NOT NULL, close REAL NOT NULL,
  volume REAL NOT NULL, amount REAL NOT NULL,
  PRIMARY KEY (code, period, ts)
);
CREATE INDEX idx_bar_minute_code_period_ts ON bar_minute(code, period, ts);

CREATE TABLE bar_sync_state (
  code TEXT PRIMARY KEY,
  last_bar_date TEXT,
  fetched_at TEXT,
  dktotal INTEGER,
  tail_qfq TEXT,                -- JSON：[[date, close], ...]，最近 10 条前复权收盘价
  updated_at TEXT NOT NULL
);

CREATE TABLE corporate_action (
  code TEXT NOT NULL,
  ex_date TEXT NOT NULL,        -- 除权除息日
  dividend_per_share REAL NOT NULL DEFAULT 0,   -- 每股现金分红 D
  split_ratio REAL NOT NULL DEFAULT 0,          -- 送转比例 r
  pre_close REAL,               -- 除权日的前收盘价（即除权参考价）
  recorded_at TEXT NOT NULL,
  PRIMARY KEY (code, ex_date)
);

CREATE TABLE watchlist_item (
  code TEXT PRIMARY KEY,
  added_at TEXT NOT NULL,
  c1 INTEGER NOT NULL, c2 INTEGER NOT NULL, c3 INTEGER NOT NULL, c4 INTEGER NOT NULL,
  chg_1m REAL, chg_3m REAL, chg_6m REAL,
  rank_pct REAL, avg_amount REAL, convergence REAL,
  screen_run_at TEXT NOT NULL
);

CREATE TABLE support_zone (
  id TEXT PRIMARY KEY,
  code TEXT NOT NULL,
  kind TEXT NOT NULL,           -- 'hline' | 'support' | 'gap' | 'avwap_start'
  low REAL NOT NULL, high REAL NOT NULL,
  anchor_date TEXT,
  ema9 REAL, ema21 REAL, ema50 REAL,
  avwap_start_date TEXT, avwap_price REAL,
  prev_high REAL, prev_low REAL,
  gap_top REAL, gap_bottom REAL,
  confidence TEXT NOT NULL,     -- '高' | '中' | '低'
  spread REAL,
  created_at TEXT NOT NULL
);
CREATE INDEX idx_support_zone_code ON support_zone(code);

CREATE TABLE signal (
  id TEXT PRIMARY KEY,
  code TEXT NOT NULL,
  triggered_at TEXT NOT NULL,
  timeframe TEXT NOT NULL,      -- '1m' | '5m' | '15m' | '30m' | '60m'
  candle_high_broken REAL NOT NULL,
  pre_candle_high REAL NOT NULL,
  zone_id TEXT,
  status TEXT NOT NULL,         -- 'waiting' | 'triggered' | 'failed'
  created_at TEXT NOT NULL
);
CREATE INDEX idx_signal_code ON signal(code);

CREATE TABLE trade_plan (
  id TEXT PRIMARY KEY,
  code TEXT NOT NULL,
  entry REAL NOT NULL, stop REAL NOT NULL, account REAL NOT NULL, risk_pct REAL NOT NULL,
  per_r REAL NOT NULL, shares INTEGER NOT NULL, capital REAL NOT NULL,
  pos_pct REAL NOT NULL, stop_distance REAL NOT NULL,
  t3 REAL NOT NULL, t5 REAL NOT NULL,
  status TEXT NOT NULL,         -- 'draft' | 'triggered' | 'executed' | 'abandoned'
  zone_id TEXT, signal_id TEXT,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE INDEX idx_trade_plan_code ON trade_plan(code);

CREATE TABLE position (
  id TEXT PRIMARY KEY,
  plan_id TEXT NOT NULL,
  code TEXT NOT NULL,
  entry_date TEXT NOT NULL,
  entry_price REAL NOT NULL,
  quantity INTEGER NOT NULL,
  current_stop REAL NOT NULL,
  follow_ema INTEGER NOT NULL DEFAULT 1,
  exit_price REAL, exit_date TEXT,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE INDEX idx_position_code ON position(code);

CREATE TABLE trade_log (
  id TEXT PRIMARY KEY,
  position_id TEXT,
  code TEXT NOT NULL, name TEXT,
  entry_date TEXT NOT NULL, entry_price REAL NOT NULL, stop_price REAL NOT NULL,
  exit_date TEXT, exit_price REAL,
  quantity INTEGER NOT NULL,
  per_r REAL, r_multiple REAL, pnl REAL,
  fees REAL NOT NULL DEFAULT 0,
  note TEXT,
  source TEXT NOT NULL,         -- 'plan' | 'manual'
  created_at TEXT NOT NULL
);
CREATE INDEX idx_trade_log_code ON trade_log(code);

CREATE TABLE setting (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

`trade_log` 里 `exit_date` 为空表示入场记录已写入、尚未出场；统计只计已出场的行。

## 6. 配置（冻结）

`~/.springload/springload.config.json`，字段：

```rust
pub struct Settings {
    pub risk_pct: f64,                 // 单笔风险比例，默认 0.005
    pub stop_distance_warn: f64,       // 止损距离警告阈值，默认 0.05
    pub liquidity_threshold: f64,      // 日均成交额下限（元），默认 1e8
    pub chg_6m_threshold: f64,         // 近 6 个月涨幅阈值，默认 0.30
    pub rank_band: (f64, f64),         // 近 1 个月涨幅排名分位区间，默认 (0.20, 0.30)
    pub convergence_threshold: f64,    // 近 20 日极差阈值，默认 0.25
    pub beginner_mode: bool,           // 小白模式，默认 true
    pub autostart: bool,               // 登录时自启，默认 false
    pub fee_model_enabled: bool,       // 费用模型，默认 false
    pub commission_rate: f64,          // 佣金费率，默认 0.00025
    pub first_run_confirmed: bool,     // 首次启动风险提示已确认，默认 false
}
```

缺失字段用默认值补齐，读到的值非法时用默认值并在返回值里带一条警告，不静默吞掉。

## 7. Tauri 命令（冻结）

命令名用 snake_case。返回类型是 `Result<T, String>`（错误类型在命令层转成可展示文案）。所有命令的入参用 camelCase 的具名参数。

| 命令 | 所有者 | 签名 |
| --- | --- | --- |
| `app_info` | #11 | `() -> AppInfo { version, data_source, last_sync_at }` |
| `glossary` | #11 | `() -> Glossary { terms: Vec<Term>, steps: Vec<StepCopy> }` |
| `settings_get` | #11 | `() -> Settings` |
| `settings_update` | #11 | `(patch: SettingsPatch) -> Settings` |
| `sync_market` | #12 | `(full: bool) -> SyncReport { stocks, bars, elapsed_ms, errors }` |
| `sync_status` | #12 | `() -> SyncStatus { running, done, total, last_sync_at, message }` |
| `stock_list` | #12 | `() -> Vec<StockRow>` |
| `corporate_actions_stale` | #13 | `() -> Vec<StaleStock { code, name, ex_date }>` |
| `resync_adjusted` | #13 | `(codes: Vec<String>) -> ResyncReport` |
| `screen_stocks` | #14 | `() -> ScreenReport { ran_at, universe, elapsed_ms, picked, rejected }` |
| `screen_last` | #14 | `() -> Option<ScreenReport>` |
| `watchlist_get` | #14 | `() -> Vec<WatchlistRow>` |
| `watchlist_add` | #14 | `(code: String) -> ()` |
| `watchlist_remove` | #14 | `(code: String) -> ()` |
| `step_unlock` | #14 | `(code: String) -> StepUnlock { unlocked: [bool; 4], reasons: [String; 4] }` |
| `bars_get` | #12 | `(code: String, period: String) -> Vec<Bar>`（`period` 取 `'day'` 或分钟周期字符串） |
| `explain` | #15 | `(code: String, step: u8) -> Explanation { text, generated_at, model, cached }` |
| `explain_retry` | #15 | `(code: String, step: u8) -> Explanation` |
| `explain_invalidate` | #15 | `(code: String, step: u8) -> ()` |
| `support_zones_get` | #16 | `(code: String) -> Vec<SupportZoneView>` |
| `support_zone_save` | #16 | `(zone: SupportZoneInput) -> SupportZoneView` |
| `support_zone_delete` | #16 | `(id: String) -> ()` |
| `support_refs` | #16 | `(code: String, avwapStartDate: Option<String>) -> ZoneRefs { ema21, avwap, prevHigh }` |
| `signal_status` | #17 | `(code: String) -> SignalStatus { stage, zoneId, zone, detail }` |
| `signal_evaluate` | #17 | `(code: String) -> SignalStatus` |
| `poll_tick` | #17 | `() -> PollReport { waiting, positions, triggered }` |
| `plan_calc` | #18 | `(input: SizingInput) -> SizingResult` |
| `plan_save` | #19 | `(plan: TradePlanInput) -> TradePlan` |
| `plan_get` | #19 | `(id: String) -> TradePlan` |
| `plan_list` | #19 | `(status: Option<String>) -> Vec<TradePlan>` |
| `plan_confirm_executed` | #19 | `(id: String, entryDate: String) -> TradeLogEntry` |
| `plan_set_status` | #19 | `(id: String, status: String) -> TradePlan` |
| `positions_get` | #20 | `() -> Vec<PositionView>` |
| `positions_refresh_quotes` | #20 | `() -> Vec<PositionView>` |
| `position_exit` | #20 | `(id: String, exitPrice: f64, exitDate: String) -> TradeLogEntry` |
| `journal_get` | #21 | `() -> JournalView { entries, stats, fee_model_enabled }` |
| `journal_add_manual` | #21 | `(entry: ManualEntry) -> TradeLogEntry` |
| `journal_export_csv` | #21 | `(path: String) -> ()` |
| `journal_review` | #21 | `() -> Explanation` |
| `import_bars_csv` | #22 | `(path: String) -> ImportReport { rows, skipped, errors }` |
| `confirm_first_run` | #22 | `() -> Settings` |

同步进度用事件推送，事件名 `sync://progress`，载荷 `SyncStatus`。轮询刷新用事件名 `poll://tick`。

`positions_refresh_quotes` 与 `poll_tick` 内部走腾讯快照，单次上限 900 只代码，按 URL 长度切分（见 `docs/research/quote-api-limits.md`）。

## 8. 前端结构

`apps/springload/src/`：

| 文件 | 所有者 |
| --- | --- |
| `main.tsx`、`App.tsx`、`styles.css`、`test-setup.ts` | #11 |
| `api/invoke.ts`（`invoke` 的统一封装与错误转换） | #11 |
| `api/app.ts`、`api/settings.ts` | #11（`settings.ts` 的界面部分由 #22 用） |
| `api/market.ts` | #12 |
| `api/adjustment.ts` | #13 |
| `api/screen.ts` | #14 |
| `api/ai.ts` | #15 |
| `api/zone.ts` | #16 |
| `api/signal.ts` | #17 |
| `api/sizing.ts` | #18 |
| `api/plan.ts` | #19 |
| `api/positions.ts` | #20 |
| `api/journal.ts` | #21 |
| `components/TermHint.tsx`、`components/Tooltip.tsx`、`components/Modal.tsx`、`components/ConfidencePill.tsx`、`components/Flag.tsx`、`components/StepGate.tsx` | #11 |
| `components/AiCard.tsx` | #15 |
| `copy/GlossaryContext.tsx`（启动时取一次 `glossary` 命令，放 context） | #11 |
| `copy/riskNotes.ts`（需求文档第 8 章风险提示文案） | #11 |
| `api/types.ts`（跨 ticket 的 DTO 类型：`StepIndex`、`ToolId`、`Period`、`StepUnlock`、`WatchlistRow`、`SupportZoneView`、`SignalStatus`、`SizingInput` 等） | #11 |
| `test-utils.tsx`（vitest 的 `invoke` 桩与 memory router 渲染辅助） | #11 |
| `flow/useFlow.ts`、`flow/FlowPage.tsx`、`flow/Steps.tsx` | #11 |
| `flow/steps/Step1.tsx` | #14 |
| `flow/steps/Step2.tsx` | #16 |
| `flow/steps/Step3.tsx` | #17 |
| `flow/steps/Step4.tsx` | #18 |
| `chart/KLineChart.tsx`、`chart/overlays.ts` | #16 |
| `routes/PlanPage.tsx` | #19 |
| `routes/PositionsPage.tsx` | #20 |
| `routes/JournalPage.tsx` | #21 |
| `routes/SettingsPage.tsx` | #22 |

`Step1.tsx`~`Step4.tsx`、`KLineChart.tsx`、`PlanPage.tsx`~`JournalPage.tsx` 在 #11 里已经建成占位（标注了归属 ticket），由各自的 ticket 重写整个文件。`SettingsPage.tsx` 在 #11 里已经建成可用的配置读写界面，由 #22 在其上补首次启动确认、筛选参数、CSV 导入与小白模式开关。

`App.tsx` 里的路由表由 #11 一次写好，指向上面五个页面文件；后续 ticket 只改自己那个页面文件，不动 `App.tsx`。

`useFlow.ts` 由 #11 一次写好并冻结对外形状：

```ts
export interface Flow {
  step: StepIndex
  setStep: (s: StepIndex) => void
  goStep: (s: StepIndex) => void
  unlock: StepUnlock          // 来自 step_unlock 命令
  canEnter: (s: StepIndex) => boolean
  selected: string | null
  selectStock: (code: string) => void
  watchlist: WatchlistRow[]
  reloadWatchlist: () => Promise<void>
  period: 'day' | 'minute'
  setPeriod: (p: 'day' | 'minute') => void
  tool: ToolId
  setTool: (t: ToolId) => void
  avwapStartDate: string | null
  setAvwapStartDate: (d: string | null) => void
  signal: SignalStatus | null
  setSignal: (s: SignalStatus | null) => void
  sizing: SizingInput
  setSizing: (s: SizingInput) => void
  collapsed: boolean
  setCollapsed: (v: boolean) => void
}
```

各步自己的数据（筛选结果、标注列表、信号明细、计算结果）由该步自己的 hook 取，不放进 `useFlow`。需要让别的步骤看到的状态（观察名单、选中股票、周期、信号）通过上面的 setter 写回。

设计令牌取原型变体 B（`prototype/src/styles.css` 的 `.app[data-variant='B']` 一段）：正文 15px、小字 13px、标题 18px、结果数字 24px、行高 44px、面板内边距 20px、圆角 12px、间距 16px、主色 `#2563eb`、图表与面板宽度比 38:62（`--flow-cols: minmax(0, 38fr) minmax(380px, 62fr)`）。

## 9. 约定

- **错误处理**：程序性错误（不变量被破坏、非法状态）就地 panic；网络失败、用户输入非法这类在边界捕获并返回错误状态，不静默吞掉，不用 0 或旧值代替。
- **命令层是薄封装**：领域规则全在 `springload-core`，命令层只做参数转换、数据库连接、错误文案。
- **前端不算指标、不判解锁、不发网络请求**：全部通过命令。
- **测试**：Rust 集成测试放 `apps/springload/src-tauri/tests/`，直接调用命令；行情走 `crates/springload-core/tests/fixtures/` 下录制好的响应样本（样本取自 `docs/research/quote-api-spec.md` 与 `docs/research/quote-api-limits.md`），存储用临时 SQLite 文件。默认忽略的联网实测用 `#[ignore]`。
- **前端测试**：vitest，`invoke` 换成桩，用 memory router 渲染路由。
- **注释与文案用中文，identifier 保持英文原名。**
- **提交**：每个 ticket 在自己的分支上提交，提交信息写清楚做了什么。

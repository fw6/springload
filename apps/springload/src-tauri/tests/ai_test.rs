//! AI 解释的集成测试：用假的模型响应覆盖成功、失败、重试三条路径，并断言缓存命中时
//! 不再调用模型。命令层与领域层都从这里调用。

use serde_json::{json, Value};
use springload_core::ai::{self, AiError, AiResult, ChatClient, ExplainOutput, Scope};
use springload_core::storage::bar::{self, BarRow};
use springload_core::storage::signal::{self, Signal};
use springload_core::storage::stock::{self, Stock};
use springload_core::storage::support_zone::{self, SupportZone};
use springload_core::storage::trade_log::{self, TradeLogEntry};
use springload_core::storage::trade_plan::{self, TradePlan};
use springload_core::storage::watchlist::{self, WatchlistItem};
use springload_core::storage::Database;
use springload_lib::commands::ai as ai_commands;
use springload_lib::state::AppState;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{App, WebviewWindow, WebviewWindowBuilder};

const CODE: &str = "600000";

// ---------------------------------------------------------------- 假模型客户端

/// 假的模型客户端：按预设顺序吐结构化结果，记下调用次数与最后一次收到的提示词。
struct FakeChat {
    model: String,
    replies: Mutex<VecDeque<Result<String, String>>>,
    calls: AtomicUsize,
    last_prompt: Mutex<String>,
}

impl FakeChat {
    /// 每次都返回同一段解释。
    fn ok(text: &str) -> Arc<Self> {
        Self::sequence(vec![Ok(text.to_string())])
    }

    /// 每次都失败（模拟模型返回空 content / 调用失败）。
    fn failing() -> Arc<Self> {
        Self::sequence(vec![Err("模型返回了空 content".to_string())])
    }

    /// 按顺序吐出预设结果，用完之后一律失败。
    fn sequence(replies: Vec<Result<String, String>>) -> Arc<Self> {
        Arc::new(Self {
            model: "deepseek-chat".to_string(),
            replies: Mutex::new(replies.into()),
            calls: AtomicUsize::new(0),
            last_prompt: Mutex::new(String::new()),
        })
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn last_prompt(&self) -> String {
        self.last_prompt.lock().expect("提示词锁").clone()
    }
}

impl ChatClient for FakeChat {
    fn model(&self) -> &str {
        &self.model
    }

    fn extract_explanation(&self, prompt: &str) -> AiResult<ExplainOutput> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        *self.last_prompt.lock().expect("提示词锁") = prompt.to_string();
        let next = self
            .replies
            .lock()
            .expect("响应队列锁")
            .pop_front()
            .unwrap_or_else(|| Err("没有更多预设响应".to_string()));
        next.map(|text| ExplainOutput { text }).map_err(AiError::Model)
    }
}

/// 把假客户端包成命令层要的延迟构造闭包。
fn client(fake: &Arc<FakeChat>) -> impl FnOnce() -> AiResult<Arc<dyn ChatClient>> {
    let fake = fake.clone();
    move || -> AiResult<Arc<dyn ChatClient>> { Ok(fake) }
}

// ---------------------------------------------------------------- 数据铺底

/// 第 index 个交易日的日期；每 20 天一个月，字典序与时间顺序一致。
fn date_at(index: usize) -> String {
    format!("2025-{:02}-{:02}", index / 20 + 1, index % 20 + 1)
}

fn seed_stock(db: &Database) {
    let conn = db.conn().expect("取连接");
    stock::upsert(
        &conn,
        &Stock {
            code: CODE.to_string(),
            name: "浦发银行".to_string(),
            market: "SH".to_string(),
            board: "main".to_string(),
            sector: None,
            listed_at: Some("2010-01-01".to_string()),
            is_st: false,
            updated_at: "2025-03-20T00:00:00Z".to_string(),
        },
    )
    .expect("写股票");
}

fn seed_bars(db: &Database) {
    let mut conn = db.conn().expect("取连接");
    let bars: Vec<BarRow> = (0..60)
        .map(|index| {
            let close = 10.0 + index as f64 * 0.5;
            BarRow {
                code: CODE.to_string(),
                date: date_at(index),
                open: close - 0.1,
                high: close + 0.5,
                low: close - 0.5,
                close,
                volume: 1000.0,
                amount: close * 1000.0,
            }
        })
        .collect();
    bar::upsert_many(&mut conn, &bars).expect("写日线");
}

fn seed_watchlist(db: &Database) {
    let conn = db.conn().expect("取连接");
    watchlist::upsert(
        &conn,
        &WatchlistItem {
            code: CODE.to_string(),
            added_at: "2025-03-20T00:00:00Z".to_string(),
            c1: true,
            c2: true,
            c3: true,
            c4: false,
            chg_1m: Some(0.12),
            chg_3m: Some(0.35),
            chg_6m: Some(0.638),
            rank_pct: Some(0.24),
            avg_amount: Some(2.0e8),
            convergence: Some(0.069),
            screen_run_at: "2025-03-20T00:00:00Z".to_string(),
        },
    )
    .expect("写观察名单");
}

/// 第 1 步的数据：股票、日线、筛选结论。
fn seed_step1(db: &Database) {
    seed_stock(db);
    seed_bars(db);
    seed_watchlist(db);
}

fn seed_zone(db: &Database) {
    let conn = db.conn().expect("取连接");
    support_zone::upsert(
        &conn,
        &SupportZone {
            id: "zone-1".to_string(),
            code: CODE.to_string(),
            kind: "support".to_string(),
            low: 12.0,
            high: 12.6,
            anchor_date: Some("2025-03-01".to_string()),
            ema9: None,
            ema21: Some(12.3),
            ema50: None,
            avwap_start_date: None,
            avwap_price: Some(12.35),
            prev_high: Some(12.4),
            prev_low: None,
            gap_top: None,
            gap_bottom: None,
            confidence: "高".to_string(),
            spread: Some(0.008),
            created_at: "2025-03-20T00:00:00Z".to_string(),
        },
    )
    .expect("写支撑区");
}

fn seed_plan(db: &Database) {
    let conn = db.conn().expect("取连接");
    trade_plan::upsert(
        &conn,
        &TradePlan {
            id: "plan-1".to_string(),
            code: CODE.to_string(),
            entry: 20.0,
            stop: 19.5,
            account: 100_000.0,
            risk_pct: 0.005,
            per_r: 0.5,
            shares: 1000,
            capital: 20_000.0,
            pos_pct: 0.2,
            stop_distance: 0.025,
            t3: 21.5,
            t5: 22.5,
            status: "draft".to_string(),
            zone_id: Some("zone-1".to_string()),
            signal_id: None,
            created_at: "2025-03-20T00:00:00Z".to_string(),
            updated_at: "2025-03-20T00:00:00Z".to_string(),
        },
    )
    .expect("写交易计划");
}

fn seed_signal(db: &Database) {
    let conn = db.conn().expect("取连接");
    signal::upsert(
        &conn,
        &Signal {
            id: "signal-1".to_string(),
            code: CODE.to_string(),
            triggered_at: "2025-03-20 10:35".to_string(),
            timeframe: "5m".to_string(),
            candle_high_broken: 12.55,
            pre_candle_high: 12.4,
            zone_id: Some("zone-1".to_string()),
            status: "triggered".to_string(),
            created_at: "2025-03-20T10:35:00Z".to_string(),
        },
    )
    .expect("写信号");
}

fn seed_trade_log(db: &Database) {
    let conn = db.conn().expect("取连接");
    for (index, (r_multiple, pnl)) in [(1.5_f64, 300.0_f64), (-1.0, -200.0), (2.0, 400.0)]
        .into_iter()
        .enumerate()
    {
        trade_log::upsert(
            &conn,
            &TradeLogEntry {
                id: format!("log-{index}"),
                position_id: None,
                code: CODE.to_string(),
                name: Some("浦发银行".to_string()),
                entry_date: format!("2025-02-{:02}", index + 1),
                entry_price: 20.0,
                stop_price: 19.5,
                exit_date: Some(format!("2025-02-{:02}", index + 10)),
                exit_price: Some(20.0 + r_multiple * 0.5),
                quantity: 1000,
                per_r: Some(0.5),
                r_multiple: Some(r_multiple),
                pnl: Some(pnl),
                fees: 0.0,
                note: None,
                source: "manual".to_string(),
                created_at: "2025-02-20T00:00:00Z".to_string(),
            },
        )
        .expect("写交易记录");
    }
}

/// 领域层的测试夹具：一个临时数据目录加装配好的应用状态。
struct Harness {
    _dir: tempfile::TempDir,
    state: AppState,
}

impl Harness {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("临时目录");
        let state = AppState::from_data_dir(dir.path().to_path_buf()).expect("装配应用状态");
        Self { _dir: dir, state }
    }

    fn db(&self) -> &Database {
        self.state.db()
    }
}

// ---------------------------------------------------------------- 领域层：成功 / 失败 / 重试 / 缓存

#[test]
fn step1_success_generates_once_and_reports_the_model() {
    let harness = Harness::new();
    seed_step1(harness.db());
    let fake = FakeChat::ok("这只股票近 6 个月上涨 63.8%。");

    let explanation =
        ai_commands::explain_with(&harness.state, CODE, Scope::Step1, client(&fake)).expect("生成解释");

    assert_eq!(explanation.text, "这只股票近 6 个月上涨 63.8%。");
    assert!(!explanation.cached);
    assert_eq!(explanation.model, "deepseek-chat");
    assert!(!explanation.generated_at.is_empty());
    assert_eq!(fake.calls(), 1);

    // 第 1 步的解释把 C1~C4 的判定逐条交给模型，并带上支撑判定的数字。
    let prompt = fake.last_prompt();
    for label in ["C1 涨幅条件", "C2 结构条件", "C3 均线条件", "C4 排除条件"] {
        assert!(prompt.contains(label), "提示词里应逐条列出 {label}：{prompt}");
    }
    assert!(prompt.contains("63.8%"), "提示词应带上近 6 个月涨幅：{prompt}");
    assert!(prompt.contains("24.0%"), "提示词应带上排名分位：{prompt}");
    assert!(prompt.contains("EMA21："), "提示词应带上 EMA21：{prompt}");
    assert!(prompt.contains("最新收盘价："), "提示词应带上收盘价：{prompt}");
}

#[test]
fn cache_hit_does_not_call_the_model() {
    let harness = Harness::new();
    seed_step1(harness.db());
    let first = FakeChat::ok("第一次生成的解释");
    ai_commands::explain_with(&harness.state, CODE, Scope::Step1, client(&first)).expect("首次生成");
    assert_eq!(first.calls(), 1);

    // 第二次换一个「一调用就失败」的客户端：命中缓存就不该碰它。
    let second = FakeChat::failing();
    let explanation =
        ai_commands::explain_with(&harness.state, CODE, Scope::Step1, client(&second)).expect("读缓存");

    assert!(explanation.cached);
    assert_eq!(explanation.text, "第一次生成的解释");
    assert_eq!(second.calls(), 0, "缓存命中时不应调用模型");
}

#[test]
fn failure_returns_an_error_and_does_not_reuse_the_previous_explanation() {
    let harness = Harness::new();
    seed_step1(harness.db());
    let ok = FakeChat::ok("上一版的解释");
    ai_commands::explain_with(&harness.state, CODE, Scope::Step1, client(&ok)).expect("首次生成");

    let failing = FakeChat::failing();
    let message = ai_commands::explain_retry_with(&harness.state, CODE, Scope::Step1, client(&failing))
        .expect_err("生成失败应返回错误");
    assert!(message.contains("模型调用失败"), "错误文案应说明模型调用失败：{message}");
    assert_eq!(failing.calls(), 1);

    // 卡片拿到的是错误而不是上一版；再点重试用能成功的客户端，拿到的是新解释。
    let ok_again = FakeChat::ok("重试后的解释");
    let explanation =
        ai_commands::explain_retry_with(&harness.state, CODE, Scope::Step1, client(&ok_again))
            .expect("重试成功");
    assert_eq!(explanation.text, "重试后的解释");
    assert!(!explanation.cached);
}

#[test]
fn retry_after_a_failed_first_attempt_succeeds() {
    let harness = Harness::new();
    seed_step1(harness.db());

    let first = FakeChat::failing();
    assert!(
        ai_commands::explain_with(&harness.state, CODE, Scope::Step1, client(&first)).is_err(),
        "第一次生成失败"
    );
    assert_eq!(first.calls(), 1);

    let second = FakeChat::ok("重试成功的解释");
    let explanation =
        ai_commands::explain_retry_with(&harness.state, CODE, Scope::Step1, client(&second))
            .expect("重试成功");
    assert_eq!(explanation.text, "重试成功的解释");
    assert!(!explanation.cached);
    assert_eq!(second.calls(), 1);
}

#[test]
fn invalidate_forces_regeneration() {
    let harness = Harness::new();
    seed_step1(harness.db());
    let first = FakeChat::ok("第一次");
    ai_commands::explain_with(&harness.state, CODE, Scope::Step1, client(&first)).expect("首次生成");

    let cached = ai_commands::explain_with(&harness.state, CODE, Scope::Step1, client(&FakeChat::failing()))
        .expect("读缓存");
    assert!(cached.cached);

    ai_commands::explain_invalidate_with(&harness.state, CODE, Scope::Step1).expect("缓存失效");

    let regenerated = FakeChat::ok("第二次");
    let explanation =
        ai_commands::explain_with(&harness.state, CODE, Scope::Step1, client(&regenerated))
            .expect("重新生成");
    assert_eq!(explanation.text, "第二次");
    assert!(!explanation.cached);
    assert_eq!(regenerated.calls(), 1);
}

#[test]
fn incomplete_context_returns_an_error_instead_of_generating() {
    let harness = Harness::new();
    seed_stock(harness.db());
    seed_bars(harness.db());
    // 没有筛选结论：第 1 步的解释数据不齐备。
    let fake = FakeChat::ok("不该被调用");

    let message = ai_commands::explain_with(&harness.state, CODE, Scope::Step1, client(&fake))
        .expect_err("数据不齐备应报错");
    assert!(message.contains("数据还不齐备"), "{message}");
    assert_eq!(fake.calls(), 0, "数据不齐备时不应调用模型");
}

#[test]
fn step2_explains_the_zones_and_their_confidence() {
    let harness = Harness::new();
    seed_stock(harness.db());
    seed_bars(harness.db());
    seed_zone(harness.db());
    let fake = FakeChat::ok("图上有一块高置信支撑区。");

    let explanation = ai_commands::explain_with(&harness.state, CODE, Scope::Step2, client(&fake))
        .expect("生成解释");

    assert_eq!(explanation.text, "图上有一块高置信支撑区。");
    let prompt = fake.last_prompt();
    assert!(prompt.contains("置信度 高"), "{prompt}");
    assert!(prompt.contains("AVWAP 12.35"), "{prompt}");
    assert!(prompt.contains("前期高点 12.40"), "{prompt}");
}

#[test]
fn step3_reports_which_level_it_is_stuck_on() {
    let harness = Harness::new();
    seed_stock(harness.db());
    seed_bars(harness.db());
    seed_zone(harness.db());
    let waiting = FakeChat::ok("还在等待触发。");

    ai_commands::explain_with(&harness.state, CODE, Scope::Step3, client(&waiting)).expect("生成解释");
    assert!(waiting.last_prompt().contains("当前状态：等待中"), "{}", waiting.last_prompt());

    // 有信号时状态变成已触发。
    seed_signal(harness.db());
    let triggered = FakeChat::ok("已经触发。");
    ai_commands::explain_retry_with(&harness.state, CODE, Scope::Step3, client(&triggered))
        .expect("生成解释");
    let prompt = triggered.last_prompt();
    assert!(prompt.contains("当前状态：已触发"), "{prompt}");
    assert!(prompt.contains("被突破的那一根最高价：12.55"), "{prompt}");
}

#[test]
fn step4_explains_the_risk_structure() {
    let harness = Harness::new();
    seed_stock(harness.db());
    seed_bars(harness.db());
    seed_plan(harness.db());
    let fake = FakeChat::ok("这一单的每股风险是 0.50 元。");

    ai_commands::explain_with(&harness.state, CODE, Scope::Step4, client(&fake)).expect("生成解释");

    let prompt = fake.last_prompt();
    assert!(prompt.contains("每股风险：0.50"), "{prompt}");
    assert!(prompt.contains("股数：1000 股"), "{prompt}");
    assert!(prompt.contains("止损距离：2.5%"), "{prompt}");
    assert!(prompt.contains("3R 目标位：21.50"), "{prompt}");
    assert!(prompt.contains("5R 目标位：22.50"), "{prompt}");
}

#[test]
fn journal_scope_summarizes_closed_trades() {
    let harness = Harness::new();
    seed_stock(harness.db());
    seed_trade_log(harness.db());
    let fake = FakeChat::ok("已出场的三笔里两笔盈利。");

    let explanation = ai_commands::explain_with(&harness.state, CODE, Scope::Journal, client(&fake))
        .expect("生成解释");

    assert_eq!(explanation.text, "已出场的三笔里两笔盈利。");
    let prompt = fake.last_prompt();
    assert!(prompt.contains("已出场 3 笔，其中盈利 2 笔"), "{prompt}");
    assert!(prompt.contains("胜率：66.7%"), "{prompt}");
    assert!(prompt.contains("未计入交易成本与滑点"), "{prompt}");
}

#[test]
fn missing_api_key_is_reported_without_calling_anything() {
    let error = match ai::RigChatClient::new(None, ai::DEFAULT_MODEL) {
        Ok(_) => panic!("缺 key 时应报错"),
        Err(error) => error,
    };
    assert!(matches!(error, AiError::MissingApiKey), "{error}");
    assert!(error.to_string().contains("DEEPSEEK_API_KEY"), "{error}");
}

// ---------------------------------------------------------------- 命令层：通过 IPC 调用三条命令

/// 通过 IPC 调命令的测试夹具：状态指向临时数据目录，只注册本 ticket 的三条命令。
struct IpcHarness {
    // 先声明 webview：释放顺序是先 webview 后 app。
    webview: WebviewWindow<MockRuntime>,
    #[allow(dead_code)]
    app: App<MockRuntime>,
    dir: tempfile::TempDir,
}

impl IpcHarness {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("临时目录");
        let state = AppState::from_data_dir(dir.path().to_path_buf()).expect("装配应用状态");
        let app = mock_builder()
            .manage(state)
            .invoke_handler(tauri::generate_handler![
                ai_commands::explain,
                ai_commands::explain_retry,
                ai_commands::explain_invalidate,
            ])
            .build(mock_context(noop_assets()))
            .expect("构建测试应用");
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("建测试窗口");
        Self { webview, app, dir }
    }

    /// 直接打开同一个库文件铺底数据。
    fn db(&self) -> Database {
        Database::open(&self.dir.path().join("springload.db")).expect("打开数据库")
    }

    /// 通过 IPC 层调用一个命令，返回命令的 JSON 结果或错误值。
    fn invoke(&self, cmd: &str, args: Value) -> Result<Value, Value> {
        let body = if args.is_null() {
            InvokeBody::default()
        } else {
            InvokeBody::Json(args)
        };
        let response = get_ipc_response(
            &self.webview,
            InvokeRequest {
                cmd: cmd.to_string(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "tauri://localhost".parse().unwrap(),
                body,
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )?;
        Ok(response.deserialize::<Value>().expect("响应可反序列化"))
    }
}

#[test]
fn explain_command_serves_the_cached_explanation() {
    let harness = IpcHarness::new();
    let db = harness.db();
    seed_step1(&db);
    // 预置一条缓存，证明缓存命中时不需要 API key 也不会调模型。
    let conn = db.conn().expect("取连接");
    ai::cache::put(
        &conn,
        CODE,
        Scope::Step1,
        "2025-03-20",
        &ai::cache::CacheEntry {
            text: "缓存里的解释".to_string(),
            generated_at: "2025-03-20T08:00:00Z".to_string(),
            model: "deepseek-chat".to_string(),
        },
    )
    .expect("写缓存");

    let result = harness
        .invoke("explain", json!({ "code": CODE, "step": 1 }))
        .expect("explain 成功");

    assert_eq!(result["text"], "缓存里的解释");
    assert_eq!(result["cached"], true);
    assert_eq!(result["model"], "deepseek-chat");
    assert_eq!(result["generatedAt"], "2025-03-20T08:00:00Z");
}

#[test]
fn explain_command_rejects_an_unknown_step() {
    let harness = IpcHarness::new();

    let error = harness
        .invoke("explain", json!({ "code": CODE, "step": 9 }))
        .expect_err("未知步骤应报错");

    assert_eq!(error.as_str().unwrap_or_default(), "未知的步骤编号: 9");
}

#[test]
fn explain_invalidate_command_removes_the_cached_row() {
    let harness = IpcHarness::new();
    let db = harness.db();
    seed_step1(&db);
    let conn = db.conn().expect("取连接");
    ai::cache::put(
        &conn,
        CODE,
        Scope::Step1,
        "2025-03-20",
        &ai::cache::CacheEntry {
            text: "缓存里的解释".to_string(),
            generated_at: "2025-03-20T08:00:00Z".to_string(),
            model: "deepseek-chat".to_string(),
        },
    )
    .expect("写缓存");
    assert!(ai::cache::get(&conn, CODE, Scope::Step1, "2025-03-20")
        .expect("读缓存")
        .is_some());

    harness
        .invoke("explain_invalidate", json!({ "code": CODE, "step": 1 }))
        .expect("explain_invalidate 成功");

    assert!(
        ai::cache::get(&conn, CODE, Scope::Step1, "2025-03-20")
            .expect("读缓存")
            .is_none(),
        "失效后缓存行应被删除"
    );
}

#[test]
fn explain_invalidate_only_touches_its_own_scope() {
    let harness = Harness::new();
    seed_step1(harness.db());
    seed_zone(harness.db());
    let conn = harness.db().conn().expect("取连接");
    ai::cache::put(
        &conn,
        CODE,
        Scope::Step1,
        "2025-03-20",
        &ai::cache::CacheEntry {
            text: "第 1 步的解释".to_string(),
            generated_at: "2025-03-20T08:00:00Z".to_string(),
            model: "deepseek-chat".to_string(),
        },
    )
    .expect("写缓存");
    ai::cache::put(
        &conn,
        CODE,
        Scope::Step2,
        "2025-03-20",
        &ai::cache::CacheEntry {
            text: "第 2 步的解释".to_string(),
            generated_at: "2025-03-20T08:00:00Z".to_string(),
            model: "deepseek-chat".to_string(),
        },
    )
    .expect("写缓存");

    ai_commands::explain_invalidate_with(&harness.state, CODE, Scope::Step2).expect("失效第 2 步");

    assert!(
        ai::cache::get(&conn, CODE, Scope::Step1, "2025-03-20")
            .expect("读缓存")
            .is_some(),
        "第 1 步的缓存不该被第 2 步的失效带走"
    );
    assert!(ai::cache::get(&conn, CODE, Scope::Step2, "2025-03-20")
        .expect("读缓存")
        .is_none());
}

#[test]
fn switching_stocks_regenerates_instead_of_reusing_the_other_stocks_explanation() {
    let harness = Harness::new();
    seed_step1(harness.db());
    // 另一只股票：同样的日线形状，不同的代码。
    let other = "000001";
    let mut conn = harness.db().conn().expect("取连接");
    let bars: Vec<BarRow> = (0..60)
        .map(|index| {
            let close = 10.0 + index as f64 * 0.5;
            BarRow {
                code: other.to_string(),
                date: date_at(index),
                open: close - 0.1,
                high: close + 0.5,
                low: close - 0.5,
                close,
                volume: 1000.0,
                amount: close * 1000.0,
            }
        })
        .collect();
    bar::upsert_many(&mut conn, &bars).expect("写日线");
    stock::upsert(
        &conn,
        &Stock {
            code: other.to_string(),
            name: "平安银行".to_string(),
            market: "SZ".to_string(),
            board: "main".to_string(),
            sector: None,
            listed_at: Some("2010-01-01".to_string()),
            is_st: false,
            updated_at: "2025-03-20T00:00:00Z".to_string(),
        },
    )
    .expect("写股票");
    let mut item = watchlist::get(&conn, CODE).expect("读观察名单").expect("有记录");
    item.code = other.to_string();
    watchlist::upsert(&conn, &item).expect("写观察名单");

    let first = FakeChat::ok("浦发银行的解释");
    ai_commands::explain_with(&harness.state, CODE, Scope::Step1, client(&first)).expect("生成");

    // 切到另一只股票：缓存键不同，必须重新生成。
    let second = FakeChat::ok("平安银行的解释");
    let explanation =
        ai_commands::explain_with(&harness.state, other, Scope::Step1, client(&second)).expect("生成");
    assert_eq!(explanation.text, "平安银行的解释");
    assert!(!explanation.cached);
    assert_eq!(second.calls(), 1);

    // 切回第一只：原来的缓存还在，直接命中。
    let back = ai_commands::explain_with(&harness.state, CODE, Scope::Step1, client(&FakeChat::failing()))
        .expect("读缓存");
    assert!(back.cached);
    assert_eq!(back.text, "浦发银行的解释");
}

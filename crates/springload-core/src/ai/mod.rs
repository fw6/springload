//! AI 解释：Rig 接 DeepSeek、五段提示词、按（代码、步骤、数据截止日）缓存。
//!
//! 领域规则都在这里，命令层只做接线。模型调用走 [`ChatClient`] 这个接缝：
//! 生产实现 [`RigChatClient`] 用 Rig 的 extractor 调 DeepSeek，测试实现返回固定
//! 的结构化结果。

pub mod cache;
pub mod context;
pub mod prompts;

pub use context::ExplainContext;

use crate::error::CoreError;
use crate::market::Market;
use crate::storage::{now, Database};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};

/// DeepSeek 的对话模型名。
pub const DEFAULT_MODEL: &str = "deepseek-chat";
/// 读取 DeepSeek API key 的环境变量名。
pub const API_KEY_ENV: &str = "DEEPSEEK_API_KEY";
/// 结构化输出失败（含模型没有调用 `submit`、返回空 content）时的重试次数。
pub const RETRIES: u64 = 2;

/// 解释的作用范围：主流程四步与交易日志页。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Step1,
    Step2,
    Step3,
    Step4,
    Journal,
}

impl Scope {
    /// 命令层的 `step` 参数：1~4 对应主流程四步，0 是交易日志页。
    pub fn from_step(step: u8) -> Option<Self> {
        match step {
            0 => Some(Scope::Journal),
            1 => Some(Scope::Step1),
            2 => Some(Scope::Step2),
            3 => Some(Scope::Step3),
            4 => Some(Scope::Step4),
            _ => None,
        }
    }

    /// 该 scope 对应的 `step` 参数。
    pub fn step(self) -> u8 {
        match self {
            Scope::Journal => 0,
            Scope::Step1 => 1,
            Scope::Step2 => 2,
            Scope::Step3 => 3,
            Scope::Step4 => 4,
        }
    }

    /// 缓存键里用的稳定名字。
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::Journal => "journal",
            Scope::Step1 => "1",
            Scope::Step2 => "2",
            Scope::Step3 => "3",
            Scope::Step4 => "4",
        }
    }
}

/// 一条解释的对外形状，前端直接渲染。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Explanation {
    pub text: String,
    pub generated_at: String,
    pub model: String,
    /// 这次结果是读缓存来的还是刚生成的。
    pub cached: bool,
}

/// 结构化输出的目标类型：Rig extractor 的 `submit` 工具参数就是它的 JSON Schema。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ExplainOutput {
    /// 解释正文。
    pub text: String,
}

/// AI 解释相关的错误。
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("缺少 {API_KEY_ENV} 环境变量，无法调用 DeepSeek 生成解释")]
    MissingApiKey,
    #[error("AI 客户端不可用: {0}")]
    Unavailable(String),
    #[error("模型调用失败: {0}")]
    Model(String),
    #[error("解释所需的数据还不齐备: {0}")]
    Incomplete(String),
    #[error(transparent)]
    Storage(#[from] CoreError),
}

/// AI 模块的结果别名。
pub type AiResult<T> = std::result::Result<T, AiError>;

/// 模型调用接缝：把提示词交给模型，拿回结构化的解释。
pub trait ChatClient: Send + Sync {
    /// 实际使用的模型名，写进 [`Explanation::model`]。
    fn model(&self) -> &str;

    /// 一次结构化抽取。
    fn extract_explanation(&self, prompt: &str) -> AiResult<ExplainOutput>;
}

/// 读缓存；没有缓存就生成一次并写回。
///
/// `client` 是延迟构造的：命中缓存时不会去建模型客户端，所以缓存能离线读出来。
pub fn explain<F>(
    db: &Database,
    client: F,
    code: &str,
    scope: Scope,
    market: Market,
) -> AiResult<Explanation>
where
    F: FnOnce() -> AiResult<Arc<dyn ChatClient>>,
{
    let conn = db.conn()?;
    let context = ready_context(&conn, code, scope)?;
    if let Some(entry) = cache::get(&conn, code, scope, &context.data_as_of)? {
        return Ok(Explanation {
            text: entry.text,
            generated_at: entry.generated_at,
            model: entry.model,
            cached: true,
        });
    }
    let client = client()?;
    generate(&conn, client.as_ref(), code, scope, &context, market)
}

/// 跳过缓存，重新生成一次并覆盖缓存。生成失败时不写缓存，也不沿用上一次的结果。
pub fn explain_retry<F>(
    db: &Database,
    client: F,
    code: &str,
    scope: Scope,
    market: Market,
) -> AiResult<Explanation>
where
    F: FnOnce() -> AiResult<Arc<dyn ChatClient>>,
{
    let conn = db.conn()?;
    let context = ready_context(&conn, code, scope)?;
    let client = client()?;
    generate(&conn, client.as_ref(), code, scope, &context, market)
}

/// 让某只股票某一步的解释缓存失效（第 2 步增删或拖动标注后由卡片调用）。
pub fn explain_invalidate(db: &Database, code: &str, scope: Scope) -> AiResult<()> {
    let conn = db.conn()?;
    cache::invalidate(&conn, code, scope)?;
    Ok(())
}

/// 取上下文并检查该 scope 的数据是否齐备。
fn ready_context(
    conn: &rusqlite::Connection,
    code: &str,
    scope: Scope,
) -> AiResult<ExplainContext> {
    let context = context::build_context(conn, code, scope)?;
    if !context.is_complete() {
        return Err(AiError::Incomplete(context.missing_reason()));
    }
    Ok(context)
}

/// 调模型生成一次，成功后写缓存。
fn generate(
    conn: &rusqlite::Connection,
    client: &dyn ChatClient,
    code: &str,
    scope: Scope,
    context: &ExplainContext,
    market: Market,
) -> AiResult<Explanation> {
    let prompt = prompts::build(scope, context, market);
    let output = client.extract_explanation(&prompt)?;
    let entry = cache::CacheEntry {
        text: output.text,
        generated_at: now(),
        model: client.model().to_string(),
    };
    cache::put(conn, code, scope, &context.data_as_of, &entry)?;
    Ok(Explanation {
        text: entry.text,
        generated_at: entry.generated_at,
        model: entry.model,
        cached: false,
    })
}

/// 生产客户端：Rig 接 DeepSeek，结构化输出走 extractor。
pub struct RigChatClient {
    runtime: tokio::runtime::Runtime,
    client: rig_core::providers::deepseek::Client,
    model: String,
    retries: u64,
}

impl RigChatClient {
    /// 用显式的 API key 建客户端；`None` 表示环境变量缺失。
    pub fn new(api_key: Option<String>, model: impl Into<String>) -> AiResult<Self> {
        let api_key = api_key.ok_or(AiError::MissingApiKey)?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|error| AiError::Unavailable(format!("创建异步运行期失败: {error}")))?;
        let client = rig_core::providers::deepseek::Client::new(api_key)
            .map_err(|error| AiError::Unavailable(format!("创建 DeepSeek 客户端失败: {error}")))?;
        Ok(Self {
            runtime,
            client,
            model: model.into(),
            retries: RETRIES,
        })
    }

    /// 从 `DEEPSEEK_API_KEY` 读 key 建客户端。
    pub fn from_env() -> AiResult<Self> {
        Self::new(std::env::var(API_KEY_ENV).ok(), DEFAULT_MODEL)
    }
}

impl ChatClient for RigChatClient {
    fn model(&self) -> &str {
        &self.model
    }

    fn extract_explanation(&self, prompt: &str) -> AiResult<ExplainOutput> {
        use rig_agent::extractor::{Extractor, ExtractorBuilder};
        use rig_core::client::CompletionClient;

        let extractor: Extractor<ExplainOutput> =
            ExtractorBuilder::new(self.client.completion_model(self.model.clone()))
                .preamble(prompts::PREAMBLE)
                // DeepSeek 只支持 json_object 响应格式，通过 additional_params 加固。
                .additional_params(serde_json::json!({
                    "response_format": { "type": "json_object" }
                }))
                .retries(self.retries)
                .build();
        let response = self
            .runtime
            .block_on(async { extractor.extract_with_usage(prompt).await })
            .map_err(|error| AiError::Model(error.to_string()))?;
        Ok(response.data)
    }
}

/// 生产客户端在进程内的单例。环境变量不会在进程运行期间改变，所以首次的结果
/// （含缺失 key 的错误）会被记住。
pub fn default_client() -> AiResult<Arc<dyn ChatClient>> {
    static CLIENT: OnceLock<std::result::Result<Arc<dyn ChatClient>, String>> = OnceLock::new();
    match CLIENT.get_or_init(|| {
        RigChatClient::from_env()
            .map(|client| Arc::new(client) as Arc<dyn ChatClient>)
            .map_err(|error| error.to_string())
    }) {
        Ok(client) => Ok(client.clone()),
        Err(message) => Err(AiError::Unavailable(message.clone())),
    }
}

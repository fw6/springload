//! AI 解释的三条命令。领域规则在 `springload-core::ai`，这里只做接线与错误文案。
//!
//! 生成解释要等模型返回，可能十几秒。Tauri 把没有 `async` 的命令放在主线程上跑，
//! 那段时间窗口既不会重绘也接不了点击，「解释生成中」的墨点都动不了，所以生成与
//! 重试都挪到线程池；只清缓存的失效命令很快，留在原地。

use crate::state::AppState;
use springload_core::ai::{self, AiResult, ChatClient, Explanation, Scope};
use springload_core::storage::Database;
use std::sync::Arc;
use tauri::State;

/// 读缓存，没有缓存就在数据齐备时生成一次。
#[tauri::command]
pub async fn explain(
    state: State<'_, AppState>,
    code: String,
    step: u8,
) -> Result<Explanation, String> {
    let scope = scope_of(step)?;
    let db = state.db().clone();
    tauri::async_runtime::spawn_blocking(move || {
        explain_with(&db, &code, scope, ai::default_client)
    })
    .await
    .map_err(|error| error.to_string())?
}

/// 跳过缓存重新生成一次。
#[tauri::command]
pub async fn explain_retry(
    state: State<'_, AppState>,
    code: String,
    step: u8,
) -> Result<Explanation, String> {
    let scope = scope_of(step)?;
    let db = state.db().clone();
    tauri::async_runtime::spawn_blocking(move || {
        explain_retry_with(&db, &code, scope, ai::default_client)
    })
    .await
    .map_err(|error| error.to_string())?
}

/// 让某只股票某一步的解释缓存失效。
#[tauri::command]
pub fn explain_invalidate(
    state: State<'_, AppState>,
    code: String,
    step: u8,
) -> Result<(), String> {
    let scope = scope_of(step)?;
    explain_invalidate_with(state.db(), &code, scope)
}

/// `explain` 的接线主体：模型客户端由调用方给出，便于测试注入。
pub fn explain_with<F>(
    db: &Database,
    code: &str,
    scope: Scope,
    client: F,
) -> Result<Explanation, String>
where
    F: FnOnce() -> AiResult<Arc<dyn ChatClient>>,
{
    ai::explain(db, client, code, scope).map_err(|error| error.to_string())
}

/// `explain_retry` 的接线主体。
pub fn explain_retry_with<F>(
    db: &Database,
    code: &str,
    scope: Scope,
    client: F,
) -> Result<Explanation, String>
where
    F: FnOnce() -> AiResult<Arc<dyn ChatClient>>,
{
    ai::explain_retry(db, client, code, scope).map_err(|error| error.to_string())
}

/// `explain_invalidate` 的接线主体。
pub fn explain_invalidate_with(db: &Database, code: &str, scope: Scope) -> Result<(), String> {
    ai::explain_invalidate(db, code, scope).map_err(|error| error.to_string())
}

/// 把命令层的 `step` 参数转成 scope：1~4 是主流程四步，0 是交易日志页。
fn scope_of(step: u8) -> Result<Scope, String> {
    Scope::from_step(step).ok_or_else(|| format!("未知的步骤编号: {step}"))
}


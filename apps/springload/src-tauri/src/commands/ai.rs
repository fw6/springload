//! AI 解释的三条命令。领域规则在 `springload-core::ai`，这里只做接线与错误文案。

use crate::state::AppState;
use springload_core::ai::{self, AiResult, ChatClient, Explanation, Scope};
use std::sync::Arc;
use tauri::State;

/// 读缓存，没有缓存就在数据齐备时生成一次。
#[tauri::command]
pub fn explain(state: State<'_, AppState>, code: String, step: u8) -> Result<Explanation, String> {
    let scope = scope_of(step)?;
    explain_with(state.inner(), &code, scope, ai::default_client)
}

/// 跳过缓存重新生成一次。
#[tauri::command]
pub fn explain_retry(
    state: State<'_, AppState>,
    code: String,
    step: u8,
) -> Result<Explanation, String> {
    let scope = scope_of(step)?;
    explain_retry_with(state.inner(), &code, scope, ai::default_client)
}

/// 让某只股票某一步的解释缓存失效。
#[tauri::command]
pub fn explain_invalidate(
    state: State<'_, AppState>,
    code: String,
    step: u8,
) -> Result<(), String> {
    let scope = scope_of(step)?;
    explain_invalidate_with(state.inner(), &code, scope)
}

/// `explain` 的接线主体：模型客户端由调用方给出，便于测试注入。
pub fn explain_with<F>(
    state: &AppState,
    code: &str,
    scope: Scope,
    client: F,
) -> Result<Explanation, String>
where
    F: FnOnce() -> AiResult<Arc<dyn ChatClient>>,
{
    ai::explain(state.db(), client, code, scope).map_err(|error| error.to_string())
}

/// `explain_retry` 的接线主体。
pub fn explain_retry_with<F>(
    state: &AppState,
    code: &str,
    scope: Scope,
    client: F,
) -> Result<Explanation, String>
where
    F: FnOnce() -> AiResult<Arc<dyn ChatClient>>,
{
    ai::explain_retry(state.db(), client, code, scope).map_err(|error| error.to_string())
}

/// `explain_invalidate` 的接线主体。
pub fn explain_invalidate_with(state: &AppState, code: &str, scope: Scope) -> Result<(), String> {
    ai::explain_invalidate(state.db(), code, scope).map_err(|error| error.to_string())
}

/// 把命令层的 `step` 参数转成 scope：1~4 是主流程四步，0 是交易日志页。
fn scope_of(step: u8) -> Result<Scope, String> {
    Scope::from_step(step).ok_or_else(|| format!("未知的步骤编号: {step}"))
}


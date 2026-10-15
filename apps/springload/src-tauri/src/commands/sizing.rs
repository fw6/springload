//! 第 4 步的仓位计算命令。领域规则在 `springload-core::sizing`，这里只做接线。

use crate::state::AppState;
use springload_core::sizing::{self, SizingInput, SizingResult};
use tauri::State;

/// 算仓位与目标位。买入价不高于止损价时返回错误文案。
///
/// 股数按当前市场的最小申报单位向下取整：A 股一手 100 股，美股按整股。
#[tauri::command]
pub fn plan_calc(state: State<'_, AppState>, input: SizingInput) -> Result<SizingResult, String> {
    sizing::size(&input, state.market()).map_err(|e| e.to_string())
}

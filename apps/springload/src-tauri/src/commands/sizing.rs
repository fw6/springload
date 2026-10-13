//! 第 4 步的仓位计算命令。领域规则在 `springload-core::sizing`，这里只做接线。

use springload_core::sizing::{self, SizingInput, SizingResult};

/// 算仓位与目标位。买入价不高于止损价时返回错误文案。
#[tauri::command]
pub fn plan_calc(input: SizingInput) -> Result<SizingResult, String> {
    sizing::size(&input).map_err(|e| e.to_string())
}

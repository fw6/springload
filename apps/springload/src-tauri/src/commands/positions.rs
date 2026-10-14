//! 持仓跟踪的三条命令：列表、刷新最新价、填实际成交价平仓。
//!
//! 领域规则在 `springload_core::alerts`，这里只做接线与系统通知。
//!
//! Tauri 把没有 `async` 的命令放在主线程上跑，主线程一停整个窗口就不再重绘、
//! 也不再接收点击。读持仓要算交易日历、抓快照要走网络，两者都得挪到线程池里。

use crate::state::AppState;
use springload_core::alerts::{self, PositionView};
use springload_core::storage::trade_log::TradeLogEntry;
use tauri::{AppHandle, State};
use tauri_plugin_notification::NotificationExt;

/// 一条提示的通知标题。
const NOTIFICATION_TITLE: &str = "蓄势 Springload";

/// 读全部未平仓持仓。最新价用已入库的日线兜底。
#[tauri::command]
pub async fn positions_get(state: State<'_, AppState>) -> Result<Vec<PositionView>, String> {
    let db = state.db().clone();
    tauri::async_runtime::spawn_blocking(move || {
        alerts::list_positions(&db).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

/// 抓一次实时快照刷新最新价，按刷新结果发 macOS 系统通知。
#[tauri::command]
pub async fn positions_refresh_quotes(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<PositionView>, String> {
    let db = state.db().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let codes: Vec<String> = {
            let conn = db.conn().map_err(|error| error.to_string())?;
            springload_core::storage::position::list_open(&conn)
                .map_err(|error| error.to_string())?
                .into_iter()
                .map(|row| row.code)
                .collect()
        };
        if codes.is_empty() {
            return Ok(Vec::new());
        }

        let quotes = alerts::fetch_quotes(&codes).map_err(|error| error.to_string())?;
        let views = alerts::refresh_positions(&db, &quotes).map_err(|error| error.to_string())?;
        for view in &views {
            for alert in &view.alerts {
                let body = format!(
                    "{} {}：{}（信号日 {}，可执行日 {}）",
                    view.code,
                    view.name.as_deref().unwrap_or(""),
                    alert.message,
                    alert.signal_date,
                    alert.executable_date
                );
                // 通知失败不影响已经刷新好的持仓视图，把原因打到标准错误。
                if let Err(error) = app
                    .notification()
                    .builder()
                    .title(NOTIFICATION_TITLE)
                    .body(body)
                    .show()
                {
                    eprintln!("发送持仓提示通知失败: {error}");
                }
            }
        }
        Ok(views)
    })
    .await
    .map_err(|error| error.to_string())?
}

/// 填实际成交价，平掉一条持仓并写出场记录。
#[tauri::command]
pub fn position_exit(
    state: State<'_, AppState>,
    id: String,
    exit_price: f64,
    exit_date: String,
) -> Result<TradeLogEntry, String> {
    let mut conn = state.db().conn().map_err(|error| error.to_string())?;
    alerts::close_position(&mut conn, &id, exit_price, &exit_date).map_err(|error| error.to_string())
}

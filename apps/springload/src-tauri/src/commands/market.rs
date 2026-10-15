//! 行情命令：股票池与日线同步、同步状态、股票列表与 K 线读取。
//!
//! 领域规则在 `springload-core`，这里只做参数转换、数据库连接与错误文案。

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use springload_core::backfill::{self, SyncOptions, SyncReport, SyncStatus};
use springload_core::market::{Bar, Market};
use springload_core::storage::{bar, bar_minute, bar_sync_state, stock};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

/// 同步进度事件名。
pub const SYNC_PROGRESS_EVENT: &str = "sync://progress";

/// `stock_list` 返回的一行。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StockRow {
    pub code: String,
    pub name: String,
    pub market: String,
    pub board: String,
    pub sector: Option<String>,
    pub listed_at: Option<String>,
    pub is_st: bool,
    pub updated_at: String,
}

impl From<stock::Stock> for StockRow {
    fn from(row: stock::Stock) -> Self {
        StockRow {
            code: row.code,
            name: row.name,
            market: row.market,
            board: row.board,
            sector: row.sector,
            listed_at: row.listed_at,
            is_st: row.is_st,
            updated_at: row.updated_at,
        }
    }
}

/// 抓当前市场的股票列表并回补日线。同步过程中通过 `sync://progress` 事件推送进度。
#[tauri::command]
pub async fn sync_market<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    full: bool,
) -> Result<SyncReport, String> {
    let db = state.db();
    let market = state.market();
    let source = state.source();
    tauri::async_runtime::spawn_blocking(move || {
        let sink: backfill::ProgressSink = Arc::new(move |status: SyncStatus| {
            backfill::publish(&status);
            let _ = app.emit(SYNC_PROGRESS_EVENT, status);
        });
        let options = SyncOptions::new(full);
        backfill::sync_market(&db, source.as_ref(), market, &options, &sink)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

/// 当前同步状态与最近同步时间。
#[tauri::command]
pub fn sync_status(state: State<'_, AppState>) -> Result<SyncStatus, String> {
    let conn = state.db().conn().map_err(|error| error.to_string())?;
    let last_sync_at =
        bar_sync_state::latest_fetched_at(&conn).map_err(|error| error.to_string())?;
    Ok(backfill::snapshot(last_sync_at))
}

/// 股票池列表。
#[tauri::command]
pub fn stock_list(state: State<'_, AppState>) -> Result<Vec<StockRow>, String> {
    let conn = state.db().conn().map_err(|error| error.to_string())?;
    let rows = stock::list(&conn).map_err(|error| error.to_string())?;
    Ok(rows.into_iter().map(StockRow::from).collect())
}

/// 读一只股票的 K 线。`period` 取 `day` 或分钟周期字符串。
#[tauri::command]
pub fn bars_get(
    state: State<'_, AppState>,
    code: String,
    period: String,
) -> Result<Vec<Bar>, String> {
    let conn = state.db().conn().map_err(|error| error.to_string())?;
    if period == "day" {
        let rows = bar::list_for_code(&conn, &code).map_err(|error| error.to_string())?;
        return Ok(rows.into_iter().map(Bar::from).collect());
    }
    let minutes: u32 = period
        .parse()
        .map_err(|_| format!("不支持的周期 {period}，只接受 day 或分钟数"))?;
    let rows = bar_minute::list(&conn, &code, minutes).map_err(|error| error.to_string())?;
    Ok(rows.into_iter().map(Bar::from).collect())
}

/// 起收盘后自动回补的定时任务。由 `app.rs` 的装配过程调用一次。
///
/// 每轮都重新问一次当前是哪个市场，切到美股之后这一轮就跟着切过去。收盘时间按
/// 市场当地时间算：A 股 15:00，美股 16:00。
pub fn start_scheduler<R: tauri::Runtime>(app: AppHandle<R>) {
    std::thread::spawn(move || {
        let mut last_run: Option<(Market, chrono::NaiveDate)> = None;
        loop {
            let state = app.state::<AppState>();
            let market = state.market();
            let now = market.local_now();
            let previous = last_run
                .as_ref()
                .filter(|(ran_market, _)| *ran_market == market)
                .map(|(_, day)| *day);
            if backfill::should_run_auto_sync(now.time(), market.close_time(), now.date(), previous) {
                let db = state.db();
                let source = state.source();
                let handle = app.clone();
                let sink: backfill::ProgressSink = Arc::new(move |status: SyncStatus| {
                    backfill::publish(&status);
                    let _ = handle.emit(SYNC_PROGRESS_EVENT, status);
                });
                let options = SyncOptions::new(false);
                if backfill::sync_market(&db, source.as_ref(), market, &options, &sink).is_ok() {
                    last_run = Some((market, now.date()));
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(60));
        }
    });
}

//! 行情命令：股票池与日线同步、同步状态、股票列表与 K 线读取。
//!
//! 领域规则在 `springload-core`，这里只做参数转换、数据库连接与错误文案。

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use springload_core::backfill::{self, HttpFetch, SyncOptions, SyncReport, SyncStatus};
use springload_core::market::Bar;
use springload_core::storage::{bar, bar_sync_state, stock};
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

/// 抓股票列表并回补日线。同步过程中通过 `sync://progress` 事件推送进度。
#[tauri::command]
pub async fn sync_market<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    full: bool,
) -> Result<SyncReport, String> {
    let db = state.db().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let fetch = HttpFetch::new().map_err(|error| error.to_string())?;
        let sink: backfill::ProgressSink = Arc::new(move |status: SyncStatus| {
            backfill::publish(&status);
            let _ = app.emit(SYNC_PROGRESS_EVENT, status);
        });
        let options = SyncOptions::new(full);
        backfill::sync_market(&db, &fetch, &options, &sink).map_err(|error| error.to_string())
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
    read_minute_bars(&conn, &code, minutes)
}

/// 读 `bar_minute` 表。分钟线由 #17 写入，这里按已冻结的命令签名读取。
fn read_minute_bars(
    conn: &springload_core::storage::DbConn,
    code: &str,
    period: u32,
) -> Result<Vec<Bar>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT code, ts, open, high, low, close, volume, amount
             FROM bar_minute WHERE code = ?1 AND period = ?2 ORDER BY ts",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![code, period], |row| {
            Ok(Bar {
                code: row.get(0)?,
                date: row.get(1)?,
                open: row.get(2)?,
                high: row.get(3)?,
                low: row.get(4)?,
                close: row.get(5)?,
                volume: row.get(6)?,
                amount: row.get(7)?,
            })
        })
        .map_err(|error| error.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|error| error.to_string())?);
    }
    Ok(out)
}

/// 起收盘后自动回补的定时任务。由 `app.rs` 的装配过程调用一次。
pub fn start_scheduler<R: tauri::Runtime>(app: AppHandle<R>) {
    let db = app.state::<AppState>().db().clone();
    let sink: backfill::ProgressSink = Arc::new(move |status: SyncStatus| {
        backfill::publish(&status);
        let _ = app.emit(SYNC_PROGRESS_EVENT, status);
    });
    backfill::spawn_daily_scheduler(db, sink);
}

//! 筛选与观察名单的六个命令，以及收盘后自动筛选的定时任务。
//!
//! 领域规则在 `springload-core::{screening,flow}`，这里只做参数转换、数据库连接与错误文案。

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use springload_core::flow::{self, StepUnlock};
use springload_core::screening::{self, ScreenParams, ScreenReport};
use springload_core::storage::watchlist::{self, WatchlistItem};
use springload_core::storage::{self, stock};
use tauri::{AppHandle, Manager, State};

/// `watchlist_get` 返回的一行。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchlistRow {
    pub code: String,
    pub name: String,
    pub added_at: String,
    pub c1: bool,
    pub c2: bool,
    pub c3: bool,
    pub c4: bool,
    pub chg_1m: Option<f64>,
    pub chg_3m: Option<f64>,
    pub chg_6m: Option<f64>,
    pub rank_pct: Option<f64>,
    pub avg_amount: Option<f64>,
    pub convergence: Option<f64>,
}

impl From<watchlist::WatchlistEntry> for WatchlistRow {
    fn from(entry: watchlist::WatchlistEntry) -> Self {
        let item = entry.item;
        WatchlistRow {
            code: item.code,
            name: entry.name,
            added_at: item.added_at,
            c1: item.c1,
            c2: item.c2,
            c3: item.c3,
            c4: item.c4,
            chg_1m: item.chg_1m,
            chg_3m: item.chg_3m,
            chg_6m: item.chg_6m,
            rank_pct: item.rank_pct,
            avg_amount: item.avg_amount,
            convergence: item.convergence,
        }
    }
}

/// 跑一遍筛选，把入选写进观察名单并返回报告。
#[tauri::command]
pub async fn screen_stocks(state: State<'_, AppState>) -> Result<ScreenReport, String> {
    let db = state.db().clone();
    let settings = state.settings_get().map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let params = ScreenParams::from_settings(&settings);
        screening::run_screen(&db, &params).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

/// 读最近一次筛选报告；还没跑过时返回 `null`。
#[tauri::command]
pub fn screen_last(state: State<'_, AppState>) -> Result<Option<ScreenReport>, String> {
    let conn = state.db().conn().map_err(|error| error.to_string())?;
    screening::last_report(&conn).map_err(|error| error.to_string())
}

/// 观察名单列表。
#[tauri::command]
pub fn watchlist_get(state: State<'_, AppState>) -> Result<Vec<WatchlistRow>, String> {
    let conn = state.db().conn().map_err(|error| error.to_string())?;
    let rows = watchlist::list_with_names(&conn).map_err(|error| error.to_string())?;
    Ok(rows.into_iter().map(WatchlistRow::from).collect())
}

/// 手工把一只股票加进观察名单。判定取值取最近一次筛选结果，筛不到就用空判定。
#[tauri::command]
pub fn watchlist_add(state: State<'_, AppState>, code: String) -> Result<(), String> {
    let conn = state.db().conn().map_err(|error| error.to_string())?;
    if stock::get(&conn, &code)
        .map_err(|error| error.to_string())?
        .is_none()
    {
        return Err(format!("股票 {code} 不在股票池里"));
    }
    if watchlist::get(&conn, &code)
        .map_err(|error| error.to_string())?
        .is_some()
    {
        return Ok(());
    }

    let added_at = storage::now();
    let report = screening::last_report(&conn).map_err(|error| error.to_string())?;
    let judged = report.as_ref().and_then(|report| {
        report
            .picked
            .iter()
            .chain(report.rejected.iter())
            .find(|row| row.code == code)
            .map(|row| (row, report.ran_at.as_str()))
    });
    let item = match judged {
        Some((row, ran_at)) => row.to_watchlist_item(&added_at, ran_at),
        None => WatchlistItem {
            code,
            added_at,
            c1: false,
            c2: false,
            c3: false,
            c4: false,
            chg_1m: None,
            chg_3m: None,
            chg_6m: None,
            rank_pct: None,
            avg_amount: None,
            convergence: None,
            screen_run_at: String::new(),
        },
    };
    watchlist::upsert(&conn, &item).map_err(|error| error.to_string())
}

/// 从观察名单里移除一只股票。
#[tauri::command]
pub fn watchlist_remove(state: State<'_, AppState>, code: String) -> Result<(), String> {
    let conn = state.db().conn().map_err(|error| error.to_string())?;
    watchlist::remove(&conn, &code).map_err(|error| error.to_string())?;
    Ok(())
}

/// 某只股票当前已解锁的步骤与阻止原因。
#[tauri::command]
pub fn step_unlock(state: State<'_, AppState>, code: String) -> Result<StepUnlock, String> {
    let conn = state.db().conn().map_err(|error| error.to_string())?;
    flow::step_unlock(&conn, &code).map_err(|error| error.to_string())
}

/// 起收盘后自动筛选的定时任务。由 `app.rs` 的装配过程调用一次。
pub fn start_scheduler<R: tauri::Runtime>(app: AppHandle<R>) {
    let state = app.state::<AppState>();
    let db = state.db().clone();
    let config_path = state.config_path().to_path_buf();
    screening::spawn_daily_scheduler(db, config_path);
}

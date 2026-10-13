//! 第 3 步的命令：分钟线刷新与两级触发判定。
//!
//! 领域规则在 `springload_core::{minute, signal}`，这里只做参数转换、数据库连接与
//! 错误文案。

use crate::commands::zone::SupportZoneView;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use springload_core::minute::{self, HttpMinuteFetch, PollReport, PERIOD_1M};
use springload_core::signal::{self, Stage};
use springload_core::storage::support_zone::SupportZone;
use tauri::{AppHandle, Emitter, Manager, State};

/// 一轮轮询结束的事件名。
pub const POLL_TICK_EVENT: &str = "poll://tick";

/// 触发点在分钟图上的位置。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalTriggerView {
    /// 触发那根分钟线的时间，`YYYY-MM-DD HH:MM`。
    pub ts: String,
    /// 触发那根分钟线的最高价。
    pub broken_high: f64,
    /// 前一根分钟线的最高价。
    pub pre_high: f64,
}

/// `signal_status` 与 `signal_evaluate` 的返回。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalStatusView {
    /// `waiting` | `triggered` | `failed`。
    pub stage: String,
    pub zone_id: Option<String>,
    pub zone: Option<SupportZoneView>,
    /// 当前卡在哪一级的可展示说明。
    pub detail: String,
    /// 已落库的触发点；还没触发过时为 `None`。
    pub trigger: Option<SignalTriggerView>,
}

impl SignalStatusView {
    /// 组装对外状态。
    ///
    /// 触发点优先取本次新判定出来的那条，没有就回落到已落库的那条，这样反复刷新
    /// 面板不会让分钟图上的标记闪没。
    fn build(
        conn: &springload_core::storage::DbConn,
        code: &str,
        status: signal::SignalStatus,
    ) -> Result<Self, String> {
        let stored = springload_core::storage::signal::latest_for_code(conn, code)
            .map_err(|error| error.to_string())?
            .filter(|row| row.status == signal::STATUS_TRIGGERED);
        let trigger = match status.trigger {
            Some(trigger) => Some(SignalTriggerView {
                ts: trigger.ts,
                broken_high: trigger.broken_high,
                pre_high: trigger.pre_high,
            }),
            None => stored.map(|row| SignalTriggerView {
                ts: row.triggered_at,
                broken_high: row.candle_high_broken,
                pre_high: row.pre_candle_high,
            }),
        };
        Ok(Self {
            stage: match status.stage {
                Stage::Waiting => "waiting",
                Stage::Triggered => "triggered",
                Stage::Failed => "failed",
            }
            .to_string(),
            zone_id: status.zone.as_ref().map(|zone| zone.id.clone()),
            zone: status.zone.map(SupportZoneView::from),
            detail: status.detail,
            trigger,
        })
    }
}

/// 看某只股票当前卡在哪一级。只读已入库的分钟线，不抓网络。
#[tauri::command]
pub fn signal_status(state: State<'_, AppState>, code: String) -> Result<SignalStatusView, String> {
    let conn = state.db().conn().map_err(|error| error.to_string())?;
    let status = signal::evaluate(&conn, &code).map_err(|error| error.to_string())?;
    SignalStatusView::build(&conn, &code, status)
}

/// 抓一次这只股票的当日 1 分钟线，再判定触发；触发时写进 `signal` 表。
#[tauri::command]
pub fn signal_evaluate(
    state: State<'_, AppState>,
    code: String,
) -> Result<SignalStatusView, String> {
    let mut conn = state.db().conn().map_err(|error| error.to_string())?;
    let fetch = HttpMinuteFetch::new().map_err(|error| error.to_string())?;
    minute::refresh_minute_bars(&mut conn, &fetch, &code, PERIOD_1M)
        .map_err(|error| error.to_string())?;
    let status = signal::evaluate(&conn, &code).map_err(|error| error.to_string())?;
    if let Some(trigger) = &status.trigger {
        signal::record_trigger(&conn, &code, status.zone.as_ref(), trigger)
            .map_err(|error| error.to_string())?;
    }
    SignalStatusView::build(&conn, &code, status)
}

/// 手动跑一轮轮询：刷新等待触发的股票与持仓的当日 1 分钟线，判定触发。
#[tauri::command]
pub fn poll_tick(state: State<'_, AppState>) -> Result<PollReport, String> {
    let mut conn = state.db().conn().map_err(|error| error.to_string())?;
    let fetch = HttpMinuteFetch::new().map_err(|error| error.to_string())?;
    minute::poll_once(&mut conn, &fetch).map_err(|error| error.to_string())
}

/// 启动每分钟一次的轮询，每轮结束推一条 `poll://tick`。
pub fn start_scheduler<R: tauri::Runtime>(app: AppHandle<R>) {
    let db = app.state::<AppState>().db().clone();
    let sink: minute::PollSink = std::sync::Arc::new(move |report: PollReport| {
        let _ = app.emit(POLL_TICK_EVENT, report);
    });
    minute::spawn_poll_scheduler(db, sink);
}

/// 把一条支撑区行转成对外形状，供测试与命令层复用。
pub fn zone_view(zone: &SupportZone) -> SupportZoneView {
    SupportZoneView::from(zone.clone())
}

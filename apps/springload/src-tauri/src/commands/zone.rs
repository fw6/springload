//! 支撑区命令：标注的读写与三个参考价的计算。
//!
//! 置信度的计算在 `springload_core::support`，这里只做参数转换、数据库连接与
//! 错误文案。

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use springload_core::storage::support_zone::{self, SupportZone};
use springload_core::storage::{bar, DbConn};
use springload_core::support;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::State;

/// 允许的标注类型，与 `support_zone.kind` 列一致。
const KINDS: [&str; 4] = ["hline", "support", "gap", "avwap_start"];

/// `support_zones_get` 与 `support_zone_save` 返回的一条标注。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportZoneView {
    pub id: String,
    pub code: String,
    pub kind: String,
    pub low: f64,
    pub high: f64,
    pub confidence: String,
    pub spread: Option<f64>,
}

impl From<SupportZone> for SupportZoneView {
    fn from(zone: SupportZone) -> Self {
        Self {
            id: zone.id,
            code: zone.code,
            kind: zone.kind,
            low: zone.low,
            high: zone.high,
            confidence: zone.confidence,
            spread: zone.spread,
        }
    }
}

/// `support_zone_save` 的入参。`id` 为空时由服务端生成。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportZoneInput {
    pub id: Option<String>,
    pub code: String,
    pub kind: String,
    pub low: f64,
    pub high: f64,
    pub anchor_date: Option<String>,
    pub avwap_start_date: Option<String>,
}

/// 列出某只股票的标注。
#[tauri::command]
pub fn support_zones_get(
    state: State<'_, AppState>,
    code: String,
) -> Result<Vec<SupportZoneView>, String> {
    let conn = state.db().conn().map_err(|error| error.to_string())?;
    let rows = support_zone::list_for_code(&conn, &code).map_err(|error| error.to_string())?;
    Ok(rows.into_iter().map(SupportZoneView::from).collect())
}

/// 写入或更新一条标注，按当前参考价重算置信度。
#[tauri::command]
pub fn support_zone_save(
    state: State<'_, AppState>,
    zone: SupportZoneInput,
) -> Result<SupportZoneView, String> {
    let conn = state.db().conn().map_err(|error| error.to_string())?;
    let saved = save_zone(&conn, &zone)?;
    Ok(SupportZoneView::from(saved))
}

/// 删除一条标注。
#[tauri::command]
pub fn support_zone_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let conn = state.db().conn().map_err(|error| error.to_string())?;
    support_zone::delete(&conn, &id).map_err(|error| error.to_string())?;
    Ok(())
}

/// 三个参考价：EMA21、AVWAP 与前期高点。`avwapStartDate` 为空时用默认起点。
#[tauri::command]
pub fn support_refs(
    state: State<'_, AppState>,
    code: String,
    avwap_start_date: Option<String>,
) -> Result<support::ZoneRefs, String> {
    let conn = state.db().conn().map_err(|error| error.to_string())?;
    let bars = bar::list_for_code(&conn, &code).map_err(|error| error.to_string())?;
    support::reference_prices(&bars, avwap_start_date.as_deref())
        .map_err(|error| error.to_string())
}

/// 写入主体：读日线、算参考价与置信度、落到 `support_zone` 行。
fn save_zone(conn: &DbConn, input: &SupportZoneInput) -> Result<SupportZone, String> {
    if !KINDS.contains(&input.kind.as_str()) {
        return Err(format!("未知的标注类型 {}", input.kind));
    }
    if !input.low.is_finite() || !input.high.is_finite() {
        return Err("标注价格非法".to_string());
    }
    let (low, high) = ordered(input.low, input.high);
    let bars = bar::list_for_code(conn, &input.code).map_err(|error| error.to_string())?;
    let snapshot = support::snapshot(&bars, input.avwap_start_date.as_deref())
        .map_err(|error| error.to_string())?;
    let evaluation = support::evaluate_box(&snapshot.refs(), low, high);

    let id = match &input.id {
        Some(id) if !id.is_empty() => id.clone(),
        _ => generate_id(),
    };
    let created_at = support_zone::get(conn, &id)
        .map_err(|error| error.to_string())?
        .map(|existing| existing.created_at)
        .unwrap_or_else(springload_core::storage::now);
    let is_gap = input.kind == "gap";

    let zone = SupportZone {
        id,
        code: input.code.clone(),
        kind: input.kind.clone(),
        low,
        high,
        anchor_date: input.anchor_date.clone(),
        ema9: snapshot.ema9,
        ema21: snapshot.ema21,
        ema50: snapshot.ema50,
        avwap_start_date: input.avwap_start_date.clone(),
        avwap_price: snapshot.avwap,
        prev_high: snapshot.prev_high,
        prev_low: snapshot.prev_low,
        gap_top: is_gap.then_some(high),
        gap_bottom: is_gap.then_some(low),
        confidence: evaluation.confidence.as_str().to_string(),
        spread: evaluation.spread,
        created_at,
    };
    support_zone::upsert(conn, &zone).map_err(|error| error.to_string())?;
    Ok(zone)
}

/// 把两个价格排成下沿、上沿。
fn ordered(low: f64, high: f64) -> (f64, f64) {
    if low <= high {
        (low, high)
    } else {
        (high, low)
    }
}

/// 生成一条新标注的 id：毫秒时间戳加进程内自增序号。
fn generate_id() -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    format!("zone-{}-{seq}", chrono::Utc::now().timestamp_millis())
}

//! `support_zone` 表的读写：支撑区与水平线等标注。

use crate::error::Result;
use rusqlite::{params, Connection, OptionalExtension, Row};

/// 一条标注。`kind` 取 `hline` | `support` | `gap` | `avwap_start`。
#[derive(Debug, Clone, PartialEq)]
pub struct SupportZone {
    pub id: String,
    pub code: String,
    pub kind: String,
    pub low: f64,
    pub high: f64,
    pub anchor_date: Option<String>,
    pub ema9: Option<f64>,
    pub ema21: Option<f64>,
    pub ema50: Option<f64>,
    pub avwap_start_date: Option<String>,
    pub avwap_price: Option<f64>,
    pub prev_high: Option<f64>,
    pub prev_low: Option<f64>,
    pub gap_top: Option<f64>,
    pub gap_bottom: Option<f64>,
    /// `高` | `中` | `低`。
    pub confidence: String,
    /// 三个参考价的离散度。
    pub spread: Option<f64>,
    pub created_at: String,
}

impl SupportZone {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            code: row.get("code")?,
            kind: row.get("kind")?,
            low: row.get("low")?,
            high: row.get("high")?,
            anchor_date: row.get("anchor_date")?,
            ema9: row.get("ema9")?,
            ema21: row.get("ema21")?,
            ema50: row.get("ema50")?,
            avwap_start_date: row.get("avwap_start_date")?,
            avwap_price: row.get("avwap_price")?,
            prev_high: row.get("prev_high")?,
            prev_low: row.get("prev_low")?,
            gap_top: row.get("gap_top")?,
            gap_bottom: row.get("gap_bottom")?,
            confidence: row.get("confidence")?,
            spread: row.get("spread")?,
            created_at: row.get("created_at")?,
        })
    }
}

/// 写入或更新一条标注。
pub fn upsert(conn: &Connection, zone: &SupportZone) -> Result<()> {
    conn.execute(
        "INSERT INTO support_zone
           (id, code, kind, low, high, anchor_date, ema9, ema21, ema50,
            avwap_start_date, avwap_price, prev_high, prev_low, gap_top, gap_bottom,
            confidence, spread, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
         ON CONFLICT(id) DO UPDATE SET
           code = excluded.code,
           kind = excluded.kind,
           low = excluded.low,
           high = excluded.high,
           anchor_date = excluded.anchor_date,
           ema9 = excluded.ema9,
           ema21 = excluded.ema21,
           ema50 = excluded.ema50,
           avwap_start_date = excluded.avwap_start_date,
           avwap_price = excluded.avwap_price,
           prev_high = excluded.prev_high,
           prev_low = excluded.prev_low,
           gap_top = excluded.gap_top,
           gap_bottom = excluded.gap_bottom,
           confidence = excluded.confidence,
           spread = excluded.spread,
           created_at = excluded.created_at",
        params![
            zone.id,
            zone.code,
            zone.kind,
            zone.low,
            zone.high,
            zone.anchor_date,
            zone.ema9,
            zone.ema21,
            zone.ema50,
            zone.avwap_start_date,
            zone.avwap_price,
            zone.prev_high,
            zone.prev_low,
            zone.gap_top,
            zone.gap_bottom,
            zone.confidence,
            zone.spread,
            zone.created_at,
        ],
    )?;
    Ok(())
}

/// 读一条标注。
pub fn get(conn: &Connection, id: &str) -> Result<Option<SupportZone>> {
    let zone = conn
        .query_row("SELECT * FROM support_zone WHERE id = ?1", params![id], SupportZone::from_row)
        .optional()?;
    Ok(zone)
}

/// 按创建时间列出某只股票的标注。
pub fn list_for_code(conn: &Connection, code: &str) -> Result<Vec<SupportZone>> {
    let mut stmt =
        conn.prepare("SELECT * FROM support_zone WHERE code = ?1 ORDER BY created_at, id")?;
    let rows = stmt.query_map(params![code], SupportZone::from_row)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// 删除一条标注。
pub fn delete(conn: &Connection, id: &str) -> Result<usize> {
    let n = conn.execute("DELETE FROM support_zone WHERE id = ?1", params![id])?;
    Ok(n)
}

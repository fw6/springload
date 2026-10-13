//! 全部建表 SQL 与后续的结构变更。
//!
//! 迁移是一组按序号排列的 SQL 字符串，从空库一次跑到最新版本；版本号记在
//! SQLite 的 `user_version` 上，已应用的迁移不会重复执行，所以重复运行不报错。
//! 已发布的条目不允许改动，结构变更一律往后追加新条目。

use crate::error::Result;
use rusqlite::Connection;

/// 序号 1：初始表结构。
const V1_INITIAL_SCHEMA: &str = r#"
CREATE TABLE stock (
  code TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  market TEXT NOT NULL,
  board TEXT NOT NULL,
  sector TEXT,
  listed_at TEXT,
  is_st INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL
);

CREATE TABLE bar (
  code TEXT NOT NULL,
  date TEXT NOT NULL,
  open REAL NOT NULL, high REAL NOT NULL, low REAL NOT NULL, close REAL NOT NULL,
  volume REAL NOT NULL, amount REAL NOT NULL,
  PRIMARY KEY (code, date)
);
CREATE INDEX idx_bar_code_date ON bar(code, date);

CREATE TABLE bar_minute (
  code TEXT NOT NULL,
  ts TEXT NOT NULL,
  period INTEGER NOT NULL,
  open REAL NOT NULL, high REAL NOT NULL, low REAL NOT NULL, close REAL NOT NULL,
  volume REAL NOT NULL, amount REAL NOT NULL,
  PRIMARY KEY (code, period, ts)
);
CREATE INDEX idx_bar_minute_code_period_ts ON bar_minute(code, period, ts);

CREATE TABLE bar_sync_state (
  code TEXT PRIMARY KEY,
  last_bar_date TEXT,
  fetched_at TEXT,
  dktotal INTEGER,
  tail_qfq TEXT,
  updated_at TEXT NOT NULL
);

CREATE TABLE corporate_action (
  code TEXT NOT NULL,
  ex_date TEXT NOT NULL,
  dividend_per_share REAL NOT NULL DEFAULT 0,
  split_ratio REAL NOT NULL DEFAULT 0,
  pre_close REAL,
  recorded_at TEXT NOT NULL,
  PRIMARY KEY (code, ex_date)
);

CREATE TABLE watchlist_item (
  code TEXT PRIMARY KEY,
  added_at TEXT NOT NULL,
  c1 INTEGER NOT NULL, c2 INTEGER NOT NULL, c3 INTEGER NOT NULL, c4 INTEGER NOT NULL,
  chg_1m REAL, chg_3m REAL, chg_6m REAL,
  rank_pct REAL, avg_amount REAL, convergence REAL,
  screen_run_at TEXT NOT NULL
);

CREATE TABLE support_zone (
  id TEXT PRIMARY KEY,
  code TEXT NOT NULL,
  kind TEXT NOT NULL,
  low REAL NOT NULL, high REAL NOT NULL,
  anchor_date TEXT,
  ema9 REAL, ema21 REAL, ema50 REAL,
  avwap_start_date TEXT, avwap_price REAL,
  prev_high REAL, prev_low REAL,
  gap_top REAL, gap_bottom REAL,
  confidence TEXT NOT NULL,
  spread REAL,
  created_at TEXT NOT NULL
);
CREATE INDEX idx_support_zone_code ON support_zone(code);

CREATE TABLE signal (
  id TEXT PRIMARY KEY,
  code TEXT NOT NULL,
  triggered_at TEXT NOT NULL,
  timeframe TEXT NOT NULL,
  candle_high_broken REAL NOT NULL,
  pre_candle_high REAL NOT NULL,
  zone_id TEXT,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE INDEX idx_signal_code ON signal(code);

CREATE TABLE trade_plan (
  id TEXT PRIMARY KEY,
  code TEXT NOT NULL,
  entry REAL NOT NULL, stop REAL NOT NULL, account REAL NOT NULL, risk_pct REAL NOT NULL,
  per_r REAL NOT NULL, shares INTEGER NOT NULL, capital REAL NOT NULL,
  pos_pct REAL NOT NULL, stop_distance REAL NOT NULL,
  t3 REAL NOT NULL, t5 REAL NOT NULL,
  status TEXT NOT NULL,
  zone_id TEXT, signal_id TEXT,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE INDEX idx_trade_plan_code ON trade_plan(code);

CREATE TABLE position (
  id TEXT PRIMARY KEY,
  plan_id TEXT NOT NULL,
  code TEXT NOT NULL,
  entry_date TEXT NOT NULL,
  entry_price REAL NOT NULL,
  quantity INTEGER NOT NULL,
  current_stop REAL NOT NULL,
  follow_ema INTEGER NOT NULL DEFAULT 1,
  exit_price REAL, exit_date TEXT,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE INDEX idx_position_code ON position(code);

CREATE TABLE trade_log (
  id TEXT PRIMARY KEY,
  position_id TEXT,
  code TEXT NOT NULL, name TEXT,
  entry_date TEXT NOT NULL, entry_price REAL NOT NULL, stop_price REAL NOT NULL,
  exit_date TEXT, exit_price REAL,
  quantity INTEGER NOT NULL,
  per_r REAL, r_multiple REAL, pnl REAL,
  fees REAL NOT NULL DEFAULT 0,
  note TEXT,
  source TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE INDEX idx_trade_log_code ON trade_log(code);

CREATE TABLE setting (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
"#;

/// 序号 2：观察名单区分筛选自动入选与手工补充。
///
/// 手工补充的行（`manual = 1`）不参与筛选的整表替换。
const V2_WATCHLIST_MANUAL: &str = r#"
ALTER TABLE watchlist_item ADD COLUMN manual INTEGER NOT NULL DEFAULT 0;
"#;

/// 按序号排列的迁移列表。新增迁移只往后追加，不改动已发布的条目。
pub const MIGRATIONS: &[&str] = &[V1_INITIAL_SCHEMA, V2_WATCHLIST_MANUAL];

/// 从当前版本跑到最新版本。已应用的迁移会跳过。
pub fn run(conn: &mut Connection) -> Result<()> {
    let current: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    for (index, sql) in MIGRATIONS.iter().enumerate() {
        let version = (index + 1) as u32;
        if version <= current {
            continue;
        }
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
    }
    Ok(())
}

/// 当前已应用到的迁移版本。
pub fn version(conn: &Connection) -> Result<u32> {
    let current: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    Ok(current)
}

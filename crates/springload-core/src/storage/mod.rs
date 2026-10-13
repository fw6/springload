//! SQLite 存储：连接池、迁移与各表的读写。

pub mod bar;
pub mod bar_minute;
pub mod bar_sync_state;
pub mod corporate_action;
pub mod migrations;
pub mod position;
pub mod setting;
pub mod signal;
pub mod stock;
pub mod support_zone;
pub mod trade_log;
pub mod trade_plan;
pub mod watchlist;

use crate::error::Result;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::path::Path;

/// 连接池类型。
pub type DbPool = Pool<SqliteConnectionManager>;
/// 从池里取出的一条连接。
pub type DbConn = r2d2::PooledConnection<SqliteConnectionManager>;

/// 数据库句柄：打开时建好目录并跑到最新迁移版本。
#[derive(Clone)]
pub struct Database {
    pool: DbPool,
}

impl Database {
    /// 打开指定路径的数据库文件，建好父目录，并跑到最新迁移版本。
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let manager = SqliteConnectionManager::file(path).with_init(|conn| {
            conn.execute_batch(
                "PRAGMA journal_mode = WAL;\
                 PRAGMA foreign_keys = ON;\
                 PRAGMA busy_timeout = 5000;",
            )
        });
        let pool = Pool::builder().max_size(8).build(manager)?;
        let database = Self { pool };
        database.migrate()?;
        Ok(database)
    }

    /// 取一条连接。
    pub fn conn(&self) -> Result<DbConn> {
        Ok(self.pool.get()?)
    }

    /// 跑到最新迁移版本。重复调用不报错。
    pub fn migrate(&self) -> Result<()> {
        let mut conn = self.conn()?;
        migrations::run(&mut conn)
    }

    /// 当前已应用到的迁移版本。
    pub fn schema_version(&self) -> Result<u32> {
        let conn = self.conn()?;
        migrations::version(&conn)
    }
}

/// 当前 UTC 时刻的 ISO 8601 字符串，供各表的时间戳字段使用。
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

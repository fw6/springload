//! Springload 蓄势的领域核心。
//!
//! 承载配置读写（`config`）、术语文案（`glossary`）、行情数据源（`market`）、
//! 股票池与日线回补（`backfill`）与 SQLite 存储（`storage`）。命令层是这里的薄封装。

pub mod backfill;
pub mod config;
pub mod error;
pub mod glossary;
pub mod market;
pub mod storage;

pub use config::{Settings, SettingsLoad, SettingsPatch};
pub use error::{CoreError, Result};
pub use glossary::{Glossary, StepCopy, Term};
pub use storage::Database;

//! Springload 蓄势的领域核心。
//!
//! 承载配置读写（`config`）、术语文案（`glossary`）、AI 解释（`ai`）与 SQLite
//! 存储（`storage`）。命令层是这里的薄封装。

pub mod ai;
pub mod config;
pub mod error;
pub mod glossary;
pub mod storage;

pub use config::{Settings, SettingsLoad, SettingsPatch};
pub use error::{CoreError, Result};
pub use glossary::{Glossary, StepCopy, Term};
pub use storage::Database;

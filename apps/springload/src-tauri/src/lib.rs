//! Springload 蓄势桌面端后端：命令面（`commands`）、应用装配（`app`）与运行期状态（`state`）。

pub mod app;
pub mod commands;
pub mod state;

pub use app::run;

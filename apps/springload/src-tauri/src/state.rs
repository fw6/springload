//! 应用运行期状态：数据库、配置路径、数据源名称与版本号。

use crate::commands::app::AppInfo;
use springload_core::config::{self, Settings, SettingsPatch};
use springload_core::error::Result;
use springload_core::glossary::{self, Glossary};
use springload_core::storage::{bar_sync_state, Database};
use std::path::{Path, PathBuf};

/// 应用状态。首次装配时建好 `~/.springload/`、数据库与配置文件。
pub struct AppState {
    db: Database,
    config_path: PathBuf,
    data_source: String,
    version: String,
}

impl AppState {
    /// 按约定的数据目录装配状态。
    pub fn load() -> Result<Self> {
        Self::from_data_dir(config::data_dir())
    }

    /// 按指定目录装配状态，供测试使用。
    pub fn from_data_dir(dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&dir)?;
        let db = Database::open(&dir.join(config::DB_FILE_NAME))?;
        let config_path = dir.join(config::CONFIG_FILE_NAME);
        if !config_path.exists() {
            config::save_settings(&config_path, &Settings::default())?;
        }
        Ok(Self {
            db,
            config_path,
            data_source: config::DATA_SOURCE.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }

    /// 版本号、数据源名称与最近同步时间。
    pub fn app_info(&self) -> Result<AppInfo> {
        let conn = self.db.conn()?;
        let last_sync_at = bar_sync_state::latest_fetched_at(&conn)?;
        Ok(AppInfo {
            version: self.version.clone(),
            data_source: self.data_source.clone(),
            last_sync_at,
        })
    }

    /// 十个术语与四步文案。
    pub fn glossary(&self) -> Glossary {
        glossary::glossary()
    }

    /// 读当前配置。非法项回落到默认值，回落情况写到标准错误，不静默吞掉。
    pub fn settings_get(&self) -> Result<Settings> {
        self.load_settings()
    }

    /// 改配置里的一部分字段并写回文件，返回改完的完整配置。
    pub fn settings_update(&self, patch: &SettingsPatch) -> Result<Settings> {
        let next = patch.apply(&self.load_settings()?);
        config::save_settings(&self.config_path, &next)?;
        Ok(next)
    }

    /// 读配置文件，把非法项的警告打到标准错误。
    fn load_settings(&self) -> Result<Settings> {
        let load = config::load_settings(&self.config_path)?;
        for warning in &load.warnings {
            eprintln!("{warning}");
        }
        Ok(load.settings)
    }

    /// 配置文件路径。
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    /// 数据库句柄。
    pub fn db(&self) -> &Database {
        &self.db
    }
}

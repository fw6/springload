//! 应用运行期状态：当前市场的数据库、行情源、配置路径与版本号。
//!
//! 两个市场各用一个库文件，切换市场就是把 [`AppState`] 里的库句柄换成另一个。
//! 库在启动时按配置里的市场打开，切换时按需打开并留着，之后来回切不再重开。

use crate::commands::app::AppInfo;
use springload_core::config::{self, Settings, SettingsPatch};
use springload_core::error::Result;
use springload_core::glossary::{self, Glossary};
use springload_core::market::{self, Market, QuoteSource};
use springload_core::storage::{bar_sync_state, Database};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// 当前市场的那一套东西。
struct Workspace {
    market: Market,
    db: Database,
    source: Arc<dyn QuoteSource>,
}

/// 应用状态。首次装配时建好 `~/.springload/`、数据库与配置文件。
pub struct AppState {
    workspace: Mutex<Workspace>,
    /// 已经打开过的库，按市场留着，来回切换不用重开。
    opened: Mutex<HashMap<Market, Database>>,
    data_dir: PathBuf,
    config_path: PathBuf,
    version: String,
}

impl AppState {
    /// 按约定的数据目录装配状态，市场取配置文件里的那一项。
    pub fn load() -> Result<Self> {
        Self::from_data_dir(config::data_dir())
    }

    /// 按指定目录装配状态，供测试使用。
    pub fn from_data_dir(dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&dir)?;
        let config_path = dir.join(config::CONFIG_FILE_NAME);
        if !config_path.exists() {
            config::save_settings(&config_path, &Settings::default())?;
        }
        let settings = config::load_settings(&config_path)?.settings;
        let market = settings.market;
        let db = Database::open(&dir.join(market.db_file_name()))?;
        let mut opened = HashMap::new();
        opened.insert(market, db.clone());
        Ok(Self {
            workspace: Mutex::new(Workspace {
                market,
                db,
                source: market::source(market),
            }),
            opened: Mutex::new(opened),
            data_dir: dir.clone(),
            config_path,
            version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }

    /// 当前市场。
    pub fn market(&self) -> Market {
        self.workspace.lock().expect("工作区锁").market
    }

    /// 当前市场的数据库句柄。
    pub fn db(&self) -> Database {
        self.workspace.lock().expect("工作区锁").db.clone()
    }

    /// 当前市场的行情数据源。
    pub fn source(&self) -> Arc<dyn QuoteSource> {
        Arc::clone(&self.workspace.lock().expect("工作区锁").source)
    }

    /// 版本号、当前市场、数据源名称与最近同步时间。
    pub fn app_info(&self) -> Result<AppInfo> {
        let market = self.market();
        let conn = self.db().conn()?;
        let last_sync_at = bar_sync_state::latest_fetched_at(&conn)?;
        Ok(AppInfo {
            version: self.version.clone(),
            market: market.as_str().to_string(),
            market_label: market.label().to_string(),
            currency: market.currency().to_string(),
            t_plus_one: market.t_plus_one(),
            data_source: market.data_source().to_string(),
            last_sync_at,
        })
    }

    /// 十个术语与当前市场的四步文案。
    pub fn glossary(&self) -> Glossary {
        glossary::glossary(self.market())
    }

    /// 读当前配置。非法项回落到默认值，回落情况写到标准错误，不静默吞掉。
    pub fn settings_get(&self) -> Result<Settings> {
        self.load_settings()
    }

    /// 改配置里的一部分字段并写回文件，返回改完的完整配置。
    ///
    /// 补丁里带了市场时同时切换工作区；切换先写配置再换库，写失败就什么都不动。
    pub fn settings_update(&self, patch: &SettingsPatch) -> Result<Settings> {
        let next = patch.apply(&self.load_settings()?);
        config::save_settings(&self.config_path, &next)?;
        if next.market != self.market() {
            self.switch_to(next.market)?;
        }
        Ok(next)
    }

    /// 切换到另一个市场，并把当前市场写回配置。
    pub fn set_market(&self, target: Market) -> Result<Settings> {
        self.settings_update(&SettingsPatch {
            market: Some(target),
            ..Default::default()
        })
    }

    /// 换掉工作区里的市场、库与行情源。
    fn switch_to(&self, target: Market) -> Result<()> {
        let db = self.database_for(target)?;
        let mut workspace = self.workspace.lock().expect("工作区锁");
        workspace.market = target;
        workspace.db = db;
        workspace.source = market::source(target);
        Ok(())
    }

    /// 取某个市场的库，没打开过就现在打开并留着。
    fn database_for(&self, target: Market) -> Result<Database> {
        if let Some(existing) = self.opened.lock().expect("已打开库锁").get(&target) {
            return Ok(existing.clone());
        }
        let db = Database::open(&self.data_dir.join(target.db_file_name()))?;
        self.opened
            .lock()
            .expect("已打开库锁")
            .insert(target, db.clone());
        Ok(db)
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
}

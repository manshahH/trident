use std::sync::Arc;

use serde::Serialize;
use specta::Type;

use crate::{db::Db, error::Result};

#[derive(Debug, Serialize, Type)]
pub struct HealthDto {
    pub database_ok: bool,
    pub watcher_last_tick_age_ms: Option<u64>,
    pub platform_error_count: u32,
    pub version: String,
}

#[derive(Clone)]
pub struct AppState {
    database: Arc<Db>,
}

impl AppState {
    pub fn new(database: Db) -> Self {
        Self {
            database: Arc::new(database),
        }
    }

    pub fn health(&self) -> Result<HealthDto> {
        Ok(HealthDto {
            database_ok: self.database.is_healthy()?,
            watcher_last_tick_age_ms: None,
            platform_error_count: 0,
            version: env!("CARGO_PKG_VERSION").to_owned(),
        })
    }

    pub fn database(&self) -> &Db {
        self.database.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use crate::{config::AppPaths, db::Db};

    use super::AppState;

    #[test]
    fn health_reports_the_connected_database_and_absent_watcher() -> crate::error::Result<()> {
        let directory = TempDir::new()?;
        let paths = AppPaths::from_root(directory.path().join("Trident"));
        paths.ensure_directories()?;
        let database = Db::open(&paths.database_path(), 1_700_000_000_000)?;
        let state = AppState::new(database);

        let health = state.health()?;

        assert!(health.database_ok);
        assert_eq!(health.watcher_last_tick_age_ms, None);
        assert_eq!(health.platform_error_count, 0);
        assert_eq!(health.version, env!("CARGO_PKG_VERSION"));
        Ok(())
    }
}

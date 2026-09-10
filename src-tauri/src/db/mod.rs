pub mod migrations;

use std::{path::Path, time::Duration};

use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension};

use crate::error::Result;

pub struct Db {
    connection: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path, applied_at: i64) -> Result<Self> {
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        migrations::apply(&mut connection, applied_at)?;

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn with_connection<T>(
        &self,
        operation: impl FnOnce(&Connection) -> Result<T>,
    ) -> Result<T> {
        let connection = self.connection.lock();
        operation(&connection)
    }

    pub fn is_healthy(&self) -> Result<bool> {
        self.with_connection(|connection| {
            connection.query_row("SELECT 1", [], |row| row.get::<_, i64>(0))?;
            Ok(true)
        })
    }

    pub fn setting_value(&self, key: &str) -> Result<Option<String>> {
        self.with_connection(|connection| {
            connection
                .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                    row.get(0)
                })
                .optional()
                .map_err(Into::into)
        })
    }

    pub fn set_setting_value(&self, key: &str, value: &str) -> Result<()> {
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                (key, value),
            )?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::Db;

    #[test]
    fn opening_a_database_below_a_missing_directory_returns_an_error() -> crate::error::Result<()> {
        let directory = TempDir::new()?;
        let database_path = directory.path().join("missing").join("trident.db");

        let result = Db::open(&database_path, 1_700_000_000_000);

        assert!(matches!(result, Err(crate::error::AppError::Db(_))));
        Ok(())
    }

    #[test]
    fn stores_and_reads_a_setting_value() -> crate::error::Result<()> {
        let directory = TempDir::new()?;
        let database_path = directory.path().join("trident.db");
        let database = Db::open(&database_path, 1_700_000_000_000)?;

        database.set_setting_value("orb.position", "saved")?;

        assert_eq!(
            database.setting_value("orb.position")?,
            Some("saved".to_owned())
        );
        assert_eq!(database.setting_value("missing")?, None);
        Ok(())
    }
}

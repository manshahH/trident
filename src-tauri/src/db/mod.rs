pub mod migrations;

use std::{path::Path, time::Duration};

use parking_lot::Mutex;
use rusqlite::Connection;

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
}

use rusqlite::Connection;

use crate::error::{AppError, Result};

const LATEST_VERSION: i64 = 1;

pub fn apply(connection: &mut Connection, applied_at: i64) -> Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (\
            version INTEGER PRIMARY KEY, \
            applied_at INTEGER NOT NULL\
        );",
    )?;

    let current_version = transaction
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get::<_, Option<i64>>(0)
        })?
        .unwrap_or_default();

    if current_version > LATEST_VERSION {
        return Err(AppError::Migration {
            version: current_version,
            message: "database is newer than this version of Trident".to_owned(),
        });
    }

    for version in (current_version + 1)..=LATEST_VERSION {
        migrate(&transaction, version).map_err(|error| AppError::Migration {
            version,
            message: error.to_string(),
        })?;
        transaction.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
            (version, applied_at),
        )?;
    }

    transaction.commit()?;
    Ok(())
}

fn migrate(connection: &Connection, version: i64) -> rusqlite::Result<()> {
    match version {
        1 => connection.execute_batch(
            "CREATE TABLE threads (\
                id TEXT PRIMARY KEY, \
                title TEXT NOT NULL CHECK (length(trim(title)) > 0), \
                note TEXT NOT NULL DEFAULT '', \
                handoff TEXT NOT NULL DEFAULT '', \
                state TEXT NOT NULL CHECK (state IN ('active','paused','away','closed')), \
                created_at INTEGER NOT NULL, \
                started_at INTEGER NOT NULL, \
                closed_at INTEGER, \
                accumulated_ms INTEGER NOT NULL DEFAULT 0, \
                last_resumed_at INTEGER, \
                source_task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL\
            ); \
            CREATE UNIQUE INDEX idx_threads_single_open ON threads ((1)) WHERE state != 'closed'; \
            CREATE INDEX idx_threads_started ON threads (started_at DESC); \
            CREATE TABLE focus_set (\
                thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE, \
                process_key TEXT NOT NULL, \
                added_at INTEGER NOT NULL, \
                source TEXT NOT NULL CHECK (source IN ('seed','user')), \
                PRIMARY KEY (thread_id, process_key)\
            ); \
            CREATE TABLE interruptions (\
                id TEXT PRIMARY KEY, \
                thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE, \
                process_key TEXT NOT NULL, \
                app_name TEXT NOT NULL, \
                window_title TEXT, \
                started_at INTEGER NOT NULL, \
                ended_at INTEGER, \
                duration_ms INTEGER, \
                status TEXT NOT NULL CHECK (status IN ('open','scratched','dismissed','promoted')), \
                dump_id TEXT REFERENCES dumps(id) ON DELETE SET NULL\
            ); \
            CREATE INDEX idx_interruptions_thread ON interruptions (thread_id, started_at DESC); \
            CREATE TABLE dumps (\
                id TEXT PRIMARY KEY, \
                body TEXT NOT NULL CHECK (length(trim(body)) > 0), \
                origin TEXT NOT NULL CHECK (origin IN ('capture','panel','interruption','handoff')), \
                created_at INTEGER NOT NULL, \
                archived_at INTEGER, \
                task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL\
            ); \
            CREATE INDEX idx_dumps_active ON dumps (created_at DESC) WHERE archived_at IS NULL; \
            CREATE TABLE tasks (\
                id TEXT PRIMARY KEY, \
                title TEXT NOT NULL CHECK (length(trim(title)) > 0), \
                notes TEXT NOT NULL DEFAULT '', \
                created_at INTEGER NOT NULL, \
                done_at INTEGER, \
                sort_key REAL NOT NULL, \
                dump_id TEXT REFERENCES dumps(id) ON DELETE SET NULL\
            ); \
            CREATE INDEX idx_tasks_order ON tasks (done_at, sort_key); \
            CREATE TABLE app_ignores (\
                process_key TEXT PRIMARY KEY, \
                app_name TEXT NOT NULL, \
                added_at INTEGER NOT NULL\
            ); \
            CREATE TABLE settings (\
                key TEXT PRIMARY KEY, \
                value TEXT NOT NULL\
            );",
        ),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::apply;

    #[test]
    fn migrates_the_schema_and_rejects_a_second_open_thread() -> crate::error::Result<()> {
        let mut connection = Connection::open_in_memory()?;

        apply(&mut connection, 1_700_000_000_000)?;
        apply(&mut connection, 1_700_000_000_001)?;

        let version: i64 =
            connection.query_row("SELECT version FROM schema_migrations", [], |row| {
                row.get(0)
            })?;
        assert_eq!(version, 1);

        connection.execute(
            "INSERT INTO threads (id, title, state, created_at, started_at) \
             VALUES ('first', 'First thread', 'active', 1, 1)",
            [],
        )?;
        let second_open_thread = connection.execute(
            "INSERT INTO threads (id, title, state, created_at, started_at) \
             VALUES ('second', 'Second thread', 'paused', 1, 1)",
            [],
        );

        assert!(second_open_thread.is_err());
        Ok(())
    }

    #[test]
    fn refuses_a_database_newer_than_this_binary() -> crate::error::Result<()> {
        let mut connection = Connection::open_in_memory()?;
        connection.execute_batch(
            "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL); \
             INSERT INTO schema_migrations (version, applied_at) VALUES (2, 1);",
        )?;

        let error = match apply(&mut connection, 1_700_000_000_000) {
            Ok(()) => {
                return Err(crate::error::AppError::Internal(
                    "a database from a newer binary must not start".to_owned(),
                ));
            }
            Err(error) => error,
        };

        assert!(matches!(
            error,
            crate::error::AppError::Migration { version: 2, .. }
        ));
        Ok(())
    }
}

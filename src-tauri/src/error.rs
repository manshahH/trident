use std::io;

use serde::Serialize;
use specta::Type;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Error, Serialize, Type)]
#[serde(tag = "kind", content = "detail")]
pub enum AppError {
    #[error("database error: {0}")]
    Db(String),
    #[error("migration failed at version {version}: {message}")]
    Migration { version: i64, message: String },
    #[error("not found: {entity} {id}")]
    NotFound { entity: String, id: String },
    #[error("invalid state: {0}")]
    InvalidState(String),
    #[error("validation: {field}: {message}")]
    Validation { field: String, message: String },
    #[error("platform call failed: {api} ({code})")]
    Platform { api: String, code: i32 },
    #[error("hotkey unavailable: {0}")]
    HotkeyConflict(String),
    #[error("window {0} not found")]
    WindowMissing(String),
    #[error("io: {0}")]
    Io(String),
    #[error("internal: {0}")]
    Internal(String),
}

impl From<rusqlite::Error> for AppError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Db(error.to_string())
    }
}

impl From<io::Error> for AppError {
    fn from(error: io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

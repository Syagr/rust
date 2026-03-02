#![warn(clippy::missing_errors_doc, clippy::result_large_err)]

use thiserror::Error;

pub mod store;

pub use store::{IndexStore, JsonStore, SqliteStore};

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, CoreError>;

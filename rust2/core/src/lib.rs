//! Core library for indexing files by tags.
//!
//! Provides storage backends and a trait for indexing operations.
#![deny(
    missing_docs,
    rustdoc::missing_crate_level_docs,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::result_large_err
)]

use thiserror::Error;

/// Storage backends and the main trait used by the library.
pub mod store;

pub use store::{IndexStore, JsonStore, SqliteStore};

/// Errors returned by the core indexing library.
#[derive(Debug, Error)]
pub enum CoreError {
    /// I/O error when reading or writing index data.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON (de)serialization error.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    /// SQLite error from the database backend.
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

/// Library result type.
pub type Result<T> = std::result::Result<T, CoreError>;

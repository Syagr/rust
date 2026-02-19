//! Практична робота 3 — core crate
//!
//! This crate provides the core indexing functionality used by the CLI crate.
//! It exposes an `IndexStore` trait and two implementations: a JSON-backed
//! store and a SQLite-backed store.
//!
//! The crate defines a typed `CoreError` using `thiserror` so callers can
//! programmatically handle error cases.
#![deny(missing_docs, missing_crate_level_docs)]

pub mod store;

pub use store::{IndexStore, JsonStore, SqliteStore, CoreError};

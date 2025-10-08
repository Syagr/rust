//! snippets-app — a tiny CLI to save, read and delete code snippets.
//!
//! This binary demonstrates two storage backends (JSON file and SQLite),
//! simple configuration through the `SNIPPETS_APP_STORAGE` environment
//! variable and a small set of integration tests.
//!
//! The crate enables several lints to keep documentation and public API
//! visibility strict for learning purposes.
#![warn(
    missing_docs,
    broken_intra_doc_links,
    missing_crate_level_docs,
    unreachable_pub
)]
#![warn(
    clippy::missing_panics_doc,
    clippy::clone_on_ref_ptr,
    clippy::similar_names
)]

use chrono::{DateTime, Utc};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

use thiserror::Error;

#[derive(Error, Debug)]
enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Snippet not found")]
    NotFound,

    #[error("Invalid storage provider string, expected PROVIDER:PATH")]
    InvalidProvider,
}

/// Application-level error enum for snippets-app.
type Result<T> = std::result::Result<T, AppError>;

#[derive(Parser)]
#[command(author, version, about = "snippets app — improved", long_about = None)]
struct Cli {
    #[arg(long = "name")]
    name: Option<String>,

    #[arg(long = "read")]
    read: Option<String>,

    #[arg(long = "delete")]
    delete: Option<String>,
    #[arg(long = "download")]
    download: Option<String>,
}

/// CLI arguments for the snippets-app binary.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Snippet {
    name: String,
    content: String,
    created_at: DateTime<Utc>,
}

/// Storage abstraction for snippets. Implementations provide add/get/remove.
trait SnippetStorage {
    fn add(&mut self, snippet: Snippet) -> Result<()>;
    fn get(&self, name: &str) -> Result<Snippet>;
    fn remove(&mut self, name: &str) -> Result<()>;
}

struct JsonFileStorage {
    path: String,
    index: HashMap<String, Snippet>,
}

/// JSON file based storage. Keeps an in-memory index and persists to disk.
impl JsonFileStorage {
    fn open(path: impl Into<String>) -> Result<Self> {
        let path = path.into();
        if !Path::new(&path).exists() {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&path)?;
            file.write_all(b"{}")?;
        }
        let data = fs::read_to_string(&path)?;
        let index = if data.trim().is_empty() {
            HashMap::new()
        } else {
            serde_json::from_str(&data).unwrap_or_default()
        };
        Ok(JsonFileStorage { path, index })
    }

    fn persist(&self) -> Result<()> {
        let data = serde_json::to_string_pretty(&self.index)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.path)?;
        file.write_all(data.as_bytes())?;
        Ok(())
    }
}

impl SnippetStorage for JsonFileStorage {
    fn add(&mut self, snippet: Snippet) -> Result<()> {
        self.index.insert(snippet.name.clone(), snippet);
        self.persist()
    }

    fn get(&self, name: &str) -> Result<Snippet> {
        self.index.get(name).cloned().ok_or(AppError::NotFound)
    }

    fn remove(&mut self, name: &str) -> Result<()> {
        if self.index.remove(name).is_some() {
            self.persist()?;
            Ok(())
        } else {
            Err(AppError::NotFound)
        }
    }
}

struct SqliteStorage {
    conn: rusqlite::Connection,
}

/// SQLite-based storage using a simple `snippets` table.
impl SqliteStorage {
    fn open(path: impl AsRef<str>) -> Result<Self> {
        let conn = rusqlite::Connection::open(path.as_ref())?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS snippets (
                name TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL
            );",
        )?;
        Ok(SqliteStorage { conn })
    }
}

impl SnippetStorage for SqliteStorage {
    fn add(&mut self, snippet: Snippet) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO snippets (name, content, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![
                snippet.name,
                snippet.content,
                snippet.created_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    fn get(&self, name: &str) -> Result<Snippet> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, content, created_at FROM snippets WHERE name = ?1")?;
        let mut rows = stmt.query(rusqlite::params![name])?;
        if let Some(row) = rows.next()? {
            let name: String = row.get(0)?;
            let content: String = row.get(1)?;
            let created_at_str: String = row.get(2)?;
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| AppError::Io(std::io::Error::other(e)))?;
            Ok(Snippet {
                name,
                content,
                created_at,
            })
        } else {
            Err(AppError::NotFound)
        }
    }

    fn remove(&mut self, name: &str) -> Result<()> {
        let n = self.conn.execute(
            "DELETE FROM snippets WHERE name = ?1",
            rusqlite::params![name],
        )?;
        if n > 0 {
            Ok(())
        } else {
            Err(AppError::NotFound)
        }
    }
}

fn build_storage_from_env() -> Result<Box<dyn SnippetStorage>> {
    // SNIPPETS_APP_STORAGE example: json:snippets.json or sqlite:snippets.db
    let raw = env::var("SNIPPETS_APP_STORAGE").unwrap_or_else(|_| "json:snippets.json".to_string());
    let mut parts = raw.splitn(2, ':');
    let provider = parts.next().ok_or(AppError::InvalidProvider)?;
    let path = parts.next().ok_or(AppError::InvalidProvider)?;
    match provider {
        "json" => Ok(Box::new(JsonFileStorage::open(path)?)),
        "sqlite" => Ok(Box::new(SqliteStorage::open(path)?)),
        _ => Err(AppError::InvalidProvider),
    }
}

/// Run the application logic. Returns an `AppError` on failure.
///
/// # Errors
///
/// Returns an error if storage initialization, IO or network download fails.
fn run_app() -> Result<()> {
    let cli = Cli::parse();
    // initialize tracing/logging according to env vars
    init_tracing_from_env();
    let mut storage = build_storage_from_env()?;

    if let Some(name) = cli.name {
        let content = if let Some(url) = cli.download {
            // download content
            match reqwest::blocking::get(&url) {
                Ok(resp) => resp
                    .text()
                    .map_err(|e| AppError::Io(std::io::Error::other(e)))?,
                Err(e) => {
                    return Err(AppError::Io(std::io::Error::other(e)))
                }
            }
        } else {
            let mut s = String::new();
            io::stdin().read_to_string(&mut s)?;
            s
        };
        let snippet = Snippet {
            name: name.clone(),
            content: content.trim_end().to_string(),
            created_at: Utc::now(),
        };
        storage.add(snippet)?;
        println!("Snippet '{}' saved.", name);
        return Ok(());
    }

    if let Some(name) = cli.read {
        match storage.get(&name) {
            Ok(snippet) => println!("{}", snippet.content),
            Err(AppError::NotFound) => {
                eprintln!("Snippet '{}' not found.", name);
                return Err(AppError::NotFound);
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                return Err(e);
            }
        }
    }

    if let Some(name) = cli.delete {
        match storage.remove(&name) {
            Ok(()) => println!("Snippet '{}' deleted.", name),
            Err(AppError::NotFound) => {
                eprintln!("Snippet '{}' not found.", name);
                return Err(AppError::NotFound);
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                return Err(e);
            }
        }
        return Ok(());
    }

    eprintln!("Usage: --name <NAME> (create from stdin), --read <NAME>, --delete <NAME>");
    Ok(())
}

fn init_tracing_from_env() {
    use std::sync::Arc;
    use tracing_subscriber::{fmt, EnvFilter};
    let default_level =
        std::env::var("SNIPPETS_APP_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    if let Ok(path) = std::env::var("SNIPPETS_APP_LOG_PATH") {
        // try to open file for append; provide a closure that clones the file per writer
        if let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let arc = Arc::new(file);
            let subscriber = fmt()
                .with_env_filter(EnvFilter::new(default_level))
                .with_writer(move || arc.as_ref().try_clone().expect("failed to clone log file"))
                .finish();
            tracing::subscriber::set_global_default(subscriber).ok();
            return;
        }
    }
    let subscriber = fmt()
        .with_env_filter(EnvFilter::new(default_level))
        .finish();
    tracing::subscriber::set_global_default(subscriber).ok();
}

fn main() {
    if let Err(e) = run_app() {
        eprintln!("Fatal error: {}", e);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn json_storage_add_get_remove() {
        let tmp = NamedTempFile::new().expect("tmp file");
        let path = tmp.path().to_str().unwrap().to_string();
        let mut s = JsonFileStorage::open(path.clone()).expect("open");

        let sn = Snippet {
            name: "a".into(),
            content: "c".into(),
            created_at: Utc::now(),
        };
        s.add(sn.clone()).expect("add");
        let got = s.get("a").expect("get");
        assert_eq!(got.name, "a");
        assert_eq!(got.content, "c");
        s.remove("a").expect("remove");
        assert!(s.get("a").is_err());
    }

    #[test]
    fn sqlite_storage_add_get_remove() {
        let tmp = NamedTempFile::new().expect("tmp file");
        let path = tmp.path().to_str().unwrap().to_string();
        let mut s = SqliteStorage::open(&path).expect("open sqlite");

        let sn = Snippet {
            name: "x".into(),
            content: "y".into(),
            created_at: Utc::now(),
        };
        s.add(sn.clone()).expect("add");
        let got = s.get("x").expect("get");
        assert_eq!(got.name, "x");
        assert_eq!(got.content, "y");
        s.remove("x").expect("remove");
        assert!(s.get("x").is_err());
    }

    #[test]
    fn build_storage_from_env_json() {
        let tmp = NamedTempFile::new().expect("tmp file");
        let path = tmp.path().to_str().unwrap().to_string();
        std::env::set_var("SNIPPETS_APP_STORAGE", format!("json:{}", path));
        let _ = build_storage_from_env().expect("build");
        std::env::remove_var("SNIPPETS_APP_STORAGE");
    }

    #[test]
    fn invalid_provider() {
        std::env::set_var("SNIPPETS_APP_STORAGE", "bad:xxx");
        let r = build_storage_from_env();
        assert!(r.is_err());
        std::env::remove_var("SNIPPETS_APP_STORAGE");
    }
}

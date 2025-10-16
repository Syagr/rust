use clap::Parser;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

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

    #[error("Parse error: {0}")]
    Parse(#[from] chrono::ParseError),

    #[error("--download requires --name")] 
    DownloadRequiresName,
}

type Result<T> = std::result::Result<T, AppError>;

#[derive(Parser)]
#[command(author, version, about = "snippets app — lab 5", long_about = None)]
struct Cli {
    /// Name for the snippet to create. If omitted, use --read or --delete.
    #[arg(long = "name")]
    name: Option<String>,

    /// Read a snippet by name and print content to STDOUT.
    #[arg(long = "read")]
    read: Option<String>,

    /// Delete a snippet by name.
    #[arg(long = "delete")]
    delete: Option<String>,

    /// Download content from URL instead of reading STDIN. Requires --name.
    #[arg(long = "download")]
    download: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Snippet {
    name: String,
    content: String,
    created_at: DateTime<Utc>,
}

trait SnippetStorage {
    fn add(&mut self, snippet: Snippet) -> Result<()>;
    fn get(&self, name: &str) -> Result<Snippet>;
    fn remove(&mut self, name: &str) -> Result<()>;
}

struct JsonFileStorage {
    path: PathBuf,
    index: HashMap<String, Snippet>,
}

impl JsonFileStorage {
    fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_owned();
        let mut index = HashMap::new();
        if path.exists() {
            let meta = fs::metadata(&path)?;
            if meta.len() > 0 {
                let file = File::open(&path)?;
                index = serde_json::from_reader(file)?;
            }
        }
        Ok(JsonFileStorage { path, index })
    }

    fn persist(&self) -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.path)?;
        serde_json::to_writer(&mut file, &self.index)?;
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

impl SqliteStorage {
    fn open(path: impl AsRef<Path>) -> Result<Self> {
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
            rusqlite::params![snippet.name, snippet.content, snippet.created_at.to_rfc3339()],
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
                .map(|dt| dt.with_timezone(&Utc))?;
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
        let n = self
            .conn
            .execute("DELETE FROM snippets WHERE name = ?1", rusqlite::params![name])?;
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
        "json" => Ok(Box::new(JsonFileStorage::open(Path::new(path))?)),
        "sqlite" => Ok(Box::new(SqliteStorage::open(Path::new(path))?)),
        _ => Err(AppError::InvalidProvider),
    }
}

fn run_app() -> Result<()> {
    let cli = Cli::parse();
    // initialize tracing/logging according to env vars
    init_tracing_from_env();
    tracing::debug!(?cli.name, ?cli.read, ?cli.delete, ?cli.download, "parsed CLI");
    let mut storage = build_storage_from_env()?;

    if cli.download.is_some() && cli.name.is_none() {
        eprintln!("--download requires --name");
        return Err(AppError::DownloadRequiresName);
    }

    if let Some(name) = cli.name {
        let content = if let Some(url) = cli.download {
            // download content
            tracing::info!(%url, %name, "downloading snippet");
            match reqwest::blocking::get(&url) {
                Ok(resp) => {
                    if let Err(status_err) = resp.error_for_status_ref() {
                        eprintln!("Download failed: {}", status_err);
                        return Err(AppError::Io(std::io::Error::new(std::io::ErrorKind::Other, status_err)));
                    }
                    resp.text().map_err(|e| AppError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?
                }
                Err(e) => return Err(AppError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))),
            }
        } else {
            let mut s = String::new();
            io::stdin().read_to_string(&mut s)?;
            s
        };
        tracing::info!(%name, len = content.len(), "saving snippet");
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
                tracing::warn!(%name, "snippet not found on read");
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
                tracing::warn!(%name, "snippet not found on delete");
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
    use tracing_subscriber::{fmt, EnvFilter};
    use std::sync::Arc;
    let default_level = std::env::var("SNIPPETS_APP_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    if let Ok(path) = std::env::var("SNIPPETS_APP_LOG_PATH") {
        if let Ok(file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let arc = Arc::new(file);
            let subscriber = fmt()
                .with_env_filter(EnvFilter::new(default_level))
                .with_writer(move || arc.as_ref().try_clone().expect("failed to clone log file"))
                .finish();
            tracing::subscriber::set_global_default(subscriber).ok();
            return;
        }
    }
    let subscriber = fmt().with_env_filter(EnvFilter::new(default_level)).finish();
    tracing::subscriber::set_global_default(subscriber).ok();
}

fn main() {
    if let Err(e) = run_app() {
        eprintln!("Fatal error: {}", e);
        std::process::exit(1);
    }
}

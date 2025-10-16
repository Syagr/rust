use clap::Parser;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

use thiserror::Error;

// --- Errors ---
#[derive(Error, Debug)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Parse error: {0}")]
    Parse(#[from] chrono::ParseError),

    #[error("Not found: {0}")]
    NotFound(String),
}

// --- Part 1: Storage trait, User and repositories ---
pub trait Storage<K, V> {
    fn set(&mut self, key: K, val: V) -> Result<(), AppError>;
    fn get(&self, key: &K) -> Result<Option<&V>, AppError>;
    fn remove(&mut self, key: &K) -> Result<Option<V>, AppError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct User {
    pub id: u64,
    pub email: Cow<'static, str>,
    pub activated: bool,
}

// Simple in-memory HashMap storage to use in tests
#[derive(Default)]
pub struct InMemoryStorage<K: std::cmp::Eq + std::hash::Hash, V> {
    inner: HashMap<K, V>,
}

impl<K: std::cmp::Eq + std::hash::Hash, V> InMemoryStorage<K, V> {
    pub fn new() -> Self {
        Self { inner: HashMap::new() }
    }
}

impl<K: std::cmp::Eq + std::hash::Hash + Clone, V> Storage<K, V> for InMemoryStorage<K, V> {
    fn set(&mut self, key: K, val: V) -> Result<(), AppError> {
        self.inner.insert(key, val);
        Ok(())
    }

    fn get(&self, key: &K) -> Result<Option<&V>, AppError> {
        Ok(self.inner.get(key))
    }

    fn remove(&mut self, key: &K) -> Result<Option<V>, AppError> {
        Ok(self.inner.remove(key))
    }
}

// Static (generic) repository
pub struct UserRepositoryStatic<S> {
    storage: S,
}

impl<S> UserRepositoryStatic<S> {
    pub fn new(storage: S) -> Self {
        Self { storage }
    }
}

impl<S> UserRepositoryStatic<S>
where
    S: Storage<u64, User>,
{
    pub fn add(&mut self, user: User) -> Result<(), AppError> {
        self.storage.set(user.id, user)
    }
    pub fn get(&self, id: &u64) -> Result<Option<&User>, AppError> {
        self.storage.get(id)
    }
    pub fn update(&mut self, user: User) -> Result<(), AppError> {
        self.storage.set(user.id, user)
    }
    pub fn remove(&mut self, id: &u64) -> Result<Option<User>, AppError> {
        self.storage.remove(id)
    }
}

// Dynamic (trait object) repository
pub struct UserRepositoryDynamic {
    storage: Box<dyn Storage<u64, User>>,
}

impl UserRepositoryDynamic {
    pub fn new(storage: Box<dyn Storage<u64, User>>) -> Self {
        Self { storage }
    }

    pub fn add(&mut self, user: User) -> Result<(), AppError> {
        self.storage.set(user.id, user)
    }
    pub fn get(&self, id: &u64) -> Result<Option<&User>, AppError> {
        self.storage.get(id)
    }
    pub fn update(&mut self, user: User) -> Result<(), AppError> {
        self.storage.set(user.id, user)
    }
    pub fn remove(&mut self, id: &u64) -> Result<Option<User>, AppError> {
        self.storage.remove(id)
    }
}

// --- Part 2: snippets app improvements with error handling ---
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Snippet {
    pub name: String,
    pub content: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub trait SnippetStorage: Send {
    fn add(&mut self, s: Snippet) -> Result<(), AppError>;
    fn get(&self, name: &str) -> Result<Option<Snippet>, AppError>;
    fn remove(&mut self, name: &str) -> Result<bool, AppError>;
}

// JSON file backend
pub struct JsonFileStorage {
    path: String,
    index: HashMap<String, Snippet>,
}

impl JsonFileStorage {
    pub fn open(path: impl Into<String>) -> Result<Self, AppError> {
        let path = path.into();
        let mut index = HashMap::new();
        if Path::new(&path).exists() {
            let data = fs::read_to_string(&path)?;
            if !data.trim().is_empty() {
                index = serde_json::from_str(&data)?;
            }
        }
        Ok(Self { path, index })
    }

    fn persist(&self) -> Result<(), AppError> {
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
    fn add(&mut self, s: Snippet) -> Result<(), AppError> {
        self.index.insert(s.name.clone(), s);
        self.persist()
    }

    fn get(&self, name: &str) -> Result<Option<Snippet>, AppError> {
        Ok(self.index.get(name).cloned())
    }

    fn remove(&mut self, name: &str) -> Result<bool, AppError> {
        let removed = self.index.remove(name).is_some();
        if removed {
            self.persist()?;
        }
        Ok(removed)
    }
}

// SQLite backend using rusqlite
pub struct SqliteStorage {
    conn: rusqlite::Connection,
}

impl SqliteStorage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AppError> {
        let conn = rusqlite::Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS snippets (
                name TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL
            );",
        )?;
        Ok(Self { conn })
    }
}

impl SnippetStorage for SqliteStorage {
    fn add(&mut self, s: Snippet) -> Result<(), AppError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO snippets (name, content, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![s.name, s.content, s.created_at.to_rfc3339()],
        )?;
        Ok(())
    }

    fn get(&self, name: &str) -> Result<Option<Snippet>, AppError> {
        let mut stmt = self.conn.prepare("SELECT name, content, created_at FROM snippets WHERE name = ?1")?;
        let mut rows = stmt.query([name])?;
        match rows.next() {
            Ok(Some(row)) => {
                let name: String = row.get(0)?;
                let content: String = row.get(1)?;
                let created_at_s: String = row.get(2)?;
                let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_s)?.with_timezone(&chrono::Utc);
                Ok(Some(Snippet { name, content, created_at }))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(AppError::Sqlite(e)),
        }
    }

    fn remove(&mut self, name: &str) -> Result<bool, AppError> {
        let n = self.conn.execute("DELETE FROM snippets WHERE name = ?1", [name])?;
        Ok(n > 0)
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Snippets app with JSON/SQLite storage and proper errors", long_about = None)]
struct Cli {
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    read: Option<String>,
    #[arg(long)]
    delete: Option<String>,
}

fn build_storage_from_env() -> Result<Box<dyn SnippetStorage>, AppError> {
    let env = std::env::var("SNIPPETS_APP_STORAGE").unwrap_or_else(|_| "JSON:snippets.json".to_string());
    let parts: Vec<_> = env.splitn(2, ':').collect();
    match parts.as_slice() {
        ["JSON", path] | ["json", path] => Ok(Box::new(JsonFileStorage::open(path.to_string())?)),
        ["SQLITE", path] | ["sqlite", path] => Ok(Box::new(SqliteStorage::open(path)?)),
        _ => Ok(Box::new(JsonFileStorage::open("snippets.json".to_string())?)),
    }
}

fn run_app() -> Result<(), AppError> {
    let cli = Cli::parse();
    let mut storage = build_storage_from_env()?;

    if let Some(name) = cli.name {
        let mut content = String::new();
        io::stdin().read_to_string(&mut content)?;
        let sn = Snippet { name: name.clone(), content: content.trim_end().to_string(), created_at: chrono::Utc::now() };
        storage.add(sn)?;
        println!("Snippet '{}' saved.", name);
        return Ok(());
    }

    if let Some(name) = cli.read {
        match storage.get(&name)? {
            Some(s) => println!("{}", s.content),
            None => eprintln!("Snippet '{}' not found.", name),
        }
        return Ok(());
    }

    if let Some(name) = cli.delete {
        if storage.remove(&name)? {
            println!("Snippet '{}' deleted.", name);
        } else {
            eprintln!("Snippet '{}' not found.", name);
        }
        return Ok(());
    }

    eprintln!("Usage: --name <NAME> (create from stdin), --read <NAME>, --delete <NAME)");
    Ok(())
}

fn main() {
    if let Err(e) = run_app() {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}

// Tests for both parts
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_repo_static() -> Result<(), AppError> {
        let storage = InMemoryStorage::<u64, User>::new();
        let mut repo = UserRepositoryStatic::new(storage);
        let user = User { id: 1, email: Cow::from("a@example.com"), activated: false };
        repo.add(user.clone())?;
        let got = repo.get(&1)?.unwrap().clone();
        assert_eq!(got, user);
        let updated = User { id: 1, email: Cow::from("b@example.com"), activated: true };
        repo.update(updated.clone())?;
        assert_eq!(repo.get(&1)?.unwrap().email, "b@example.com");
        let removed = repo.remove(&1)?.unwrap();
        assert_eq!(removed, updated);
        assert!(repo.get(&1)?.is_none());
        Ok(())
    }

    #[test]
    fn test_user_repo_dynamic() -> Result<(), AppError> {
        let storage = InMemoryStorage::<u64, User>::new();
        let boxed: Box<dyn Storage<u64, User>> = Box::new(storage);
        let mut repo = UserRepositoryDynamic::new(boxed);
        let user = User { id: 2, email: Cow::from("x@example.com"), activated: false };
        repo.add(user.clone())?;
        let got = repo.get(&2)?.unwrap().clone();
        assert_eq!(got, user);
        let updated = User { id: 2, email: Cow::from("y@example.com"), activated: true };
        repo.update(updated.clone())?;
        assert_eq!(repo.get(&2)?.unwrap().email, "y@example.com");
        let removed = repo.remove(&2)?.unwrap();
        assert_eq!(removed, updated);
        assert!(repo.get(&2)?.is_none());
        Ok(())
    }

    #[test]
    fn test_json_storage_snippet_roundtrip() -> Result<(), AppError> {
        let temp = tempfile::NamedTempFile::new().map_err(|e| AppError::Io(e))?;
        let path = temp.path().to_string_lossy().to_string();
        let mut s = JsonFileStorage::open(path.clone())?;
        let sn = Snippet { name: "n1".to_string(), content: "c1".to_string(), created_at: chrono::Utc::now() };
        s.add(sn.clone())?;
        let got = s.get("n1")?.unwrap();
        assert_eq!(got.name, "n1");
        assert_eq!(got.content, "c1");
        assert!(s.remove("n1")?);
        assert!(s.get("n1")?.is_none());
        Ok(())
    }

    #[test]
    fn test_sqlite_storage_snippet_roundtrip() -> Result<(), AppError> {
        let temp = tempfile::NamedTempFile::new().map_err(|e| AppError::Io(e))?;
        let path = temp.path().to_string_lossy().to_string();
        let mut s = SqliteStorage::open(&path)?;
        let sn = Snippet { name: "n2".to_string(), content: "c2".to_string(), created_at: chrono::Utc::now() };
        s.add(sn.clone())?;
        let got = s.get("n2")?.unwrap();
        assert_eq!(got.name, "n2");
        assert_eq!(got.content, "c2");
        assert!(s.remove("n2")?);
        assert!(s.get("n2")?.is_none());
        Ok(())
    }

    #[test]
    fn test_json_open_invalid_json() -> Result<(), AppError> {
        // create a temp file with invalid JSON
        let temp = tempfile::NamedTempFile::new().map_err(|e| AppError::Io(e))?;
        std::fs::write(temp.path(), "{ this is not valid json").map_err(|e| AppError::Io(e))?;
        // opening should fail with a JSON error
        match JsonFileStorage::open(temp.path().to_string_lossy().to_string()) {
            Err(AppError::Json(_)) => Ok(()),
            Ok(_) => panic!("expected Json error"),
            Err(e) => panic!("unexpected error: {}", e),
        }
    }

    #[test]
    fn test_json_persist_io_error() -> Result<(), AppError> {
        // choose a path in a non-existent subdirectory
        let tempdir = tempfile::tempdir().map_err(|e| AppError::Io(e))?;
        let path = tempdir.path().join("no_such_dir").join("snippets.json");
        let path_s = path.to_string_lossy().to_string();
        // open will succeed (file doesn't exist yet)
        let mut s = JsonFileStorage::open(path_s.clone())?;
        let sn = Snippet { name: "x".to_string(), content: "y".to_string(), created_at: chrono::Utc::now() };
        // adding should attempt to create the file and fail with an IO error because parent dir is missing
        match s.add(sn) {
            Err(AppError::Io(_)) => Ok(()),
            Ok(_) => panic!("expected IO error when persisting to missing dir"),
            Err(e) => panic!("unexpected error: {}", e),
        }
    }

    #[test]
    fn test_sqlite_parse_error_on_bad_date() -> Result<(), AppError> {
        // create a sqlite db and insert a row with malformed created_at
        let temp = tempfile::NamedTempFile::new().map_err(|e| AppError::Io(e))?;
        let path = temp.path().to_string_lossy().to_string();
        let conn = rusqlite::Connection::open(&path).map_err(|e| AppError::Sqlite(e))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS snippets (
                name TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL
            );",
        ).map_err(|e| AppError::Sqlite(e))?;
        conn.execute("INSERT OR REPLACE INTO snippets (name, content, created_at) VALUES (?1, ?2, ?3)", rusqlite::params!["bad", "c", "not-a-date"]).map_err(|e| AppError::Sqlite(e))?;
        let s = SqliteStorage::open(&path)?;
        match s.get("bad") {
            Err(AppError::Parse(_)) => Ok(()),
            Ok(_) => panic!("expected parse error"),
            Err(e) => panic!("unexpected error: {}", e),
        }
    }
}

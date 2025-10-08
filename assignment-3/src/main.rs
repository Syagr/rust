use clap::Parser;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

// --- Part 1: Storage trait, User and repositories ---
pub trait Storage<K, V> {
    fn set(&mut self, key: K, val: V);
    fn get(&self, key: &K) -> Option<&V>;
    fn remove(&mut self, key: &K) -> Option<V>;
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
    fn set(&mut self, key: K, val: V) {
        self.inner.insert(key, val);
    }

    fn get(&self, key: &K) -> Option<&V> {
        self.inner.get(key)
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        self.inner.remove(key)
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
    pub fn add(&mut self, user: User) {
        self.storage.set(user.id, user);
    }
    pub fn get(&self, id: &u64) -> Option<&User> {
        self.storage.get(id)
    }
    pub fn update(&mut self, user: User) {
        self.storage.set(user.id, user);
    }
    pub fn remove(&mut self, id: &u64) -> Option<User> {
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

    pub fn add(&mut self, user: User) {
        self.storage.set(user.id, user);
    }
    pub fn get(&self, id: &u64) -> Option<&User> {
        self.storage.get(id)
    }
    pub fn update(&mut self, user: User) {
        self.storage.set(user.id, user);
    }
    pub fn remove(&mut self, id: &u64) -> Option<User> {
        self.storage.remove(id)
    }
}

// --- Part 2: snippets app improvements ---
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Snippet {
    name: String,
    content: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

pub trait SnippetStorage: Send {
    fn add(&mut self, s: Snippet);
    fn get(&self, name: &str) -> Option<Snippet>;
    fn remove(&mut self, name: &str) -> bool;
}

// JSON file backend
pub struct JsonFileStorage {
    path: String,
    index: HashMap<String, Snippet>,
}

impl JsonFileStorage {
    pub fn open(path: impl Into<String>) -> Self {
        let path = path.into();
        let mut index = HashMap::new();
        if Path::new(&path).exists() {
            if let Ok(data) = fs::read_to_string(&path) {
                index = serde_json::from_str(&data).unwrap_or_default();
            }
        }
        Self { path, index }
    }

    fn persist(&self) {
        if let Ok(data) = serde_json::to_string_pretty(&self.index) {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&self.path)
                .unwrap();
            file.write_all(data.as_bytes()).unwrap();
        }
    }
}

impl SnippetStorage for JsonFileStorage {
    fn add(&mut self, s: Snippet) {
        self.index.insert(s.name.clone(), s);
        self.persist();
    }

    fn get(&self, name: &str) -> Option<Snippet> {
        self.index.get(name).cloned()
    }

    fn remove(&mut self, name: &str) -> bool {
        let removed = self.index.remove(name).is_some();
        if removed {
            self.persist();
        }
        removed
    }
}

// SQLite backend using rusqlite
pub struct SqliteStorage {
    conn: rusqlite::Connection,
}

impl SqliteStorage {
    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
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
    fn add(&mut self, s: Snippet) {
        let _ = self.conn.execute(
            "INSERT OR REPLACE INTO snippets (name, content, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![s.name, s.content, s.created_at.to_rfc3339()],
        );
    }

    fn get(&self, name: &str) -> Option<Snippet> {
        let mut stmt = self.conn.prepare("SELECT name, content, created_at FROM snippets WHERE name = ?1").ok()?;
        let mut rows = stmt.query([name]).ok()?;
        match rows.next() {
            Ok(Some(row)) => {
                let name: String = row.get(0).ok()?;
                let content: String = row.get(1).ok()?;
                let created_at_s: String = row.get(2).ok()?;
                if let Ok(created_at) = chrono::DateTime::parse_from_rfc3339(&created_at_s) {
                    return Some(Snippet { name, content, created_at: created_at.with_timezone(&chrono::Utc) });
                }
            }
            Ok(None) => {}
            Err(_) => {}
        }
        None
    }

    fn remove(&mut self, name: &str) -> bool {
        match self.conn.execute("DELETE FROM snippets WHERE name = ?1", [name]) {
            Ok(n) => n > 0,
            Err(_) => false,
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Snippets app with JSON/SQLite storage", long_about = None)]
struct Cli {
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    read: Option<String>,
    #[arg(long)]
    delete: Option<String>,
}

fn build_storage_from_env() -> Box<dyn SnippetStorage> {
    let env = std::env::var("SNIPPETS_APP_STORAGE").unwrap_or_else(|_| "JSON:snippets.json".to_string());
    let parts: Vec<_> = env.splitn(2, ':').collect();
    match parts.as_slice() {
        ["JSON", path] | ["json", path] => Box::new(JsonFileStorage::open(path.to_string())),
        ["SQLITE", path] | ["sqlite", path] => {
            match SqliteStorage::open(path) {
                Ok(s) => Box::new(s),
                Err(_) => Box::new(JsonFileStorage::open("snippets.json".to_string())),
            }
        }
        _ => Box::new(JsonFileStorage::open("snippets.json".to_string())),
    }
}

fn main() {
    let cli = Cli::parse();
    let mut storage = build_storage_from_env();

    if let Some(name) = cli.name {
        let mut content = String::new();
        io::stdin().read_to_string(&mut content).unwrap();
        let sn = Snippet { name: name.clone(), content: content.trim_end().to_string(), created_at: chrono::Utc::now() };
        storage.add(sn);
        println!("Snippet '{}' saved.", name);
        return;
    }

    if let Some(name) = cli.read {
        match storage.get(&name) {
            Some(s) => println!("{}", s.content),
            None => eprintln!("Snippet '{}' not found.", name),
        }
        return;
    }

    if let Some(name) = cli.delete {
        if storage.remove(&name) {
            println!("Snippet '{}' deleted.", name);
        } else {
            eprintln!("Snippet '{}' not found.", name);
        }
        return;
    }

    eprintln!("Usage: --name <NAME> (create from stdin), --read <NAME>, --delete <NAME>");
}

// Tests for both parts
#[cfg(test)]
mod tests {
    use super::*;
    // uuid not used in current tests

    #[test]
    fn test_user_repo_static() {
    let storage = InMemoryStorage::<u64, User>::new();
    let mut repo = UserRepositoryStatic::new(storage);
        let user = User { id: 1, email: Cow::from("a@example.com"), activated: false };
        repo.add(user.clone());
        let got = repo.get(&1).unwrap().clone();
        assert_eq!(got, user);
        let updated = User { id: 1, email: Cow::from("b@example.com"), activated: true };
        repo.update(updated.clone());
        assert_eq!(repo.get(&1).unwrap().email, "b@example.com");
        let removed = repo.remove(&1).unwrap();
        assert_eq!(removed, updated);
        assert!(repo.get(&1).is_none());
    }

    #[test]
    fn test_user_repo_dynamic() {
        let storage = InMemoryStorage::<u64, User>::new();
        let boxed: Box<dyn Storage<u64, User>> = Box::new(storage);
        let mut repo = UserRepositoryDynamic::new(boxed);
        let user = User { id: 2, email: Cow::from("x@example.com"), activated: false };
        repo.add(user.clone());
        let got = repo.get(&2).unwrap().clone();
        assert_eq!(got, user);
        let updated = User { id: 2, email: Cow::from("y@example.com"), activated: true };
        repo.update(updated.clone());
        assert_eq!(repo.get(&2).unwrap().email, "y@example.com");
        let removed = repo.remove(&2).unwrap();
        assert_eq!(removed, updated);
        assert!(repo.get(&2).is_none());
    }

    #[test]
    fn test_json_storage_snippet_roundtrip() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        let path = temp.path().to_string_lossy().to_string();
        let mut s = JsonFileStorage::open(path.clone());
        let sn = Snippet { name: "n1".to_string(), content: "c1".to_string(), created_at: chrono::Utc::now() };
        s.add(sn.clone());
        let got = s.get("n1").unwrap();
        assert_eq!(got.name, "n1");
        assert_eq!(got.content, "c1");
        assert!(s.remove("n1"));
        assert!(s.get("n1").is_none());
    }

    #[test]
    fn test_sqlite_storage_snippet_roundtrip() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        let path = temp.path().to_string_lossy().to_string();
        let mut s = SqliteStorage::open(&path).unwrap();
        let sn = Snippet { name: "n2".to_string(), content: "c2".to_string(), created_at: chrono::Utc::now() };
        s.add(sn.clone());
        let got = s.get("n2").unwrap();
        assert_eq!(got.name, "n2");
        assert_eq!(got.content, "c2");
        assert!(s.remove("n2"));
        assert!(s.get("n2").is_none());
    }
}

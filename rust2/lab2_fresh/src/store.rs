use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

pub trait IndexStore {
    fn add(&self, path: &str, tags: &[String]) -> Result<()>;
    fn get(&self, tags: &[String]) -> Result<Vec<String>>;
}

#[derive(Clone)]
pub struct JsonStore {
    path: PathBuf,
}

#[derive(Serialize, Deserialize, Default)]
struct JsonIndex {
    files: HashMap<String, Vec<String>>,
}

impl JsonStore {
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        Self { path: path.into() }
    }

    fn load(&self) -> Result<JsonIndex> {
        if !self.path.exists() {
            return Ok(JsonIndex::default());
        }
        let s = fs::read_to_string(&self.path).with_context(|| format!("read {}", self.path.display()))?;
        let idx: JsonIndex = serde_json::from_str(&s).context("parse json")?;
        Ok(idx)
    }

    fn save(&self, idx: &JsonIndex) -> Result<()> {
        let s = serde_json::to_string_pretty(idx).context("serialize json")?;
        fs::write(&self.path, s).with_context(|| format!("write {}", self.path.display()))?;
        Ok(())
    }
}

impl IndexStore for JsonStore {
    fn add(&self, path: &str, tags: &[String]) -> Result<()> {
        let mut idx = self.load()?;
        let entry = idx.files.entry(path.to_string()).or_default();
        let mut set: HashSet<String> = entry.iter().cloned().collect();
        for t in tags { set.insert(t.clone()); }
        entry.clear();
        entry.extend(set.into_iter());
        self.save(&idx)?;
        Ok(())
    }

    fn get(&self, tags: &[String]) -> Result<Vec<String>> {
        let idx = self.load()?;
        if tags.is_empty() {
            return Ok(idx.files.keys().cloned().collect());
        }
        let tags_set: HashSet<&String> = tags.iter().collect();
        let mut res = Vec::new();
        for (p, tlist) in idx.files.iter() {
            let file_tags: HashSet<&String> = tlist.iter().collect();
            if tags_set.is_subset(&file_tags) {
                res.push(p.clone());
            }
        }
        Ok(res)
    }
}

pub struct SqliteStore {
    path: PathBuf,
}

impl SqliteStore {
    pub fn new<P: Into<PathBuf>>(path: P) -> Result<Self> {
        let path = path.into();
        let conn = rusqlite::Connection::open(&path).with_context(|| format!("open {}", path.display()))?;
        conn.execute_batch(
            "BEGIN;
            CREATE TABLE IF NOT EXISTS files (id INTEGER PRIMARY KEY, path TEXT UNIQUE);
            CREATE TABLE IF NOT EXISTS tags (id INTEGER PRIMARY KEY, name TEXT UNIQUE);
            CREATE TABLE IF NOT EXISTS file_tags (file_id INTEGER, tag_id INTEGER, UNIQUE(file_id, tag_id));
            COMMIT;",
        )?;
        Ok(Self { path })
    }

    fn conn(&self) -> Result<rusqlite::Connection> {
        let c = rusqlite::Connection::open(&self.path).with_context(|| format!("open {}", self.path.display()))?;
        Ok(c)
    }
}

impl IndexStore for SqliteStore {
    fn add(&self, path: &str, tags: &[String]) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute("INSERT OR IGNORE INTO files (path) VALUES (?1)", rusqlite::params![path])?;
        let file_id: i64 = tx.query_row("SELECT id FROM files WHERE path = ?1", rusqlite::params![path], |r| r.get(0))?;
        for t in tags {
            tx.execute("INSERT OR IGNORE INTO tags (name) VALUES (?1)", rusqlite::params![t])?;
            let tag_id: i64 = tx.query_row("SELECT id FROM tags WHERE name = ?1", rusqlite::params![t], |r| r.get(0))?;
            tx.execute("INSERT OR IGNORE INTO file_tags (file_id, tag_id) VALUES (?1, ?2)", rusqlite::params![file_id, tag_id])?;
        }
        tx.commit()?;
        Ok(())
    }

    fn get(&self, tags: &[String]) -> Result<Vec<String>> {
        let conn = self.conn()?;
        if tags.is_empty() {
            let mut stmt = conn.prepare("SELECT path FROM files")?;
            let rows = stmt.query_map([], |r| r.get(0))?;
            let mut res = Vec::new();
            for row in rows { res.push(row?); }
            return Ok(res);
        }
        let placeholders: Vec<String> = (0..tags.len()).map(|i| format!("?{}", i+1)).collect();
        let sql = format!("SELECT f.path FROM files f
            JOIN file_tags ft ON ft.file_id = f.id
            JOIN tags t ON t.id = ft.tag_id
            WHERE t.name IN ({})
            GROUP BY f.id
            HAVING COUNT(DISTINCT t.name) = {}", placeholders.join(","), tags.len());
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> = tags.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(rusqlite::params_from_iter(params), |r| r.get(0))?;
        let mut res = Vec::new();
        for row in rows { res.push(row?); }
        Ok(res)
    }
}

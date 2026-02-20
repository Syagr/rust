//! Практична робота 3 — CLI
//!
//! Binary crate that provides a small CLI around the `files_index_core` library.
#![deny(missing_docs, missing_crate_level_docs)]

use anyhow::Context;
use clap::{Parser, Subcommand};
use std::env;

use files_index_core::{JsonStore, SqliteStore, IndexStore};

#[derive(Parser, Debug)]
#[command(author, version, about = "Files index CLI (lab3)")]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Add {
        #[arg(long)]
        path: String,
        #[arg(long)]
        tags: String,
    },
    Get {
        #[arg(long, default_value = "")]
        tags: String,
    },
}

/// Parse comma-separated tags into a vector of trimmed tag strings.
fn parse_tags(s: &str) -> Vec<String> {
    s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let cfg = env::var("FILES_INDEX_PATH").context("FILES_INDEX_PATH not set")?;
    let mut parts = cfg.splitn(2, ':');
    let kind = parts.next().unwrap_or("");
    let path = parts.next().ok_or_else(|| anyhow::anyhow!("FILES_INDEX_PATH must be type:path"))?;

    // construct store using core crate types
    let store: Box<dyn IndexStore> = match kind {
        "json" => Box::new(JsonStore::new(path)),
        "sqlite" => Box::new(SqliteStore::new(path)?),
        other => anyhow::bail!("unknown store type '{}', supported: json, sqlite", other),
    };

    match cli.cmd {
        Commands::Add { path, tags } => {
            let t = parse_tags(&tags);
            store.add(&path, &t).map_err(|e| anyhow::anyhow!("add failed: {}", e))?;
            println!("Added: {} with tags {}", path, t.join(", "));
        }
        Commands::Get { tags } => {
            let t = parse_tags(&tags);
            let res = store.get(&t).map_err(|e| anyhow::anyhow!("get failed: {}", e))?;
            for p in res { println!("{}", p); }
        }
    }

    Ok(())
}

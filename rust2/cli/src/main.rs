//! CLI for indexing files by tags.
#![deny(
    missing_docs,
    rustdoc::missing_crate_level_docs,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::result_large_err
)]

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use files_index_core::{IndexStore, JsonStore, SqliteStore};
use std::env;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about = "Files index CLI (Practical work 3)", long_about = None)]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Add a file with tags
    Add {
        /// Path to the file
        #[arg(long)]
        path: String,
        /// Comma-separated tags
        #[arg(long)]
        tags: String,
    },
    /// Get files matching tags (comma-separated)
    Get {
        /// Comma-separated tags (use empty to list all)
        #[arg(long, default_value = "")]
        tags: String,
    },
}

fn parse_tags(s: &str) -> Vec<String> {
    s.split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

fn make_store_from_env() -> Result<Box<dyn IndexStore>> {
    let var = env::var("FILES_INDEX_PATH").context("FILES_INDEX_PATH not set")?;
    let mut parts = var.splitn(2, ':');
    let kind = parts.next().unwrap_or("");
    let path = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("FILES_INDEX_PATH must be in the form type:path"))?;
    match kind {
        "json" => Ok(Box::new(JsonStore::new(PathBuf::from(path)))),
        "sqlite" => Ok(Box::new(SqliteStore::new(PathBuf::from(path))?)),
        other => bail!("unknown store type '{}', supported: json, sqlite", other),
    }
}

fn run_from_args(argv: Vec<String>) -> Result<()> {
    let cli = Cli::parse_from(argv);
    let store = make_store_from_env()?;

    match cli.cmd {
        Commands::Add { path, tags } => {
            let list = parse_tags(&tags);
            store.add(&path, &list)?;
            println!("Added: {} with tags {}", path, list.join(", "));
        }
        Commands::Get { tags } => {
            let list = parse_tags(&tags);
            let files = store.get(&list)?;
            for f in files {
                println!("{}", f);
            }
        }
    }

    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if let Err(e) = run_from_args(args) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

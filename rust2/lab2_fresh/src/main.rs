mod store;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::env;
use std::path::PathBuf;
use store::{IndexStore, JsonStore, SqliteStore};

#[derive(Parser, Debug)]
#[command(author, version, about = "Files index CLI (lab2_fresh)")]
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

fn parse_tags(s: &str) -> Vec<String> {
    s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
}

fn make_store_from_env() -> Result<Box<dyn IndexStore>> {
    let var = env::var("FILES_INDEX_PATH").context("FILES_INDEX_PATH not set")?;
    let mut parts = var.splitn(2, ':');
    let kind = parts.next().unwrap_or("");
    let path = parts.next().ok_or_else(|| anyhow::anyhow!("FILES_INDEX_PATH must be in the form type:path"))?;
    match kind {
        "json" => Ok(Box::new(JsonStore::new(PathBuf::from(path)))),
        "sqlite" => Ok(Box::new(SqliteStore::new(PathBuf::from(path))?)),
        other => anyhow::bail!("unknown store type '{}', supported: json, sqlite", other),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let store = make_store_from_env()?;
    match cli.cmd {
        Commands::Add { path, tags } => {
            let t = parse_tags(&tags);
            store.add(&path, &t)?;
            println!("Added: {} with tags {}", path, t.join(","));
        }
        Commands::Get { tags } => {
            let t = parse_tags(&tags);
            let res = store.get(&t)?;
            for p in res { println!("{}", p); }
        }
    }
    Ok(())
}

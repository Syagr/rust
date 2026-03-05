//! CLI for indexing files by tags.
#![deny(
    missing_docs,
    rustdoc::missing_crate_level_docs,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::result_large_err
)]

use anyhow::{bail, Context, Result};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::Aes256Gcm;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use clap::{Parser, Subcommand};
use crossbeam_channel as channel;
use files_index_core::{IndexStore, JsonStore, SqliteStore};
use rand::rngs::OsRng;
use rand::RngCore;
use rayon::prelude::*;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};
use walkdir::WalkDir;
use std::{cell::{Cell, RefCell}, rc::Rc};

#[derive(Parser, Debug)]
#[command(author, version, about = "Files index CLI (Practical work 5)", long_about = None)]
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
    /// Generate matrices and compute sums in parallel
    Matrix {
        /// Matrix size N (N x N)
        #[arg(long, default_value_t = 4096)]
        size: usize,
        /// Number of matrices to generate
        #[arg(long, default_value_t = 1)]
        count: usize,
    },
    /// Encrypt files in a directory using AES-256-GCM
    Encrypt {
        /// Directory to scan
        #[arg(long)]
        dir: PathBuf,
        /// Base64-encoded 32-byte key (optional)
        #[arg(long)]
        key: Option<String>,
    },
    /// Decode, resize and re-encode images in single and multi-thread modes
    ProcessImages {
        /// Input directory with source images (recursive)
        #[arg(long)]
        dir: PathBuf,
        /// Output directory for processed images
        #[arg(long)]
        out: PathBuf,
        /// Target width
        #[arg(long, default_value_t = 800)]
        width: u32,
        /// Number of worker threads for threaded mode
        #[arg(long, default_value_t = 4)]
        workers: usize,
    },
    /// Analyze std types for Send/Sync behavior
    AnalyzeSync,
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

fn run_matrix(size: usize, count: usize) -> Result<()> {
    let (tx, rx) = channel::unbounded::<Arc<Vec<f32>>>();
    let rx2 = rx.clone();

    let consumer = |id: usize, rx: channel::Receiver<Arc<Vec<f32>>>| -> thread::JoinHandle<()> {
        thread::spawn(move || {
            while let Ok(mat) = rx.recv() {
                let sum: f32 = mat.par_iter().sum();
                println!("consumer {} sum: {}", id, sum);
            }
        })
    };

    let c1 = consumer(1, rx);
    let c2 = consumer(2, rx2);

    let tx_prod = tx.clone();
    let producer = thread::spawn(move || {
        for _ in 0..count {
            let mut data = Vec::with_capacity(size * size);
            for i in 0..(size * size) {
                data.push(i as f32);
            }
            let shared = Arc::new(data);
            if tx_prod.send(shared).is_err() {
                break;
            }
        }
    });

    producer.join().ok();
    drop(tx);
    c1.join().ok();
    c2.join().ok();

    Ok(())
}

fn decode_or_generate_key(key_b64: Option<String>) -> Result<Vec<u8>> {
    if let Some(k) = key_b64 {
        let decoded = B64
            .decode(k.trim())
            .context("failed to decode base64 key")?;
        if decoded.len() != 32 {
            bail!("key must be 32 bytes (base64-encoded)");
        }
        return Ok(decoded);
    }

    let mut key = vec![0u8; 32];
    OsRng.fill_bytes(&mut key);
    println!("Generated key (base64): {}", B64.encode(&key));
    Ok(key)
}

fn encrypt_bytes(cipher: &Aes256Gcm, plaintext: &[u8]) -> Result<Vec<u8>> {
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = aes_gcm::Nonce::from_slice(&nonce_bytes);
    let mut ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| anyhow::anyhow!("encryption failed"))?;
    let mut out = Vec::with_capacity(12 + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.append(&mut ciphertext);
    Ok(out)
}

fn output_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    PathBuf::from(format!("{}.data", s))
}

fn run_encrypt(dir: PathBuf, key_b64: Option<String>) -> Result<()> {
    let key = decode_or_generate_key(key_b64)?;
    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|_| anyhow::anyhow!("invalid key"))?;

    let (tx, rx) = channel::unbounded::<(PathBuf, Vec<u8>)>();
    let counter = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicBool::new(false));

    let producer = {
        let tx = tx.clone();
        thread::spawn(move || {
            for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
                if !entry.file_type().is_file() {
                    continue;
                }
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("data") {
                    continue;
                }
                match fs::read(path) {
                    Ok(bytes) => {
                        let _ = tx.send((path.to_path_buf(), bytes));
                    }
                    Err(_) => {
                        continue;
                    }
                }
            }
        })
    };

    let mut consumers = Vec::new();
    for _ in 0..3 {
        let rx = rx.clone();
        let cipher = cipher.clone();
        let counter = Arc::clone(&counter);
        consumers.push(thread::spawn(move || {
            while let Ok((path, data)) = rx.recv() {
                if let Ok(out) = encrypt_bytes(&cipher, &data) {
                    let out_path = output_path(&path);
                    if fs::write(&out_path, out).is_ok() {
                        counter.fetch_add(1, Ordering::SeqCst);
                    }
                }
            }
        }));
    }

    let monitor = {
        let counter = Arc::clone(&counter);
        let done = Arc::clone(&done);
        thread::spawn(move || {
            let mut last = 0usize;
            while !done.load(Ordering::SeqCst) {
                let current = counter.load(Ordering::SeqCst);
                if current != last {
                    println!("processed files: {}", current);
                    last = current;
                }
                thread::sleep(Duration::from_millis(200));
            }
            let final_count = counter.load(Ordering::SeqCst);
            if final_count != last {
                println!("processed files: {}", final_count);
            }
        })
    };

    producer.join().ok();
    drop(tx);
    for h in consumers {
        h.join().ok();
    }
    done.store(true, Ordering::SeqCst);
    monitor.join().ok();

    Ok(())
}

fn is_image_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
            .as_deref(),
        Some("jpg") | Some("jpeg") | Some("png") | Some("bmp") | Some("gif") | Some("webp")
    )
}

fn collect_image_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let p = entry.path();
            if is_image_path(p) {
                files.push(p.to_path_buf());
            }
        }
    }
    files
}

fn resized_output_path(input: &Path, out_dir: &Path) -> PathBuf {
    let name = input
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("image");
    out_dir.join(format!("resized_{}", name))
}

fn process_one_image(path: &Path, out_dir: &Path, width: u32) -> Result<()> {
    let img = image::open(path).with_context(|| format!("failed to decode {}", path.display()))?;
    let src_w = img.width();
    let src_h = img.height();
    let height = if src_w == 0 {
        1
    } else {
        ((src_h as u64 * width as u64) / src_w as u64).max(1) as u32
    };
    let resized = img.resize_exact(width, height, image::imageops::FilterType::Lanczos3);
    let out = resized_output_path(path, out_dir);
    resized
        .save(&out)
        .with_context(|| format!("failed to encode {}", out.display()))?;
    Ok(())
}

fn run_process_images(dir: PathBuf, out: PathBuf, width: u32, workers: usize) -> Result<()> {
    if workers == 0 {
        bail!("workers must be >= 1");
    }
    fs::create_dir_all(&out)
        .with_context(|| format!("failed to create output dir {}", out.display()))?;
    let out_single = out.join("single");
    let out_threaded = out.join("threaded");
    fs::create_dir_all(&out_single)
        .with_context(|| format!("failed to create output dir {}", out_single.display()))?;
    fs::create_dir_all(&out_threaded)
        .with_context(|| format!("failed to create output dir {}", out_threaded.display()))?;

    let files = collect_image_files(&dir);
    if files.is_empty() {
        println!("No image files found in {}", dir.display());
        return Ok(());
    }

    let start_single = Instant::now();
    for path in &files {
        process_one_image(path, &out_single, width)?;
    }
    let dur_single = start_single.elapsed();

    let start_threaded = Instant::now();
    let (tx, rx) = channel::unbounded::<PathBuf>();
    let mut handles = Vec::with_capacity(workers);
    for _ in 0..workers {
        let rx = rx.clone();
        let out_dir = out_threaded.clone();
        handles.push(thread::spawn(move || -> Result<()> {
            while let Ok(path) = rx.recv() {
                process_one_image(&path, &out_dir, width)?;
            }
            Ok(())
        }));
    }
    for path in &files {
        tx.send(path.clone())
            .with_context(|| format!("failed to send task for {}", path.display()))?;
    }
    drop(tx);
    for h in handles {
        h.join()
            .map_err(|_| anyhow::anyhow!("worker thread panicked"))??;
    }
    let dur_threaded = start_threaded.elapsed();

    let single_ms = dur_single.as_secs_f64() * 1000.0;
    let threaded_ms = dur_threaded.as_secs_f64() * 1000.0;
    let delta_pct = if single_ms > 0.0 {
        ((threaded_ms - single_ms) / single_ms) * 100.0
    } else {
        0.0
    };

    println!("single-threaded:  {:.2} ms", single_ms);
    println!("multi-threaded:   {:.2} ms", threaded_ms);
    println!("delta (%):        {:.2}", delta_pct);

    Ok(())
}

fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}

fn run_analyze_sync() -> Result<()> {
    // Positive compile-time checks.
    assert_send::<String>();
    assert_sync::<String>();
    assert_send::<std::sync::Arc<String>>();
    assert_sync::<std::sync::Arc<String>>();

    // These std types are intentionally not checked with assert_send/assert_sync
    // because they do NOT satisfy those traits:
    // - Rc<T>: !Send and !Sync
    // - Cell<T>: !Sync
    // - RefCell<T>: !Sync
    let _rc_example: Rc<i32> = Rc::new(10);
    let _cell_example: Cell<i32> = Cell::new(1);
    let _ref_cell_example: RefCell<i32> = RefCell::new(5);

    println!("Send/Sync analysis (std types):");
    println!("1) Rc<T>: !Send, !Sync (non-atomic refcount)");
    println!("2) Cell<T>: Send (depends on T), !Sync (interior mutability without sync)");
    println!("3) RefCell<T>: Send (depends on T), !Sync (runtime borrow rules are not thread-safe)");
    println!("4) Mutex<T>/Arc<T>: common thread-safe primitives for shared access");
    println!("5) Send = move value across threads; Sync = share &T across threads");

    Ok(())
}

fn run_from_args(argv: Vec<String>) -> Result<()> {
    let cli = Cli::parse_from(argv);

    match cli.cmd {
        Commands::Add { path, tags } => {
            let store = make_store_from_env()?;
            let list = parse_tags(&tags);
            store.add(&path, &list)?;
            println!("Added: {} with tags {}", path, list.join(", "));
        }
        Commands::Get { tags } => {
            let store = make_store_from_env()?;
            let list = parse_tags(&tags);
            let files = store.get(&list)?;
            for f in files {
                println!("{}", f);
            }
        }
        Commands::Matrix { size, count } => {
            run_matrix(size, count)?;
        }
        Commands::Encrypt { dir, key } => {
            run_encrypt(dir, key)?;
        }
        Commands::ProcessImages {
            dir,
            out,
            width,
            workers,
        } => {
            run_process_images(dir, out, width, workers)?;
        }
        Commands::AnalyzeSync => {
            run_analyze_sync()?;
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

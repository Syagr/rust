use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use std::thread;

use anyhow::Result;
use aes_gcm::{Aes256Gcm, Nonce, KeyInit};
use aes_gcm::aead::Aead;
use base64::{engine::general_purpose, Engine as _};
use clap::{Parser, Subcommand};
use crossbeam_channel::{unbounded, Receiver, Sender};
use rand::{RngCore, rngs::OsRng};
use rayon::prelude::*;
use walkdir::WalkDir;
use std::time::Instant;
use image::imageops::FilterType;

#[derive(Parser)]
#[command(name = "lab5")]
#[command(about = "Лабораторна робота 5: Потоки, канали та шифрування", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate matrices and have two worker threads compute sums
    Matrix {
        /// Matrix size (N for NxN). Default 4096.
        #[arg(long, default_value_t = 4096usize)]
        size: usize,

        /// How many matrices to generate and send
        #[arg(long, default_value_t = 2usize)]
        count: usize,
    },

    /// Walk directory, send file contents to 3 encrypting consumers
    Encrypt {
        /// Directory to walk
        #[arg(long, default_value = ".")]
        dir: String,

        /// Base64-encoded 32-byte key. If omitted, a key will be generated and printed.
        #[arg(long)]
        key: Option<String>,
    },

    /// Process images in a directory: compare single-threaded and threaded execution
    ProcessImages {
        /// Directory to read images from
        #[arg(long, default_value = ".")]
        dir: String,

        /// Output directory
        #[arg(long, default_value = "out")]
        out: String,

        /// Resize width in pixels (preserves aspect ratio)
        #[arg(long, default_value_t = 800u32)]
        width: u32,

        /// Worker threads for threaded mode
        #[arg(long, default_value_t = 4usize)]
        workers: usize,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Matrix { size, count } => run_matrix(size, count)?,
        Commands::Encrypt { dir, key } => run_encrypt(dir.into(), key)?,
        Commands::ProcessImages { dir, out, width, workers } => {
            process_images_cli(&dir, &out, width, workers)?;
        }
    }

    Ok(())
}

fn run_matrix(size: usize, count: usize) -> Result<()> {
    println!("Matrix size: {}x{}, count: {}", size, size, count);

    let (s1, r1): (Sender<Arc<Vec<f32>>>, Receiver<Arc<Vec<f32>>>) = unbounded();
    let (s2, r2): (Sender<Arc<Vec<f32>>>, Receiver<Arc<Vec<f32>>>) = unbounded();

    let consumer = |id: usize, rx: Receiver<Arc<Vec<f32>>>| {
        thread::spawn(move || {
            while let Ok(mat) = rx.recv() {
                let sum: f64 = mat.par_iter().map(|v| *v as f64).sum();
                println!("consumer {}: sum = {}", id, sum);
            }
            println!("consumer {}: channel closed", id);
        })
    };

    let h1 = consumer(1, r1);
    let h2 = consumer(2, r2);

    let prod = thread::spawn(move || {
        for i in 0..count {
            eprintln!("producer: generating matrix {}/{}", i + 1, count);
            let n = size.checked_mul(size).expect("size too large");
            let mut v = Vec::with_capacity(n);
            for _ in 0..n { v.push(1.0f32); }

            let arc = Arc::new(v);
            if let Err(e) = s1.send(arc.clone()) { eprintln!("send to s1 failed: {}", e); }
            if let Err(e) = s2.send(arc) { eprintln!("send to s2 failed: {}", e); }
        }
        drop(s1); drop(s2);
        eprintln!("producer: finished");
    });

    prod.join().expect("producer join failed");
    h1.join().expect("consumer1 join failed");
    h2.join().expect("consumer2 join failed");

    Ok(())
}

fn run_encrypt(dir: PathBuf, key_opt: Option<String>) -> Result<()> {
    let key_bytes = if let Some(kb64) = key_opt {
        general_purpose::STANDARD.decode(kb64)?
    } else {
        let mut k = vec![0u8; 32];
        OsRng.fill_bytes(&mut k);
        let printed = general_purpose::STANDARD.encode(&k);
        println!("Generated key (base64): {}", printed);
        k
    };

    if key_bytes.len() != 32 { anyhow::bail!("key must be 32 bytes (base64-decoded)"); }
    let cipher = Aes256Gcm::new_from_slice(&key_bytes).map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let (s, r) = unbounded::<(PathBuf, Vec<u8>)>();
    let counter = Arc::new(AtomicUsize::new(0));

    let cmon = counter.clone();
    let monitor = thread::spawn(move || {
        let mut last = cmon.load(Ordering::SeqCst);
        loop {
            let cur = cmon.load(Ordering::SeqCst);
            if cur != last { println!("processed files: {}", cur); last = cur; }
            if cur == usize::MAX { break; }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    });

    let mut handles = Vec::new();
    for id in 0..3 {
        let rx = r.clone();
        let ctr = counter.clone();
        let cipher = cipher.clone();
        let handle = thread::spawn(move || {
            while let Ok((path, bytes)) = rx.recv() {
                let mut nonce_bytes = [0u8; 12];
                OsRng.fill_bytes(&mut nonce_bytes);
                let nonce = Nonce::from_slice(&nonce_bytes);

                match cipher.encrypt(nonce, bytes.as_ref()) {
                    Ok(ct) => {
                        let mut out = Vec::new();
                        out.extend_from_slice(&nonce_bytes);
                        out.extend_from_slice(&ct);
                        let mut out_path = path.with_extension("");
                        out_path.set_file_name(format!("{}.data", path.file_name().and_then(|n| n.to_str()).unwrap_or("out")));
                        if let Err(e) = fs::write(&out_path, &out) {
                            eprintln!("consumer {}: write {} failed: {}", id, out_path.display(), e);
                        } else { ctr.fetch_add(1, Ordering::SeqCst); }
                    }
                    Err(e) => eprintln!("consumer {}: encrypt error: {}", id, e),
                }
            }
            eprintln!("consumer {}: exiting", id);
        });
        handles.push(handle);
    }

    let walker = thread::spawn(move || {
        for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.is_file() {
                match fs::read(p) {
                    Ok(bytes) => { let path = p.to_path_buf(); if let Err(e) = s.send((path, bytes)) { eprintln!("producer: send failed: {}", e); } }
                    Err(e) => eprintln!("producer: read {} failed: {}", p.display(), e),
                }
            }
        }
        drop(s);
    });

    walker.join().expect("walker thread panicked");
    for h in handles { h.join().expect("consumer panicked"); }
    counter.store(usize::MAX, Ordering::SeqCst);
    std::thread::sleep(std::time::Duration::from_millis(300));
    let _ = monitor.join();

    Ok(())
}

fn process_images_cli(dir: &str, out: &str, width: u32, workers: usize) -> Result<()> {
    let indir = std::path::Path::new(dir);
    let outdir = std::path::Path::new(out);
    std::fs::create_dir_all(outdir)?;

    let mut files = Vec::new();
    for entry in WalkDir::new(indir).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.is_file() {
            if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                let ext_l = ext.to_ascii_lowercase();
                if ["jpg", "jpeg", "png", "bmp", "tiff", "webp"].contains(&ext_l.as_str()) { files.push(p.to_path_buf()); }
            }
        }
    }

    println!("Found {} image files", files.len());

    let start_single = Instant::now();
    for p in &files { if let Err(e) = process_single(p, outdir, width) { eprintln!("single: failed {}: {}", p.display(), e); } }
    let dur_single = start_single.elapsed();
    println!("Single-threaded duration: {:.3}s", dur_single.as_secs_f64());

    let start_threaded = Instant::now();
    process_threaded(files, outdir.to_path_buf(), width, workers)?;
    let dur_threaded = start_threaded.elapsed();
    println!("Threaded duration ({} workers): {:.3}s", workers, dur_threaded.as_secs_f64());

    let single_ms = dur_single.as_secs_f64();
    let threaded_ms = dur_threaded.as_secs_f64();
    if single_ms > 0.0 { let diff = (threaded_ms - single_ms) / single_ms * 100.0; println!("Time change: {:+.2}% (threaded vs single)", diff); }

    Ok(())
}

fn process_single(path: &std::path::Path, outdir: &std::path::Path, width: u32) -> Result<()> {
    let img = image::open(path)?;
    let h = (img.height() as f32 * (width as f32 / img.width() as f32)) as u32;
    let resized = img.resize(width, h, FilterType::Lanczos3);
    let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("out");
    let outpath = outdir.join(format!("{}_processed", fname));
    resized.save(&outpath)?;
    Ok(())
}

fn process_threaded(files: Vec<std::path::PathBuf>, outdir: std::path::PathBuf, width: u32, workers: usize) -> Result<()> {
    let (s, r) = unbounded::<std::path::PathBuf>();
    let counter = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    for id in 0..workers {
        let rx = r.clone();
        let out = outdir.clone();
        let ctr = counter.clone();
        let handle = thread::spawn(move || {
            while let Ok(p) = rx.recv() {
                // perform image processing and log errors locally (avoid returning errors across threads)
                match image::open(&p) {
                    Ok(img) => {
                        let h = (img.height() as f32 * (width as f32 / img.width() as f32)) as u32;
                        let resized = img.resize(width, h, FilterType::Lanczos3);
                        let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("out");
                        let outpath = out.join(format!("{}_processed", fname));
                        if let Err(e) = resized.save(&outpath) {
                            eprintln!("worker {}: save failed {}: {}", id, outpath.display(), e);
                        }
                    }
                    Err(e) => eprintln!("worker {}: open failed {}: {}", id, p.display(), e),
                }
                ctr.fetch_add(1, Ordering::SeqCst);
            }
        });
        handles.push(handle);
    }

    for p in files { s.send(p)?; }
    drop(s);

    for h in handles { let _ = h.join(); }

    println!("Processed {} files", counter.load(Ordering::SeqCst));
    Ok(())
}

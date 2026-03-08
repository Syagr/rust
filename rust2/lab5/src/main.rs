use anyhow::Result;
use aes_gcm::{Aes256Gcm, Nonce, KeyInit};
use aes_gcm::aead::Aead;
use base64::{engine::general_purpose, Engine as _};
use clap::{Parser, Subcommand};
use rayon::prelude::*;
use rand::{RngCore, rngs::OsRng};
use walkdir::WalkDir;
use std::sync::Arc;
use std::path::{PathBuf, Path};
use std::time::Instant;
use std::thread;
// stdfs removed (unused)
use image::imageops::FilterType;
use image::{ImageBuffer, Rgba, DynamicImage, ImageOutputFormat};
use std::io::Cursor;
use futures::stream::{self, StreamExt};
use tokio::task::spawn_blocking;
use tokio::fs;

#[derive(Parser)]
#[command(name = "lab5")]
#[command(about = "Лабораторна робота 5: Потоки, канали та шифрування (async refactor)", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Matrix {
        #[arg(long, default_value_t = 4096usize)]
        size: usize,
        #[arg(long, default_value_t = 2usize)]
        count: usize,
    },

    Encrypt {
        #[arg(long, default_value = ".")]
        dir: String,
        #[arg(long)]
        key: Option<String>,
    },

    ProcessImages {
        #[arg(long, default_value = ".")]
        dir: String,
        #[arg(long, default_value = "out")]
        out: String,
        #[arg(long, default_value_t = 800u32)]
        width: u32,
        #[arg(long, default_value_t = 4usize)]
        workers: usize,
    },
    GenImages {
        #[arg(long, default_value = "sample_images")]
        out: String,
        #[arg(long, default_value_t = 2usize)]
        count: usize,
    },
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Matrix { size, count } => {
            spawn_blocking(move || run_matrix(size, count)).await??;
        }
        Commands::Encrypt { dir, key } => {
            run_encrypt_async(PathBuf::from(dir), key).await?;
        }
        Commands::ProcessImages { dir, out, width, workers } => {
            process_images_async(dir, out, width, workers).await?;
        }
        Commands::GenImages { out, count } => {
            gen_images_async(out, count).await?;
        }
    }

    Ok(())
}

fn run_matrix(size: usize, count: usize) -> Result<()> {
    println!("Matrix size: {}x{}, count: {}", size, size, count);

    let (s1, r1) = crossbeam_channel::unbounded::<Arc<Vec<f32>>>();
    let (s2, r2) = crossbeam_channel::unbounded::<Arc<Vec<f32>>>();

    let consumer = |id: usize, rx: crossbeam_channel::Receiver<Arc<Vec<f32>>>| {
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
            let _ = s1.send(arc.clone());
            let _ = s2.send(arc);
        }
        drop(s1); drop(s2);
        eprintln!("producer: finished");
    });

    prod.join().expect("producer join failed");
    h1.join().expect("consumer1 join failed");
    h2.join().expect("consumer2 join failed");

    Ok(())
}

async fn run_encrypt_async(dir: PathBuf, key_opt: Option<String>) -> Result<()> {
    // collect files (blocking traversal)
    let files: Vec<PathBuf> = tokio::task::spawn_blocking(move || {
        let mut v = Vec::new();
        for entry in WalkDir::new(&dir).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.is_file() { v.push(p.to_path_buf()); }
        }
        v
    }).await?;

    // Prepare key
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

    // Async-read files with limited concurrency
    let read_futs = stream::iter(files.into_iter()).map(|p| async move {
        let bytes = fs::read(&p).await;
        (p, bytes)
    }).buffer_unordered(16);

    let mut reads: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    tokio::pin!(read_futs);
    while let Some((path, res)) = read_futs.next().await {
        match res {
            Ok(b) => reads.push((path, b)),
            Err(e) => eprintln!("read {} failed: {}", path.display(), e),
        }
    }

    // Encrypt in blocking pool (CPU-bound)
    let mut enc_tasks = Vec::new();
    for (path, bytes) in reads {
        let cipher = cipher.clone();
        enc_tasks.push(tokio::task::spawn_blocking(move || -> Result<(PathBuf, Vec<u8>)> {
            let mut nonce_bytes = [0u8; 12];
            OsRng.fill_bytes(&mut nonce_bytes);
            let nonce = Nonce::from_slice(&nonce_bytes);
            let ct = cipher.encrypt(nonce, bytes.as_ref()).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let mut out = Vec::new();
            out.extend_from_slice(&nonce_bytes);
            out.extend_from_slice(&ct);
            let mut out_path = path.with_extension("");
            out_path.set_file_name(format!("{}.data", path.file_name().and_then(|n| n.to_str()).unwrap_or("out")));
            Ok((out_path, out))
        }));
    }

    for t in enc_tasks {
        match t.await {
            Ok(Ok((out_path, out_bytes))) => {
                if let Err(e) = fs::write(&out_path, &out_bytes).await {
                    eprintln!("write {} failed: {}", out_path.display(), e);
                }
            }
            Ok(Err(e)) => eprintln!("encryption error: {}", e),
            Err(e) => eprintln!("spawn blocking join error: {}", e),
        }
    }

    println!("Encryption finished");
    Ok(())
}

async fn process_images_async(dir: String, out: String, width: u32, workers: usize) -> Result<()> {
    let indir = Path::new(&dir).to_path_buf();
    let outdir = Path::new(&out).to_path_buf();
    fs::create_dir_all(&outdir).await.ok();

    // collect image files (blocking traversal)
    let files: Vec<PathBuf> = tokio::task::spawn_blocking(move || {
        let mut v = Vec::new();
        for entry in WalkDir::new(&indir).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.is_file() {
                if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                    let ext_l = ext.to_ascii_lowercase();
                    if ["jpg","jpeg","png","bmp","tiff","webp"].contains(&ext_l.as_str()) { v.push(p.to_path_buf()); }
                }
            }
        }
        v
    }).await?;

    println!("Found {} image files", files.len());

    let start_single = Instant::now();
    for p in &files {
        if let Err(e) = process_image_file(p.clone(), outdir.clone(), width).await {
            eprintln!("single: failed {}: {}", p.display(), e);
        }
    }
    let dur_single = start_single.elapsed();
    println!("Single-threaded duration: {:.3}s", dur_single.as_secs_f64());

    let start_threaded = Instant::now();
    let sem = Arc::new(tokio::sync::Semaphore::new(workers));
    let mut tasks = Vec::new();
    for p in files {
        let permit = sem.clone().acquire_owned().await.unwrap();
        let outdir = outdir.clone();
        tasks.push(tokio::spawn(async move {
            let _permit = permit;
            let _ = process_image_file(p, outdir, width).await;
        }));
    }
    for t in tasks { let _ = t.await; }
    let dur_threaded = start_threaded.elapsed();
    println!("Threaded duration ({} workers): {:.3}s", workers, dur_threaded.as_secs_f64());

    Ok(())
}

async fn gen_images_async(out: String, count: usize) -> Result<()> {
    let outdir = Path::new(&out).to_path_buf();
    fs::create_dir_all(&outdir).await?;

    for i in 0..count {
        let outpath = outdir.join(format!("img{}.png", i + 1));
        let buf = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
            let img = ImageBuffer::from_pixel(100, 100, Rgba([255, 0, 0, 255]));
            let dynimg = DynamicImage::ImageRgba8(img);
            let mut buf = Vec::new();
            dynimg.write_to(&mut Cursor::new(&mut buf), ImageOutputFormat::Png)?;
            Ok(buf)
        }).await??;
        fs::write(&outpath, &buf).await?;
    }

    println!("Generated {} images into {}", count, out);
    Ok(())
}

async fn process_image_file(path: PathBuf, outdir: PathBuf, width: u32) -> Result<()> {
    let bytes = fs::read(&path).await?;
    let res = tokio::task::spawn_blocking(move || -> Result<(String, Vec<u8>)> {
        let img = image::load_from_memory(&bytes)?;
        let h = (img.height() as f32 * (width as f32 / img.width() as f32)) as u32;
        let resized = img.resize(width, h, FilterType::Lanczos3);
        let mut buf = Vec::new();
        resized.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageOutputFormat::Jpeg(80))?;
        let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("out").to_string();
        Ok((fname, buf))
    }).await?;
    let (fname, out_bytes) = res?;
    let outpath = outdir.join(format!("{}_processed.jpg", fname));
    fs::write(&outpath, &out_bytes).await?;
    Ok(())
}

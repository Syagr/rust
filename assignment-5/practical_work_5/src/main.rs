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

#[derive(Parser)]
#[command(name = "practical_work_5")]
#[command(about = "Практична робота 5: Потоки, канали та шифрування", long_about = None)]
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Matrix { size, count } => run_matrix(size, count)?,
        Commands::Encrypt { dir, key } => run_encrypt(dir.into(), key)?,
    }

    Ok(())
}

fn run_matrix(size: usize, count: usize) -> Result<()> {
    println!("Matrix size: {}x{}, count: {}", size, size, count);

    // Create two channels (one per consumer) so producer can broadcast
    let (s1, r1): (Sender<Arc<Vec<f32>>>, Receiver<Arc<Vec<f32>>>) = unbounded();
    let (s2, r2): (Sender<Arc<Vec<f32>>>, Receiver<Arc<Vec<f32>>>) = unbounded();

    // Consumer threads
    let consumer = |id: usize, rx: Receiver<Arc<Vec<f32>>>| {
        thread::spawn(move || {
            while let Ok(mat) = rx.recv() {
                // Parallel sum using rayon
                let sum: f64 = mat.par_iter().map(|v| *v as f64).sum();
                println!("consumer {}: sum = {}", id, sum);
            }
            println!("consumer {}: channel closed", id);
        })
    };

    let h1 = consumer(1, r1);
    let h2 = consumer(2, r2);

    // Producer thread
    let prod = thread::spawn(move || {
        for i in 0..count {
            eprintln!("producer: generating matrix {}/{}", i + 1, count);
            // Create NxN matrix as flat Vec
            let n = size.checked_mul(size).expect("size too large");
            let mut v = Vec::with_capacity(n);
            // Fill with some values (e.g., 1.0)
            for _ in 0..n {
                v.push(1.0f32);
            }

            let arc = Arc::new(v);
            // Broadcast by cloning the Arc and sending to each consumer
            if let Err(e) = s1.send(arc.clone()) {
                eprintln!("send to s1 failed: {}", e);
            }
            if let Err(e) = s2.send(arc) {
                eprintln!("send to s2 failed: {}", e);
            }
        }
        // drop senders to close channels
        drop(s1);
        drop(s2);
        eprintln!("producer: finished");
    });

    prod.join().expect("producer join failed");
    h1.join().expect("consumer1 join failed");
    h2.join().expect("consumer2 join failed");

    Ok(())
}

fn run_encrypt(dir: PathBuf, key_opt: Option<String>) -> Result<()> {
    // Prepare key: 32 bytes
    let key_bytes = if let Some(kb64) = key_opt {
        general_purpose::STANDARD.decode(kb64)?
    } else {
        let mut k = vec![0u8; 32];
        OsRng.fill_bytes(&mut k);
        let printed = general_purpose::STANDARD.encode(&k);
        println!("Generated key (base64): {}", printed);
        k
    };

    if key_bytes.len() != 32 {
        anyhow::bail!("key must be 32 bytes (base64-decoded)");
    }

    let cipher = Aes256Gcm::new_from_slice(&key_bytes).map_err(|e| anyhow::anyhow!(e.to_string()))?;

    // channel for file jobs (path, bytes)
    let (s, r) = unbounded::<(PathBuf, Vec<u8>)>();

    // Atomic counter of processed files
    let counter = Arc::new(AtomicUsize::new(0));

    // Spawn monitor thread
    let cmon = counter.clone();
    let monitor = thread::spawn(move || {
        let mut last = cmon.load(Ordering::SeqCst);
        loop {
            let cur = cmon.load(Ordering::SeqCst);
            if cur != last {
                println!("processed files: {}", cur);
                last = cur;
            }
            if cur == usize::MAX { break; }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    });

    // Create 3 consumer threads
    let mut handles = Vec::new();
    for id in 0..3 {
        let rx = r.clone();
        let ctr = counter.clone();
        let cipher = cipher.clone();
        let handle = thread::spawn(move || {
            while let Ok((path, bytes)) = rx.recv() {
                // generate nonce
                let mut nonce_bytes = [0u8; 12];
                OsRng.fill_bytes(&mut nonce_bytes);
                let nonce = Nonce::from_slice(&nonce_bytes);

                match cipher.encrypt(nonce, bytes.as_ref()) {
                    Ok(ct) => {
                        let mut out = Vec::new();
                        out.extend_from_slice(&nonce_bytes);
                        out.extend_from_slice(&ct);
                        let out_path = path.with_extension("");
                        let mut out_path = out_path.clone();
                        out_path.set_file_name(format!("{}.data", path.file_name().and_then(|n| n.to_str()).unwrap_or("out")));
                        if let Err(e) = fs::write(&out_path, &out) {
                            eprintln!("consumer {}: write {} failed: {}", id, out_path.display(), e);
                        } else {
                            // increment counter
                            ctr.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                    Err(e) => eprintln!("consumer {}: encrypt error: {}", id, e),
                }
            }
            eprintln!("consumer {}: exiting", id);
        });
        handles.push(handle);
    }

    // Producer: walk directory recursively and send file contents
    let walker = thread::spawn(move || {
        for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.is_file() {
                match fs::read(p) {
                    Ok(bytes) => {
                        let path = p.to_path_buf();
                        if let Err(e) = s.send((path, bytes)) {
                            eprintln!("producer: send failed: {}", e);
                        }
                    }
                    Err(e) => eprintln!("producer: read {} failed: {}", p.display(), e),
                }
            }
        }
        // drop sender to close channel
        drop(s);
    });

    // Wait for producer
    walker.join().expect("walker thread panicked");

    // Wait for consumers to finish
    for h in handles {
        h.join().expect("consumer panicked");
    }

    // Signal monitor to stop by setting counter to usize::MAX and give it a moment
    counter.store(usize::MAX, Ordering::SeqCst);
    std::thread::sleep(std::time::Duration::from_millis(300));
    let _ = monitor.join();

    Ok(())
}

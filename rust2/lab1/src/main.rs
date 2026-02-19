use std::env;
use std::fs::{self, File};
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use image::imageops::FilterType;

/// Image editor CLI: reads a text file with URLs or local paths, resizes images, saves to MYME_FILES_PATH
#[derive(Parser, Debug)]
#[command(author, version, about = "Synchronous image resizing CLI", long_about = None)]
struct Args {
    /// Path to a text file containing one image URL or local path per line
    #[arg(long, value_name = "FILE")]
    files: PathBuf,

    /// Resize in the form WIDTHxHEIGHT (e.g. 800x600)
    #[arg(long, value_name = "WxH")]
    resize: String,
}

fn parse_resize(s: &str) -> Result<(u32, u32)> {
    let mut parts = s.split('x');
    let w = parts
        .next()
        .context("missing width")?
        .parse::<u32>()
        .context("invalid width")?;
    let h = parts
        .next()
        .context("missing height")?
        .parse::<u32>()
        .context("invalid height")?;
    Ok((w, h))
}

fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

fn filename_from_url(s: &str) -> Option<String> {
    s.rsplit('/')
        .find(|part| !part.is_empty())
        .map(|s| s.to_string())
}

fn process_line(line: &str, out_dir: &Path, size: (u32, u32)) -> Result<()> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(());
    }

    let img = if is_url(line) {
        let resp = reqwest::blocking::get(line)
            .with_context(|| format!("failed to GET '{}'", line))?;
        let bytes = resp.bytes().context("reading response bytes")?;
        image::load_from_memory(bytes.as_ref()).context("decoding image from bytes")?
    } else {
        image::open(line).with_context(|| format!("opening local image '{}'", line))?
    };

    let resized = img.resize(size.0, size.1, FilterType::Lanczos3);

    // determine output filename
    let out_name = if is_url(line) {
        filename_from_url(line).unwrap_or_else(|| "downloaded.png".to_string())
    } else {
        Path::new(line)
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "image.png".to_string())
    };

    let mut out_path = out_dir.join(out_name);
    if out_path.extension().is_none() {
        out_path.set_extension("png");
    }

    resized
        .save(&out_path)
        .with_context(|| format!("saving image to {}", out_path.display()))?;

    println!("Saved: {}", out_path.display());
    Ok(())
}

fn run() -> Result<()> {
    let args = Args::parse();
    let (w, h) = parse_resize(&args.resize)?;

    let out_dir = env::var("MYME_FILES_PATH").context("MYME_FILES_PATH not set")?;
    let out_dir = Path::new(&out_dir);
    fs::create_dir_all(out_dir).context("creating output directory")?;

    let f = File::open(&args.files).with_context(|| format!("opening file {}", args.files.display()))?;
    let reader = io::BufReader::new(f);

    for (idx, line) in reader.lines().enumerate() {
        let line = line.context("reading line")?;
        let trimmed = line.trim();
        // skip empty lines and comments starting with '#'
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Err(e) = process_line(trimmed, out_dir, (w, h)) {
            eprintln!("[{idx}] error processing '{}': {:#}", trimmed, e);
        }
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}

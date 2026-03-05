use clap::Parser;
use futures::future::join_all;
use reqwest::Client;
use std::path::PathBuf;
use std::sync::Arc;
use std::io::Read;
use tokio::fs;
use tokio::sync::Semaphore;

/// Простий асинхронний завантажувач веб-сторінок
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Максимальна кількість одночасних задач (якщо відсутня — кількість ядер)
    #[arg(long = "max-threads")]
    max_threads: Option<usize>,

    /// Файл зі списком URL по одному в рядку. Якщо відсутній — читаємо зі stdin.
    file: Option<PathBuf>,
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();
    let max = args
        .max_threads
        .unwrap_or_else(|| num_cpus::get().max(1));

    // Read URLs from file or stdin (do blocking read before starting runtime)
    let urls: Vec<String> = if let Some(path) = args.file {
        // read file using tokio fs inside runtime below; here we use std blocking read for simplicity
        let s = std::fs::read_to_string(path)?;
        s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()
    } else {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()
    };

    if urls.is_empty() {
        eprintln!("No URLs provided");
        return Ok(());
    }

    // Build a Tokio runtime sized by `max` worker threads
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(max)
        .enable_all()
        .build()?;

    // Run async work inside the runtime
    rt.block_on(async move {
        fs::create_dir_all("out").await.ok();

        let client = Arc::new(Client::builder().build()?);
        let sem = Arc::new(Semaphore::new(max));

        println!("Starting downloads with max concurrency = {}", max);

        // spawn tasks, each acquires a permit
        let mut handles = Vec::with_capacity(urls.len());
        for (i, url) in urls.into_iter().enumerate() {
            let client = Arc::clone(&client);
            let sem = Arc::clone(&sem);
            let url_str = url.clone();
            let filename = format!("out/{}_{}.html", sanitize_filename(&url_str), i);
            let handle = tokio::spawn(async move {
                // acquire permit
                let _permit = sem.acquire().await.unwrap();
                match fetch_and_save(&client, &url_str, &filename).await {
                    Ok(sz) => println!("OK: {} -> {} bytes", url_str, sz),
                    Err(e) => eprintln!("ERR: {} -> {}", url_str, e),
                }
            });
            handles.push(handle);
        }

        // wait for all
        let _ = join_all(handles).await;

        println!("All done");
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    })?;

    Ok(())
}

async fn fetch_and_save(
    client: &Client,
    url: &str,
    filename: &str,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    let resp = client.get(url).send().await?;
    let bytes = resp.bytes().await?;
    let data = bytes.to_vec();
    fs::write(filename, &data).await?;
    Ok(data.len())
}

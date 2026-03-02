use lab6_sync_primitives::{MyArc, MyMutex};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::thread;
use std::env;

fn run_std_arc_mutex(threads: usize, increments: usize) -> (f64, usize) {
    let counter = Arc::new(Mutex::new(0usize));
    let start = Instant::now();
    let mut handles = Vec::new();
    for _ in 0..threads {
        let c = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            for _ in 0..increments {
                let mut g = c.lock().unwrap();
                *g += 1;
            }
        }));
    }
    for h in handles { h.join().unwrap(); }
    let final_value = *counter.lock().unwrap();
    (start.elapsed().as_secs_f64(), final_value)
}

fn run_my_arc_mutex(threads: usize, increments: usize) -> (f64, usize) {
    let counter = MyArc::new(MyMutex::new(0usize));
    let start = Instant::now();
    let mut handles = Vec::new();
    for _ in 0..threads {
        let c = counter.clone();
        handles.push(thread::spawn(move || {
            for _ in 0..increments {
                let mut g = c.lock();
                *g += 1;
            }
        }));
    }
    for h in handles { h.join().unwrap(); }
    let final_value = *counter.lock();
    (start.elapsed().as_secs_f64(), final_value)
}

fn main() {
    let threads = env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or_else(|| num_cpus::get());
    let increments = env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(50_000usize);
    let expected = threads * increments;

    println!("Threads: {}, increments per thread: {}", threads, increments);
    println!("Expected final counter: {}", expected);

    let (std_time, std_count) = run_std_arc_mutex(threads, increments);
    println!("std Arc+Mutex time: {:.6} s", std_time);
    println!("std Arc+Mutex final counter: {}", std_count);

    let (my_time, my_count) = run_my_arc_mutex(threads, increments);
    println!("my Arc+Mutex time:  {:.6} s", my_time);
    println!("my Arc+Mutex final counter:  {}", my_count);

    if std_count != expected || my_count != expected {
        eprintln!("Benchmark correctness check failed: unexpected final counter value");
        std::process::exit(1);
    }

    let pct = (my_time - std_time) / std_time * 100.0;
    println!("Percentage change (my vs std): {:+.2}%", pct);
}

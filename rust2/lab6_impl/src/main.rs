use lab6_sync_primitives::{MyArc, MyMutex};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::thread;
use std::env;

fn run_std_arc_mutex(threads: usize, increments: usize) -> f64 {
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
    start.elapsed().as_secs_f64()
}

fn run_my_arc_mutex(threads: usize, increments: usize) -> f64 {
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
    start.elapsed().as_secs_f64()
}

fn main() {
    let threads = env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or_else(|| num_cpus::get());
    let increments = env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(50_000usize);

    println!("Threads: {}, increments per thread: {}", threads, increments);

    let std_time = run_std_arc_mutex(threads, increments);
    println!("std Arc+Mutex time: {:.6} s", std_time);

    let my_time = run_my_arc_mutex(threads, increments);
    println!("my Arc+Mutex time:  {:.6} s", my_time);

    let pct = (my_time - std_time) / std_time * 100.0;
    println!("Percentage change (my vs std): {:+.2}%", pct);
}

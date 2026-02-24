use std::env;

mod prakt2;

fn main() {
    // Run practical work 2 CLI directly from the shared crate's `main`.
    let args: Vec<String> = env::args().collect();
    if let Err(e) = prakt2::run_from_args(args) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

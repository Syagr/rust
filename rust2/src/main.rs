use std::env;

mod prakt2;

fn main() {
    // If the first CLI arg is "lab2", dispatch to the lab2 CLI implementation.
    let mut args: Vec<String> = env::args().collect();
    if args.len() > 1 && args[1] == "prakt2" {
        // remove the "prakt2" token and pass the rest to prakt2 parser
        args.remove(1);
        if let Err(e) = prakt2::run_from_args(args) {
            eprintln!("prakt2 error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    // Default demo when not running a lab: print a short help
    println!("Run a lab via: cargo run --bin rust2 -- lab2 --help");
}

//! The two BrokenPipe fixes, side by side.  Run each with its output cut short:
//!
//! ```text
//! cargo run --example broken_pipe -- list | head -3   # ends quietly once head exits (Unix)
//! cargo run --example broken_pipe -- work | head -3   # finishes all its work anyway
//! ```

use std::io::{self, Write};

use cli_contract::{exit, ignore_broken_pipe, reset_sigpipe};

const ITEMS: u32 = 100_000;

fn main() -> io::Result<()> {
    match std::env::args().nth(1).as_deref() {
        // A read-only enumerator: the output is the product, so dying when no one reads is fine.
        Some("list") => {
            reset_sigpipe();
            for n in 1..=ITEMS {
                println!("item {n}");
            }
        }
        // A mutating tool with progress: the work must finish even if the pipe closes.
        Some("work") => {
            let mut out = io::stdout().lock();
            let mut done = 0;
            for n in 1..=ITEMS {
                done += 1; // stands in for real work, which a closed pipe must never interrupt
                ignore_broken_pipe(writeln!(out, "did item {n}"))?;
            }
            eprintln!("finished all {done} items");
        }
        _ => {
            eprintln!("usage: broken_pipe list|work");
            std::process::exit(exit::USAGE);
        }
    }
    Ok(())
}

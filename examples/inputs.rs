//! The input contract: a caller-asserted input must exist, and a marker proves it is the one meant.
//!
//! ```text
//! cargo run --example inputs -- <path>            # must exist; never created
//! cargo run --example inputs -- <dir> <marker>    # must exist AND contain the marker
//! ```

use std::path::Path;

use cli_contract::{exit, require_input_path, require_marker, MarkerName};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [path] => require_input_path(Path::new(path)),
        [dir, marker] => {
            // A marker that is not a name is wrong on its face: exit 2, before looking at the disk.
            // The error carries no exit code; from a config file it would be exit 1.
            let marker = marker.parse::<MarkerName>().unwrap_or_else(|e| {
                eprintln!("error: {e}");
                std::process::exit(exit::USAGE.into());
            });
            // Refuses a missing directory, or a file, as `require_input_dir` does; then the marker.
            require_marker(Path::new(dir), &marker)
        }
        _ => {
            eprintln!("usage: inputs <path>  |  inputs <dir> <marker>");
            std::process::exit(exit::USAGE.into());
        }
    };
    // Every path-contract error names the path and the fact on disk, and exits 1.
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(e.exit_code().into());
    }
    println!("ok");
}

//! The input contract: a caller-asserted input must exist, and a marker proves it is the one meant.
//!
//! ```text
//! cargo run --example inputs -- <path>            # must exist; never created
//! cargo run --example inputs -- <dir> <marker>    # must exist AND contain the marker
//! ```

use std::path::Path;

use cli_contract::{
    exit, require_input_dir, require_input_path, CreateMode, DestPolicy, PolicyError,
};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [path] => require_input_path(Path::new(path)),
        [dir, marker] => {
            // A marker that is not a name is wrong on its face: exit 2, before looking at the disk.
            if let Err(PolicyError::MarkerNotAName { fault, .. }) =
                DestPolicy::from_flags(CreateMode::None, Some(marker.clone()))
            {
                eprintln!("error: the marker '{marker}' is not a name: the value {fault}");
                std::process::exit(exit::USAGE);
            }
            require_input_dir(Path::new(dir), Some(marker.as_str()))
        }
        _ => {
            eprintln!("usage: inputs <path>  |  inputs <dir> <marker>");
            std::process::exit(exit::USAGE);
        }
    };
    // Every path-contract error names the path and the fact on disk, and exits 1.
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(e.exit_code());
    }
    println!("ok");
}

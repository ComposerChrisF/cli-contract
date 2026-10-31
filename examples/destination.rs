//! The output-directory contract, with `--dry-run`: the check alone, or the check and then the
//! create.
//!
//! ```text
//! cargo run --example destination -- [--create-destination none|final|all]
//!     [--require-marker <name>] [--dry-run] <dir>
//! ```
//!
//! The flags are parsed by hand, to show the `FromStr` path any argument parser can use.  With the
//! `clap` feature, `CreateMode` is also a `clap::ValueEnum`.

use std::path::PathBuf;

use cli_contract::{
    check_destination, ensure_destination, exit, CreateMode, DestPolicy, MarkerName,
};

fn main() {
    let mut mode = CreateMode::None;
    let mut marker: Option<MarkerName> = None;
    let mut dry_run = false;
    let mut dest = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--create-destination" => match args.next().map(|value| value.parse()) {
                Some(Ok(parsed)) => mode = parsed,
                Some(Err(e)) => usage(&format!("--create-destination: {e}")),
                None => usage("--create-destination needs a value"),
            },
            // A marker that is not a name is wrong on its face: exit 2, before looking at the disk.
            "--require-marker" => match args.next().map(|value| value.parse()) {
                Some(Ok(parsed)) => marker = Some(parsed),
                Some(Err(e)) => usage(&format!("--require-marker: {e}")),
                None => usage("--require-marker needs a value"),
            },
            "--dry-run" => dry_run = true,
            _ if dest.is_none() && !arg.starts_with('-') => dest = Some(PathBuf::from(arg)),
            _ => usage(&format!("unexpected argument: {arg}")),
        }
    }
    let Some(dest) = dest else {
        usage("a destination directory is required")
    };

    // A marker with a creating mode is wrong on its face: exit 2.
    let policy = DestPolicy::from_flags(mode, marker).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(e.exit_code().into());
    });
    // The same contract verdicts either way; only the live run writes.
    let result = if dry_run {
        check_destination(&dest, &policy)
    } else {
        ensure_destination(&dest, &policy)
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(e.exit_code().into());
    }
    if dry_run {
        println!("ok: {} (dry run: nothing was created)", dest.display());
    } else {
        println!("ok: {}", dest.display());
    }
}

fn usage(message: &str) -> ! {
    eprintln!("error: {message}");
    eprintln!(
        "usage: destination [--create-destination none|final|all] [--require-marker <name>] \
         [--dry-run] <dir>"
    );
    std::process::exit(exit::USAGE.into())
}

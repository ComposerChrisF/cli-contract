# Changelog

All notable changes to `cli-contract` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0/).

## [0.6.0] - 2026-10-31

The first public release.

### Added

- **Exit codes:** the constants `OK`, `TOOL_ERROR`, `USAGE`, `FINDINGS`, and `SETUP` (0–4), and `HELP_BLOCK`, the table ready for clap’s `after_long_help`.
- **The path contract:**
  - inputs: `require_input_path`, `require_input_dir`, and `require_marker`;
  - output directories: `check_destination` (the dry run) and `ensure_destination` (the live run), under a `DestPolicy` built from `CreateMode` and an optional marker by `DestPolicy::from_flags`;
  - output files: `prepare_output_file`, with `Clobber`;
  - the mount-root floor, `is_mount_root`, which no mode can override;
  - symlink primitives for a caller’s own policy: `reject_symlink` and `symlink_info`.
- **BrokenPipe:** `reset_sigpipe` for a tool whose output is the product, and `ignore_broken_pipe` (with `is_broken_pipe`) for a tool that must finish its work when its output pipe closes.
- **The `clap` feature:** `clap::ValueEnum` for `CreateMode` and `Clobber`.  Both implement `FromStr` and `Display` with the same spellings whether or not the feature is on.
- Runnable examples in `examples/`.

### Changed

- **`clap` is no longer a default feature.**  A tool that takes `CreateMode` or `Clobber` as a clap `value_enum` adds `features = ["clap"]`.
- **The exit codes are `u8`**: the five constants and both `exit_code()` methods, as `std::process::ExitCode::from` takes them.  `std::process::exit` takes one with `.into()`, and an `as u8` cast at a call site is now redundant.  Code that holds or compares a code as `i32` (`status.code() == Some(…)`, a helper that takes an `i32`) converts it with `.into()` or `i32::from`.
- **`SymlinkInfo` and `DestPolicy` are `#[non_exhaustive]`**, so a later field or policy is not a breaking change.  A `DestPolicy` is still built from its variants or by `from_flags`; a `match` on one needs a wildcard arm.  A `SymlinkInfo` comes only from `symlink_info`, and a pattern on one needs `..`.

These affect only users of the unpublished versions before 0.6.0.

[0.6.0]: https://github.com/ComposerChrisF/cli-contract/releases/tag/cli-contract-v0.6.0

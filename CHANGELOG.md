# Changelog

All notable changes to `cli-contract` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0/).

## [0.6.0] - Unreleased

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

- **`clap` is no longer a default feature.**  A tool that takes `CreateMode` or `Clobber` as a clap `value_enum` adds `features = ["clap"]`.  This affects only users of the unpublished versions before 0.6.0.

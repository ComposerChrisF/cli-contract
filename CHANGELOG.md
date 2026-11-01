# Changelog

All notable changes to `cli-contract` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0/).

## [0.6.1] - 2026-10-31

### Fixed

- **Under `All`, a `..` after a directory that does not exist yet is refused**, as the new `PathError::DotDotAfterMissing`, by `check_destination`, `ensure_destination`, `check_output_file`, and `prepare_output_file` alike.  In every mode, so is an output file whose own path ends in such a `..`.  Until that directory is built, the OS has not resolved where the `..` leads, so the checks were judging a path that did not exist yet:
  - On macOS, with `L` a link to `/opt`, `L/zz/../../Volumes/X` was approved.  Once `zz` is built it is `/Volumes/X`, so run as root the create made a mount point (bug-0041).
  - The output file `a/../a`, with `a` absent, was approved under `All`, and `x/a/..`, with `x/a` absent, under `Final` and `All`.  Once the parent was built, the path named a directory, so the write failed (bug-0040).

  A destination such as `new/../new`, which was built before, is refused too: spell it without the `..`.  On Windows `..` is resolved before the file system sees it, so a `..` among the directories to be built is refused only in a verbatim (`\\?\`) path, where `..` is a literal name that no create can make.

## [0.6.0] - 2026-10-31

The first public release.

### Added

- **Exit codes:** the constants `OK`, `TOOL_ERROR`, `USAGE`, `FINDINGS`, and `SETUP` (0–4), and `HELP_BLOCK`, the table ready for clap’s `after_long_help`.
- **The path contract:**
  - inputs: `require_input_path`, `require_input_dir(path)`, and `require_marker`;
  - output directories: `check_destination` (the dry run) and `ensure_destination` (the live run), under a `DestPolicy` built from `CreateMode` and an optional marker by `DestPolicy::from_flags`;
  - `MarkerName`, a marker validated as one path component when it is made, by `MarkerName::new` or `FromStr`, so a clap flag can be typed `Option<MarkerName>`; and `MarkerNameError`, its error, which leaves the exit code to the caller;
  - output files: `check_output_file` (the dry run) and `prepare_output_file` (the live run), with `Clobber`;
  - the mount-root floor, `is_mount_root`, which no mode can override;
  - a symlink primitive for a caller’s own policy: `reject_symlink`.
- **BrokenPipe:** `reset_sigpipe` for a tool whose output is the product, and `ignore_broken_pipe` (with `is_broken_pipe`) for a tool that must finish its work when its output pipe closes.
- **The `clap` feature:** `clap::ValueEnum` for `CreateMode` and `Clobber`.  Both implement `FromStr` and `Display` with the same spellings whether or not the feature is on.
- Runnable examples in `examples/`.

### Changed

- **`clap` is no longer a default feature.**  A tool that takes `CreateMode` or `Clobber` as a clap `value_enum` adds `features = ["clap"]`.
- **The exit codes are `u8`**: the five constants and both `exit_code()` methods, as `std::process::ExitCode::from` takes them.  `std::process::exit` takes one with `.into()`, and an `as u8` cast at a call site is now redundant.  Code that holds or compares a code as `i32` (`status.code() == Some(…)`, a helper that takes an `i32`) converts it with `.into()` or `i32::from`.
- **`symlink_info` and `SymlinkInfo` are removed.**  Their `resolved` and `target` fields were `Option`s that turned a `canonicalize` or `read_link` failure, after the link had been stat’d, into `None`, so a recipe that looked only at `resolved` could skip the mount-root floor; and `target_is_dir` and `dangling` admitted an impossible state.
- **`require_input_dir` takes no marker.**  For an input directory that needs one, call `require_marker(dir, &marker)` in its place; it refuses a missing directory or a non-directory too.
- **`DestPolicy::Marked` holds a `MarkerName`, `require_marker` takes `&MarkerName`, and `DestPolicy::from_flags` takes `Option<MarkerName>`.**  Make one with `MarkerName::new` or by parsing; a clap flag typed `Option<MarkerName>` passes straight to `from_flags`.
- **`PolicyError::MarkerNotAName` and `PathError::MarkerNotAName` are gone.**  A marker that is not a name is a `MarkerNameError` when the `MarkerName` is made, so it reaches neither `from_flags` nor a probe, and `from_flags` refuses only `PolicyError::MarkerWithCreate`.
- **`is_mount_root`, `is_broken_pipe`, and both `exit_code()` methods are `#[must_use]`**, since an ignored floor check is a skipped floor.
- **`DestPolicy` is `#[non_exhaustive]`**, so a later policy is not a breaking change.  A `DestPolicy` is still built from its variants or by `from_flags`; a `match` on one needs a wildcard arm.

These affect only users of the unpublished versions before 0.6.0.

[0.6.1]: https://github.com/ComposerChrisF/cli-contract/releases/tag/cli-contract-v0.6.1
[0.6.0]: https://github.com/ComposerChrisF/cli-contract/releases/tag/cli-contract-v0.6.0

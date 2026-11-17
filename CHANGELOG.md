# Changelog

All notable changes to `cli-contract` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0/).

## [0.6.8] - 2026-11-17

### Changed

- **The minimum Rust version is now 1.81**, up from 1.79.  1.81 stabilized `#[expect]` and a lint’s `reason`, which the crate’s own code now uses for every lint it suppresses.  A project on Rust 1.79 or 1.80 can stay on 0.6.7.

## [0.6.7] - 2026-11-16

### Added

- **`write_file`, the guarded write.**  `write_file(path, contents, clobber)` runs `check_output_file`’s checks with `CreateMode::None`, then writes the file without ever leaving a half-written file in place of the old one.  Under `Clobber::Allow` it opens the path for writing first, as `fs::write` does, so a file the caller cannot write is refused.  A regular file is replaced: a temporary file beside it, `.<name>.<pid>.<n>.tmp`, made with `create_new`, is flushed to disk and renamed over it, and only over the file that was opened, as compared on Unix by device and inode just before the rename.  On Unix the new file keeps the old one’s permissions, setuid and setgid included, and the temporary file is owner-only until then.  A link is written through: the temporary file goes beside the file it names.  A device or a FIFO is written in place, through the file already open.  A regular file that a descriptor holds, such as `/dev/stdout` when stdout is a file (any path whose links pass through `/dev/fd`), is written through the descriptor itself, where it stands.  Under `Clobber::Deny` it makes the file with `create_new`, so a file that appears after the check is `PathError::Clobber`, never replaced, and flushes it.  If the write fails it removes the file, on Unix only while the path is still that file, by device and inode.  The temporary file is refused as `PathError::MountRoot` where the mount-root floor refuses a new file.  A failure after the checks is `PathError::Io`, naming the path; after any error the temporary file is removed, if it can be.  The directory is not flushed after the rename.  Under `Allow` the write needs to write the directory, so it fails in some places where `fs::write` would not; the README’s Known limitations lists them (plan-0020, part 2).
- **`testing`, contract tests for a tool’s integration tests, behind a feature of that name.**  Each runs the tool’s built binary as a `Command`, panics naming what broke, and fails the test on an exit code the tool’s `ExitTable` does not list, in its main table or a mode, or on a panic, which is exit 101 whatever the table says; a panic is told by its exit alone, never by reading the tool’s output.  `closed_stdout(cmd, &table)` and `closed_stderr(cmd, &table)` run the tool with the stream refusing every write, as a pipe does once its reader has gone (on Unix a socket whose writing side is shut down before the tool starts, so its first write fails whoever else holds the socket), and pass it when it exits with a listed code or, on Unix, when `SIGPIPE` ends it; a tool that writes its messages with `eprintln!` fails `closed_stderr` on a command that writes one.  `dry_run_writes_nothing(cmd, tree, &table)` snapshots the directory `tree` (each path’s type, a file’s bytes, a link’s target; a link at `tree` itself is followed), runs the tool, and asserts the tree is unchanged, naming each path added, removed, or changed, even when the run also exits wrongly or removes `tree`.  `help_has_exit_table(cmd, &table)` runs `cmd --help`, with `COLUMNS` removed, and asserts it exits 0 and prints `table.render()`, saying so when the table is there with other line breaks, as clap’s `wrap_help` makes them.  Each returns the run’s `Output`, and names the command by its program, arguments, and directory, never its environment.  No new dependency: on Unix the socket is made with `libc`, already one there (plan-0020, part 4).

## [0.6.5] - 2026-11-15

### Added

- **`exit::ExitTable`, a tool’s exit-code table, built from the codes it returns.**  `ExitTable::canonical()` renders as `HELP_BLOCK`.  `.detail(n, text)` adds lines under one of the canonical five, which keeps its wording, for what the code covers in the tool: the usage errors it checks itself, say.  `.code(n, meaning)` adds a code of the tool’s own, and `.mode(name, &[(code, meaning)])` the table of a mode that gives codes meanings of its own, rendered after the main table under `EXIT CODES (<name>):`.  `.render()` is the text for clap’s `after_long_help`, each section sorted by code, the codes aligned on the widest.  `detail` panics on a code above 4; `code` panics on 0 to 4, whose meanings are canonical, on a code above 125, and on a code already in the table; `mode` panics on an empty list of codes, a code above 125, a code listed twice, and a mode name already in the table.  `HELP_BLOCK` is unchanged (plan-0020, part 3).

## [0.6.4] - 2026-11-14

### Added

- **`presence` and `presence_resolved`, the crate’s three-state probe, are public**, with `Presence`, their answer: `Ok(Presence::Absent)` is a proven `NotFound`, `Ok(Presence::Present(file_type))` is what is there, and an `Err` is never a verdict, whether the probe could not tell (`Undeterminable`) or found something in the way (`NotADirectory`, `DanglingSymlink`).  `presence_resolved` follows a link, as `Path::exists`, `is_dir`, and `is_file` do, so it is what a tool calls in their place, since they answer `false` to every error; a link to nothing is `PathError::DanglingSymlink`, where they answer `false`.  `presence` sees a link as itself.  `Presence` is exhaustive, so a match on it needs no `_` arm.  An unmounted volume answers `Absent`, as the OS reports it, so a tool proves an external disk is there before trusting an `Absent` below it (plan-0020, part 1).

### Changed

- **The empty path is `PathError::Undeterminable`, never absent.**  `require_input_path("")`, `require_input_dir("")`, and `require_marker("", …)` answered `MissingInput`, and still exit 1; `reject_symlink("")` passed it, and now refuses it.  An empty string where a path was required is a value that went missing upstream, not proof that a file is not there.

## [0.6.3] - 2026-11-14

### Fixed

- **`is_mount_root` answers `true` for a dangling link, at the path or above it**, which the four contract functions already refuse as `PathError::DanglingSymlink`.  On macOS, `mkdir("link/")` and `std::fs::create_dir_all("link/")` create a dangling link’s target, so with `link` pointing into an absent `/Volumes/X`, a caller that gated its own create on `is_mount_root` could, run as root, make a mount point (bug-0046).  The maintainer ruled that every dangling link is refused, wherever it points, so a caller that reports the refusal may call a link to an ordinary folder a mount root.

## [0.6.2] - 2026-11-02

### Fixed

- **`is_mount_root` answers `true` for a path with a `..` after a level that does not exist yet**, which 0.6.1’s contract functions already refuse.  On macOS, with `L` a link to `/usr`, it answered `false` for `L/zz/../../Volumes/X`, which is `/Volumes/X` once `zz` is built, so a caller that gated its own create on it could, run as root, make a mount point (bug-0045).  A stat that fails on the way to that level, for any reason but absence, refuses too.  A `..` after a level that exists, as in `/usr/../etc`, is judged as before; on Linux, which has no `/Volumes`, `/Volumes/../etc` is now refused.  On Windows, where `..` is resolved first, only a verbatim (`\\?\`) path keeps one.

## [0.6.1] - 2026-11-01

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

[0.6.8]: https://github.com/ComposerChrisF/cli-contract/releases/tag/cli-contract-v0.6.8
[0.6.7]: https://github.com/ComposerChrisF/cli-contract/releases/tag/cli-contract-v0.6.7
[0.6.5]: https://github.com/ComposerChrisF/cli-contract/releases/tag/cli-contract-v0.6.5
[0.6.4]: https://github.com/ComposerChrisF/cli-contract/releases/tag/cli-contract-v0.6.4
[0.6.3]: https://github.com/ComposerChrisF/cli-contract/releases/tag/cli-contract-v0.6.3
[0.6.2]: https://github.com/ComposerChrisF/cli-contract/releases/tag/cli-contract-v0.6.2
[0.6.1]: https://github.com/ComposerChrisF/cli-contract/releases/tag/cli-contract-v0.6.1
[0.6.0]: https://github.com/ComposerChrisF/cli-contract/releases/tag/cli-contract-v0.6.0

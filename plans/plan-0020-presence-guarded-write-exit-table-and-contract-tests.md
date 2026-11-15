# Plan: A Public Presence Probe, a Guarded Write, a Built Exit Table, and Shared Contract Tests

Filed on 14-Nov-2026, from a survey of the maintainer’s own tools: 46 Rust repositories, their lint results, and the 459 open bug reports among them, each classed by fault and by what would have prevented it.

## Problem

The survey’s finding, for this crate: **the tools follow the house contracts at their edges and break them inside.**  The argument parsing, the exit codes, and the output paths are mostly right, because this crate gives them a function to call.  The faults sit in internal helpers, where a tool probes a file with a bare `bool`, because this crate gives it nothing to call there.

- **No public presence probe.**  `Presence` and `probe` answer three ways, and the README promises that every probe here does, but both are private.  A tool that must decide “is my manifest there?” or “does this rule’s target exist?” reaches for `Path::exists`, which answers `false` to every error.  Four of the six destruction faults the survey found unfiled were this one fault: a state file read as empty because its `stat` failed, then saved over; a rule run because its target “did not exist” when the directory could not be read; an existing file overwritten because it could not be parsed; and a page’s inherited resources hidden because the page had none of its own.  The survey also counted 123 `Path::exists`, 72 `is_dir`, and 64 `is_file` calls in production code, not all of them gating a write.
- **No guarded write.**  `check_output_file` and `prepare_output_file` decide whether a write may happen, then leave the write to the tool, which uses `fs::write`.  That truncates before writing, so a failed write loses the old file, and under `Clobber::Deny` it races with anything that creates the file between the check and the write.
- **The exit table cannot drift only where a tool has no codes of its own.**  `HELP_BLOCK` is the canonical table as a fixed string.  A tool with codes above 4, or a mode whose codes differ, writes its own table by hand, and a hand-written table is where documentation drift lives.  Doc drift was the largest class of report that a mechanism could reach: 54 of the 330 reports in command-line tools.
- **The cheapest contract tests are written per tool, or not at all.**  A closed stdout or stderr must not panic, `--dry-run` must write nothing, and `--help` must carry the exit table.  5% of the reports were one of these, and each tool that has the tests wrote its own.

The survey also plans a `clippy.toml` per tool whose `disallowed-methods` entries name the fix in their `reason`.  `Path::exists` cannot be banned until there is something to name, which is this plan’s first part.

## Proposed Change

Four additive parts, each able to land in its own 0.6.x or 0.7.0 release.

### 1.  `paths::presence`, public

```rust
/// What a probe learned: a proven absence, or what was there.  "Could not tell" is the `Err`
/// of the enclosing `Result`, never a variant, so a caller cannot reach a verdict without it.
pub enum Presence {
    /// `NotFound`, and nothing else.
    Absent,
    /// Present, with the type the probe saw (a link as itself).
    Present(fs::FileType),
}

/// `probe`, public: `lstat`, the ancestor walk, and the trailing-separator rule, unchanged.
pub fn presence(path: &Path) -> Result<Presence, PathError>;

/// The same, following a symbolic link to what it names (`probe_resolved`); a link to
/// nothing is `PathError::DanglingSymlink`, never `Absent`.
pub fn presence_resolved(path: &Path) -> Result<Presence, PathError>;
```

The rustdoc keeps the existing explanation of why there is no `Unknown` variant, and adds the one-line recipe a lint can point to: `match presence(p)? { Presence::Absent => …, Presence::Present(_) => … }` in place of `if p.exists()`.  The README’s “every probe here answers three ways” then names a function a caller can use.

**Landed 14-Nov-2026** in 0.6.4, as the amended text proposes, with these additions from its review.  `Presence` also derives `PartialEq` and `Eq`, and the private `probe` and `probe_resolved` were renamed `presence` and `presence_resolved`, not wrapped, so each has one name.  The recipe names `presence_resolved`, not `presence`: `Path::exists`, `is_dir`, and `is_file` follow a link, so a rewrite that keeps their link behavior uses it, and `presence` is for a question about the entry itself.  The empty path is `Undeterminable`, never `Absent`, under the house rule that an empty string where a value was required is unknown; that changes `require_input_path("")`, `require_input_dir("")`, `require_marker("", …)`, and `reject_symlink("")` too.  The rustdoc says that an unmounted volume answers `Absent`, as the OS reports it, and names the guard; plan-0021 proposes catching it in the probe.  Black-box tests pin the three answers, the link, loop, and empty-path cases, and the ancestor walk, and their two-arm match stops compiling if the enum is made `#[non_exhaustive]`.  Six mutants are caught: an `EACCES`, an `ELOOP`, or a dangling link read as `Absent`, the trailing separator kept, the ancestor walk skipped, and the empty-path check removed.

### 2.  `paths::write_file`, the guarded write

```rust
/// Write `contents` to `path`, never leaving a half-written file in its place.
/// `Clobber::Deny`: created with `create_new`, so a file that appears after the check is refused
/// (`PathError::Clobber`), never replaced.  `Clobber::Allow`: written to a temporary file in the
/// same directory, flushed to disk, then renamed over `path`, so the old file survives a failed write.
pub fn write_file(path: &Path, contents: &[u8], clobber: Clobber) -> Result<(), PathError>;
```

It runs `check_output_file`’s checks on the leaf, with `CreateMode::None`, so a caller has already made the parent with `prepare_output_file` when it wanted one made.  A caller with a policy of its own keeps calling `fs` itself; this is the default.

### 3.  `exit::ExitTable`, a built exit table

```rust
let table = ExitTable::canonical()
    .detail(USAGE, "also --fix with --check, which the tool refuses itself")
    .code(5, "lock held by another run")
    .mode("--check", &[(3, "drift found"), (1, "the check could not run")]);
Command::new("tool").after_long_help(table.render());
```

The canonical five lines come from the same constants the code returns, and `ExitTable::canonical().render()` equals `HELP_BLOCK`, so its text does not change.  `code` refuses a number from 0 to 4 and one above 125.  `detail` adds lines under one of the canonical five, so that a tool can say what the code covers in it without changing the code’s wording: the usage errors it checks itself beyond clap’s, which the exit-code convention says a tool names in its table, or that it never returns the code.  A mode may list a canonical code, to say what it means in that mode, but 2 keeps its meaning in every mode, since clap exits 2 before any mode runs: a mode lists 2 only for usage errors of its own.  A tool declares its extra codes next to its table, so the table and the codes are written once.  A code the tool returns without declaring it is not caught here; part 4’s help test catches the codes the tool’s tests see.

_Amended 15-Nov-2026, from part 3’s review:_ `detail` is added, since every tool surveyed that has codes above 4 says more under the canonical codes than their one line, and a table that could not say it could not be adopted.  The sentence on modes and 2 is added, and “`HELP_BLOCK` becomes” is now “equals”, since a `String` cannot be a constant, and a tool that copies `HELP_BLOCK` into a `const` of its own needs it to stay one.

### 4.  `testing`, shared contract tests (behind a `testing` feature)

For a tool’s integration tests, run against its built binary (`env!("CARGO_BIN_EXE_<name>")`):

- `closed_stdout(cmd)` and `closed_stderr(cmd)`: run with the stream’s reader gone, and assert that it did not panic (no exit 101, no panic message) and exited with a code from the table.
- `dry_run_writes_nothing(cmd, tree)`: snapshot a directory tree (paths, types, sizes, and bytes), run, and assert it is unchanged.
- `help_has_exit_table(cmd, &table)`: assert that `--help` contains `table.render()`, and that every exit code the tests see is one the table lists.

`std` only, so the feature adds no dependency.  The closed-stderr test fails today in every tool that writes with `eprintln!`, so it lands with plan-0011’s `errln!`, or is introduced as a ratchet: new tools pass it, and old ones adopt it by plan.

## Implementation Notes

- **Part 1 is a visibility change plus docs.**  `Presence` stays exhaustive when it goes public (amended 14-Nov-2026; the first draft made it `#[non_exhaustive]`).  Outside this crate, `#[non_exhaustive]` would make the recipe above fail to compile: a match would need a `_` arm, and a wildcard arm on a verdict is the silent default this plan exists to remove, since the caller must guess what an answer it has never seen means.  A new variant would break every match instead, which is the honest signal for a change to what a probe can answer.  The tests that pin `probe` today pin `presence`, and a mutation check confirms that `Absent` cannot come back for an `EACCES` or an `ELOOP`.
- **Part 2 is the one with design weight.**  The temporary file’s name must not collide (`create_new` on a name with the process id, retried), it must be removed on any error, and the rename must stay in one directory, so it never crosses a filesystem.  Whether to `fsync` the directory after the rename is a durability-against-power-loss question; propose no, and say so in the rustdoc.  On Windows, `rename` over an existing file needs `MoveFileEx` with `MOVEFILE_REPLACE_EXISTING`, which `std::fs::rename` already uses.
- **Part 3 keeps `HELP_BLOCK`** as a constant, its text pinned by the existing test, so nothing that uses it changes.
- **Part 4 is a public test API**, and semver binds it too.  Keep it small: these four functions, and `#[non_exhaustive]` on any options type.
- Plans 0015 and 0017 are siblings: the floor and symlink facts, three ways.  Part 1 is the presence probe they both stand on, and the three together would make the README’s probe promise true of the public API.  None depends on another.
- Order: part 1 first, since the lints wait on it; then part 3, since it is small and removes a drift source at once; then parts 2 and 4.

## Why Not a Workaround

Every tool that lacks these writes its own, and the survey shows what that produces: the three-state probe got right at each tool’s edges, where this crate is called, and the bare `bool` inside, where it is not.  A lint that bans `Path::exists` with nothing to name sends each tool off to write its own probe again.  A per-tool exit table drifts from the per-tool code, and the shared tests, copied into each repository, drift as well.

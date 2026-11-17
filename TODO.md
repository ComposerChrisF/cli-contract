# TODO

The order of open work.  The detail lives in each plan under `plans/`; a bug’s report lives under
`bugs/` until the commit that fixes it deletes it.  This repository was cut from a private
predecessor at 0.5.24, so IDs continue from `bugs/FLOOR` and `plans/FLOOR`.

## The release

- [x] **0.6.0 is on crates.io** (31-Oct-2026), tagged `cli-contract-v0.6.0`.  docs.rs renders
  the page with the clap impls visible, and the `.crate`’s normalized `Cargo.toml` says
  `default = []`.

- [x] **0.6.1 is on crates.io** (1-Nov-2026), tagged `cli-contract-v0.6.1`.  It fixes bug-0040
  and bug-0041: under `All`, a `..` after a directory that does not exist yet is refused.  That
  also refuses a link-free `new/../new`, which 0.6.0 built, and the maintainer ruled it a patch
  all the same: a narrowing that only refuses, made to close a safety hole, ships as a patch so
  that every dependent on `"0.6"` picks it up (bug-0042).

- [x] **0.6.2 is on crates.io** (4-Nov-2026), tagged `cli-contract-v0.6.2`.  It fixes bug-0045:
  `is_mount_root` fails closed on a `..` after a level that does not exist, as the four contract
  functions have since 0.6.1.

- [x] **0.6.3 is on crates.io** (15-Nov-2026), tagged `cli-contract-v0.6.3`.  It fixes bug-0046:
  `is_mount_root` refuses every dangling link, as the four contract functions do, since on macOS
  `mkdir("link/")` creates the link’s target.

- [x] **0.6.4 is on crates.io** (15-Nov-2026), tagged `cli-contract-v0.6.4`.  It makes the
  three-state probe public, as `presence` and `presence_resolved` (plan-0020, part 1).

- [x] **0.6.5 is on crates.io** (15-Nov-2026), tagged `cli-contract-v0.6.5`.  It adds
  `exit::ExitTable`, a tool’s exit-code table built from the codes it returns (plan-0020, part 3).

- [x] **0.6.7 is on crates.io** (16-Nov-2026), tagged `cli-contract-v0.6.7`.  It adds
  `write_file`, the guarded write (plan-0020, part 2), and the `testing` feature, the shared
  contract tests (part 4, the last).  0.6.6 was held, by the maintainer’s choice on 16-Nov-2026, to
  go out with part 4, so it was never published on its own.

- [ ] **Publish 0.6.8**, which raises the minimum Rust version to 1.81, for the `#[expect]`
  attributes the lint set asks for (`[lints.clippy]` in `Cargo.toml`, with `clippy.toml`).  It
  waits on the maintainer, who may call it 0.7.0 instead.  Cargo’s SemVer guide files a raised
  minimum Rust version as possibly-breaking, and recommends treating it as a minor change, which
  in a 0.y.z crate is a z bump, as here.  But the Cargo of Rust 1.79 and 1.80 resolves without
  reading `rust-version`, so a project there that runs `cargo update` would take 0.6.8 and fail
  to build.

- [ ] **Pin that the two `..` checks agree**: `floor_check_chain` and `dotdot_after_missing` each
  decide “a `..` after a level that does not exist” their own way, and only bug-0045’s path is
  tested against both.  A table test over both, or one walk they share.

- [ ] **Set the semver baseline**, so every later release is checked against 0.6.0.  Nothing
  checks it yet: no CI job, and `cargo-semver-checks` is not installed here.

## Open plans

- [ ] [plan-0010](plans/plan-0010-exit-3-mode-scoping.md) — **mode-scope exit 3**: findings codes
  belong to reporting modes (`check`, `validate`, `--dry-run`, `status`), never to modes that do
  work, since a generic runner can branch only on zero or non-zero.  The principle is in
  `exit::FINDINGS`’s rustdoc, and `HELP_BLOCK` stays unchanged; what enforces the assignment is
  open.

- [ ] [plan-0011](plans/plan-0011-stderr-writer-that-never-panics.md) — **a stderr writer that
  never panics**: `errln!` / `errprint!`, which ignore every stderr write error and evaluate to
  `()`, because `eprintln!` exits 101 when stderr’s reader has gone, and a mid-run notice then
  stops a mutating run partway.

- [ ] [plan-0012](plans/plan-0012-linux-mount-roots-by-identity.md) — **the Linux mount roots by
  identity**, as `/Volumes` is recognised by device and inode, so a symlinked or bind-mounted
  `/mnt` cannot pass the floor.  The README lists the gap under Known limitations.

- [ ] [plan-0014](plans/plan-0014-a-create-below-a-mount-point-that-outlives-its-volume.md) —
  **a create below a mount point that outlives its volume**: on Linux an fstab, systemd, bind, or
  hand-made mount point stays after the unmount, so `Final` and `All` build the backup on the root
  filesystem one level down.  A marker on an ancestor, or proof of a live mount.  Listed under
  Known limitations.

- [ ] [plan-0015](plans/plan-0015-the-floors-three-state-answer-in-public.md) — **the floor’s
  three-state answer, in public**: `is_mount_root` folds “could not tell” into `true`, so a caller
  composing its own floor check cannot say why it refused.  Make the private
  `Result<bool, PathError>` form public.  Additive.

- [ ] [plan-0017](plans/plan-0017-a-three-state-symlink-facts-primitive.md) — **a three-state
  symlink-facts primitive**: the “crate reports, caller decides” primitive behind a per-tool
  symlink policy, back in a shape that cannot fail open, every unknown an `Err`, never a `None`.
  `symlink_info` was removed before 0.6.0 for failing open.  Additive.

- [ ] [plan-0018](plans/plan-0018-refuse-an-output-file-path-that-ends-in-a-separator.md) — **refuse
  an output file path that ends in a separator**: `out.pdf/` is checked as `out.pdf` and approved,
  though no file can be written there.  The write fails loudly today.

- [ ] [plan-0021](plans/plan-0021-could-not-tell-below-a-missing-mount-point.md) — **“could not
  tell” below a missing mount point**: `presence` answers `Absent` for a path on an unmounted
  volume, as the OS reports it, so a tool must prove the volume first.  Catch it in the probe.

- [ ] [plan-0023](plans/plan-0023-a-dry-run-that-touches-nothing.md) — **a dry run that touches
  nothing, metadata included**: `dry_run_writes_nothing` also compares permissions and a regular
  file’s modification time.  It grows a public test API’s contract.

- [ ] [plan-0024](plans/plan-0024-the-examples-return-exitcode-from-main.md) — **the examples
  return `ExitCode` from `main`**: `broken_pipe`, `destination`, and `inputs` end with
  `process::exit`, which the lint set in `clippy.toml` refuses, so they teach the call it refuses.
  Each call carries an `#[expect]` naming this plan.

## Smaller items

- [ ] **CI runs clippy on Linux only**, so nothing checks the suppressions compiled only off Unix:
  the two `cfg_attr(not(unix), expect(unused_mut, …))` in `tests/check_equals_ensure.rs`, and the
  `expect` on the non-Unix `run_with_writes_refused` in `src/testing.rs`.  A clippy step with
  `--target x86_64-pc-windows-msvc` would check them, once the toolchain step installs that
  target.  Under `-D warnings` it would first fail on `src/testing.rs`’s test module, whose
  `use super::*` is unused off Unix, since every test in it is `cfg(unix)`.

- [ ] **`dry_run_writes_nothing` names a `tree` that is not a directory with a trailing
  separator** (`…/a.txt/`), since its walk reads `tree.join("")`.  Cosmetic: the message still
  names the path and fails the test.

- [ ] **The leak gate trusts a lowered floor.**  It reads `bugs/FLOOR` and `plans/FLOOR` from the
  index, so one commit can lower a floor and cite the IDs that newly pass.  A floor is never
  lowered, so the gate could compare the index’s floor with `HEAD`’s and stop on a lower one.
  That adds to its contract, so it wants a plan.

- [ ] **Nits from the pre-launch audit**, none a broken promise:
  - `Undeterminable`’s rustdoc says “see the module docs”, which say nothing about it;
  - `create_after_check`’s bare `is_dir()` is correct, but reads as the bool collapse the module
    forbids;
  - the README cites `ROADMAP.md`, which the package leaves out;
  - `floor_check_chain` re-walks the missing ancestors once per level, O(k²);
  - the marked paths probe the directory twice;
  - `examples/destination` cannot take a directory whose name begins with a dash;
  - the leak gate takes an empty `LEAK_GATE_PATTERNS` as unset, so it reads the default list;
  - `--require-private` exits 4 before the findings summary and exit 3.

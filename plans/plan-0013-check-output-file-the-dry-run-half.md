# Plan: `check_output_file`, the Dry-Run Half of `prepare_output_file`

Re-filed at the public launch (31-Oct-2026) from the predecessor repository, where it was filed on 30-Oct-2026.

## Problem

The README’s `--dry-run` section says a mutating tool that previews its work “needs both halves” of the call, and gives them for a destination directory: `check_destination` and `ensure_destination`.  An output **file** has only one: `prepare_output_file`, which under `CreateMode::Final` and `All` creates the missing parent directories.  A dry-run tool that calls it writes to disk; one that does not must rebuild the leaf’s three-state clobber probe and its dangling-link, `NotAFile`, and mount-point checks itself, which is the per-tool reimplementation this crate exists to prevent.

It was found before the launch by a black-box review that read only the README and the rustdoc.  The README’s Known limitations section lists it; it is to be fixed after the launch.

## Proposed Change

```rust
/// [`prepare_output_file`], creating nothing: the same verdicts, zero filesystem writes.
pub fn check_output_file(path: &Path, dir_mode: CreateMode, clobber: Clobber) -> Result<(), PathError>;
```

`prepare_output_file` becomes `check_output_file`, then `ensure_destination` on the parent, as `ensure_destination` is `check_destination` and then the create.  The README’s dry-run snippet gains the file pair, and `tests/check_equals_ensure.rs` gains the file entry points, so the two can never disagree about the contract.

## Implementation Notes

- The leaf classification in `prepare_output_file` moves into `check_output_file` unchanged, including the two refusals added to it before the launch: an absent file named for a mount point is `MountRoot` in every mode, and the empty path, which cannot be resolved, is `Undeterminable`.  The parent goes through `check_destination` there, and through `ensure_destination` in `prepare_output_file`.
- The parent’s edge cases move with it: an empty parent (writing into the current directory) needs no check, and a path with no parent at all (on Windows, an absent drive or share) is `MissingDestination`.
- Additive and non-breaking, so it may land in any 0.6.x.  Sooner is better: a stranger’s first dry-run tool meets the gap.
- The item under Known limitations goes when it lands.

## Why Not a Workaround

A caller can compose `check_destination` on the parent with its own leaf probe today, but the leaf probe is the safety-critical half (a leaf that could not be inspected must not slip past `Clobber::Deny`), and every caller writing it again is how one tool gets it wrong.  Rejecting this plan is a reasonable exit if no adopter has a dry run over output files; whether any does has not been checked.

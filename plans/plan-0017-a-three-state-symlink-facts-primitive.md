# Plan: A Three-State Symlink-Facts Primitive

Filed on 31-Oct-2026, when `symlink_info` and `SymlinkInfo` were removed before the 0.6.0 release.

## Problem

The README’s Symlinks section says the crate does not guess a symlink policy: a tool follows a link (the default), rejects one (`reject_symlink`), or proves the disk with a marker.  Until 0.6.0 it also offered `symlink_info`, the “crate reports, caller decides” primitive: the facts about a link, so a tool could apply a policy of its own — prompt, log, refuse a link that leaves a tree, or re-apply the mount-root floor to where the link resolves.  That intent was sound.  The right symlink policy depends on what a tool is for (a mirror must preserve a link, a publisher must follow one), so no fixed set of policies in the crate covers every tool, and the facts primitive is what lets each tool decide.

Its shape could fail open, and shipping it would have made that shape a semver commitment.  A link whose target could not be probed (`EACCES` on a directory along its target, `ELOOP`) was already an error: `symlink_info` probed the link and then its target, three ways, and returned `Err(PathError::Undeterminable)` for those.  The fault was in the fields it read after the link had been stat’d:

- `resolved` and `target` were `Option`s, `canonicalize().ok()` and `read_link().ok()`, so a `canonicalize` or `read_link` failure came back as `None`, which for `resolved` is also what a dangling link reads as.  A recipe that looked only at `resolved`, as the README’s resolve-and-re-floor recipe `is_mount_root(resolved)` did, then had no path to check, and could skip the floor.
- `target_is_dir` and `dangling` were two `bool`s, which admit an impossible state: both `true`.

## Proposed Change

A primitive that answers three ways, where every unknown is `Err(PathError::Undeterminable)`, never a `None`:

```rust
/// What a symbolic link resolves to, as the OS follows it.  The canonical path lives in the
/// variants that have one, so "resolves, but to where?" cannot be asked of a dangling link.
#[non_exhaustive]
pub enum LinkTarget {
    /// A directory, at this canonical path.
    Directory(PathBuf),
    /// Something that exists and is not a directory, at this canonical path.
    Other(PathBuf),
    /// Nothing: a proven `NotFound`, and nothing else.
    Dangling,
}

#[non_exhaustive]
pub struct SymlinkFacts {
    /// The link, as probed (its last component bare).
    pub link: PathBuf,
    /// The target stored in the link, unresolved.
    pub target: PathBuf,
    /// What it resolves to.
    pub resolves_to: LinkTarget,
}

/// `Ok(None)`: proven not a link (including a proven-absent path).  `Ok(Some(_))`: a link, and
/// every fact about it was read.  `Err(Undeterminable)`: something could not be read.
pub fn symlink_facts(path: &Path) -> Result<Option<SymlinkFacts>, PathError>;
```

With plan-0015’s three-state floor, the resolve-and-re-floor recipe returns as a check that names its cause: `mount_root(path)?` on the path in `Directory` or `Other`.  The README’s Symlinks section gains its “caller decides” bullet back.

Additive, so it can land in any 0.6.x.

## Implementation Notes

- `probe` and `probe_target` already answer three ways; build on them.  A `read_link` or `canonicalize` error is `Undeterminable`, naming the link.  A `NotFound` from `canonicalize` after `probe_target` saw the target is a race, and it is `Undeterminable` too, never `Dangling`: only the target probe’s own proven `NotFound` may say dangling.
- A path below a file or a dangling link is the error that names that entry, as for every entry point (`probe` already does this), and a trailing separator is dropped first, so `link/` is the link.
- The facts are a snapshot: the link can change between the reads.  Say so in the rustdoc, as the other entry points do for creation.
- The name is open.  `symlink_info` never shipped, so it is free, but users of the unpublished versions knew a different shape under it.
- Tests: the locked-directory fixture, which the target probe refuses before any field is read, and a mutation check that an unresolvable target never comes back as `Dangling` or `None`.

## Why Not a Workaround

A tool can call `symlink_metadata`, `read_link`, and `canonicalize` itself, and that is what the crate exists to stop.  Each tool would re-derive the three-way split, the trailing-separator rule, the walk that names a file or a dangling link above the path, and the Windows `ERROR_BAD_NETPATH` carve-out, and each would be free to fold an error into a `None`, as the removed shape did.

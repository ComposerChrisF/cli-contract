# Plan: The Floor’s Three-State Answer, in Public

Re-filed at the public launch (31-Oct-2026) from the predecessor repository, where it was filed on 30-Oct-2026.

## Problem

Inside the crate the floor answers three ways: the private `mount_root(path) -> Result<bool, PathError>` keeps “could not tell” apart as `PathError::Undeterminable`, so the crate’s own contract functions report that fact rather than claim a mount point.  The public `is_mount_root(path) -> bool` folds that third answer into `true`, failing closed.

That is safe, but a caller cannot say why it refused.  The README’s resolve-and-re-floor recipe (`is_mount_root(resolved)` on the `resolved` field of `symlink_info(&dest)?`) reports “resolves onto a mount root” when the real fact is that a stat of `/Volumes` failed, or the current directory could not be read.  The answer also differs by platform: on macOS `is_mount_root("/etc/hosts/x")` is `true` (the identity stat fails with `ENOTDIR`, and the floor fails closed), while on Linux, which has no `/Volumes` to compare against, it is `false`.  And the bool sits awkwardly beside the README’s “Every probe here answers three ways … never the two ways a `bool` can”.

It was raised by a code audit before the launch.

## Proposed Change

A public three-state form of the floor, beside `is_mount_root`:

```rust
/// [`is_mount_root`], with a path it cannot resolve, or an identity probe it could not make,
/// kept apart as [`PathError::Undeterminable`].
pub fn mount_root(path: &Path) -> Result<bool, PathError>;
```

The README’s recipe is rewritten to use it, so a refusal names its cause.  `is_mount_root` stays as the fail-closed shorthand.  If the README’s “every probe” sentence is meant to cover the floor, it gains a clause for `is_mount_root`; otherwise it says it means presence probes.

## Implementation Notes

- The function exists; this makes it `pub`, documents it, and re-exports it from `lib.rs`.
- The name is worth a look first: `mount_root` reads as a noun (“the mount root of this path”).  `check_mount_root` or `floor(path)` may say better that it answers a yes-or-no question.
- Additive, so it can land in any 0.6.x.  Doing it before 0.6.0 is published only saves the recipe’s rewrite from being a second docs pass.

## Why Not a Workaround

A caller cannot rebuild the answer from outside: the identity check against `/Volumes` and the resolution against the current directory are the crate’s, and reimplementing them is the per-tool drift the crate exists to end.

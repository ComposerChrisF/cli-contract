# Plan: “Could Not Tell” Below a Missing Mount Point

Filed on 14-Nov-2026, from the review of plan-0020 part 1.

## Problem

`presence` and `presence_resolved` answer `Absent` for a path on a volume that is not mounted.  On macOS, `/Volumes/Backup/state.json` while `Backup` is unplugged is `NotFound` to `lstat`.  The ancestor walk passes the missing `/Volumes/Backup` and stops at `/Volumes`, a directory, so the leaf’s `Absent` stands.  The OS is right that the path is not there.  But an unmounted volume is the house rule’s canonical “could not tell”, and the probe is now what a tool calls in place of `Path::exists`.  A sync tool whose prune pass asks the probe about each source file would delete the whole mirror while the source disk is unplugged.

0.6.4 documents the answer and names the guard: `require_marker` or `require_input_dir` on the volume root, before trusting an `Absent` below it.  A tool that forgets the guard has nothing else to stop it.

## Proposed Change

When the walk passes a missing ancestor that the floor recognizes as a mount root (on macOS `/Volumes/<name>`, by identity; on Linux the roots `is_mount_root` lists), answer an error for the path in place of `Absent`, naming that ancestor: “is the volume mounted?”.

Open questions, to settle before it lands:

- **Which error.**  `Undeterminable` with a message naming the mount point, or a new variant such as `PathError::VolumeAbsent(mount_point)`.  `PathError` is `#[non_exhaustive]`, so a variant is additive.  A variant lets a tool say “mount the volume” without parsing a message.
- **The mount point itself.**  `presence("/Volumes/Backup")` while it is unmounted answers `Absent` today, and that is the honest answer to “is my volume there?”.  This plan changes only paths below it.
- **The contract functions inherit it.**  `require_input_path("/Volumes/Backup/in")` would answer the new error in place of `MissingInput`, exit 1 either way.  The output side is unchanged, since the floor already refuses to create a mount point.

## Implementation Notes

- The walk in `presence` already visits each ancestor, and the floor’s identity check for `/Volumes` exists (`is_mount_root`).  The new test runs only on a missing ancestor, so a path that exists costs nothing more.
- A relative path is absolutized only when the walk meets a missing ancestor, as the floor does, so that a relative path into `/Volumes` is caught too.
- Linux’s stale mount points (plan-0014) are a different case: the directory is there, empty, after the unmount, so this plan does not see them.
- Tests: an absent `/Volumes/<unique>` on macOS, as the floor tests use, and a mutation check that the walk’s new branch is load-bearing.

## Why Not a Workaround

The guard is the workaround, and every tool has to remember it.  The house lints will send every tool to this probe in place of `Path::exists`, so the probe is the one place where the case can be caught once, for all of them.

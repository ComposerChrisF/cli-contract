# Plan: A Dry Run That Touches Nothing, Metadata Included

Filed on 16-Nov-2026, from the review of plan-0020 part 4.

## Problem

`testing::dry_run_writes_nothing` (0.6.7) compares each path’s type, a file’s bytes, and a link’s target, before the run and after it.  So a dry run that writes without changing those passes.  That covers rewriting a file with the same bytes (a temporary file renamed over it, as `write_file` does), `touch`, `chmod`, and a lock file made and removed, which changes only its directory’s modification time.  The function’s name and rustdoc (“changed nothing in it”) promise more than it checks.

## Proposed Change

The snapshot also records what `symlink_metadata` already returns:
- the permissions;
- on Unix, the inode and the modification time;
- for a directory, its modification time.

A difference in any of them is reported as “changed”, naming which.

## Implementation Notes

- A read changes only the access time, which stays out of the snapshot, so a dry run that only reads still passes.  Check that no supported filesystem updates the modification time on an open for reading; if one does, leave it out there.
- A directory’s modification time catches a file made and removed.  It also changes when the tool’s own run leaves a temporary file in `tree`.  That is a write, and the point of the check.
- This grows a public test API’s contract, so a tool whose dry run passes today could fail.  Land it in 0.7.0, or in a 0.6.x with the maintainer’s agreement.

## Why Not a Workaround

Each tool’s test would otherwise snapshot the metadata itself, and the helper exists so that it does not have to.

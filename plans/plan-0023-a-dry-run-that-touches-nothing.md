# Plan: A Dry Run That Touches Nothing, Metadata Included

Filed on 16-Nov-2026, from the review of plan-0020 part 4.  Narrowed the same day to what a well-meaning dry run changes by accident, by the maintainer’s choice (option A of the good-faith review).

## Problem

`testing::dry_run_writes_nothing` (0.6.7) compares each path’s type, a file’s bytes, and a link’s target, before the run and after it.  So a dry run that writes without changing those passes.  That covers rewriting a file with the same bytes (a temporary file renamed over it, as `write_file` does), `touch`, and `chmod`: a sync tool that gates its copies but not its metadata pass, say, or an installer that still sets the exec bit.  The function’s name and rustdoc (“changed nothing in it”) promise more than it checks.

## Proposed Change

The snapshot also records what `symlink_metadata` already returns: the permissions, and a regular file’s modification time.  A difference in either is reported as “changed”, naming which.

## Implementation Notes

- A read changes only the access time, which stays out of the snapshot, so a dry run that only reads still passes.  Check that no supported filesystem updates the modification time on an open for reading; if one does, leave it out there.  Check, too, how finely each one stamps it.
- A regular file’s modification time catches a rewrite with the same bytes once the clock has moved on, since a write or a rename over it sets the time anew.  A filesystem that stamps times coarsely, as Linux did with ext4 and tmpfs before 6.13, can miss a rewrite in the same tick as the fixture’s own write.  The inode would catch a rename over the file even then, but it is left out: a rewrite with the same bytes leaves nothing a user sees, and the inode would add a Unix-only split.
- A directory’s modification time is left out too.  It changes whenever a child is added or removed, which the path list already reports, so its only extra is a file made and removed again, and that would fail a good-faith dry run that takes a lock file inside `tree` and removes it.  README’s Known limitations says that such a file is not seen.
- This grows a public test API’s contract, so a tool whose dry run passes today could fail.  Land it in 0.7.0, or in a 0.6.x with the maintainer’s agreement.

## Why Not a Workaround

Each tool’s test would otherwise snapshot the metadata itself, and the helper exists so that it does not have to.

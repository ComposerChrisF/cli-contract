# Plan: Refuse an Output File Path That Ends in a Separator

## Problem

The README’s symlink section says a trailing separator is dropped before any check: `link/` and `link/.` are the link, and a file spelled `file/` is checked as `file`.  That reading is right for every probe that asks what is there.  For an **output file** it approves a path no file can ever be written to.  `check_output_file` and `prepare_output_file` both return `Ok` for an absent `dir/new.pdf/` or `dir/new.pdf/.`, in every mode and under both clobber policies, and for an existing `old.pdf/` under `Clobber::Allow`.  The caller’s write then fails: `NotFound` on macOS for an absent file, `NotADirectory` for an existing one, and (inferred) `EISDIR` on Linux.

It was found by the review of `check_output_file`.  The function’s rustdoc was narrowed to say what it checks, so nothing promises otherwise today; this plan is the stronger contract.

## Proposed Change

The shared leaf check refuses an output file path whose last component carries a trailing separator, or is `.`, before it probes anything: a new `PathError` variant naming the path (for example `NotAFileName`), or `NotAFile`, which already means “no file goes here”.  Both halves then refuse it the same way, with no write.  The README’s symlink section and both functions’ rustdoc gain one sentence each.

## Implementation Notes

- A refusal must keep refusing where `Clobber::Deny` refuses today: the black-box test `a_plain_file_spelled_with_a_separator_is_checked_as_the_file` pins `Clobber` for an existing `file/` and `file/.`, so those cases move to the new refusal, not to `Ok`.
- `PathError` is `#[non_exhaustive]`, so a new variant is additive; reusing `NotAFile` adds nothing.  Turning today’s `Ok` into an error is a behavior change, so it suits a minor release, or a patch release if read as a fix.
- The property test in `tests/check_equals_ensure.rs` builds these spellings for every shape; with the refusal in, both halves refuse them alike, and the test can assert that the approved file is writable.

## Why Not a Workaround

Each tool would check its output argument’s spelling itself, and the first one that forgets writes a report of success for a file it then cannot create.  Rejecting the plan is a reasonable exit: the write fails loudly, and a tool can print a clear error then.

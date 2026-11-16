# Plan: `ExitTable` Refuses 101, a Rust Panic’s Code

Filed on 16-Nov-2026, from the review of plan-0020 part 4.

## Problem

`ExitTable::code` refuses 0 to 4, whose meanings are canonical, and codes above 125, where the shell’s own codes are.  `mode` refuses only those above 125.  Both accept 101, which is the code a Rust program exits with when it panics.  A tool that declares 101 for a condition of its own can never be told apart from one that crashed.  The `testing` helpers (0.6.7) fail exit 101 whatever the table says, so such a tool can never pass them.  Since 0.6.7 their message says why, but the table should not have accepted the code in the first place.

## Proposed Change

`code(101, …)` and a mode listing 101 panic, as `code` does for the codes it already refuses, with a message saying that 101 is a Rust panic’s code.  The rustdoc of both, and README § 2, name the refusal.

## Implementation Notes

- This changes the published contract of 0.6.5’s `ExitTable`: a table that builds today would panic.  So it is a breaking change in the semver sense, and lands in 0.7.0, or in a 0.6.x only once the maintainer agrees that no tool declares 101.
- The house exit-code rule does not reserve 101 today.  If this lands, the rule should say why the code is reserved.
- Test: `code(101, …)` and `mode(…, &[(101, …)])` each panic, and the canonical codes still render as `HELP_BLOCK`.

## Why Not a Workaround

A tool would have to know about Rust’s panic code to avoid it; the table is the one place every tool declares its codes, so it is where the mistake can be refused.

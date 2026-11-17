# Plan: The Examples Return `ExitCode` From `main`

## Problem

Three examples end with a code by calling `std::process::exit`, seven times in all: `broken_pipe` once, `destination` three times, and `inputs` three times.  The lint set in `clippy.toml` refuses `process::exit`, because it skips destructors and buffered output and scatters a tool’s exit table across its code.  The lint’s remedy is to return `std::process::ExitCode` from `main`, with the codes from `cli_contract::exit`.

The examples are where the crate shows a tool using those codes, so today they teach the call the lint set refuses.  Nothing is lost today: no output is buffered at any of the seven calls, and the only destructors they skip free memory.  So this is a cleanup, not a bug.  Each call carries an `#[expect(clippy::disallowed_methods, reason = "plan-0024: …")]` until this lands.

## Proposed Change

Each example’s `main` returns `ExitCode`, and every exit goes through it:

- **`broken_pipe`** returns `io::Result<()>` today, so an I/O error prints `Error: …` and exits 1.  Its body moves into a function that returns `io::Result<()>`.  `main` maps `Ok` to `exit::OK`, and an error to a message and `exit::TOOL_ERROR`.  The usage line returns `exit::USAGE`.
- **`destination`** parses its flags by hand, and its `usage` helper returns `!`.  The parse moves into a function that returns the parsed flags, or the usage message.  `main` maps the message to `exit::USAGE`, and a `PolicyError` from `DestPolicy::from_flags` or a `PathError` to its `exit_code()`.
- **`inputs`** does the same with its two usage exits and its contract error.

Each `#[expect]` goes with its call.

## Implementation Notes

- `exit::USAGE`, `PolicyError::exit_code`, and `PathError::exit_code` are `u8`, so `ExitCode::from` takes them as they are.
- Two test files already run examples:
  - `tests/testing.rs` builds `broken_pipe` and runs the `testing` helpers against it.  Those tests pin its codes with a closed stdout.
  - `tests/blackbox.rs` builds `inputs` and pins exit 2 for a marker that is not a name.

  The change must keep both passing.  No test runs `destination`, so the change wants a test of its usage exit, 2, and of one contract refusal, 1.  `inputs` wants one contract refusal too.
- The `exit` module’s doc says that `std::process::exit` takes a code with `.into()`.  Once the examples no longer call it, that sentence can name `ExitCode` alone.
- The examples ship in the tarball, so the change goes out with the next release.  It changes no API.

## Why Not a Workaround

The `#[expect]` keeps the gate green, but it leaves the crate’s own examples teaching the call its lint set refuses.  Rejecting this plan is a reasonable exit: the examples are correct as they are.

# Plan: A Stderr Writer That Never Panics and Never Changes the Exit Code

Re-filed at the public launch (31-Oct-2026) from the predecessor repository, where it was filed on 28-Oct-2026.

## Problem

`eprintln!` panics when its write fails.  A write to a pipe whose reader has gone fails with `EPIPE`, because Rust ignores `SIGPIPE`, so the write returns an error instead of killing the process.  The panic message goes to the same dead stderr, so the process exits 101 and says nothing.  This crate’s `pipe` module and README § 3 cover stdout only, and stderr has the same failure.

It was reproduced on 28-Oct-2026 with a scratch binary.  The binary sleeps 300 ms so the stderr reader has exited, writes one notice to stderr, then does its “step 2”: it creates a marker file.  If the marker is missing afterwards, the notice stopped the run partway.

| The notice written with | stderr to an exited reader (`2> >(true)`) | stderr closed (`2>&-`) | `2>/dev/null` |
|---|---|---|---|
| `eprintln!(…)` | **exit 101, step 2 never ran** | 0, ran | 0, ran |
| `writeln!(std::io::stderr(), …)?` | **exit 1, step 2 never ran** | 0, ran | 0, ran |
| `let _ = writeln!(std::io::stderr(), …)` | 0, ran | 0, ran | 0, ran |
| `reset_sigpipe()`, then `let _ = writeln!(…)` | **exit 141, step 2 never ran** | 0, ran | 0, ran |

A closed stderr is harmless, because std treats `EBADF` on its standard streams as success.  Only a pipe whose reader has gone triggers the failure.  A common one is the agent idiom `tool 2>&1 | head`.  Run as `tool 2>&1 | true`, the `eprintln!` row stopped before step 2, **and the pipeline exited 0**, since a pipeline reports its last command.  Only under `set -o pipefail` did the 101 show.  So the caller is told nothing went wrong.

There are two consequences:

- **In `main`’s error path**, the panic turns exit 1 into 101: a tool error reported as a crash.  Some tools built on this crate, and a template that scaffolds new ones, already write `main`’s error with the hand-rolled `let _ = writeln!(std::io::stderr(), …)` for this reason.
- **Mid-run it is worse.**  A notice written while a mutating run is under way stops the run partway, and `writeln!(stderr, …)?` stops it too, just with exit 1.  That is the hazard `pipe::ignore_broken_pipe` already removes for stdout progress.

The crate has no helper, so every adopter hand-rolls the `let _ =` form, or keeps the panic.  Reviews of the crate’s dependents found both, in a list that is not exhaustive:

- **Mid-run**, in six mutating tools: notices written while a store is being changed, an update mode’s notices, `writeln!(stderr, …)?` sites, a scan’s tripwire warning and its store writes, and an installer’s publish and configuration steps.
- **In `main`**, in ten more.

For scale, a grep of the dependents’ `src/` trees for `eprint!` and `eprintln!` found 755 sites in 30 repositories.  Tests were not separated out, so this is an upper bound on what needs migrating, and many sites sit in read-only tools, where the exposure is lower.

## Proposed Change

Add two macros to `pipe`, beside `ignore_broken_pipe`, and re-export them at the crate root:

```rust
cli_contract::errln!("warning: skipped {}: {reason}", path.display());   // like eprintln!
cli_contract::errprint!("\rscanned {n} files");                         // like eprint!
```

- **The same syntax as `eprintln!` and `eprint!`**, so migrating a site changes only the macro’s name.
- **It never panics.  It ignores every write error and evaluates to `()`.**  With no `Result` to return, a caller cannot `?` it into stopping the run, and the macro cannot change the exit code.  `main` still returns 1 after reporting an error, whether or not the report reached anyone.
- **The pipe policy: ignore a failed stderr write in every mode, for every error kind**, not only `BrokenPipe`.  stderr is the channel of last resort, so there is nowhere to report a failure to write to it, and the exit code is the signal that survives.  That makes it deliberately broader than `ignore_broken_pipe`, which lets every non-pipe error through.  stdout carries the product, so a real stdout I/O error must stay loud.  A stderr notice is never the product.
- **One function underneath**, `pipe::write_stderr(args: fmt::Arguments<'_>)`, which the macros expand to.  It locks stderr and calls `write_fmt`.  It is public, so a tool that wants its own prefix can wrap it without re-deriving the policy.

**The `reset_sigpipe` interaction is documented, not engineered around.**  The table’s last row shows that once a read-only tool restores `SIG_DFL`, the kernel ends the process with `SIGPIPE` inside the write, before the macro sees any error.  Under that disposition, a gone stderr reader ends the process at 141.  That is the crate’s documented read-only policy (“there is nothing left to do once no one is reading”), now applied to stderr.  It does no harm, because a tool that calls `reset_sigpipe` mutates nothing.  The mutating tools, where stopping partway does harm, leave `SIGPIPE` ignored, and the macro protects them completely.  Two ways to engineer around it were considered and not proposed:

- `fcntl(F_SETNOSIGPIPE)` on fd 2 is macOS-only, and Linux has no per-descriptor equivalent for pipes.
- Blocking `SIGPIPE` around each write and draining the pending signal is unsafe signal-mask code for a rare case, and macOS lacks `sigtimedwait`, which the usual drain relies on.

**How adopters migrate:**

1. Raise the `cli-contract` dependency to the release that ships the macros.
2. Replace `eprintln!` and `eprint!` with `errln!` and `errprint!`.  Also replace the `writeln!(std::io::stderr(), …)` forms, whether they end in `?`, `.unwrap()`, or `let _ =`: the first two carry the bug, and the third is correct but hand-rolled.  A single spelling is what lets the gate below be complete.
3. **Gate it mechanically.**  Add a `clippy.toml` that bans the panicking macros:

   ```toml
   disallowed-macros = [
     { path = "std::eprintln", reason = "panics (exit 101) when stderr's reader has gone; use cli_contract::errln!" },
     { path = "std::eprint", reason = "panics (exit 101) when stderr's reader has gone; use cli_contract::errprint!" },
   ]
   ```

   Verified on 28-Oct-2026 with clippy 0.1.97: it flags `eprintln!` (“use of a disallowed macro `std::eprintln`”).  The gate does not catch `writeln!(…stderr…)?`, because `writeln!` is not banned, so the migration also greps for that form.  Whether clippy’s `disallowed-methods` on `std::io::stderr` could catch it without also flagging the macros’ own expansion is unverified.  Check it at implementation time, and do not assume it.
4. **Order the work by exposure:** first the mutating tools with mid-run notices, then the `main`-only sites, then the rest.  Each adopter’s mid-run instance is a bug in that adopter, so it is fixed and closed there.  This plan supplies the tool those fixes use.
5. **A template that scaffolds new tools on this crate** switches its `main` from the hand-rolled `let _ = writeln!` to `errln!`, and gains the `clippy.toml`, once the macros ship.

## Implementation Notes

- **Where it lives:** `src/pipe.rs`, with `#[macro_export]` macros (which land at the crate root, `cli_contract::errln!`), plus `write_stderr`.  The module docs and README § 3 gain a stderr paragraph and the table’s four rows.
- **The expansion** must name everything by absolute path (`::std::io::Write`, `$crate::pipe::write_stderr`), so a caller needs no `use` for it to compile.  `errln!()` with no arguments prints a bare newline, as `eprintln!()` does.
- **The test** must fail when the macro is replaced with `eprintln!`, and must assert on the side effect, not only on the exit code.  Shape: the test re-executes its own binary (`std::env::current_exe()`, an env var selecting the child branch, `--exact <name>`), as `reset_sigpipe`’s test in `src/pipe.rs` already does, with the child’s stderr piped and its stdin piped.  The parent drops the stderr read end, then closes stdin.  The child blocks reading stdin until then, which takes the timing out of it, then calls `errln!` and prints a sentinel on stdout.  The test asserts that the child exited 0 **and** that the sentinel arrived.  Verify the revert check by hand once: with `eprintln!` in its place, the child must exit 101 with no sentinel.
- **A second test pins the documented 141** under `reset_sigpipe` on Unix, so the behavior the docs describe is tested.
- **An additive change**, with no change to any existing signature.  If it lands before 0.6.0 is published, it can ride in that release.
- **Prior art:** one dependent already ships an `errln!` macro of its own, beside an `outln!` for stdout, at its 19 store-changing `eprintln!` sites.  The name and the shape have been tried in practice.

**Considered and rejected:**

- **Shadowing std’s macro**, by exporting a macro named `eprintln` for `use cli_contract::eprintln;`.  It is a one-line migration, but a forgotten import silently compiles to the panicking std macro, and no grep or lint can tell the two apart.  A distinct name is what makes the clippy gate possible.
- **`report_error(&e)`**, a function that writes `error: {e:#}`.  Once `errln!` exists it saves only the format string, and adopters disagree on the prefix (`error:`, `ERROR:`, `Error:`).  A tool that wants one can wrap `write_stderr`.  Revisit only if the `--help` section builders in `ROADMAP.md` grow a standard error format.
- **Catching the panic** (`catch_unwind` around `main`, or a panic hook).  The run has already unwound out of its work by then, so the mid-run stop still happens, and only the exit code is repaired.

## Why Not a Workaround

The workaround is the status quo: each adopter writes `let _ = writeln!(std::io::stderr(), …)` by hand.  It is correct where it is written.  The 755 `eprint!` and `eprintln!` sites in 30 repositories are what it leaves behind.

- **Hand-rolled copies drift.**  The scaffolding template got it right, and ten tools built on this crate still have `eprintln!` in `main`.  One wrote the plausible `writeln!(stderr, …)?`, which still stops a run partway, just with exit 1.  Each copy is one more place for the next variant of the mistake.
- **One tested implementation.**  The crate exists so that a fix, or a bug, never lives in one tool and not another.  The broken-pipe policy for stdout is already here, so the stderr half belongs beside it.  The test above runs once, here, instead of once per copy, or never.
- **It makes the gate possible.**  Without a sanctioned replacement, a `disallowed-macros` ban on `std::eprintln` has nothing to point to.  With one, a lint ban plus one grep for `writeln!(…stderr…)?` makes the migration checkable, and keeps it from regressing.

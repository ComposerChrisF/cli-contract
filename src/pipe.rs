//! BrokenPipe handling for CLIs whose output can be closed early (`tool | head`).
//!
//! Rust ignores `SIGPIPE` by default, so a write to a closed pipe returns an `EPIPE` error that the
//! `print!`/`println!` macros unwrap into a panic (exit 101) — and for a tool doing real work while
//! it streams, the job is truncated mid-flight.  There are two correct fixes, and which one a tool
//! wants depends on whether its output *is* the product or is merely incidental progress:
//!
//! - **Read-only / enumerating tools** (a listing, a report — the output is the whole point): call
//!   [`reset_sigpipe`] once at the top of `main`.  When the reader goes away the process is
//!   terminated quietly by `SIGPIPE` (the shell sees exit 141), the standard Unix behavior — there
//!   is nothing left to do once no one is reading.  Windows has no `SIGPIPE`: there the call does
//!   nothing, and the tool still panics unless it handles the write error itself.
//! - **Mutating tools that stream progress** (a backup that prints each copied file): the output is
//!   incidental, and the operation must *not* be aborted just because a progress pipe closed.  Do
//!   **not** call [`reset_sigpipe`] there (it would kill the job on the first write after the reader
//!   closed); instead write progress through [`ignore_broken_pipe`], which swallows a broken-pipe
//!   error so the work runs to completion ("suppress and finish") while still surfacing any real
//!   I/O error.

use std::io;

/// Restore the default disposition for `SIGPIPE` on Unix (terminate the process on a broken pipe).
///
/// Call once at the very top of `main`, before any output or threads, in a **read-only /
/// enumerating** tool.  On a broken pipe the process ends via the signal (the shell sees 141), with
/// no panic and no stderr noise — the standard behavior of Unix tools in a pipeline.  No-op on
/// non-Unix.
///
/// Do **not** use this in a tool that mutates state while streaming progress: it would abort the
/// operation on the first write after the reader closed (turning the exit-101 truncation bug into an
/// exit-141 truncation bug).  Use [`ignore_broken_pipe`] there instead.
pub fn reset_sigpipe() {
    #[cfg(unix)]
    // SAFETY: setting SIGPIPE to SIG_DFL is a process-global disposition change using an
    // async-signal-safe call; intended to run once at startup before threads or output exist.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

/// True if `err` is a broken-pipe error — the reader on the other end of the pipe has closed.
pub fn is_broken_pipe(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::BrokenPipe
}

/// Turn a broken-pipe write error into `Ok(())`, so a **mutating** tool can keep working after its
/// output pipe closes ("suppress and finish") while still propagating every other I/O error.
///
/// Wrap progress writes with it — `ignore_broken_pipe(writeln!(out, "{line}"))?` — in a tool that
/// must complete its operation regardless of whether anyone is still reading.  Relies on `SIGPIPE`
/// remaining ignored (Rust's default), i.e. the tool must **not** also call [`reset_sigpipe`].
pub fn ignore_broken_pipe(result: io::Result<()>) -> io::Result<()> {
    match result {
        Err(e) if is_broken_pipe(&e) => Ok(()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error, ErrorKind};

    #[test]
    fn ignore_broken_pipe_swallows_only_broken_pipe() {
        assert!(ignore_broken_pipe(Ok(())).is_ok());
        assert!(ignore_broken_pipe(Err(Error::from(ErrorKind::BrokenPipe))).is_ok());
        // A real I/O error still propagates.
        let other = ignore_broken_pipe(Err(Error::from(ErrorKind::PermissionDenied)));
        assert_eq!(other.unwrap_err().kind(), ErrorKind::PermissionDenied);
    }

    #[test]
    fn is_broken_pipe_detects_the_kind() {
        assert!(is_broken_pipe(&Error::from(ErrorKind::BrokenPipe)));
        assert!(!is_broken_pipe(&Error::from(ErrorKind::NotFound)));
    }

    /// Set only in the child process that [`reset_sigpipe_ends_the_process_on_a_closed_pipe`]
    /// starts, so the child half never runs in the test process itself, which it would kill.
    #[cfg(unix)]
    const SIGPIPE_CHILD: &str = "CLI_CONTRACT_SIGPIPE_CHILD";

    /// The child half: reset `SIGPIPE`, then write to a pipe whose reader is closed.  The write
    /// must end the process by the signal; returning at all means it did not.
    #[cfg(unix)]
    #[test]
    #[ignore = "run only as the child of reset_sigpipe_ends_the_process_on_a_closed_pipe"]
    fn sigpipe_child() {
        use std::io::Write;
        use std::os::unix::io::FromRawFd;
        if std::env::var_os(SIGPIPE_CHILD).is_none() {
            return;
        }
        let mut fds = [0; 2];
        // SAFETY: `pipe` writes two new descriptors into `fds`, which has room for both.
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        // SAFETY: `fds[0]` is the read end just created, and nothing else closes it.
        unsafe { libc::close(fds[0]) };
        reset_sigpipe();
        // SAFETY: `fds[1]` is the write end just created, owned by this `File` alone.
        let mut writer = unsafe { std::fs::File::from_raw_fd(fds[1]) };
        let _ = writer.write_all(b"no one is reading");
    }

    #[cfg(unix)]
    #[test]
    fn reset_sigpipe_ends_the_process_on_a_closed_pipe() {
        use std::os::unix::process::ExitStatusExt;
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "pipe::tests::sigpipe_child", "--include-ignored"])
            .env(SIGPIPE_CHILD, "1")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert_eq!(
            status.signal(),
            Some(libc::SIGPIPE),
            "the child was not ended by SIGPIPE: {status}"
        );
    }
}

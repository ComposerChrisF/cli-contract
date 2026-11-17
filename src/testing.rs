//! Contract tests that every tool needs, for its integration tests: a closed stdout or stderr does
//! not crash it, a dry run writes nothing, and `--help` carries its exit table.  Behind the
//! `testing` feature, which adds no dependency.
//!
//! The helpers aid a tool's author, and assume that the tool and its tests act in good faith.  Each
//! catches the mistake a well-meaning tool makes by accident, such as a dry run that still writes,
//! or an `eprintln!` that panics once stderr's reader has gone.  None tries to catch a tool that
//! hides a change on purpose, or a case too unlikely to arise in a real tool's tests: such a gap is
//! stated as a limitation, never closed with code.
//!
//! Each runs the tool as a [`Command`] — for a binary in the same package,
//! `Command::new(env!("CARGO_BIN_EXE_<name>"))` — and panics, naming the command and what broke,
//! when the tool does not keep the contract, as `assert!` does.  Each also checks the exit code it
//! sees against the tool's [`ExitTable`]: a code the table does not list, in its main table or in
//! a mode, fails the test, and so does a panic, which is exit 101 whatever the table says (or,
//! under `panic = "abort"`, an end by a signal).  A panic is told by its exit alone, never by
//! reading the tool's output, so a panic in a thread the tool survives is not seen.  So these
//! helpers' own runs keep the table true of the codes they see; a code the tool's other tests see
//! is not checked.  Each returns the run's [`Output`], for a test that asserts more.  A message
//! names the command by its program, arguments, and directory, never its environment, which may
//! hold a secret.
//!
//! ```no_run
//! use std::process::Command;
//! use cli_contract::exit::ExitTable;
//! use cli_contract::testing::{closed_stdout, help_has_exit_table};
//!
//! // In a tool's tests/contract.rs, with the binary at env!("CARGO_BIN_EXE_<name>"):
//! let table = ExitTable::canonical();
//! let tool = || Command::new("mytool");
//! closed_stdout(tool().arg("list"), &table);
//! help_has_exit_table(&mut tool(), &table);
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use crate::exit::ExitTable;
use crate::paths::{presence_resolved, Presence};

/// The exit code of a Rust program that panicked.
const PANICKED: i32 = 101;

/// Run `cmd` with its stdout a stream that refuses every write, as a pipe does once its reader has
/// gone (`tool | head` after `head` has exited), and assert that the tool did not panic.  It passes
/// when the tool exits with a code `table` lists, or, on Unix, when `SIGPIPE` ends it, as it ends a
/// tool that calls [`reset_sigpipe`](crate::reset_sigpipe).  Elsewhere `reset_sigpipe` does
/// nothing, so a tool that relies on it alone panics on that write and fails this (see the
/// README's BrokenPipe section).  Give it a command that writes to stdout, or it passes without
/// testing anything.
///
/// On Unix the stream is a socket whose writing side is shut down before the tool starts, so its
/// first write fails, with `EPIPE` and `SIGPIPE`, as a write to a pipe with no reader does, whoever
/// else holds the socket.  Elsewhere it is a pipe whose reader is closed as soon as the tool has
/// started, so a tool that writes at once may write a little first.  Sets the command's stdin
/// (empty), stdout, and stderr (captured).  A mutating tool must finish its work whether or not
/// anyone reads, so its test should assert on the work itself: a run that stopped at its first
/// failed write may still exit with a listed code, such as 1.
///
/// # Panics
///
/// When the tool panics, exits with a code `table` does not list, or is ended by any other
/// signal, and when it cannot be run.
#[track_caller]
pub fn closed_stdout(cmd: &mut Command, table: &ExitTable) -> Output {
    cmd.stdin(Stdio::null()).stderr(Stdio::piped());
    let output = run_with_writes_refused(cmd, Stream::Stdout);
    check(cmd, &output, table, "with stdout closed", true);
    output
}

/// [`closed_stdout`] for stderr: run `cmd` with its stderr a stream that refuses every write, as
/// `tool 2> >(true)` leaves it once `true` has exited, and assert that the tool did not panic.
/// `eprintln!` panics on that write, so a tool that writes its messages with it fails this; one
/// that writes them with `let _ = writeln!(std::io::stderr(), …)` passes.  Give it a command that
/// writes to stderr, such as one that fails or warns, or it passes without testing anything.  Sets
/// the command's stdin (empty), stdout (captured), and stderr.
///
/// # Panics
///
/// As [`closed_stdout`].
#[track_caller]
pub fn closed_stderr(cmd: &mut Command, table: &ExitTable) -> Output {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped());
    let output = run_with_writes_refused(cmd, Stream::Stderr);
    check(cmd, &output, table, "with stderr closed", true);
    output
}

/// Snapshot the directory `tree`, run `cmd`, and assert that the run changed nothing in it: no
/// path added or removed, and each the same type, and for a file the same size and bytes, for a
/// link the same target.  The tool must exit with a code `table` lists, without panicking.  Give
/// `cmd` the tool's dry-run flag and targets inside `tree`, and fill `tree` so that the live run
/// would change it, or this passes without testing anything.  A link at `tree` itself is followed,
/// since the caller named it; links below it are compared as links, never followed, so a change
/// outside `tree` is not seen.  Nor is any other write outside `tree`, so point the tool's own
/// state, such as a ledger or a cache, inside it too, as by setting `HOME`, or the variable that
/// names the tool's config, to a directory there.  The changes are reported first, with how the run
/// ended, even when it also exited with a code `table` does not list or panicked.  A run that
/// removes `tree` is reported as removing everything in it.  A link put in its place is followed,
/// so a run that moves `tree` and links its name to the new place is not seen.  Sets the command's
/// stdin (empty), stdout, and stderr (captured).
///
/// # Panics
///
/// When the run changed `tree`, naming each path added, removed, or changed, `tree` itself as
/// `.`; when the tool panics, exits with a code `table` does not list, or is ended by a signal;
/// when the tool cannot be run; and when `tree` does not exist, is not a directory, or cannot be
/// read.  A read that fails after the run names the run and how it ended.
#[track_caller]
pub fn dry_run_writes_nothing(cmd: &mut Command, tree: &Path, table: &ExitTable) -> Output {
    let before = match snapshot(tree) {
        Ok(Tree::Dir(entries)) => entries,
        Ok(Tree::Gone) => panic!("{} does not exist", tree.display()),
        Err(why) => panic!("{why}"),
    };
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = run(cmd);
    let after = match snapshot(tree) {
        Ok(Tree::Dir(entries)) => entries,
        Ok(Tree::Gone) => BTreeMap::new(),
        Err(why) => panic!(
            "after {}, a dry run, which ended with {}, {why}",
            describe(cmd),
            output.status
        ),
    };
    let changed: Vec<String> = before
        .keys()
        .chain(after.keys().filter(|path| !before.contains_key(*path)))
        .filter(|path| before.get(*path) != after.get(*path))
        .map(|path| {
            let how = match (before.contains_key(path), after.contains_key(path)) {
                (false, _) => "added",
                (_, false) => "removed",
                _ => "changed",
            };
            format!("{how}: {}", shown(path))
        })
        .collect();
    assert!(
        changed.is_empty(),
        "{}, a dry run, changed {} and ended with {}:\n{}",
        describe(cmd),
        tree.display(),
        output.status,
        changed.join("\n")
    );
    check(cmd, &output, table, "in a dry run", false);
    output
}

/// Run `cmd --help` and assert that it exits 0 and prints `table.render()` whole, as a tool does
/// that gives the table to clap's `after_long_help`.  Clap's `wrap_help` feature wraps that text,
/// at 100 columns when stdout is not a terminal, so with it each line of the table must fit in
/// 100 columns; a meaning that spans lines is indented under its first.  Adds `--help` to the
/// command's arguments, so a subcommand's help is tested by naming the subcommand first.  The
/// command keeps it, so give a helper run after this one a command of its own.  Sets the command's
/// stdin (empty), stdout, and stderr (captured), and removes `COLUMNS` from its environment, from
/// which clap would take a width of its own.
///
/// # Panics
///
/// When the help does not carry the table, when the tool panics, exits with anything but 0, or is
/// ended by a signal, and when it cannot be run.
#[track_caller]
pub fn help_has_exit_table(cmd: &mut Command, table: &ExitTable) -> Output {
    cmd.arg("--help")
        .env_remove("COLUMNS")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = run(cmd);
    check(cmd, &output, table, "for --help", false);
    assert_eq!(
        output.status.code(),
        Some(i32::from(crate::exit::OK)),
        "{} did not exit 0:\n{}",
        describe(cmd),
        String::from_utf8_lossy(&output.stderr)
    );
    let help = String::from_utf8_lossy(&output.stdout);
    let rendered = table.render();
    if !help.contains(&rendered) {
        let words = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
        let respaced = if words(&help).contains(&words(&rendered)) {
            "; it is there with other line breaks or spacing, as when clap's wrap_help re-wraps \
             a line over 100 columns"
        } else {
            ""
        };
        panic!(
            "{} does not print the exit table{respaced}:\n{rendered}\n--- it printed ---\n{help}",
            describe(cmd)
        );
    }
    output
}

/// The stream that refuses every write.
#[derive(Clone, Copy)]
enum Stream {
    Stdout,
    Stderr,
}

/// The command as it would be typed, with its directory, but not its environment, which may hold
/// a secret.
fn describe(cmd: &Command) -> String {
    use std::fmt::Write as _;
    let mut line = format!("{:?}", cmd.get_program());
    for arg in cmd.get_args() {
        let _ = write!(line, " {arg:?}");
    }
    match cmd.get_current_dir() {
        Some(dir) => format!("cd {dir:?} && {line}"),
        None => line,
    }
}

/// Run `cmd`, or panic naming it.
#[track_caller]
fn run(cmd: &mut Command) -> Output {
    match cmd.output() {
        Ok(output) => output,
        Err(e) => panic!("could not run {}: {e}", describe(cmd)),
    }
}

/// Run `cmd` with `stream` a socket whose writing side is shut down before the tool starts.
#[cfg(unix)]
#[track_caller]
fn run_with_writes_refused(cmd: &mut Command, stream: Stream) -> Output {
    let (end, peer) = socket_pair_refusing_writes();
    drop(peer);
    let refusing = Stdio::from(end);
    match stream {
        Stream::Stdout => cmd.stdout(refusing),
        Stream::Stderr => cmd.stderr(refusing),
    };
    run(cmd)
}

/// Run `cmd` with `stream` a pipe whose reader is dropped as soon as the tool has started.
#[cfg(not(unix))]
#[track_caller]
fn run_with_writes_refused(cmd: &mut Command, stream: Stream) -> Output {
    match stream {
        Stream::Stdout => cmd.stdout(Stdio::piped()),
        Stream::Stderr => cmd.stderr(Stdio::piped()),
    };
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => panic!("could not run {}: {e}", describe(cmd)),
    };
    match stream {
        Stream::Stdout => drop(child.stdout.take()),
        Stream::Stderr => drop(child.stderr.take()),
    }
    match child.wait_with_output() {
        Ok(output) => output,
        Err(e) => panic!("could not wait for {}: {e}", describe(cmd)),
    }
}

/// A new socket pair, both ends close-on-exec, with the first end's writing side shut down, so
/// that every write to it fails with `EPIPE` and `SIGPIPE`, whoever holds the second: a child that
/// another thread starts meanwhile may inherit it, and that changes nothing.
#[cfg(unix)]
#[track_caller]
fn socket_pair_refusing_writes() -> (std::os::unix::io::OwnedFd, std::os::unix::io::OwnedFd) {
    use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};
    let mut fds = [0; 2];
    // SAFETY: socketpair writes two new descriptors into the array, which outlives the call.
    let made = unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) };
    assert_eq!(
        made,
        0,
        "could not make a socket pair: {}",
        std::io::Error::last_os_error()
    );
    // SAFETY: both descriptors are new, and each is owned here alone from now on.
    let (end, peer) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    for fd in [&end, &peer] {
        // SAFETY: fcntl sets a flag on a descriptor that `end` or `peer` owns.
        let set = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) };
        assert_ne!(
            set,
            -1,
            "could not make a socket close-on-exec: {}",
            std::io::Error::last_os_error()
        );
    }
    // Shut while the pair is still connected, which macOS requires.
    // SAFETY: shutdown acts on a descriptor that `end` owns.
    let shut = unsafe { libc::shutdown(end.as_raw_fd(), libc::SHUT_WR) };
    assert_eq!(
        shut,
        0,
        "could not shut a socket's writing side: {}",
        std::io::Error::last_os_error()
    );
    (end, peer)
}

/// Assert that a run did not panic and ended with a code `table` lists, or, where `sigpipe_ends`
/// allows, by `SIGPIPE`.
#[track_caller]
fn check(cmd: &Command, output: &Output, table: &ExitTable, when: &str, sigpipe_ends: bool) {
    let what = describe(cmd);
    match output.status.code() {
        Some(PANICKED) => panic!("{what} exited 101 {when}: it panicked"),
        Some(code) => assert!(
            u8::try_from(code).is_ok_and(|code| table.lists(code)),
            "{what} exited {code} {when}, a code its exit table does not list"
        ),
        None => assert!(
            sigpipe_ends && ended_by_sigpipe(output.status),
            "{what} was ended by a signal {when}: {}",
            output.status
        ),
    }
}

#[cfg(unix)]
fn ended_by_sigpipe(status: std::process::ExitStatus) -> bool {
    use std::os::unix::process::ExitStatusExt;
    status.signal() == Some(libc::SIGPIPE)
}

#[cfg(not(unix))]
fn ended_by_sigpipe(_: std::process::ExitStatus) -> bool {
    false
}

/// What a snapshot keeps of one entry.
#[derive(Debug, PartialEq, Eq)]
enum Entry {
    Dir,
    /// A regular file's bytes, and so its size.
    File(Vec<u8>),
    /// A link's target, as written.
    Link(PathBuf),
    /// Anything else (a FIFO, a socket, a device): its type alone.
    Other(fs::FileType),
}

/// What stands at a snapshot's root.
enum Tree {
    /// A directory: every entry in it, by path relative to it, the root itself as the empty path.
    Dir(BTreeMap<PathBuf, Entry>),
    /// Provably nothing: `NotFound`, and nothing else.
    Gone,
}

/// Snapshot `root`, following a link there, as [`presence_resolved`] does; links below it are not
/// followed.  `Err` says what could not be read.
fn snapshot(root: &Path) -> Result<Tree, String> {
    match presence_resolved(root) {
        Ok(Presence::Absent) => return Ok(Tree::Gone),
        Ok(Presence::Present(_)) => {}
        Err(e) => return Err(e.to_string()),
    }
    let mut entries = BTreeMap::from([(PathBuf::new(), Entry::Dir)]);
    let mut dirs = vec![PathBuf::new()];
    while let Some(dir) = dirs.pop() {
        let listed = root.join(&dir);
        for child in read(&listed, fs::read_dir(&listed))? {
            let relative = dir.join(read(&listed, child)?.file_name());
            let path = root.join(&relative);
            let file_type = read(&path, fs::symlink_metadata(&path))?.file_type();
            let entry = if file_type.is_dir() {
                dirs.push(relative.clone());
                Entry::Dir
            } else if file_type.is_file() {
                Entry::File(read(&path, fs::read(&path))?)
            } else if file_type.is_symlink() {
                Entry::Link(read(&path, fs::read_link(&path))?)
            } else {
                Entry::Other(file_type)
            };
            entries.insert(relative, entry);
        }
    }
    Ok(Tree::Dir(entries))
}

/// What a read of `path` gave, or an error naming it.
fn read<T>(path: &Path, result: std::io::Result<T>) -> Result<T, String> {
    result.map_err(|e| format!("could not read {}: {e}", path.display()))
}

/// A snapshot's path as a message names it, the root as `.`.
fn shown(path: &Path) -> String {
    if path.as_os_str().is_empty() {
        ".".to_owned()
    } else {
        path.display().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// rustdoc of closed_stdout: "whoever else holds the socket".  The other end is held open, as a
    /// copy inherited by another child would be, and the first write still fails.
    #[cfg(unix)]
    #[test]
    fn the_first_write_fails_while_the_other_end_is_held() {
        use std::os::unix::process::ExitStatusExt;
        let (end, peer) = socket_pair_refusing_writes();
        let status = Command::new("sh")
            .args(["-c", "printf x; sleep 5; exit 3"])
            .stdout(Stdio::from(end))
            .status()
            .unwrap();
        drop(peer);
        assert_eq!(status.signal(), Some(libc::SIGPIPE), "{status}");
    }
}

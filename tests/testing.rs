//! Black-box tests of the `testing` feature's helpers, each run against `sh -c` standing in for a
//! tool.  Every test comment quotes the rustdoc sentence it pins.
#![cfg(all(unix, feature = "testing"))]

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::process::Command;

use cli_contract::exit::ExitTable;
use cli_contract::testing::{
    closed_stderr, closed_stdout, dry_run_writes_nothing, help_has_exit_table,
};

fn sh(script: &str) -> Command {
    let mut cmd = Command::new("sh");
    cmd.args(["-c", script, "sh"]);
    cmd
}

/// The panic message of `f`, which must panic.
fn panics<T: std::fmt::Debug>(f: impl FnOnce() -> T) -> String {
    let payload = catch_unwind(AssertUnwindSafe(f)).expect_err("it did not panic");
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_default()
}

// --- a closed stdout or stderr ----------------------------------------------------------------

/// rustdoc of closed_stdout: "It passes when the tool exits with a code `table` lists, or, on
/// Unix, when `SIGPIPE` ends it".  `yes` is ended by `SIGPIPE` on its first write; under `sh` it
/// is `exec`'d, since `sh` would report that as its own exit 141.
#[test]
fn a_tool_ended_by_sigpipe_or_exiting_a_listed_code_passes() {
    let table = ExitTable::canonical();
    closed_stdout(&mut Command::new("yes"), &table);
    closed_stdout(&mut sh("exit 3"), &table);
    closed_stderr(&mut sh("exec yes >&2"), &table);
    closed_stderr(&mut sh("exit 0"), &table);
}

/// rustdoc of closed_stdout: "On Unix the stream is a socket whose writing side is shut down
/// before the tool starts, so its first write fails, with `EPIPE` and `SIGPIPE`".  A tool that
/// writes one byte and then sleeps is ended by that write at once.  `printf` is a builtin of `sh`,
/// so the signal ends `sh` itself, the process the helper sees.
#[test]
fn the_first_write_fails() {
    use std::os::unix::process::ExitStatusExt;
    let output = closed_stdout(
        &mut sh("printf x; sleep 5; exit 3"),
        &ExitTable::canonical(),
    );
    assert_eq!(
        output.status.signal(),
        Some(libc::SIGPIPE),
        "{:?}",
        output.status
    );
}

/// rustdoc of closed_stdout: "Sets the command's stdin (empty), stdout, and stderr (captured)".  A
/// command reused for a second run gets a stream of its own, and the last run's changes nothing for
/// the next.
#[test]
fn every_run_gets_a_stream_that_refuses_writes() {
    use std::os::unix::process::ExitStatusExt;
    let table = ExitTable::canonical();
    let mut cmd = sh("printf x; exit 3");
    for _ in 0..3 {
        let output = closed_stdout(&mut cmd, &table);
        assert_eq!(
            output.status.signal(),
            Some(libc::SIGPIPE),
            "{:?}",
            output.status
        );
    }
}

/// rustdoc of the module: "a panic, which is exit 101 whatever the table says", and "A panic is
/// told by its exit alone, never by reading the tool's output": a tool that only prints the words
/// of a panic message, and exits 0, passes.
#[test]
fn a_panic_fails_whatever_the_table_lists() {
    let table = ExitTable::canonical().code(101, "a code no table should give");
    let said = panics(|| {
        closed_stdout(&mut sh("exit 101"), &table);
    });
    assert!(said.contains("give that condition another code"), "{said}");
    let said = panics(|| {
        closed_stdout(&mut sh("exit 101"), &ExitTable::canonical());
    });
    assert!(said.contains("exited 101"), "{said}");
    let said = panics(|| {
        closed_stderr(&mut sh("exit 101"), &table);
    });
    assert!(said.contains("exited 101"), "{said}");
    closed_stdout(
        &mut sh("echo \"thread 'main' panicked at src/main.rs:1:1\" >&2; exit 0"),
        &table,
    );
}

/// rustdoc of the module: "a code the table does not list, in its main table or in a mode, fails
/// the test".
#[test]
fn a_code_the_table_does_not_list_fails_and_a_mode_lists_its_own() {
    let said = panics(|| {
        closed_stdout(&mut sh("exit 7"), &ExitTable::canonical());
    });
    assert!(said.contains("exited 7"), "{said}");
    let table = ExitTable::canonical().mode("--check", &[(7, "drift found")]);
    closed_stdout(&mut sh("exit 7"), &table);
}

/// rustdoc of closed_stdout: "When the tool panics, exits with a code `table` does not list, or is
/// ended by any other signal".
#[test]
fn another_signal_fails() {
    let said = panics(|| {
        closed_stdout(&mut sh("kill -TERM $$"), &ExitTable::canonical());
    });
    assert!(said.contains("signal"), "{said}");
}

/// Builds an example from the current source, in a target directory of its own, and returns its
/// path, as tests/blackbox.rs does.
fn build_example(name: &str) -> std::path::PathBuf {
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join("testing-examples");
    let out = Command::new(env!("CARGO"))
        .args([
            "build",
            "--quiet",
            "--no-default-features",
            "--example",
            name,
        ])
        .env("CARGO_TARGET_DIR", &target)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "building the {name} example failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    target.join("debug/examples").join(name)
}

/// rustdoc of closed_stdout: "when `SIGPIPE` ends it"; of closed_stderr: "`eprintln!` panics on
/// that write, so a tool that writes its messages with it fails this".  The crate's broken_pipe
/// example, a real Rust program: `list` calls `reset_sigpipe`, `work` suppresses a closed stdout
/// and finishes, and then reports with `eprintln!`.
#[test]
fn the_broken_pipe_example_passes_closed_stdout_and_its_eprintln_fails_closed_stderr() {
    use std::os::unix::process::ExitStatusExt;
    let bin = build_example("broken_pipe");
    let table = ExitTable::canonical();
    let status = closed_stdout(Command::new(&bin).arg("list"), &table).status;
    assert_eq!(status.signal(), Some(libc::SIGPIPE), "list: {status}");
    let status = closed_stdout(Command::new(&bin).arg("work"), &table).status;
    assert_eq!(status.code(), Some(0), "work: {status}");
    let said = panics(|| {
        closed_stderr(Command::new(&bin).arg("work"), &table);
    });
    assert!(said.contains("exited 101"), "{said}");
}

// --- a dry run --------------------------------------------------------------------------------

/// A tree with a file, a directory, and a link in it.
fn tree() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("a.txt"), b"a").unwrap();
    std::fs::create_dir(d.path().join("sub")).unwrap();
    std::fs::write(d.path().join("sub/b.txt"), b"b").unwrap();
    std::os::unix::fs::symlink("a.txt", d.path().join("link")).unwrap();
    d
}

fn in_tree(script: &str, tree: &Path) -> Command {
    let mut cmd = sh(script);
    cmd.current_dir(tree);
    cmd
}

/// rustdoc of dry_run_writes_nothing: "assert that the run changed nothing in it: no path added or
/// removed, and each the same type, and for a file the same size and bytes, for a link the same
/// target".
#[test]
fn a_dry_run_that_writes_is_caught_and_named() {
    let table = ExitTable::canonical();
    for (script, said) in [
        ("touch new.txt", "added: new.txt"),
        ("rm sub/b.txt", "removed: sub/b.txt"),
        ("printf b >> a.txt", "changed: a.txt"),
        ("printf c > sub/b.txt", "changed: sub/b.txt"),
        ("ln -sf sub/b.txt link", "changed: link"),
        ("rm a.txt && mkdir a.txt", "changed: a.txt"),
    ] {
        let d = tree();
        let message = panics(|| {
            dry_run_writes_nothing(&mut in_tree(script, d.path()), d.path(), &table);
        });
        assert!(message.contains(said), "{script}: {message}");
    }
}

/// rustdoc of the module: "Each returns the run's [`Output`], for a test that asserts more."  And
/// a run that only reads passes.
#[test]
fn a_dry_run_that_only_reads_passes_and_returns_its_output() {
    let d = tree();
    let output = dry_run_writes_nothing(
        &mut in_tree("cat a.txt sub/b.txt; exit 3", d.path()),
        d.path(),
        &ExitTable::canonical(),
    );
    assert_eq!(output.stdout, b"ab");
    assert_eq!(output.status.code(), Some(3));
}

/// rustdoc of dry_run_writes_nothing: "The tool must exit with a code `table` lists, without
/// panicking."
#[test]
fn a_dry_run_exiting_an_unlisted_code_fails() {
    let d = tree();
    let said = panics(|| {
        dry_run_writes_nothing(
            &mut in_tree("exit 9", d.path()),
            d.path(),
            &ExitTable::canonical(),
        );
    });
    assert!(said.contains("exited 9"), "{said}");
}

/// rustdoc of dry_run_writes_nothing: "links below it are compared as links, never followed, so a
/// change outside `tree` is not seen".  A link to a file outside, whose bytes change, and a link to
/// nothing, are each compared by target alone.
#[test]
fn links_in_the_tree_are_compared_as_links() {
    let table = ExitTable::canonical();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("x.txt"), b"x").unwrap();
    let d = tree();
    std::os::unix::fs::symlink(outside.path().join("x.txt"), d.path().join("out")).unwrap();
    std::os::unix::fs::symlink("nowhere", d.path().join("dangling")).unwrap();
    let script = format!("printf y >> '{}'", outside.path().join("x.txt").display());
    dry_run_writes_nothing(&mut in_tree(&script, d.path()), d.path(), &table);
    let said = panics(|| {
        dry_run_writes_nothing(
            &mut in_tree("ln -sf elsewhere dangling", d.path()),
            d.path(),
            &table,
        );
    });
    assert!(said.contains("changed: dangling"), "{said}");
}

/// Entry::Other's rustdoc: "Anything else (a FIFO, a socket, a device): its type alone."  A FIFO in
/// the tree is never read, and its removal is named.
#[test]
fn a_fifo_in_the_tree_is_kept_by_its_type() {
    use std::os::unix::ffi::OsStrExt;
    let table = ExitTable::canonical();
    let d = tree();
    let fifo = std::ffi::CString::new(d.path().join("pipe").as_os_str().as_bytes()).unwrap();
    // SAFETY: mkfifo reads a NUL-terminated path that outlives the call.
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    dry_run_writes_nothing(&mut in_tree("true", d.path()), d.path(), &table);
    let said = panics(|| {
        dry_run_writes_nothing(&mut in_tree("rm pipe", d.path()), d.path(), &table);
    });
    assert!(said.contains("removed: pipe"), "{said}");
}

/// rustdoc of dry_run_writes_nothing: "The changes are reported first, with how the run ended,
/// even when it also exited with a code `table` does not list or panicked.  A run that removes
/// `tree`, or puts something other than a directory or a link to one in its place, is reported as
/// removing everything in it."  And the panics: "`tree` itself as `.`".
#[test]
fn the_writes_are_named_whatever_the_exit() {
    let table = ExitTable::canonical();
    let parent = tempfile::tempdir().unwrap();
    let t = parent.path().join("t");
    std::fs::create_dir(&t).unwrap();
    std::fs::write(t.join("a.txt"), b"a").unwrap();
    let said = panics(|| {
        dry_run_writes_nothing(&mut in_tree("rm -r t", parent.path()), &t, &table);
    });
    assert!(
        said.lines().any(|l| l == "removed: .") && said.contains("removed: a.txt"),
        "{said}"
    );
    std::fs::create_dir(&t).unwrap();
    let said = panics(|| {
        dry_run_writes_nothing(
            &mut in_tree("rm -r t && echo x > t", parent.path()),
            &t,
            &table,
        );
    });
    assert!(said.lines().any(|l| l == "changed: ."), "{said}");
    let d = tree();
    let said = panics(|| {
        dry_run_writes_nothing(&mut in_tree("rm a.txt; exit 9", d.path()), d.path(), &table);
    });
    assert!(
        said.contains("removed: a.txt") && said.contains("exit status: 9"),
        "{said}"
    );
}

/// rustdoc of the module: "A message names the command by its program, arguments, and
/// directory, never its environment, which may hold a secret."
#[test]
fn a_message_never_carries_the_commands_environment() {
    let mut cmd = sh("exit 9");
    cmd.env("API_TOKEN", "hunter2-secret");
    let said = panics(|| closed_stdout(&mut cmd, &ExitTable::canonical()));
    assert!(
        said.contains("\"exit 9\"") && !said.contains("hunter2"),
        "{said}"
    );
}

/// rustdoc of dry_run_writes_nothing: "A read that fails after the run names the run and how it
/// ended."
#[test]
fn a_tree_the_run_left_unreadable_is_named_as_the_runs_doing() {
    use std::os::unix::fs::PermissionsExt;
    // SAFETY: geteuid reads a process attribute and cannot fail.
    if unsafe { libc::geteuid() } == 0 {
        eprintln!("SKIPPED: running as root, where a permission proves nothing");
        return;
    }
    let d = tree();
    let said = panics(|| {
        dry_run_writes_nothing(
            &mut in_tree("chmod 000 sub; exit 9", d.path()),
            d.path(),
            &ExitTable::canonical(),
        )
    });
    std::fs::set_permissions(d.path().join("sub"), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        said.starts_with("after ")
            && said.contains("\"chmod 000 sub; exit 9\"")
            && said.contains("exit status: 9"),
        "{said}"
    );
}

/// rustdoc of dry_run_writes_nothing: "A link at `tree` itself is followed, since the caller named
/// it", and the panics: "is not a directory (a link to nothing included)".  A link to nothing above
/// `tree` is named for what it is.
#[test]
fn the_tree_is_a_directory_and_a_link_to_one_is_followed() {
    let table = ExitTable::canonical();
    let d = tree();
    let parent = tempfile::tempdir().unwrap();
    let link = parent.path().join("link");
    std::os::unix::fs::symlink(d.path(), &link).unwrap();
    let said = panics(|| {
        dry_run_writes_nothing(&mut in_tree("touch new.txt", d.path()), &link, &table);
    });
    assert!(said.contains("added: new.txt"), "{said}");
    let said = panics(|| {
        dry_run_writes_nothing(&mut sh("true"), &d.path().join("a.txt"), &table);
    });
    assert!(said.contains("is not a directory"), "{said}");
    let dangling = parent.path().join("dangling");
    std::os::unix::fs::symlink("nowhere", &dangling).unwrap();
    let said = panics(|| {
        dry_run_writes_nothing(&mut sh("true"), &dangling, &table);
    });
    assert!(said.contains("is a link to nothing"), "{said}");
    let said = panics(|| {
        dry_run_writes_nothing(&mut sh("true"), &dangling.join("sub"), &table);
    });
    assert!(
        said.contains("broken symbolic link") && said.contains(&*dangling.to_string_lossy()),
        "{said}"
    );
}

// --- the exit table in --help -----------------------------------------------------------------

/// rustdoc of help_has_exit_table: "Run `cmd --help` and assert that it exits 0 and prints
/// `table.render()` whole".  The script prints its first argument, so it prints the table, a part
/// of it, or nothing, and `--help` comes after.
#[test]
fn help_must_print_the_whole_table_and_exit_0() {
    let table = ExitTable::canonical().code(5, "lock held by another run");
    let rendered = table.render();
    let print = |text: &str, exit: u8| {
        let mut cmd = sh(&format!("printf '%s\\n' \"$1\"; exit {exit}"));
        cmd.arg(text);
        cmd
    };
    help_has_exit_table(&mut print(&format!("Usage: tool\n\n{rendered}"), 0), &table);
    let said =
        panics(|| help_has_exit_table(&mut print(cli_contract::exit::HELP_BLOCK, 0), &table));
    assert!(said.contains("does not print the exit table"), "{said}");
    let said = panics(|| help_has_exit_table(&mut print(&rendered, 3), &table));
    assert!(said.contains("did not exit 0"), "{said}");
}

/// rustdoc of help_has_exit_table: "Clap's `wrap_help` feature wraps that text, at 100 columns when
/// stdout is not a terminal, so with it each line of the table must fit in 100 columns".  A table
/// printed re-wrapped is still refused, and the message says why.
#[test]
fn a_re_wrapped_table_is_refused_and_named() {
    let table = ExitTable::canonical();
    let rewrapped = table.render().replace("findings (", "findings\n  (");
    let mut cmd = sh("printf '%s\\n' \"$1\"");
    cmd.arg(&rewrapped);
    let said = panics(|| {
        help_has_exit_table(&mut cmd, &table);
    });
    assert!(said.contains("other line breaks or spacing"), "{said}");
}

/// rustdoc of help_has_exit_table: "Adds `--help` to the command's arguments, so a subcommand's
/// help is tested by naming the subcommand first".
#[test]
fn help_is_asked_for_after_the_arguments_given() {
    let table = ExitTable::canonical();
    // The script prints the table only when its arguments are "sub", then "--help".
    let mut cmd = sh("[ \"$1\" = sub ] && [ \"$2\" = --help ] && printf '%s' \"$TABLE\"");
    cmd.arg("sub").env("TABLE", table.render());
    help_has_exit_table(&mut cmd, &table);
}

/// rustdoc of closed_stdout, help_has_exit_table, and dry_run_writes_nothing: "Sets the command's
/// stdin (empty), stdout, and stderr (captured)"; of closed_stderr: "Sets the command's stdin
/// (empty), stdout (captured), and stderr".  A command already run by closed_stdout or
/// closed_stderr, whose stream is still the one that refuses writes, or one set to discard a
/// stream, is captured all the same.
#[test]
fn a_command_whose_streams_are_already_set_is_captured() {
    let table = ExitTable::canonical();
    let mut cmd = sh("printf o");
    closed_stdout(&mut cmd, &table);
    assert_eq!(closed_stderr(&mut cmd, &table).stdout, b"o");
    let mut cmd = sh("printf e >&2");
    closed_stderr(&mut cmd, &table);
    assert_eq!(closed_stdout(&mut cmd, &table).stderr, b"e");
    let mut cmd = sh("printf e >&2; [ \"$1\" = --help ] && printf '%s' \"$TABLE\"; exit 0");
    cmd.env("TABLE", table.render());
    closed_stdout(&mut cmd, &table);
    help_has_exit_table(&mut cmd, &table);
    closed_stderr(&mut cmd, &table);
    assert_eq!(help_has_exit_table(&mut cmd, &table).stderr, b"e");
    let d = tree();
    let mut cmd = in_tree("cat a.txt; printf e >&2", d.path());
    cmd.stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let output = dry_run_writes_nothing(&mut cmd, d.path(), &table);
    assert_eq!(output.stdout, b"a");
    assert_eq!(output.stderr, b"e");
}

/// rustdoc of help_has_exit_table: "removes `COLUMNS` from its environment, from which clap would
/// take a width of its own".
#[test]
fn help_is_asked_for_without_columns() {
    let table = ExitTable::canonical();
    let mut cmd = sh("[ -z \"${COLUMNS+set}\" ] && printf '%s' \"$TABLE\"");
    cmd.env("TABLE", table.render()).env("COLUMNS", "80");
    help_has_exit_table(&mut cmd, &table);
}

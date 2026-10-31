//! Black-box tests of the public contract, written from the README and the rustdoc alone.
//!
//! Every test here cites the sentence it pins.  None of it reads the implementation: where a
//! test fails, either the code or the sentence is wrong, and the test does not decide which.

use std::path::{Path, PathBuf};

use cli_contract::{
    check_destination, ensure_destination, exit, prepare_output_file, reject_symlink,
    require_input_dir, require_input_path, symlink_info, Clobber, CreateMode, DestPolicy,
    PathError,
};
// Used only by the floor tests, which do not run on Windows.
#[cfg(not(windows))]
use cli_contract::is_mount_root;

const MODES: [CreateMode; 3] = [CreateMode::None, CreateMode::Final, CreateMode::All];

fn absent(p: &Path) -> bool {
    matches!(std::fs::symlink_metadata(p), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
}

/// Restores the previous working directory on drop; pair with `#[serial_test::serial]`.
struct CwdGuard(PathBuf);

impl CwdGuard {
    fn set(dir: &Path) -> Self {
        let prev = std::env::current_dir().expect("cwd readable");
        std::env::set_current_dir(dir).expect("cwd settable");
        Self(prev)
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

#[cfg(unix)]
fn is_root() -> bool {
    // SAFETY: geteuid reads a process attribute and cannot fail.
    let root = unsafe { libc::geteuid() } == 0;
    if root {
        eprintln!("SKIPPED: running as root, where chmod 000 proves nothing");
    }
    root
}

/// A `chmod 000` directory, restored on drop so the temp dir can be removed.
#[cfg(unix)]
struct Locked {
    _tmp: tempfile::TempDir,
    dir: PathBuf,
}

#[cfg(unix)]
impl Locked {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("locked");
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("real.txt"), b"x").unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o000)).unwrap();
        Self { _tmp: tmp, dir }
    }

    fn unlock(&self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o755));
    }
}

#[cfg(unix)]
impl Drop for Locked {
    fn drop(&mut self) {
        self.unlock();
    }
}

// --- the mount-root floor, reached through the output-FILE entry point ----------------------
//
// README: "No mode, not even `All`, will create a mount point."  rustdoc of prepare_output_file:
// "then ensure its parent directory per `dir_mode` and the mount-root floor."

#[cfg(not(windows))]
#[test]
fn output_file_parent_is_floored_and_nothing_is_created() {
    let vol = PathBuf::from(format!("/Volumes/cli_contract_bb_{}", std::process::id()));
    assert!(absent(&vol), "{} must start absent", vol.display());
    let shallow = vol.join("out.pdf");
    let deep = vol.join("sub").join("out.pdf");
    let mut got = Vec::new();
    for clobber in [Clobber::Allow, Clobber::Deny] {
        got.push((
            "Final shallow",
            prepare_output_file(&shallow, CreateMode::Final, clobber),
        ));
        got.push((
            "All shallow",
            prepare_output_file(&shallow, CreateMode::All, clobber),
        ));
        got.push((
            "All deep",
            prepare_output_file(&deep, CreateMode::All, clobber),
        ));
    }
    let none = prepare_output_file(&shallow, CreateMode::None, Clobber::Allow);
    // The side effect first, before judging any verdict.
    assert!(absent(&vol), "a mount point was created: {}", vol.display());
    for (what, r) in got {
        assert!(
            matches!(&r, Err(PathError::MountRoot(p)) if p == &vol),
            "{what}: expected MountRoot({}), got {r:?}",
            vol.display()
        );
    }
    assert!(none.is_err(), "None approved a missing parent: {none:?}");

    // The Linux roots, matched by spelling (none of these exists on macOS, and `/` is read-only).
    for p in [
        "/mnt/cli_contract_bb/out.pdf",
        "/media/alice/cli_contract_bb/out.pdf",
        "/run/media/alice/cli_contract_bb/out.pdf",
    ] {
        let r = prepare_output_file(Path::new(p), CreateMode::All, Clobber::Allow);
        assert!(matches!(r, Err(PathError::MountRoot(_))), "{p}: {r:?}");
    }
}

// --- the floor compares path MEANING: `.`, `..`, doubled and trailing separators -------------
//
// README: "the floor compares what the path means from the current directory, not its text."  A
// `..` that the OS resolves into a mount root is a mount root.

#[cfg(not(windows))]
#[test]
fn the_floor_sees_a_mount_root_reached_through_dotdot() {
    for p in [
        "/usr/../mnt/usb",
        "/usr/../media/alice/USBDRIVE",
        "/usr/../Volumes/cli_contract_bb",
        "/Volumes/./cli_contract_bb",
        "/Volumes/cli_contract_bb/",
        "//Volumes/cli_contract_bb",
    ] {
        assert!(is_mount_root(Path::new(p)), "should be a mount root: {p}");
    }
    for (p, mode) in [
        ("/usr/../mnt/cli_contract_bb", CreateMode::All),
        ("/usr/../Volumes/cli_contract_bb", CreateMode::Final),
        ("/usr/../Volumes/cli_contract_bb", CreateMode::All),
        ("/Volumes/cli_contract_bb/", CreateMode::Final),
    ] {
        let r = check_destination(Path::new(p), &DestPolicy::Create(mode));
        assert!(
            matches!(r, Err(PathError::MountRoot(_))),
            "{p} {mode}: {r:?}"
        );
    }
}

/// README: "only the identity check for `/Volumes` looks through a link", and rustdoc: "Each probe
/// follows links, as a create does".  A create of `L/../Volumes/X`, with `L` a link to `/usr`,
/// makes `/Volumes/X`: the kernel resolves `L` before `..`.
#[cfg(target_os = "macos")]
#[test]
fn the_floor_sees_volumes_through_a_link_then_dotdot() {
    let d = tempfile::tempdir().unwrap();
    let to_usr = d.path().join("to_usr");
    std::os::unix::fs::symlink("/usr", &to_usr).unwrap();
    let dest = to_usr.join("..").join("Volumes").join("cli_contract_bb");
    let mut wrong = Vec::new();
    if !is_mount_root(&dest) {
        wrong.push(format!("is_mount_root({}) = false", dest.display()));
    }
    for mode in [CreateMode::Final, CreateMode::All] {
        let r = check_destination(&dest, &DestPolicy::Create(mode));
        if !matches!(r, Err(PathError::MountRoot(_))) {
            wrong.push(format!("check {mode}: {r:?}"));
        }
    }
    // The same through a link to a real directory directly under /Volumes.
    if let Some(child) = real_volume_dirs().into_iter().next() {
        let to_vol = d.path().join("to_vol");
        std::os::unix::fs::symlink(&child, &to_vol).unwrap();
        let dest = to_vol.join("..").join("cli_contract_bb");
        if !is_mount_root(&dest) {
            wrong.push(format!("is_mount_root({}) = false", dest.display()));
        }
        let r = check_destination(&dest, &DestPolicy::Create(CreateMode::Final));
        if !matches!(r, Err(PathError::MountRoot(_))) {
            wrong.push(format!("check Final via {}: {r:?}", child.display()));
        }
    }
    assert!(absent(Path::new("/Volumes/cli_contract_bb")));
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// README: "a bare `Backup` with a cwd of `/Volumes` is refused" — the same resolution from `/`
/// and from a sibling of the mount root.
#[cfg(not(windows))]
#[test]
#[serial_test::serial]
fn relative_paths_resolving_into_mount_roots_are_floored() {
    let _cwd = CwdGuard::set(Path::new("/"));
    for p in [
        "Volumes/cli_contract_bb",
        "./Volumes/cli_contract_bb",
        "mnt/usb",
        "media/alice/USBDRIVE",
    ] {
        assert!(is_mount_root(Path::new(p)), "cwd /: {p}");
    }
    let r = check_destination(
        Path::new("Volumes/cli_contract_bb"),
        &DestPolicy::Create(CreateMode::Final),
    );
    assert!(matches!(r, Err(PathError::MountRoot(_))), "{r:?}");
    drop(_cwd);
    let _cwd = CwdGuard::set(Path::new("/usr"));
    for p in ["../Volumes/cli_contract_bb", "../mnt/usb"] {
        assert!(is_mount_root(Path::new(p)), "cwd /usr: {p}");
    }
}

/// The real directories directly under /Volumes (mounted volumes), not links.
#[cfg(target_os = "macos")]
fn real_volume_dirs() -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir("/Volumes") else {
        return Vec::new();
    };
    rd.filter_map(Result::ok)
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter(|e| std::fs::metadata(e.path()).is_ok())
        .map(|e| e.path())
        .collect()
}

/// README: the floor bars *creating* a mount point, and "paths _below_ an existing mount point are
/// created normally".  A mounted volume must stay a usable destination.  Checks only: nothing is
/// written to any volume.
#[cfg(target_os = "macos")]
#[test]
fn an_existing_mount_point_is_a_usable_destination() {
    for mode in MODES {
        let r = check_destination(Path::new("/Volumes"), &DestPolicy::Create(mode));
        assert!(r.is_ok(), "/Volumes {mode}: {r:?}");
    }
    let vols = real_volume_dirs();
    if vols.is_empty() {
        eprintln!("SKIPPED: no mounted volume under /Volumes");
    }
    for v in vols {
        for mode in MODES {
            let r = check_destination(&v, &DestPolicy::Create(mode));
            assert!(r.is_ok(), "{} {mode}: {r:?}", v.display());
        }
        let below = v.join("cli_contract_bb_nosuch").join("x");
        let r = check_destination(&below, &DestPolicy::Create(CreateMode::All));
        assert!(r.is_ok(), "{}: {r:?}", below.display());
        assert!(absent(&v.join("cli_contract_bb_nosuch")));
    }
}

// --- output files ---------------------------------------------------------------------------

/// A bare relative output name has the current directory as its parent, which exists.
#[test]
#[serial_test::serial]
fn output_file_accepts_a_bare_relative_name() {
    let d = tempfile::tempdir().unwrap();
    let _cwd = CwdGuard::set(d.path());
    for mode in MODES {
        for clobber in [Clobber::Allow, Clobber::Deny] {
            let r = prepare_output_file(Path::new("out.pdf"), mode, clobber);
            assert!(r.is_ok(), "{mode} {clobber}: {r:?}");
        }
    }
    assert!(absent(&d.path().join("out.pdf")), "the file was created");
    std::fs::write(d.path().join("out.pdf"), b"old").unwrap();
    let r = prepare_output_file(Path::new("out.pdf"), CreateMode::None, Clobber::Deny);
    assert!(matches!(r, Err(PathError::Clobber(_))), "{r:?}");
}

/// README: `None` — "create nothing; the directory must already exist, else exit 1".
/// rustdoc: "Does not create the file".  The error names the parent, the path that is missing.
#[test]
fn output_file_under_none_names_the_missing_parent_and_creates_nothing() {
    let d = tempfile::tempdir().unwrap();
    let parent = d.path().join("missing");
    let file = parent.join("out.pdf");
    for clobber in [Clobber::Allow, Clobber::Deny] {
        let r = prepare_output_file(&file, CreateMode::None, clobber);
        assert!(
            matches!(&r, Err(PathError::MissingDestination(p)) if p == &parent),
            "{clobber}: {r:?}"
        );
        assert!(absent(&parent), "{clobber}: the parent was created");
    }
    // Under Deny an absent leaf is no clobber; and the file itself is never created.
    std::fs::create_dir(&parent).unwrap();
    prepare_output_file(&file, CreateMode::None, Clobber::Deny).unwrap();
    assert!(absent(&file), "prepare_output_file created the file");
}

/// README: "a directory sitting where an output file was required (`NotAFile`)", whatever the
/// clobber policy.
#[test]
fn output_file_reports_a_directory_as_not_a_file_under_deny_too() {
    let d = tempfile::tempdir().unwrap();
    let target = d.path().join("out.pdf");
    std::fs::create_dir(&target).unwrap();
    for clobber in [Clobber::Allow, Clobber::Deny] {
        let r = prepare_output_file(&target, CreateMode::None, clobber);
        assert!(
            matches!(&r, Err(PathError::NotAFile(p)) if p == &target),
            "{clobber}: {r:?}"
        );
    }
}

/// README: "A path that could not be inspected ... comes back as `Undeterminable` ... a guard
/// refuses it rather than approving it."  Under `Allow` the leaf is still guarded (a directory
/// there is `NotAFile`, a dangling link `DanglingSymlink`), so a leaf that could not be inspected
/// must not be approved either.
#[cfg(unix)]
#[test]
fn output_file_allow_does_not_approve_a_leaf_it_could_not_inspect() {
    if is_root() {
        return;
    }
    let f = Locked::new();
    let mut wrong = Vec::new();
    for leaf in ["real.txt", "nope.pdf"] {
        for mode in MODES {
            let r = prepare_output_file(&f.dir.join(leaf), mode, Clobber::Allow);
            if !matches!(r, Err(PathError::Undeterminable { .. })) {
                wrong.push(format!("{leaf} {mode} Allow: {r:?}"));
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

// --- positive evidence of absence: an entry that may or may not be there ---------------------
//
// rustdoc of MissingInput: "Claimed only on a proven `NotFound`".  An entry under a `chmod 000`
// directory is neither provably present nor provably absent, whether or not it exists.

#[cfg(unix)]
#[test]
fn an_unknowable_absent_entry_is_undeterminable_and_never_created() {
    if is_root() {
        return;
    }
    let f = Locked::new();
    let nope = f.dir.join("nope");
    let deeper = nope.join("sub");
    let mut wrong = Vec::new();
    type Half = fn(&Path, &DestPolicy) -> Result<(), PathError>;
    let halves: [(&str, Half); 2] = [("check", check_destination), ("ensure", ensure_destination)];
    let mut verdicts = vec![
        ("require_input_path".to_string(), require_input_path(&nope)),
        (
            "require_input_dir".to_string(),
            require_input_dir(&nope, None),
        ),
        ("reject_symlink".to_string(), reject_symlink(&nope)),
        ("symlink_info".to_string(), symlink_info(&nope).map(drop)),
    ];
    for mode in MODES {
        for (half, run) in halves {
            for (tag, p) in [("nope", &nope), ("nope/sub", &deeper)] {
                verdicts.push((
                    format!("{half} {mode} {tag}"),
                    run(p, &DestPolicy::Create(mode)),
                ));
            }
        }
        verdicts.push((
            format!("prepare_output_file {mode}"),
            prepare_output_file(&nope.join("out.pdf"), mode, Clobber::Deny),
        ));
    }
    for (what, r) in verdicts {
        match &r {
            Err(PathError::Undeterminable { .. }) => {}
            other => wrong.push(format!("{what}: {other:?}")),
        }
    }
    f.unlock();
    assert!(
        absent(&nope),
        "something was created under the locked directory"
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
}

// --- `.`, `..`, `/`, and trailing separators -------------------------------------------------

/// README: "A trailing separator is dropped before any check" and "a file spelled `file/` is
/// checked as `file`".  `.`, `..`, and `/` name directories that exist.
#[test]
#[serial_test::serial]
fn dot_dotdot_and_root_name_the_directories_they_mean() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir(d.path().join("sub")).unwrap();
    let _cwd = CwdGuard::set(&d.path().join("sub"));
    let mut wrong = Vec::new();
    let mut names = vec![".", "./", "..", "../", "/"];
    // On Unix `//` is the root.  On Windows it begins a UNC path with no server, an invalid name,
    // which every entry point rightly reports as `Undeterminable`.
    if cfg!(unix) {
        names.push("//");
    }
    for p in names {
        let p = Path::new(p);
        for (what, r) in [
            ("require_input_path", require_input_path(p)),
            ("require_input_dir", require_input_dir(p, None)),
            ("reject_symlink", reject_symlink(p)),
            (
                "check None",
                check_destination(p, &DestPolicy::Create(CreateMode::None)),
            ),
            (
                "check All",
                check_destination(p, &DestPolicy::Create(CreateMode::All)),
            ),
        ] {
            if r.is_err() {
                wrong.push(format!("{what}({}): {r:?}", p.display()));
            }
        }
        match symlink_info(p) {
            Ok(None) => {}
            other => wrong.push(format!("symlink_info({}): {other:?}", p.display())),
        }
        let r = prepare_output_file(p, CreateMode::None, Clobber::Allow);
        if !matches!(r, Err(PathError::NotAFile(_))) {
            wrong.push(format!("prepare_output_file({}): {r:?}", p.display()));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[cfg(unix)]
#[test]
fn a_plain_file_spelled_with_a_separator_is_checked_as_the_file() {
    let d = tempfile::tempdir().unwrap();
    let file = d.path().join("file");
    std::fs::write(&file, b"x").unwrap();
    for suffix in ["/", "/."] {
        let mut s = file.clone().into_os_string();
        s.push(suffix);
        let spelled = PathBuf::from(s);
        assert!(require_input_path(&spelled).is_ok(), "{suffix}");
        let r = require_input_dir(&spelled, None);
        assert!(
            matches!(r, Err(PathError::NotADirectory(_))),
            "{suffix}: {r:?}"
        );
        let r = check_destination(&spelled, &DestPolicy::Create(CreateMode::None));
        assert!(
            matches!(r, Err(PathError::NotADirectory(_))),
            "{suffix}: {r:?}"
        );
        let r = prepare_output_file(&spelled, CreateMode::None, Clobber::Deny);
        assert!(matches!(r, Err(PathError::Clobber(_))), "{suffix}: {r:?}");
    }
    assert_eq!(std::fs::read(&file).unwrap(), b"x");
}

// --- symlinks ------------------------------------------------------------------------------

/// README: "a **dangling** link is reported as `DanglingSymlink`, not as a missing path", at
/// every policy, with no create through it.
#[cfg(unix)]
#[test]
fn a_bare_dangling_link_is_dangling_everywhere_and_nothing_is_created() {
    let d = tempfile::tempdir().unwrap();
    let ghost = d.path().join("ghost");
    let dang = d.path().join("dang");
    std::os::unix::fs::symlink(&ghost, &dang).unwrap();
    let mut wrong = Vec::new();
    let mut verdicts = vec![
        ("require_input_path".to_string(), require_input_path(&dang)),
        (
            "require_input_dir".to_string(),
            require_input_dir(&dang, None),
        ),
    ];
    for policy in [
        DestPolicy::Create(CreateMode::None),
        DestPolicy::Create(CreateMode::Final),
        DestPolicy::Create(CreateMode::All),
        DestPolicy::Marked("m".into()),
    ] {
        verdicts.push((
            format!("check {policy:?}"),
            check_destination(&dang, &policy),
        ));
        verdicts.push((
            format!("ensure {policy:?}"),
            ensure_destination(&dang, &policy),
        ));
    }
    for (what, r) in verdicts {
        if !matches!(&r, Err(PathError::DanglingSymlink(p)) if p == &dang) {
            wrong.push(format!("{what}: {r:?}"));
        }
    }
    assert!(absent(&ghost), "the dangling link's target was created");
    // symlink_info: the facts of a dangling link.
    let info = symlink_info(&dang).unwrap().expect("a link");
    assert_eq!(info.link, dang);
    assert_eq!(info.target.as_deref(), Some(ghost.as_path()));
    assert!(
        info.dangling && !info.target_is_dir && info.resolved.is_none(),
        "{info:?}"
    );
    // reject_symlink: "refuses a destination that is itself a link (`IsSymlink`)".
    let r = reject_symlink(&dang);
    assert!(matches!(r, Err(PathError::IsSymlink(_))), "{r:?}");
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// README: "`link/sub` passes when `link` leads to a directory".  rustdoc of `SymlinkInfo`:
/// `resolved` is "The fully canonicalized target"; `target` is "The immediate (unresolved)
/// target".  And the composition: "`is_mount_root(resolved)` ... catches a link that resolves
/// onto a mount root".
#[cfg(unix)]
#[test]
fn symlink_facts_and_the_reject_policy_below_a_link() {
    let d = tempfile::tempdir().unwrap();
    let real = d.path().join("real");
    std::fs::create_dir(&real).unwrap();
    let ldir = d.path().join("ldir");
    std::os::unix::fs::symlink(&real, &ldir).unwrap();
    let chain = d.path().join("chain");
    std::os::unix::fs::symlink(&ldir, &chain).unwrap();

    reject_symlink(&ldir.join("sub")).expect("an absent path below a linked dir passes");
    std::fs::create_dir(real.join("sub")).unwrap();
    reject_symlink(&ldir.join("sub")).expect("a real dir below a linked dir passes");

    let info = symlink_info(&chain).unwrap().expect("a link");
    assert_eq!(
        info.target.as_deref(),
        Some(ldir.as_path()),
        "immediate target"
    );
    assert_eq!(
        info.resolved,
        Some(real.canonicalize().unwrap()),
        "canonical target"
    );
    assert!(info.target_is_dir && !info.dangling);

    #[cfg(target_os = "macos")]
    {
        let to_volumes = d.path().join("to_volumes");
        std::os::unix::fs::symlink("/Volumes", &to_volumes).unwrap();
        let info = symlink_info(&to_volumes).unwrap().expect("a link");
        assert!(is_mount_root(info.resolved.as_deref().unwrap()), "{info:?}");
    }
}

// --- errors --------------------------------------------------------------------------------

/// README: "distinct variants, each naming the offending path"; rustdoc: "Every variant maps to
/// [`crate::exit::TOOL_ERROR`] (exit 1)".
#[test]
fn every_path_error_names_its_path_and_exits_1() {
    let p = PathBuf::from("/some/where/xyz");
    let io = || std::io::Error::from(std::io::ErrorKind::PermissionDenied);
    let errs = [
        PathError::MissingInput(p.clone()),
        PathError::MissingDestination(p.clone()),
        PathError::NotADirectory(p.clone()),
        PathError::NotAFile(p.clone()),
        PathError::IsSymlink(p.clone()),
        PathError::DanglingSymlink(p.clone()),
        PathError::MissingParent {
            target: p.join("t"),
            parent: p.clone(),
        },
        PathError::MountRoot(p.clone()),
        PathError::MarkerMissing {
            dir: p.clone(),
            marker: "~Bak~".into(),
        },
        PathError::Clobber(p.clone()),
        PathError::Undeterminable {
            path: p.clone(),
            source: io(),
        },
        PathError::Io {
            path: p.clone(),
            source: io(),
        },
    ];
    for e in &errs {
        assert_eq!(e.exit_code(), exit::TOOL_ERROR, "{e:?}");
        assert!(e.to_string().contains("/some/where/xyz"), "{e:?}: {e}");
    }
}

/// README: "`exit::HELP_BLOCK` is ready to hand to clap’s `after_long_help` so every tool
/// documents the table identically" — the README's own table is that table.
#[test]
fn the_readme_table_is_the_help_block() {
    let readme = include_str!("../README.md");
    for line in exit::HELP_BLOCK.lines().skip(1) {
        assert!(readme.contains(line.trim()), "README lacks: {line}");
    }
}

// --- BrokenPipe, in a child process ----------------------------------------------------------
//
// README: with `ignore_broken_pipe`, "a closed pipe just stops output" and "the work runs to
// completion".  (`reset_sigpipe`'s half is pinned in `src/pipe.rs`, on a pipe of its own.)

#[cfg(unix)]
const PIPE_CHILD: &str = "CLI_CONTRACT_BB_PIPE_CHILD";

/// Not a test of its own: the body the pipe test below re-runs in a child process.
#[cfg(unix)]
#[test]
fn pipe_child() {
    use std::io::Write;
    let Ok(done_file) = std::env::var(PIPE_CHILD) else {
        return;
    };
    let mut out = std::io::stdout().lock();
    let mut done = 0u32;
    for n in 0..100_000 {
        done += 1;
        cli_contract::ignore_broken_pipe(writeln!(out, "did {n}")).expect("a real I/O error");
    }
    std::fs::write(done_file, done.to_string()).unwrap();
    // Before the harness prints its summary into the closed pipe.
    std::process::exit(0);
}

#[cfg(unix)]
#[test]
fn ignore_broken_pipe_lets_the_work_finish_after_the_reader_leaves() {
    use std::io::Read;
    let d = tempfile::tempdir().unwrap();
    let done = d.path().join("done");
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "pipe_child", "--test-threads=1", "--quiet"])
        .env(PIPE_CHILD, &done)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut buf = [0u8; 64];
    let _ = stdout.read(&mut buf).unwrap();
    drop(stdout); // the reader leaves, as `head` does
    let status = child.wait().unwrap();
    assert!(status.success(), "{status:?}");
    assert_eq!(std::fs::read_to_string(&done).unwrap(), "100000");
}

/// Module docs: "A path the invocation asserted ... that does not match the filesystem is a
/// runtime failure the tool refuses to guess past"; README: "a dry run never disagrees with the
/// live run about the _contract_".  The empty path names nothing, so nothing approves it.
#[test]
fn an_empty_path_is_never_approved() {
    let p = Path::new("");
    let mut wrong = Vec::new();
    for (what, r) in [
        ("require_input_path", require_input_path(p)),
        ("require_input_dir", require_input_dir(p, None)),
    ] {
        if r.is_ok() {
            wrong.push(format!("{what}: Ok"));
        }
    }
    for mode in MODES {
        for clobber in [Clobber::Allow, Clobber::Deny] {
            let r = prepare_output_file(p, mode, clobber);
            if r.is_ok() {
                wrong.push(format!("prepare_output_file {mode} {clobber}: Ok"));
            }
        }
        let c = check_destination(p, &DestPolicy::Create(mode));
        let e = ensure_destination(p, &DestPolicy::Create(mode));
        if c.is_ok() || format!("{c:?}") != format!("{e:?}") {
            wrong.push(format!("{mode}: check {c:?}; ensure {e:?}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

// --- the runnable examples ------------------------------------------------------------------
//
// README: "Runnable versions of the snippets above are in `examples/`", and of a marker that is
// not a name: "validate at the flag boundary to get the usage code".  The house exit-code table:
// a value wrong on its face, decidable from the argument text alone, is exit 2, whatever the disk
// holds.  The `inputs` example takes its marker from argv.

/// Builds an example from the current source and returns its path.  It builds in a target
/// directory of its own, because a run that names its targets (`cargo test --test blackbox`) does
/// not rebuild `target/<profile>/examples/<name>`, so a test that ran that file could run a stale
/// build.
fn build_example(name: &str) -> PathBuf {
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join("examples");
    let out = std::process::Command::new(env!("CARGO"))
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
    target
        .join("debug")
        .join("examples")
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

#[test]
fn the_inputs_example_exits_2_for_a_marker_on_argv_that_is_not_a_name() {
    let bin = build_example("inputs");
    let d = tempfile::tempdir().unwrap();
    let missing = d.path().join("nosuch");
    let mut wrong = Vec::new();
    for (dir, marker) in [
        (d.path(), ""),
        (d.path(), ".."),
        (d.path(), "/"),
        (missing.as_path(), ""),
        (missing.as_path(), ".."),
    ] {
        let out = std::process::Command::new(&bin)
            .arg(dir)
            .arg(marker)
            .output()
            .unwrap();
        if out.status.code() != Some(exit::USAGE) {
            wrong.push(format!(
                "inputs {} {marker:?}: exit {:?}: {}",
                dir.display(),
                out.status.code(),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "a marker on argv that is not a name is a usage error (exit 2), but the inputs example \
         passes it unvalidated to require_input_dir: {wrong:#?}"
    );
}

//! The README's strongest claim, tested over every case rather than by example: `check_destination`
//! and `ensure_destination` never disagree about the contract, only an environmental failure at
//! creation time can separate them, and the check writes nothing.  The same holds for an output
//! file, between `check_output_file` and `prepare_output_file`.
//!
//! The space is small enough to enumerate, so it is enumerated, not sampled: every shape of a one-
//! or two-level chain below a scratch directory, every policy, and five spellings of the same
//! destination.  For each case the test asserts:
//!
//! 1. `check_destination` changes nothing on disk;
//! 2. when the check refuses, `ensure_destination` refuses with the same error, and writes nothing;
//! 3. when the check approves, `ensure_destination` succeeds and leaves a directory there, unless
//!    the fixture staged an environmental failure (a read-only directory), which may only surface
//!    as `PathError::Io` with `PermissionDenied`;
//! 4. whatever `ensure_destination` creates is a directory, never a file or a link.
//!
//! The output file is enumerated the same way, as the deepest level of a chain of up to three, so
//! its parent's chain can be two levels deep and `Final` and `All` differ: every shape, every mode,
//! both clobber policies, and the same five spellings.  The four assertions are the same, with
//! `check_output_file` and `prepare_output_file` in place of the destination pair, and in 3 the
//! file's parent is the directory left there.

use cli_contract::{
    check_destination, check_output_file, ensure_destination, prepare_output_file, Clobber,
    CreateMode, DestPolicy, PathError,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// What stands at one level of the chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Absent,
    File,
    Dir,
    /// A directory with mode 0o555: it can be read, but nothing can be created in it.  The one
    /// environmental failure the fixture stages.
    #[cfg(unix)]
    ReadOnly,
    /// A directory with mode 0o000: nothing below it can be inspected.
    #[cfg(unix)]
    Locked,
    #[cfg(unix)]
    LinkToDir,
    #[cfg(unix)]
    LinkToFile,
    #[cfg(unix)]
    Dangling,
    /// A link to itself: following it fails with ELOOP.
    #[cfg(unix)]
    Loop,
    /// A link to `/Volumes`, so the level below it is a mount root by identity.
    #[cfg(target_os = "macos")]
    LinkToVolumes,
}

impl Shape {
    fn all(privileged: bool) -> Vec<Shape> {
        #[cfg_attr(not(unix), allow(unused_mut))] // only the Unix shapes extend it
        let mut v = vec![Shape::Absent, Shape::File, Shape::Dir];
        #[cfg(unix)]
        {
            // chmod does not stop a privileged user, so these two would test nothing there.
            if !privileged {
                v.extend([Shape::ReadOnly, Shape::Locked]);
            }
            v.extend([
                Shape::LinkToDir,
                Shape::LinkToFile,
                Shape::Dangling,
                Shape::Loop,
            ]);
        }
        #[cfg(target_os = "macos")]
        v.push(Shape::LinkToVolumes);
        let _ = privileged;
        v
    }

    /// Whether a level can be built below this one in the fixture.
    fn holds_a_level(self) -> bool {
        match self {
            Shape::Dir => true,
            #[cfg(unix)]
            Shape::ReadOnly | Shape::Locked | Shape::LinkToDir => true,
            _ => false,
        }
    }

    /// Whether a marker can be placed inside this level, when it is the destination.
    fn holds_a_marker(self) -> bool {
        self.holds_a_level()
    }
}

/// The same destination, spelled five ways.
#[derive(Debug, Clone, Copy)]
enum Spelling {
    Absolute,
    AbsoluteTrailingSeparator,
    Relative,
    RelativeTrailingDot,
    RelativeThroughDotDot,
}

const SPELLINGS: [Spelling; 5] = [
    Spelling::Absolute,
    Spelling::AbsoluteTrailingSeparator,
    Spelling::Relative,
    Spelling::RelativeTrailingDot,
    Spelling::RelativeThroughDotDot,
];

const LEVELS: [&str; 3] = ["a", "b", "c"];
const MARKER: &str = "m";
const MODES: [CreateMode; 3] = [CreateMode::None, CreateMode::Final, CreateMode::All];
const CLOBBERS: [Clobber; 2] = [Clobber::Allow, Clobber::Deny];

/// Every chain of up to `max` levels, with the depth of the path under test: each shape at the
/// first level, and every shape again below a level that can hold one.  A deeper path under a
/// level that cannot hold one is a case of its own (a file or a link in the way), so that chain
/// stops there while the depth goes on.
fn chains(shapes: &[Shape], max: usize) -> Vec<(Vec<Shape>, usize)> {
    let mut out = Vec::new();
    let mut above: Vec<Vec<Shape>> = vec![Vec::new()];
    for depth in 1..=max {
        let mut here = Vec::new();
        for chain in above {
            match chain.last() {
                Some(last) if !last.holds_a_level() => here.push(chain),
                _ => {
                    for &shape in shapes {
                        let mut longer = chain.clone();
                        longer.push(shape);
                        here.push(longer);
                    }
                }
            }
        }
        out.extend(here.iter().map(|chain| (chain.clone(), depth)));
        above = here;
    }
    out
}

fn policies() -> Vec<DestPolicy> {
    vec![
        DestPolicy::Create(CreateMode::None),
        DestPolicy::Create(CreateMode::Final),
        DestPolicy::Create(CreateMode::All),
        DestPolicy::Marked(MARKER.parse().unwrap()),
    ]
}

/// A scratch workspace: `ws/` holds the chain, plus `tdir/` and `tfile` as link targets.  The
/// chmods are undone on drop, deepest first, so the temp dir can be deleted even after a failed
/// assertion.
struct Fixture {
    _tmp: tempfile::TempDir,
    ws: PathBuf,
    #[cfg(unix)]
    chmodded: Vec<PathBuf>,
}

impl Fixture {
    fn build(chain: &[Shape], marker: bool) -> Fixture {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().join("ws");
        fs::create_dir(&ws).unwrap();
        fs::create_dir(ws.join("tdir")).unwrap();
        fs::write(ws.join("tfile"), b"t").unwrap();
        #[cfg_attr(not(unix), allow(unused_mut))] // only the Unix shapes change it
        let mut fx = Fixture {
            _tmp: tmp,
            ws,
            #[cfg(unix)]
            chmodded: Vec::new(),
        };
        #[cfg(unix)]
        let mut to_chmod: Vec<(PathBuf, u32)> = Vec::new();
        let mut level = fx.ws.clone();
        for (i, &shape) in chain.iter().enumerate() {
            level = level.join(LEVELS[i]);
            match shape {
                Shape::Absent => break,
                Shape::File => fs::write(&level, b"f").unwrap(),
                Shape::Dir => fs::create_dir(&level).unwrap(),
                #[cfg(unix)]
                Shape::ReadOnly => {
                    fs::create_dir(&level).unwrap();
                    to_chmod.push((level.clone(), 0o555));
                }
                #[cfg(unix)]
                Shape::Locked => {
                    fs::create_dir(&level).unwrap();
                    to_chmod.push((level.clone(), 0o000));
                }
                #[cfg(unix)]
                Shape::LinkToDir => std::os::unix::fs::symlink(fx.ws.join("tdir"), &level).unwrap(),
                #[cfg(unix)]
                Shape::LinkToFile => {
                    std::os::unix::fs::symlink(fx.ws.join("tfile"), &level).unwrap()
                }
                #[cfg(unix)]
                Shape::Dangling => {
                    std::os::unix::fs::symlink(fx.ws.join("nowhere"), &level).unwrap()
                }
                #[cfg(unix)]
                Shape::Loop => std::os::unix::fs::symlink(&level, &level).unwrap(),
                #[cfg(target_os = "macos")]
                Shape::LinkToVolumes => std::os::unix::fs::symlink("/Volumes", &level).unwrap(),
            }
            if i + 1 == chain.len() && marker {
                fs::write(level.join(MARKER), b"m").unwrap();
            }
        }
        // Deepest first, so a locked directory never hides a level still to be chmodded.
        #[cfg(unix)]
        for (path, mode) in to_chmod.into_iter().rev() {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
            fx.chmodded.push(path);
        }
        fx
    }

    /// The destination, spelled as asked, and the directory a relative spelling is relative to.
    fn destination(&self, depth: usize, spelling: Spelling) -> PathBuf {
        let rel: PathBuf = LEVELS[..depth].iter().collect();
        match spelling {
            Spelling::Absolute => self.ws.join(&rel),
            Spelling::AbsoluteTrailingSeparator => PathBuf::from(format!(
                "{}{}",
                self.ws.join(&rel).display(),
                std::path::MAIN_SEPARATOR
            )),
            Spelling::Relative => rel,
            Spelling::RelativeTrailingDot => rel.join("."),
            Spelling::RelativeThroughDotDot => {
                // `a/../a` or `a/../a/b`: lexically the same path, but the OS must walk `a` first.
                let mut p = PathBuf::from(LEVELS[0]).join("..");
                p.push(&rel);
                p
            }
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        #[cfg(unix)]
        for path in &self.chmodded {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o755));
        }
    }
}

/// Every entry under `root`, by relative path, with its kind (and, on Unix, its mode).  Links are
/// recorded, never followed; a directory that cannot be read is recorded as such.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, String> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, String>) {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) => {
                out.insert(
                    dir.strip_prefix(root).unwrap().join("<unreadable>"),
                    format!("{:?}", e.kind()),
                );
                return;
            }
        };
        for entry in entries {
            let path = entry.unwrap().path();
            let md = fs::symlink_metadata(&path).unwrap();
            let kind = if md.file_type().is_symlink() {
                format!("link -> {}", fs::read_link(&path).unwrap().display())
            } else if md.is_dir() {
                "dir".to_string()
            } else {
                "file".to_string()
            };
            #[cfg(unix)]
            let kind = {
                use std::os::unix::fs::PermissionsExt;
                format!("{kind} {:o}", md.permissions().mode() & 0o777)
            };
            out.insert(path.strip_prefix(root).unwrap().to_path_buf(), kind);
            if md.is_dir() {
                walk(root, &path, out);
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// An error, compared by what it says: its variant and its message, which names the path.
fn verdict(r: &Result<(), PathError>) -> Option<String> {
    r.as_ref().err().map(|e| format!("{e:?} / {e}"))
}

/// Whether `chmod 000` fails to stop this process (root, or a capability), which would make the
/// permission fixtures vacuous.
#[cfg(unix)]
fn privileged() -> bool {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let probe = tmp.path().join("probe");
    fs::create_dir(&probe).unwrap();
    fs::set_permissions(&probe, fs::Permissions::from_mode(0o000)).unwrap();
    let readable = fs::read_dir(&probe).is_ok();
    fs::set_permissions(&probe, fs::Permissions::from_mode(0o755)).unwrap();
    readable
}

#[cfg(not(unix))]
fn privileged() -> bool {
    false
}

/// Restores the working directory on drop, so a failed assertion cannot strand the process in a
/// deleted temp dir.
struct CwdGuard(PathBuf);

impl CwdGuard {
    fn set(dir: &Path) -> CwdGuard {
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir).unwrap();
        CwdGuard(prev)
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

#[test]
#[serial_test::serial] // set_current_dir is process-global
fn check_and_ensure_agree_on_every_shape_policy_and_spelling() {
    let privileged = privileged();
    if privileged {
        eprintln!("SKIPPED the read-only and locked shapes: chmod does not stop this process");
    }
    let shapes = Shape::all(privileged);

    // Every chain: one level, or two where the first can hold a second.
    let mut cases = 0;
    for (chain, depth) in &chains(&shapes, 2) {
        let leaf = if chain.len() == *depth {
            chain[depth - 1]
        } else {
            Shape::Absent
        };
        let markers: &[bool] = if leaf.holds_a_marker() {
            &[false, true]
        } else {
            &[false]
        };
        for &marker in markers {
            for policy in policies() {
                for spelling in SPELLINGS {
                    // `a/..` through a link to `/Volumes` is `/`, so `a/../a` would have the
                    // create walk out of the fixture to `/a`.
                    #[cfg(target_os = "macos")]
                    if chain.contains(&Shape::LinkToVolumes)
                        && matches!(spelling, Spelling::RelativeThroughDotDot)
                    {
                        continue;
                    }
                    let fx = Fixture::build(chain, marker);
                    let _cwd = CwdGuard::set(&fx.ws);
                    let dest = fx.destination(*depth, spelling);
                    let case = format!(
                        "chain {chain:?}, depth {depth}, marker {marker}, {policy:?}, {spelling:?} ({})",
                        dest.display()
                    );

                    let before = snapshot(&fx.ws);
                    let checked = check_destination(&dest, &policy);
                    assert_eq!(snapshot(&fx.ws), before, "check wrote to disk: {case}");

                    let ensured = ensure_destination(&dest, &policy);
                    let after = snapshot(&fx.ws);
                    match (&checked, &ensured) {
                        (Err(_), _) => {
                            assert_eq!(
                                verdict(&ensured),
                                verdict(&checked),
                                "ensure disagreed with a refusing check: {case}"
                            );
                            assert_eq!(after, before, "a refused ensure wrote to disk: {case}");
                        }
                        (Ok(()), Ok(())) => {
                            // The bare spelling: `b/.` cannot be stat'd through a locked `b`,
                            // though `b` is there.
                            let bare = fx.destination(*depth, Spelling::Absolute);
                            assert!(
                                fs::metadata(&bare).map(|m| m.is_dir()).unwrap_or(false),
                                "ensure returned Ok without a directory there: {case}"
                            );
                            assert!(
                                check_destination(&dest, &policy).is_ok(),
                                "check refused what ensure had just approved and built: {case}"
                            );
                        }
                        (Ok(()), Err(PathError::Io { source, .. })) => {
                            #[cfg(unix)]
                            let staged = chain.contains(&Shape::ReadOnly);
                            #[cfg(not(unix))]
                            let staged = false;
                            assert!(
                                staged && source.kind() == std::io::ErrorKind::PermissionDenied,
                                "ensure failed where check approved, with no environmental \
                                 failure staged: {source}: {case}"
                            );
                        }
                        (Ok(()), Err(e)) => {
                            panic!("ensure refused ({e:?}) what check approved: {case}")
                        }
                    }
                    for (path, kind) in &after {
                        if !before.contains_key(path) {
                            assert!(
                                kind.starts_with("dir"),
                                "ensure created a {kind} at {}: {case}",
                                path.display()
                            );
                        }
                    }
                    cases += 1;
                }
            }
        }
    }
    eprintln!("{cases} cases");
}

#[test]
#[serial_test::serial] // set_current_dir is process-global
fn check_and_prepare_agree_on_every_output_file_shape_mode_clobber_and_spelling() {
    let privileged = privileged();
    if privileged {
        eprintln!("SKIPPED the read-only and locked shapes: chmod does not stop this process");
    }
    let shapes = Shape::all(privileged);

    // The output file is the deepest level of a chain of up to three, so its parent's chain is up
    // to two levels deep: deep enough for `Final` to refuse what `All` builds.
    let mut cases = 0;
    for (chain, depth) in &chains(&shapes, 3) {
        for mode in MODES {
            for clobber in CLOBBERS {
                for spelling in SPELLINGS {
                    // As above: `a/..` through a link to `/Volumes` is `/`.
                    #[cfg(target_os = "macos")]
                    if chain.contains(&Shape::LinkToVolumes)
                        && matches!(spelling, Spelling::RelativeThroughDotDot)
                    {
                        continue;
                    }
                    let fx = Fixture::build(chain, false);
                    let _cwd = CwdGuard::set(&fx.ws);
                    let file = fx.destination(*depth, spelling);
                    let case = format!(
                        "chain {chain:?}, depth {depth}, {mode}, {clobber}, {spelling:?} ({})",
                        file.display()
                    );

                    let before = snapshot(&fx.ws);
                    let checked = check_output_file(&file, mode, clobber);
                    assert_eq!(snapshot(&fx.ws), before, "check wrote to disk: {case}");

                    let prepared = prepare_output_file(&file, mode, clobber);
                    let after = snapshot(&fx.ws);
                    match (&checked, &prepared) {
                        (Err(_), _) => {
                            assert_eq!(
                                verdict(&prepared),
                                verdict(&checked),
                                "prepare disagreed with a refusing check: {case}"
                            );
                            assert_eq!(after, before, "a refused prepare wrote to disk: {case}");
                        }
                        (Ok(()), Ok(())) => {
                            let parent = fx.destination(depth - 1, Spelling::Absolute);
                            assert!(
                                fs::metadata(&parent).map(|m| m.is_dir()).unwrap_or(false),
                                "prepare returned Ok without the file's parent there: {case}"
                            );
                            assert!(
                                check_output_file(&file, mode, clobber).is_ok(),
                                "check refused what prepare had just approved and built: {case}"
                            );
                        }
                        (Ok(()), Err(PathError::Io { source, .. })) => {
                            #[cfg(unix)]
                            let staged = chain.contains(&Shape::ReadOnly);
                            #[cfg(not(unix))]
                            let staged = false;
                            assert!(
                                staged && source.kind() == std::io::ErrorKind::PermissionDenied,
                                "prepare failed where check approved, with no environmental \
                                 failure staged: {source}: {case}"
                            );
                        }
                        (Ok(()), Err(e)) => {
                            panic!("prepare refused ({e:?}) what check approved: {case}")
                        }
                    }
                    for (path, kind) in &after {
                        if !before.contains_key(path) {
                            assert!(
                                kind.starts_with("dir"),
                                "prepare created a {kind} at {}: {case}",
                                path.display()
                            );
                        }
                    }
                    cases += 1;
                }
            }
        }
    }
    eprintln!("{cases} cases");
}

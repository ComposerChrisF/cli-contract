//! The caller-asserted path contract.
//!
//! The unifying principle (from the rule): **exit 1 is the caller-claim/world-mismatch code.**  A
//! path the invocation asserted — an input that should exist, the directory an output is written
//! into — that does not match the filesystem is a runtime failure the tool refuses to guess past.
//! Every [`PathError`] here maps to [`crate::exit::TOOL_ERROR`].
//!
//! Output directories default to **must-exist** (`CreateMode::None`), because the output's parent
//! is itself a caller-asserted location, exactly as an input path is.  The escalations
//! ([`CreateMode::Final`], [`CreateMode::All`]) are opt-in, and the mount-root floor
//! ([`is_mount_root`]) is unconditional — no mode may create a mount point.

use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// How many missing directory levels a tool may create for a caller-asserted output path.
///
/// The default is [`CreateMode::None`]: the destination (or an output file's parent) must already
/// exist.  This mirrors the input contract and matches Unix convention (`cp`, `>` both require the
/// parent).  Expose it as a flag, e.g. with the `clap` feature
/// `#[arg(long, value_enum, default_value_t)] create_destination: CreateMode`, or parse it with
/// `FromStr` under any other argument parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
pub enum CreateMode {
    /// Create no directories; the destination (or output parent) must already exist, else exit 1.
    #[default]
    None,
    /// Create the one final directory, only if *its* parent already exists.
    Final,
    /// Create the whole missing chain, one level at a time below the deepest directory that
    /// exists — still bounded by the mount-root floor.
    All,
}

/// Whether an output **file** may overwrite an existing file at its path.  Expose it as a flag,
/// e.g. with the `clap` feature `#[arg(long, value_enum, default_value_t)] clobber: Clobber`, or
/// parse it with `FromStr` under any other argument parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
pub enum Clobber {
    /// Overwrite an existing output file (the create-if-missing default for a file leaf).
    #[default]
    Allow,
    /// Refuse to overwrite an existing output file — the must-not-exist (no-clobber) deviation.
    Deny,
}

// `FromStr` and `Display` spell the values exactly as clap's `ValueEnum` derive does, so a tool on
// any other argument parser takes the same flag values; a test under the feature pins the two.

impl fmt::Display for CreateMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            CreateMode::None => "none",
            CreateMode::Final => "final",
            CreateMode::All => "all",
        })
    }
}

impl std::str::FromStr for CreateMode {
    type Err = ParseValueError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(CreateMode::None),
            "final" => Ok(CreateMode::Final),
            "all" => Ok(CreateMode::All),
            _ => Err(ParseValueError {
                value: s.to_string(),
                expected: &["none", "final", "all"],
            }),
        }
    }
}

impl fmt::Display for Clobber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Clobber::Allow => "allow",
            Clobber::Deny => "deny",
        })
    }
}

impl std::str::FromStr for Clobber {
    type Err = ParseValueError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "allow" => Ok(Clobber::Allow),
            "deny" => Ok(Clobber::Deny),
            _ => Err(ParseValueError {
                value: s.to_string(),
                expected: &["allow", "deny"],
            }),
        }
    }
}

/// A string that names none of the values of the type being parsed, [`CreateMode`] or
/// [`Clobber`]: the error of their `FromStr`.
///
/// `FromStr` cannot see where the string came from, so the exit code is the caller's to choose: a
/// usage error ([`crate::exit::USAGE`], exit 2) for a flag value on its command line, a tool error
/// ([`crate::exit::TOOL_ERROR`], exit 1) for a value in its own config file.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ParseValueError {
    /// The string as given.
    pub value: String,
    /// The spellings that would have parsed.
    pub expected: &'static [&'static str],
}

impl fmt::Display for ParseValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "'{}' is not one of: {}",
            self.value,
            self.expected.join(", ")
        )
    }
}

impl std::error::Error for ParseValueError {}

/// A validated creation-and-marker policy for a destination directory.
///
/// Take the two flags as a [`CreateMode`] and an `Option<MarkerName>`: with the `clap` feature,
/// `#[arg(long)] require_marker: Option<MarkerName>` beside the mode's `value_enum`, or parsed by
/// any other argument parser through their `FromStr`.  Then construct the policy with
/// `DestPolicy::from_flags(args.create_destination, args.require_marker)?`.
///
/// Two things are refused as usage errors.  A marker that is not a name is refused first, when
/// its [`MarkerName`] is made, as a [`MarkerNameError`] (clap reports it for a flag so typed).
/// [`DestPolicy::from_flags`] refuses the one incoherent combination — a marker together with a
/// *creating* mode — since a marked directory must already exist, so it cannot also be created.
/// Neither can be represented at all: `Marked` carries no mode, and it holds a [`MarkerName`],
/// which cannot be a non-name, so a `Marked` value built directly is as sound as one from
/// `from_flags`.  Refusing both (rather than silently dropping the mode, or silently probing the
/// wrong path) keeps every consumer's behavior identical.  A tool with several output folders
/// holds one `DestPolicy` per folder.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DestPolicy {
    /// Create missing directories per the mode (`CreateMode::None` = must already exist); no marker.
    Create(CreateMode),
    /// The directory must already exist and contain the named marker file/dir.  The value is a
    /// name — one path component — never a path.
    Marked(MarkerName),
}

impl DestPolicy {
    /// Map the two command-line flags — `--create-destination <mode>` and an optional
    /// `--require-marker <name>` — to a validated policy.  Its one refusal is `--require-marker`
    /// combined with `--create-destination=final|all`, as [`PolicyError::MarkerWithCreate`]: a
    /// usage error, since it needs no filesystem to decide, and the caller's only remedy is to
    /// change the command line.
    ///
    /// The marker's name was checked before this call, when its [`MarkerName`] was made; for a
    /// clap flag typed `Option<MarkerName>`, clap reports a non-name as a usage error before
    /// `from_flags` runs.  So the name fault is still reported first: with a non-name marker and a
    /// creating mode the caller has two mistakes, and fixing the mode alone would only surface the
    /// name a run later.
    pub fn from_flags(mode: CreateMode, marker: Option<MarkerName>) -> Result<Self, PolicyError> {
        let Some(marker) = marker else {
            return Ok(DestPolicy::Create(mode));
        };
        match mode {
            CreateMode::None => Ok(DestPolicy::Marked(marker)),
            CreateMode::Final | CreateMode::All => Err(PolicyError::MarkerWithCreate),
        }
    }
}

/// The name of a marker: a file or directory whose presence inside a directory proves it is the
/// one meant ([`require_marker`], [`DestPolicy::Marked`]).  It is exactly one path component, never
/// a path, so the guard always looks inside the directory it was handed.
///
/// [`MarkerName::new`], and `FromStr`, which calls it, refuse every value that is not a name, as
/// a [`MarkerNameError`]: the empty string, an absolute path, a value containing a path
/// separator, `.` or `..`, and on Windows a value made only of dots and spaces (Windows drops a
/// name's trailing dots and spaces, so it names the directory itself) or a drive prefix (`C:`,
/// `C:x`).  `Path::join` reinterprets each of those — an absolute value *replaces* the directory,
/// `..` escapes it, a separator descends, the empty string is the directory itself — so a guard
/// handed one would test some other path, and pass for every directory on earth.
///
/// A clap flag can be typed `Option<MarkerName>`, and clap reports a bad value as a usage error
/// (exit 2).  Where else the error goes, and with which exit code, is the caller's to choose (see
/// [`MarkerNameError`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MarkerName(String);

impl MarkerName {
    /// Take `name` as a marker name, or refuse it as a [`MarkerNameError`] when it is not one path
    /// component.
    pub fn new(name: impl Into<String>) -> Result<MarkerName, MarkerNameError> {
        let marker = name.into();
        match marker_name_fault(&marker) {
            Some(fault) => Err(MarkerNameError { marker, fault }),
            None => Ok(MarkerName(marker)),
        }
    }

    /// The name, as given.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::str::FromStr for MarkerName {
    type Err = MarkerNameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        MarkerName::new(s)
    }
}

impl fmt::Display for MarkerName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A value that is not a marker name: the error of [`MarkerName::new`] and of its `FromStr`.  It
/// is empty, absolute, contains a path separator, is `.`/`..`, or (on Windows) is only dots and
/// spaces or a drive prefix (`C:`, `C:x`); `fault` says which.  Its message names no flag, since a
/// [`MarkerName`] is made from a flag of any spelling, a config file, or anywhere else.
///
/// `FromStr` cannot see where the string came from, so the exit code is the caller's to choose: a
/// usage error ([`crate::exit::USAGE`], exit 2) for a value from the command line, which clap
/// reports as one for a flag typed `MarkerName`; a tool error ([`crate::exit::TOOL_ERROR`], exit 1)
/// for a value from the tool's own config file.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MarkerNameError {
    /// The value as given.
    pub marker: String,
    /// Why it is not a name, in words that complete "the value ...".
    pub fault: &'static str,
}

impl fmt::Display for MarkerNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "marker '{}' is not a name: the value {}.  A marker is a single file or directory name \
             looked for inside a directory, never a path",
            self.marker, self.fault
        )
    }
}

impl std::error::Error for MarkerNameError {}

/// Why `marker` is not a name — `None` when it is one.  A marker is exactly one normal path
/// component: `Path::join` reinterprets every other shape (an absolute value *replaces* the
/// directory, `..` escapes it, a separator descends, the empty string is the directory itself), and
/// each of those made the wrong-disk guard pass for every destination on earth.  The
/// phrase completes "the value ...".
///
/// The checks run in the order the message should name them; after the empty, absolute, and
/// separator checks, the component walk on Unix can only distinguish `.` and `..` from a name.
fn marker_name_fault(marker: &str) -> Option<&'static str> {
    if marker.is_empty() {
        return Some("is empty");
    }
    let path = Path::new(marker);
    if path.is_absolute() || path.has_root() {
        return Some("is absolute");
    }
    // Checked before the component walk: `components()` drops a trailing separator and folds an
    // interior `.`, so "a/" and "a/./b" would otherwise read as clean.
    if marker.chars().any(std::path::is_separator) {
        return Some("contains a path separator");
    }
    let mut components = path.components();
    match (components.next(), components.next()) {
        // Windows strips trailing dots and spaces from a name, so `...` or `" "` names the directory.
        (Some(Component::Normal(_)), None)
            if cfg!(windows) && marker.trim_matches(|c| c == '.' || c == ' ').is_empty() =>
        {
            Some("is only dots and spaces, which Windows reads as the directory itself")
        }
        (Some(Component::Normal(_)), None) => None,
        (Some(Component::CurDir), None) => Some("is '.'"),
        (Some(Component::ParentDir), None) => Some("is '..'"),
        // Reachable only on Windows (a drive prefix, as in `C:` or `C:x`): on Unix every other
        // shape was refused above, so only a Windows-only test can pin this arm.
        _ => Some("is not a single path component"),
    }
}

/// A policy wrong on its face — a contradictory combination of flags — distinct from a runtime
/// [`PathError`] (exit 1): the error of [`DestPolicy::from_flags`].  A marker that is not a name
/// is not one: it is refused before, as a [`MarkerNameError`], when its [`MarkerName`] is made.
///
/// [`PolicyError::MarkerWithCreate`]'s message names the conventional flags, `--require-marker` and
/// `--create-destination`.  A tool that spells them otherwise renders the variant itself.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PolicyError {
    /// `--require-marker` was combined with `--create-destination=final|all`.
    MarkerWithCreate,
}

impl PolicyError {
    /// Always [`crate::exit::USAGE`] (exit 2), for flags: an invocation wrong on its face, not a
    /// runtime failure.  A policy assembled from a config file is the tool's own input, so exit 1
    /// ([`crate::exit::TOOL_ERROR`]); the caller chooses.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        crate::exit::USAGE
    }
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PolicyError::MarkerWithCreate => write!(
                f,
                "--require-marker cannot be combined with --create-destination=final|all: a marked \
                 directory must already exist (a marker cannot be pre-placed in a directory you are \
                 also creating)"
            ),
        }
    }
}

impl std::error::Error for PolicyError {}

/// A violation of the path contract.  Every variant maps to [`crate::exit::TOOL_ERROR`] (exit 1);
/// render it to stderr and exit with [`PathError::exit_code`].  Most are caller-claim/world
/// mismatches.  [`PathError::Undeterminable`] (the path could not be inspected) and
/// [`PathError::Io`] (a create that failed after the checks passed) are the tool failing instead.
#[derive(Debug)]
#[non_exhaustive]
pub enum PathError {
    /// A caller-asserted input path does not exist.  Claimed only on a proven `NotFound` — a path
    /// that merely could not be inspected is [`PathError::Undeterminable`], never this.
    MissingInput(PathBuf),
    /// A caller-asserted output/destination directory does not exist (and the mode forbids creating it).
    MissingDestination(PathBuf),
    /// A non-directory (a file) already occupies a path where a directory was required — reported
    /// specifically, rather than as "does not exist", so the message states the actual fact.
    NotADirectory(PathBuf),
    /// A directory already occupies a path where an output file was required.
    NotAFile(PathBuf),
    /// The path is itself a symbolic link, and the caller required a real path ([`reject_symlink`]).
    IsSymlink(PathBuf),
    /// The path is a symbolic link whose target does not exist (a dangling link) — reported
    /// specifically, since it neither exists as a usable directory nor is genuinely absent.
    DanglingSymlink(PathBuf),
    /// The parent of a to-be-created directory does not exist (`CreateMode::Final`).
    MissingParent {
        /// The directory that was to be created.
        target: PathBuf,
        /// Its parent, which does not exist.
        parent: PathBuf,
    },
    /// Creating this path would materialize a mount point — never allowed, at any create mode.
    MountRoot(PathBuf),
    /// A `..` after a directory that does not exist yet: a level of the chain [`CreateMode::All`]
    /// would create, or, in every mode, the last component of an output file's path.  Where it
    /// leads is known only once that directory is built, and then from wherever the directory
    /// really is, so neither the floor nor an output file's own check can be made first.  Names
    /// the path ending in that `..`: the level, absolute, or the output file as given.
    DotDotAfterMissing(PathBuf),
    /// A required marker file/dir is not present inside the directory.
    MarkerMissing {
        /// The directory searched.
        dir: PathBuf,
        /// The marker name not found inside it.
        marker: String,
    },
    /// The output file already exists and clobbering was refused ([`Clobber::Deny`]).
    Clobber(PathBuf),
    /// **The path could not be inspected at all** — permission denied, an I/O error, a symlink
    /// loop, a network share or mount that cannot be reached.  Distinct from every "does not exist"
    /// variant above, and that distinction is the point: *we could not look* is not *it is not there*, and a message that
    /// confuses the two points diagnosis away from the actual cause (the fix is usually `chmod`,
    /// not a corrected path).  Guards refuse on this rather than approve — see the module docs.
    Undeterminable {
        /// The path that could not be inspected: `.` when the current directory could not be
        /// resolved.
        path: PathBuf,
        /// The error the probe met.
        source: std::io::Error,
    },
    /// An underlying I/O error while creating a directory.
    Io {
        /// The directory being created.
        path: PathBuf,
        /// The error the create returned.
        source: std::io::Error,
    },
}

impl PathError {
    /// The process exit code for any path-contract violation: always [`crate::exit::TOOL_ERROR`].
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        crate::exit::TOOL_ERROR
    }
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathError::MissingInput(p) => write!(f, "input path does not exist: {}", p.display()),
            PathError::MissingDestination(p) => {
                write!(f, "destination directory does not exist: {}", p.display())
            }
            PathError::NotADirectory(p) => {
                write!(
                    f,
                    "expected a directory but a file exists at: {}",
                    p.display()
                )
            }
            PathError::NotAFile(p) => {
                write!(
                    f,
                    "expected a file path but a directory exists at: {}",
                    p.display()
                )
            }
            PathError::IsSymlink(p) => {
                write!(
                    f,
                    "refusing a symbolic link (a real path is required): {}",
                    p.display()
                )
            }
            PathError::DanglingSymlink(p) => write!(
                f,
                "path is a broken symbolic link (its target does not exist): {}",
                p.display()
            ),
            PathError::MissingParent { target, parent } => write!(
                f,
                "cannot create {}: its parent directory does not exist: {}",
                target.display(),
                parent.display()
            ),
            PathError::MountRoot(p) => write!(
                f,
                "refusing to create a mount point: {} (is the volume mounted?)",
                p.display()
            ),
            PathError::DotDotAfterMissing(p) => write!(
                f,
                "refusing '..' after a directory that does not exist yet, since where it leads \
                 cannot be checked before the create: {}",
                p.display()
            ),
            PathError::MarkerMissing { dir, marker } => write!(
                f,
                "directory {} is missing its required marker '{}' (is this the intended target?)",
                dir.display(),
                marker
            ),
            PathError::Clobber(p) => {
                write!(f, "refusing to overwrite an existing file: {}", p.display())
            }
            PathError::Undeterminable { path, source } => write!(
                f,
                "cannot determine whether {} exists: {}",
                path.display(),
                source
            ),
            PathError::Io { path, source } => {
                write!(f, "I/O error creating {}: {}", path.display(), source)
            }
        }
    }
}

// No `source()`: `Undeterminable` and `Io` already put their `io::Error` in the message, and a
// chain print (anyhow's `{:#}`) would print it twice.  The field stays public for a caller that
// wants the error itself.
impl std::error::Error for PathError {}

/// What a probe actually learned about a path.
///
/// There is deliberately **no `Unknown` variant**: Unknown is the `Err` of the enclosing
/// `Result`, so a caller cannot reach a verdict without dealing with it.  That is the whole
/// point — a probe that gates a destructive action must tell "absent" from "could not tell",
/// and this is the reason [`Absent`](Presence::Absent) can be trusted: it means a proven
/// `NotFound`, and nothing else.
///
/// Do not "simplify" any of this back into a `bool` ([`Path::exists`], `is_dir`, `is_ok`): those
/// answer two ways to a question the world answers three ways, and every such collapse records
/// "I could not look" as "it is not there".  That single inference is the source of the whole
/// bug family this crate exists to prevent.
#[derive(Debug, Clone, Copy)]
enum Presence {
    /// Proven absent: the probe returned `NotFound`, and nothing else.
    Absent,
    /// Present, with the file type the probe saw.
    Present(fs::FileType),
}

/// `path` with its last component bare: no trailing separator, and no trailing `/.`.  Either one
/// makes the path name a link's *target* (`lstat("link/")` and `lstat("link/.")` both follow the
/// link), so every probe takes the path through this first: the `lstat` that must see a link as
/// itself, and the `stat` that then follows it, so both examine the same entry.  Only
/// the last component is rebuilt; the rest is the caller's text, so a Windows verbatim path keeps
/// its literal `..`.
fn without_trailing_separator(path: &Path) -> PathBuf {
    match (path.parent(), path.file_name()) {
        // `join` re-parses the name it appends: on Windows `C:` or `a:s` would replace the parent,
        // and in a verbatim path a `/` would split it.  A name that does not parse as itself is
        // left as the caller wrote it, for the OS to judge.
        (Some(parent), Some(name)) if Path::new(name).file_name() == Some(name) => {
            parent.join(name)
        }
        _ => path.to_path_buf(),
    }
}

/// Whether a lookup's error proves the path absent: `NotFound`, and nothing else.  On Windows the
/// standard library also maps `ERROR_BAD_NETPATH` (53) to `NotFound`, though it means a server that
/// could not be reached, so that one is not proof.  `ERROR_BAD_NET_NAME` (67) stays proof, on
/// purpose: the server answered that the share does not exist, as an absent drive or an unmounted
/// local volume is reported as missing.
fn proves_absence(e: &std::io::Error) -> bool {
    const ERROR_BAD_NETPATH: i32 = 53;
    e.kind() == std::io::ErrorKind::NotFound
        && !(cfg!(windows) && e.raw_os_error() == Some(ERROR_BAD_NETPATH))
}

/// Probe a path with `lstat`: a symlink is seen **as itself**, not as its target.
///
/// A path below a file or a dangling link cannot exist, and the OS says so in different words:
/// Unix refuses the lookup (ENOTDIR), Windows answers `NotFound`, and a dangling link above answers
/// `NotFound` everywhere.  So when `path` is not there, or could not be looked up, its ancestors
/// are examined from the parent up, and the first one that can be looked up decides: a file, a
/// link to one, or a dangling link there is the error that names it, as [`probe_dir`] names it,
/// never "absent" or "cannot determine" about the path below it.
fn probe(path: &Path) -> Result<Presence, PathError> {
    let verdict = match fs::symlink_metadata(without_trailing_separator(path)) {
        Ok(md) => return Ok(Presence::Present(md.file_type())),
        Err(e) if proves_absence(&e) => Ok(Presence::Absent),
        Err(source) => Err(PathError::Undeterminable {
            path: path.to_path_buf(),
            source,
        }),
    };
    // Deliberately any error at the leaf, not only ENOTDIR (whose `ErrorKind` is newer than the
    // MSRV): for an error that nothing above can cause, the walk finds nothing in the way, and the
    // error stands.  A loop, not a recursion through `probe_dir`, so that a path of any length is
    // answered, never a stack overflow.
    for above in path.ancestors().skip(1) {
        // An ancestor that cannot be looked up is passed over: what stops the lookup is higher.
        let Ok(md) = fs::symlink_metadata(without_trailing_separator(above)) else {
            continue;
        };
        let in_the_way = if md.file_type().is_symlink() {
            match probe_target(above) {
                Ok(Presence::Absent) => Some(PathError::DanglingSymlink(above.to_path_buf())),
                Ok(Presence::Present(target)) if !target.is_dir() => {
                    Some(PathError::NotADirectory(above.to_path_buf()))
                }
                // A link to a directory, or one whose target could not be looked up.
                _ => None,
            }
        } else if md.is_dir() {
            None
        } else {
            Some(PathError::NotADirectory(above.to_path_buf()))
        };
        return in_the_way.map_or(verdict, Err);
    }
    verdict
}

/// Probe a path with `stat`: a symlink is **followed**, so this reports its target.
///
/// `Absent` therefore means the target is genuinely missing (a dangling link).  A symlink loop
/// fails with `ELOOP`, which is not `NotFound`, so it lands in [`PathError::Undeterminable`] and
/// names the real errno — a loop is not a missing target, and reporting it as one misdirects
/// diagnosis exactly the way a permission error reported as absence does.
fn probe_target(path: &Path) -> Result<Presence, PathError> {
    match fs::metadata(without_trailing_separator(path)) {
        Ok(md) => Ok(Presence::Present(md.file_type())),
        Err(e) if proves_absence(&e) => Ok(Presence::Absent),
        Err(source) => Err(PathError::Undeterminable {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Probe a path where a directory is required: a destination, the parent `Final` creates in, or
/// an ancestor `All` builds on.  `Present` means a directory (a symlink to one is followed,
/// matching OS convention); `Absent` means a proven `NotFound`.  Anything else at `path` is the
/// error that names it, never "absent": a file, or a link to one, is [`PathError::NotADirectory`],
/// and a link to nothing is [`PathError::DanglingSymlink`].  So is either one above `path`, under
/// every policy and on every platform, because [`probe`] names what stands in the way.
fn probe_dir(path: &Path) -> Result<Presence, PathError> {
    match probe_resolved(path)? {
        Presence::Present(ft) if !ft.is_dir() => Err(PathError::NotADirectory(path.to_path_buf())),
        presence => Ok(presence),
    }
}

/// [`probe`], with a symlink followed to its target, as the OS resolves it.  `Present` is the
/// type of what the path resolves to — never a link — and `Absent` is a proven `NotFound` of the
/// path itself.  A link to nothing is [`PathError::DanglingSymlink`]: it exists as a link but
/// resolves to nothing, and that fact is reported, never "missing".
fn probe_resolved(path: &Path) -> Result<Presence, PathError> {
    match probe(path)? {
        Presence::Present(ft) if ft.is_symlink() => match probe_target(path)? {
            Presence::Absent => Err(PathError::DanglingSymlink(path.to_path_buf())),
            target => Ok(target),
        },
        presence => Ok(presence),
    }
}

/// Refuse a caller-asserted input path that does not exist.  Never creates it (creating it would
/// launder a typo into permanent world state and manufacture the silent-wrong-output failure).
///
/// A path that could not be inspected is [`PathError::Undeterminable`], never
/// [`PathError::MissingInput`]: absence is claimed only on a proven `NotFound`.
pub fn require_input_path(path: &Path) -> Result<(), PathError> {
    match probe_resolved(path)? {
        Presence::Absent => Err(PathError::MissingInput(path.to_path_buf())),
        Presence::Present(_) => Ok(()),
    }
}

/// Refuse a caller-asserted input **directory** that does not exist ([`PathError::MissingInput`]),
/// or that is not a directory.  Never creates anything.
///
/// A caller wanting a marker — the same "is this the directory I actually mean?" guard used on
/// output folders (see [`ensure_destination`]), for an input dir that must be a specific one, e.g.
/// a media directory that must contain its mapped subfolders — calls
/// [`require_marker(dir, &marker)`](require_marker) instead.  That call also refuses a missing
/// directory ([`PathError::MissingInput`]) or a non-directory, and the name is checked before
/// anything is probed: when its [`MarkerName`] is made.
pub fn require_input_dir(path: &Path) -> Result<(), PathError> {
    // A symlink is followed, so a link to a directory is usable as one; a link to nothing, or a
    // file, is reported as what it is rather than as missing.
    match probe_dir(path)? {
        Presence::Absent => Err(PathError::MissingInput(path.to_path_buf())),
        Presence::Present(_) => Ok(()),
    }
}

/// Verify a marker file or directory named `marker` is present inside `dir`.
///
/// `marker` is a [`MarkerName`], one path component, so `dir.join(marker)` is an entry of `dir`:
/// a value that would make it test some other path entirely — and a guard that tests the wrong
/// path passes when it should not — was refused when the name was made.
///
/// `MarkerMissing` is claimed only on a proven `NotFound`, inside a directory that exists: a `dir`
/// that does not exist is [`PathError::MissingInput`].  This is a wrong-disk guard: reporting
/// a marker we merely could not stat as "missing" would be a false claim about the very fact the
/// guard exists to establish.
///
/// A marker that is a symbolic link is followed: a link to something that exists is the marker
/// (the entry is inside `dir`, which is what proves the disk).  A **dangling** link is reported
/// as [`PathError::DanglingSymlink`], never as missing — an entry of that name is present, so
/// "missing" would be a false statement of fact by the same standard (ruled 2026-09-04).
pub fn require_marker(dir: &Path, marker: &MarkerName) -> Result<(), PathError> {
    // A marker cannot be missing from a directory that is not there.
    if let Presence::Absent = probe_dir(dir)? {
        return Err(PathError::MissingInput(dir.to_path_buf()));
    }
    let path = dir.join(marker.as_str());
    match probe_resolved(&path)? {
        Presence::Absent => Err(PathError::MarkerMissing {
            dir: dir.to_path_buf(),
            marker: marker.to_string(),
        }),
        Presence::Present(_) => Ok(()),
    }
}

/// Refuse `path` if it is a symbolic link — the "reject" policy as a composable guard.  Call it
/// before [`ensure_destination`] in a tool that refuses a symlinked destination.  It asks whether
/// `path` itself is a link, not whether its ancestors are: a path under a linked directory passes,
/// and resolves through the link.  A caller that needs more proves the disk with the marker guard
/// ([`require_marker`]), or checks each ancestor itself, in a walk that refuses on every error but
/// the one that proves an ancestor absent: `NotFound`, and nothing else.
///
/// **Fails closed.**  A path that could not be lstat'd is refused as
/// [`PathError::Undeterminable`], never approved: this guard exists to distrust the path it is
/// handed, so "I could not look" must not come back as "it is not a link".  A proven-absent path
/// passes through — it is not a symlink, and the must-exist checks own that report.  A path below
/// a file or a dangling link is refused, naming that entry, as those checks would name it.
pub fn reject_symlink(path: &Path) -> Result<(), PathError> {
    match probe(path)? {
        Presence::Present(ft) if ft.is_symlink() => Err(PathError::IsSymlink(path.to_path_buf())),
        Presence::Present(_) | Presence::Absent => Ok(()),
    }
}

/// Validate a caller-asserted **destination directory** against a [`DestPolicy`] and the
/// unconditional mount-root floor, **creating nothing**.  On `Ok`, the directory either already
/// exists and passes every check, or is absent and the policy permits [`ensure_destination`] to
/// create it.  Neither this nor [`ensure_destination`] checks that the directory is writable.
///
/// This is the `--dry-run` half of the contract.  A mutating tool previewing its work must still
/// refuse an unmounted volume (the floor does where the mount point goes away with its volume, and
/// a marker does elsewhere) — a dry run that reports a plausible backup onto a disk that is not
/// there is the very failure the contract exists to kill — yet it must not create the directory it
/// is only pretending to write into.  Call this in a dry run, and [`ensure_destination`] in a live
/// run.  Both run the same contract checks, so a dry run never disagrees with the live run about
/// the _contract_ — only an environmental failure at creation time (permissions, a full disk, a
/// race, a name the filesystem refuses) can separate them.
pub fn check_destination(dir: &Path, policy: &DestPolicy) -> Result<(), PathError> {
    approved_levels(dir, policy).map(drop)
}

/// [`check_destination`], with the levels it approved creating: under `All`, the missing
/// directories of the chain from [`floor_check_chain`]; none under every other policy, nor for a
/// directory that exists.  `All`'s create builds these and nothing the check did not see.
fn approved_levels(dir: &Path, policy: &DestPolicy) -> Result<Vec<PathBuf>, PathError> {
    // Classify what is at `dir` once, three ways, before any policy runs.  A destination that
    // could not be inspected propagates as Undeterminable — a check that guessed green here would
    // out-promise the live run, which is the disagreement this pair of functions exists to rule out.
    // (A symlink to a directory follows through as a directory, matching OS convention; use
    // reject_symlink first if the caller wants a different policy.)
    // A non-directory already sitting at the path, or above it, is a specific, early error under
    // any policy — no create mode can help, and "does not exist" would be a lie.  Handling it here
    // also means the create branches below only ever run against a genuinely-absent path.
    let is_existing_dir = matches!(probe_dir(dir)?, Presence::Present(_));
    match policy {
        DestPolicy::Marked(marker) => {
            if !is_existing_dir {
                return Err(PathError::MissingDestination(dir.to_path_buf()));
            }
            require_marker(dir, marker).map(|()| Vec::new())
        }
        DestPolicy::Create(_) if is_existing_dir => Ok(Vec::new()),
        DestPolicy::Create(CreateMode::None) => {
            Err(PathError::MissingDestination(dir.to_path_buf()))
        }
        DestPolicy::Create(CreateMode::Final) => {
            if mount_root(dir)? {
                return Err(PathError::MountRoot(dir.to_path_buf()));
            }
            match dir.parent() {
                // A bare relative name (`newdir`) has the empty path as its parent, which names
                // the current directory: where `mkdir newdir` creates it, as `All` does too.
                Some(parent) if parent.as_os_str().is_empty() => Ok(Vec::new()),
                // A file or a dangling link in the parent's place is reported as what it is.
                Some(parent) => match probe_dir(parent)? {
                    Presence::Present(_) => Ok(Vec::new()),
                    Presence::Absent => Err(PathError::MissingParent {
                        target: dir.to_path_buf(),
                        parent: parent.to_path_buf(),
                    }),
                },
                None => Err(PathError::MissingDestination(dir.to_path_buf())),
            }
        }
        DestPolicy::Create(CreateMode::All) => floor_check_chain(dir),
    }
}

/// Prepare a caller-asserted **destination directory** — a folder that files are written into —
/// per a validated [`DestPolicy`] and the unconditional mount-root floor.  On `Ok` the directory
/// exists and passes every check.  Whether it is writable is not checked, so the first write into
/// it can still fail for permissions.
///
/// Runs every check in [`check_destination`], then creates the directory if the policy allows and
/// it is absent.  A tool with a `--dry-run` mode calls [`check_destination`] instead on that path.
pub fn ensure_destination(dir: &Path, policy: &DestPolicy) -> Result<(), PathError> {
    check_destination(dir, policy)?;
    create_after_check(dir, policy)
}

/// The rest of [`ensure_destination`], once its check has passed: a function of its own so a test
/// can reach the case where the directory vanished in between.
fn create_after_check(dir: &Path, policy: &DestPolicy) -> Result<(), PathError> {
    // The check judged the bare name, so the create acts on it too, or a dry run and the live run
    // could disagree about `dir/.`.
    let bare = without_trailing_separator(dir);
    if bare.is_dir() {
        return Ok(()); // already exists — nothing to create
    }
    // The check passed, but the directory is not there now: run the check again, under every
    // policy.  A destination that existed at check time never met the floor, which guards only
    // what is to be created, and an unmount is how it goes; so the create waits for the floor.
    // Under `Marked` and `None` the error names what is there instead, or `Ok` means a directory
    // is there again, marker included.
    let levels = approved_levels(dir, policy)?;
    match policy {
        DestPolicy::Marked(_) | DestPolicy::Create(CreateMode::None) => Ok(()),
        DestPolicy::Create(CreateMode::Final) => fs::create_dir(&bare).map_err(|e| PathError::Io {
            path: dir.to_path_buf(),
            source: e,
        }),
        DestPolicy::Create(CreateMode::All) => create_levels(dir, &levels),
    }
}

/// `All`'s create: the levels its check approved, one `create_dir` each from the base down.  Never
/// `create_dir_all`, which creates from the leaf upward until one succeeds, so it would build a
/// base gone since the check (an unmounted volume) again: as root, the mount point itself.  Here
/// that base fails the first create with `NotFound`.  A function of its own so a test can take the
/// base away between the check and the create.
fn create_levels(dir: &Path, levels: &[PathBuf]) -> Result<(), PathError> {
    for level in levels {
        // The bare name, as the check judged it: `mkdir("link/")` creates a dangling link's target.
        let level = without_trailing_separator(level);
        match fs::create_dir(&level) {
            Ok(()) => {}
            // Made since the walk saw it absent.  A directory there is built on, following a link
            // to one as the walk follows one; the floor is not checked again below it (a racing
            // writer is out of reach without `mkdirat`).  Anything else is named as what it is.
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if let Presence::Absent = probe_dir(&level)? {
                    return Err(PathError::Io {
                        path: dir.to_path_buf(),
                        source: e,
                    });
                }
            }
            Err(source) => {
                return Err(PathError::Io {
                    path: dir.to_path_buf(),
                    source,
                })
            }
        }
    }
    Ok(())
}

/// Validate a caller-asserted **output file** path as [`prepare_output_file`] does — the clobber
/// policy on the file itself, then its parent directory per `dir_mode` and the mount-root floor —
/// **creating nothing**.  On `Ok`, the file passed the clobber policy and every check on the file
/// itself, and its parent either already exists and passes every check, or is absent and
/// `dir_mode` permits [`prepare_output_file`] to create it.  The file itself is neither created
/// nor checked for writability, by this or by [`prepare_output_file`]: a spelling that ends in a
/// separator is checked as the file it names (see the README's symlink section), though no file
/// can be written under that spelling.
///
/// This is the `--dry-run` half of [`prepare_output_file`]: the same verdicts, and zero filesystem
/// writes.  Call this in a dry run, and [`prepare_output_file`] in a live run.  Both probe the file
/// the same way, and the parent goes through [`check_destination`] here and
/// [`ensure_destination`] there, so, as for a destination directory, only an environmental failure
/// at creation time can separate them.
///
/// The probe of the file is the safety-critical half, so do not pair [`check_destination`] on the
/// parent with a probe of your own: a file that could not be inspected is
/// [`PathError::Undeterminable`], never approved, so it cannot slip past [`Clobber::Deny`].  A
/// directory in the file's place is [`PathError::NotAFile`], and a dangling link is
/// [`PathError::DanglingSymlink`].  In every mode, an absent file named for a mount point is
/// [`PathError::MountRoot`], and one whose path ends in `..` is [`PathError::DotDotAfterMissing`].
pub fn check_output_file(
    path: &Path,
    dir_mode: CreateMode,
    clobber: Clobber,
) -> Result<(), PathError> {
    check_output_leaf(path, clobber)?;
    match output_parent(path)? {
        Some(parent) => check_destination(parent, &DestPolicy::Create(dir_mode)),
        None => Ok(()),
    }
}

/// Prepare a caller-asserted **output file** path: enforce the clobber policy on the file itself,
/// then ensure its parent directory per `dir_mode` and the mount-root floor.  Does not create the
/// file; returns once the parent is ready and the clobber check passes.  An absent file named for a
/// mount point (`/Volumes/Backup`) is refused as [`PathError::MountRoot`] in every mode, since
/// writing it would create one.  So is an absent file whose path ends in `..`, as
/// [`PathError::DotDotAfterMissing`], since once its parent is built that path names a directory.
///
/// Runs every check in [`check_output_file`], then creates the parent's missing levels if
/// `dir_mode` allows.  A tool with a `--dry-run` mode calls [`check_output_file`] instead on that
/// path.
pub fn prepare_output_file(
    path: &Path,
    dir_mode: CreateMode,
    clobber: Clobber,
) -> Result<(), PathError> {
    check_output_leaf(path, clobber)?;
    match output_parent(path)? {
        Some(parent) => ensure_destination(parent, &DestPolicy::Create(dir_mode)),
        None => Ok(()),
    }
}

/// The checks an output file meets on the file itself, before its parent is checked or ensured:
/// the same for [`check_output_file`] and [`prepare_output_file`].
fn check_output_leaf(path: &Path, clobber: Clobber) -> Result<(), PathError> {
    // One classification of the leaf, three ways.  A leaf that could not be stat'd must not slip
    // past `Clobber::Deny`: the refusal is the guard, and skipping it silently for a file we could
    // not inspect is the guard failing open.
    // A link to nothing is refused: the write would create its target somewhere never validated.
    match probe_resolved(path)? {
        // A directory in the output file's place is a specific error, distinct from clobbering a
        // file.
        Presence::Present(ft) if ft.is_dir() => Err(PathError::NotAFile(path.to_path_buf())),
        Presence::Present(_) if clobber == Clobber::Deny => {
            Err(PathError::Clobber(path.to_path_buf()))
        }
        // The write would create the mount point, as a file.
        Presence::Absent if mount_root(path)? => Err(PathError::MountRoot(path.to_path_buf())),
        // An absent leaf ending in `..` follows a directory not there yet, its own parent: once
        // that is built, the path names a directory, never a file (bug-0040).
        Presence::Absent if matches!(path.components().next_back(), Some(Component::ParentDir)) => {
            Err(PathError::DotDotAfterMissing(path.to_path_buf()))
        }
        Presence::Present(_) | Presence::Absent => Ok(()),
    }
}

/// The directory an output file is written into, when there is one to check: `None` for the
/// current directory, and [`PathError::MissingDestination`] for a path with no parent at all.
fn output_parent(path: &Path) -> Result<Option<&Path>, PathError> {
    match path.parent() {
        // The output's parent directory is itself a caller-asserted destination (no marker).
        Some(parent) if !parent.as_os_str().is_empty() => Ok(Some(parent)),
        Some(_) => Ok(None), // writing into the current directory: nothing to check or create
        // A root that is not there (on Windows, an absent drive or share): no file can be written.
        None => Err(PathError::MissingDestination(path.to_path_buf())),
    }
}

/// True if `path` names a mount root that must never be auto-created — the mount point itself,
/// whose existence *is* the signal that the volume is mounted, where the mount point goes away at
/// unmount (as on macOS, and under udisks2 on Linux).
///
/// Everywhere but Windows, checks a fixed union of known mount roots (harmless to over-cover): macOS
/// `/Volumes` and its immediate child `/Volumes/<name>`; Linux (best-effort, less canonical) the
/// mount dirs under `/mnt`, `/media`, and `/run/media`.  Paths *below* an existing mount point are
/// not mount roots and may be created normally, except at the top level of a volume mounted
/// directly at `/mnt`, `/media/<name>`, or `/run/media/<name>`, which the list's depths reach.
///
/// **On Unix, `/Volumes` is recognized by identity**, not only by spelling: a path that is
/// `/Volumes`, or whose parent is, by device and inode.  So `/volumes/X` on a case-insensitive
/// volume, the firmlink `/System/Volumes/Data/Volumes/X`, and `X` under a symbolic link to
/// `/Volumes` are mount roots too.  The Linux roots are matched by spelling only.
///
/// **On Windows the floor checks no list.**  The mount roots there are drive letters and UNC
/// shares, which `create_dir` cannot fabricate: creating under an absent drive fails.  The Unix
/// list would only refuse ordinary folders, such as `C:\media\photos`.  So this is `false` for
/// every path it can resolve there, but a verbatim (`\\?\`) path with a `..` after a level that
/// does not exist (below); an absent drive or share root is refused by `All`'s chain walk
/// instead.  A volume mounted into an empty NTFS folder is not detected, just as a Unix mount
/// outside the listed roots is not.
///
/// **A relative path is resolved against the current directory first**, because that is what the
/// OS will do when the path is created: the floor must examine the components the create will
/// actually walk, not the caller's spelling of them.  Resolution is lexical
/// ([`std::path::absolute`]) — it touches no filesystem and does *not* resolve symlinks, so the
/// list is matched against the literal path, including `..`, which after a symbolic link the OS
/// resolves from the link's target instead; only the identity check for `/Volumes`, and the check
/// for a `..` after a level that does not exist, look through a link (any other symlink redirection
/// stays the caller's policy; see the README's symlink section).  A consequence worth knowing:
/// under `All`, [`ensure_destination`] names the absolute path of the level it refuses, even when
/// the caller passed a relative one; under `Final`, and for an output file named for a mount point
/// in every mode, a [`PathError::MountRoot`] names the path as given.
///
/// **Fails closed.**  If the current directory cannot be read (deleted cwd, EACCES) the path
/// cannot be *proven* safe, so this returns `true` and the creation is refused; so it does when
/// `/Volumes`, the path, or its parent cannot be stat'd for any reason but `NotFound`.  So it does,
/// too, for a path with a `..` after a level that does not exist (`new/../x`, with `new` absent;
/// on Windows, where `..` is resolved first, only a verbatim path keeps one): where that leads is
/// known only once the level is built, so no check made now can prove it safe, and
/// [`check_destination`] refuses the same path under `All`.  The floor gates creation, so the
/// conservative branch is to refuse: a wrongly-refused create is a nuisance, a fabricated mount
/// point may be a mis-targeted backup.
#[must_use]
pub fn is_mount_root(path: &Path) -> bool {
    // An identity probe that could not be made refuses, as an unreadable cwd does.
    !matches!(mount_root(path), Ok(false)) || !matches!(dotdot_after_missing(path), Ok(false))
}

/// [`is_mount_root`] without its `..` check, which the contract functions make in their own way,
/// and with a path it cannot resolve (a deleted current directory, the empty path) or an identity
/// probe that could not be made kept apart as [`PathError::Undeterminable`], so the contract
/// functions report that fact rather than claim a mount point.
fn mount_root(path: &Path) -> Result<bool, PathError> {
    let abs = absolute(path)?;
    Ok(is_mount_root_abs(&abs) || is_volumes_or_a_child_of_it(&abs)?)
}

/// [`std::path::absolute`], with a failure as [`PathError::Undeterminable`] naming what could not
/// be resolved: the current directory (`.`) for a relative path, since that is what failed, or the
/// empty path itself.
fn absolute(path: &Path) -> Result<PathBuf, PathError> {
    std::path::absolute(path).map_err(|source| PathError::Undeterminable {
        path: if path.as_os_str().is_empty() {
            PathBuf::new()
        } else {
            PathBuf::from(".")
        },
        source,
    })
}

/// Whether `path` is `/Volumes`, or an immediate child of it, **by identity**: the device and
/// inode of the path, or of its parent, are `/Volumes`'s.  This is the authority for `/Volumes`,
/// because the spelling is not: a case-insensitive volume, the firmlink
/// `/System/Volumes/Data/Volumes`, and a symbolic link all reach it under other names.
///
/// Each probe follows links, as a create does, and answers three ways.  An absent `/Volumes` (as
/// on Linux) proves that nothing is it, and an absent path or parent proves that it is not it.
/// Any other failure is [`PathError::Undeterminable`]: a probe that could not be made never
/// proves a path safe.
#[cfg(unix)]
fn is_volumes_or_a_child_of_it(path: &Path) -> Result<bool, PathError> {
    use std::os::unix::fs::MetadataExt;
    let identity = |p: &Path| match fs::metadata(without_trailing_separator(p)) {
        Ok(md) => Ok(Some((md.dev(), md.ino()))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(PathError::Undeterminable {
            path: p.to_path_buf(),
            source,
        }),
    };
    let Some(volumes) = identity(Path::new("/Volumes"))? else {
        return Ok(false);
    };
    for p in [Some(path), path.parent()].into_iter().flatten() {
        if identity(p)? == Some(volumes) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Whether `path`, made absolute, has a `..` after a level that does not exist: the walk up from
/// it meets an absent level ending in `..` before one that exists, as [`floor_check_chain`]'s
/// does (bug-0045).  A path with no `..` is not probed.  Each probe is [`probe_target`]'s, which
/// follows links, as a create does.
fn dotdot_after_missing(path: &Path) -> Result<bool, PathError> {
    let abs = absolute(path)?;
    if !abs.components().any(|c| c == Component::ParentDir) {
        return Ok(false);
    }
    for p in abs.ancestors() {
        match probe_target(p)? {
            Presence::Present(_) => return Ok(false),
            Presence::Absent
                if matches!(p.components().next_back(), Some(Component::ParentDir)) =>
            {
                return Ok(true);
            }
            Presence::Absent => {}
        }
    }
    Ok(false)
}

/// Off Unix there is no device and inode to compare, and on Windows the floor checks no list.
#[cfg(not(unix))]
fn is_volumes_or_a_child_of_it(_path: &Path) -> Result<bool, PathError> {
    Ok(false)
}

/// The matcher on Windows, which checks no list (see [`is_mount_root`]).
#[cfg(windows)]
fn is_mount_root_abs(_path: &Path) -> bool {
    false
}

/// The list, matched by spelling over an already-absolute path: the floor's first pass, which
/// needs no filesystem.  [`is_volumes_or_a_child_of_it`] follows it.
///
/// Split out from [`is_mount_root`] so the (safety-critical) component matching is testable with
/// synthetic paths, without any test having to mutate the process-global current directory.
#[cfg(not(windows))]
fn is_mount_root_abs(path: &Path) -> bool {
    // Normalize to the Normal components, resolving `..`/`.` best-effort.
    let mut comps: Vec<String> = Vec::new();
    for c in path.components() {
        match c {
            Component::Normal(s) => comps.push(s.to_string_lossy().into_owned()),
            Component::ParentDir => {
                comps.pop();
            }
            Component::RootDir | Component::CurDir | Component::Prefix(_) => {}
        }
    }
    let comps: Vec<&str> = comps.iter().map(String::as_str).collect();
    matches!(
        comps.as_slice(),
        // macOS: /Volumes and /Volumes/<name>
        ["Volumes"] | ["Volumes", _]
        // Linux: /mnt and /mnt/<name>
        | ["mnt"] | ["mnt", _]
        // Linux: /media, /media/<user>, /media/<user>/<label> (conservative: over-refusing is safe)
        | ["media"] | ["media", _] | ["media", _, _]
        // Linux: /run/media and one/two levels below
        | ["run", "media"] | ["run", "media", _] | ["run", "media", _, _]
    )
}

/// Refuse if any missing level of the chain is a mount root or a `..`, if the walk reaches a root
/// that is absent (on Windows, a drive or share that is not there), or if the existing ancestor the
/// chain would be built on is not a directory.  Otherwise return the missing levels, absolute, from
/// the one just below that base down to `path`: all that `All`'s create may build.
fn floor_check_chain(path: &Path) -> Result<Vec<PathBuf>, PathError> {
    // Absolutize ONCE, here, and walk the absolute path.  Two reasons, both load-bearing:
    //
    //  1. The floor must see what the OS will resolve, not the caller's spelling (`is_mount_root`).
    //  2. A relative path's ancestor walk ends at the EMPTY path — `Path::new("media").parent()` is
    //     `Some("")` — and `Path::new("").exists()` is false, so the empty path would be pushed onto
    //     `to_create` as though it were a directory to make.  `std::path::absolute("")` is an error,
    //     and the floor fails closed on an error, so every relative chain would be refused as
    //     unresolvable.  An absolute path's walk terminates at its root, so the empty path never
    //     enters the loop.  Do not "simplify" this by absolutizing inside the loop.
    //
    // On Unix the root is `/`, which exists.  On Windows it is a drive or a share, which can be
    // absent, and no create can make it; so an absent root is refused below, as the volume that is
    // not there.
    // Cannot resolve the path at all -> cannot prove it is safe -> refuse (same asymmetry as
    // `is_mount_root`: this gates creation, so Unknown must not license the create).
    let abs = absolute(path)?;

    let mut to_create: Vec<&Path> = Vec::new();
    let mut cur = Some(abs.as_path());
    while let Some(p) = cur {
        // An ancestor we could not inspect is not an ancestor we may assume is missing: treating
        // it as absent would put it on the create list and hand `check` a green verdict for a
        // chain the live create cannot build.  Nor may a file or a dangling link in the
        // way pass as the base, for the same reason: each is refused here, by name.
        match probe_dir(p)? {
            Presence::Present(_) => break, // the existing base of the chain, a directory
            Presence::Absent if p.parent().is_none() => {
                return Err(PathError::MountRoot(p.to_path_buf()));
            }
            Presence::Absent => {
                to_create.push(p);
                cur = p.parent();
            }
        }
    }
    for p in &to_create {
        if is_mount_root_abs(p) || is_volumes_or_a_child_of_it(p)? {
            return Err(PathError::MountRoot(p.to_path_buf()));
        }
    }
    // Every level here is absent, so a `..` among them follows one not yet built, and it and
    // everything after it resolve only once that level exists: the probes above saw `NotFound`
    // where the create would find, say, `/Volumes` (bug-0040, bug-0041).  So the first such level
    // the create would meet is refused, after the floor, so that a mount root is still named as
    // one.  On Windows `absolute` resolves `..` as the file system does, except in a verbatim
    // (`\\?\`) path, where `..` is a literal name that no create can make.
    let dotdot = |p: &&Path| matches!(p.components().next_back(), Some(Component::ParentDir));
    if let Some(p) = to_create.iter().rev().copied().find(dotdot) {
        return Err(PathError::DotDotAfterMissing(p.to_path_buf()));
    }
    Ok(to_create.into_iter().rev().map(Path::to_path_buf).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn tmp(tag: &str) -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("cli_contract_{tag}_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A marker name the test knows is valid.
    fn marker_name(name: &str) -> MarkerName {
        MarkerName::new(name).unwrap()
    }

    /// Restores the previous working directory on drop.
    ///
    /// Load-bearing, not boilerplate: a `#[serial]` test that panics mid-way would otherwise leave
    /// every later test running in a temp dir that is about to be deleted, which fails them for a
    /// reason that has nothing to do with what they assert.  Pair with `#[serial_test::serial]` —
    /// the cwd is process-global, and cargo runs tests in parallel threads.
    struct CwdGuard(PathBuf);

    impl CwdGuard {
        fn set(dir: &Path) -> Self {
            Self::try_set(dir).expect("cwd should be settable")
        }

        fn try_set(dir: &Path) -> Option<Self> {
            let prev = std::env::current_dir().ok()?;
            std::env::set_current_dir(dir).ok()?;
            Some(Self(prev))
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }

    // --- the unreadable-directory fixture ------------------------------------------------------
    //
    // The whole three-state sweep is pinned by one fixture: a readable object inside a `chmod 000`
    // directory, so the probe under test gets EACCES for something that demonstrably exists.

    /// A `chmod 000` directory holding known contents, restored to `0o755` on drop.
    #[cfg(unix)]
    struct LockedDir {
        tmp: tempfile::TempDir,
        locked: PathBuf,
    }

    #[cfg(unix)]
    impl LockedDir {
        /// `locked/` contains: `real.txt` (a file), `dest/` (a directory), `marker` (a marker
        /// file), and `link` (a symlink to /tmp).  All exist; none can be stat'd through the
        /// locked parent.
        fn new() -> Self {
            use std::os::unix::fs::PermissionsExt;
            let tmp = tempfile::tempdir().unwrap();
            let locked = tmp.path().join("locked");
            std::fs::create_dir(&locked).unwrap();
            std::fs::create_dir(locked.join("dest")).unwrap();
            std::fs::write(locked.join("real.txt"), b"content").unwrap();
            std::fs::write(locked.join("marker"), b"m").unwrap();
            std::os::unix::fs::symlink("/tmp", locked.join("link")).unwrap();
            std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
            Self { tmp, locked }
        }
    }

    #[cfg(unix)]
    impl Drop for LockedDir {
        /// Load-bearing, and NOT collapsible boilerplate: `TempDir::drop` cannot remove a
        /// `chmod 000` directory, so without this restore every run of the suite leaves
        /// undeletable litter in the temp dir.  The struct's own `drop` runs before its fields',
        /// so the mode is back to `0o755` by the time `tmp` tries to delete the tree.
        fn drop(&mut self) {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.locked, std::fs::Permissions::from_mode(0o755));
        }
    }

    /// `chmod 000` does not stop root, so every fixture above is vacuous there — the tests would
    /// pass while proving nothing.  Skip loudly rather than silently.
    #[cfg(unix)]
    fn skipped_as_root() -> bool {
        // SAFETY: geteuid is always safe; it reads a process attribute and cannot fail.
        let root = unsafe { libc::geteuid() } == 0;
        if root {
            eprintln!(
                "SKIPPED: running as root, where chmod 000 is a no-op — this test proves nothing"
            );
        }
        root
    }

    /// Assert an error is `Undeterminable`, names the real cause, and — the point of the whole
    /// sweep — no longer claims the thing is absent.
    #[cfg(unix)]
    fn assert_undeterminable(err: &PathError, forbidden_claim: &str) {
        assert!(
            matches!(err, PathError::Undeterminable { .. }),
            "expected Undeterminable, got {err:?}"
        );
        let msg = err.to_string();
        assert!(
            !msg.contains(forbidden_claim),
            "message still claims {forbidden_claim:?}: {msg}"
        );
        assert!(
            msg.contains("Permission denied"),
            "message must name the real cause: {msg}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_trailing_separator_does_not_hide_a_link() {
        // `lstat("link/")` and `lstat("link/.")` follow the link, so every check once read a link
        // spelled either way as whatever it points at, or as absent when it points at nothing.
        let d = tmp("trailing_separator");
        let real = d.join("real");
        std::fs::create_dir(&real).unwrap();
        let ldir = d.join("ldir");
        std::os::unix::fs::symlink(&real, &ldir).unwrap();
        let ghost = d.join("ghost");
        let dang = d.join("dang");
        std::os::unix::fs::symlink(&ghost, &dang).unwrap();
        for suffix in ["/", "/."] {
            let spell = |p: &Path| {
                let mut s = p.as_os_str().to_owned();
                s.push(suffix);
                PathBuf::from(s)
            };
            // The side effect first: no create through the dangling link.
            let policy = DestPolicy::Create(CreateMode::Final);
            assert!(
                matches!(
                    ensure_destination(&spell(&dang), &policy),
                    Err(PathError::DanglingSymlink(_))
                ),
                "{suffix}"
            );
            assert!(
                matches!(std::fs::symlink_metadata(&ghost), Err(e) if e.kind() == std::io::ErrorKind::NotFound),
                "{suffix}: the dangling link's target was created"
            );
            assert!(
                matches!(
                    check_destination(&spell(&dang), &policy),
                    Err(PathError::DanglingSymlink(_))
                ),
                "{suffix}"
            );
            assert!(
                matches!(
                    prepare_output_file(&spell(&dang), CreateMode::None, Clobber::Deny),
                    Err(PathError::DanglingSymlink(_))
                ),
                "{suffix}"
            );
            assert!(
                matches!(
                    require_input_path(&spell(&dang)),
                    Err(PathError::DanglingSymlink(_))
                ),
                "{suffix}"
            );
            assert!(
                matches!(
                    require_input_dir(&spell(&dang)),
                    Err(PathError::DanglingSymlink(_))
                ),
                "{suffix}"
            );
            assert!(
                matches!(reject_symlink(&spell(&ldir)), Err(PathError::IsSymlink(_))),
                "{suffix}"
            );
        }
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_link_to_a_file_spelled_with_a_separator_reports_what_the_bare_link_does() {
        // `stat("flink/.")` fails with ENOTDIR, so once `probe` saw the link, the `stat` after it
        // still answered "cannot determine" where the bare `flink` is plainly not a directory.
        let d = tmp("file_link_separator");
        let file = d.join("real.txt");
        std::fs::write(&file, b"x").unwrap();
        let flink = d.join("flink");
        std::os::unix::fs::symlink(&file, &flink).unwrap();
        let mut spelled = flink.as_os_str().to_owned();
        spelled.push("/.");
        let spelled = PathBuf::from(spelled);
        // Each answer for the spelling must be the bare link's, and the bare link's is a fact.
        let kind = |r: Result<(), PathError>| r.map_err(|e| std::mem::discriminant(&e));
        let policy = DestPolicy::Create(CreateMode::Final);
        let bare = check_destination(&flink, &policy);
        assert!(matches!(bare, Err(PathError::NotADirectory(_))), "{bare:?}");
        assert_eq!(kind(check_destination(&spelled, &policy)), kind(bare));
        let bare = require_input_dir(&flink);
        assert!(matches!(bare, Err(PathError::NotADirectory(_))), "{bare:?}");
        assert_eq!(kind(require_input_dir(&spelled)), kind(bare));
        let bare = prepare_output_file(&flink, CreateMode::None, Clobber::Deny);
        assert!(matches!(bare, Err(PathError::Clobber(_))), "{bare:?}");
        assert_eq!(
            kind(prepare_output_file(
                &spelled,
                CreateMode::None,
                Clobber::Deny
            )),
            kind(bare)
        );
        require_input_path(&spelled).unwrap();
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_dry_run_and_the_live_run_agree_on_a_destination_spelled_with_a_trailing_dot() {
        // The check judges `new/.` as `new`; the create once acted on `new/.` and failed ENOENT.
        let d = tmp("dot_destination");
        let mut spelled = d.join("new").into_os_string();
        spelled.push("/.");
        let spelled = PathBuf::from(spelled);
        let policy = DestPolicy::Create(CreateMode::Final);
        check_destination(&spelled, &policy).unwrap();
        ensure_destination(&spelled, &policy).unwrap();
        assert!(d.join("new").is_dir(), "the live run created nothing");
        std::fs::remove_dir_all(&d).unwrap();
    }

    // --- flag values ------------------------------------------------------------------------

    #[test]
    fn the_flag_values_parse_what_they_print() {
        for mode in [CreateMode::None, CreateMode::Final, CreateMode::All] {
            assert_eq!(mode.to_string().parse::<CreateMode>(), Ok(mode));
        }
        for clobber in [Clobber::Allow, Clobber::Deny] {
            assert_eq!(clobber.to_string().parse::<Clobber>(), Ok(clobber));
        }
        // Case matters, as it does in clap's derive.
        let err = "None".parse::<CreateMode>().unwrap_err();
        assert_eq!(err.to_string(), "'None' is not one of: none, final, all");
        let err = "".parse::<Clobber>().unwrap_err();
        assert_eq!(err.to_string(), "'' is not one of: allow, deny");
    }

    #[cfg(feature = "clap")]
    #[test]
    fn the_flag_values_are_spelled_as_clap_spells_them() {
        use clap::ValueEnum;
        for mode in CreateMode::value_variants() {
            let name = mode.to_possible_value().unwrap().get_name().to_string();
            assert_eq!(mode.to_string(), name);
            assert_eq!(name.parse::<CreateMode>(), Ok(*mode));
        }
        for clobber in Clobber::value_variants() {
            let name = clobber.to_possible_value().unwrap().get_name().to_string();
            assert_eq!(clobber.to_string(), name);
            assert_eq!(name.parse::<Clobber>(), Ok(*clobber));
        }
        // The error's list of spellings is a third copy: it must name every value, too.
        let names = |values: Vec<clap::builder::PossibleValue>| -> Vec<String> {
            values.iter().map(|v| v.get_name().to_string()).collect()
        };
        assert_eq!(
            "".parse::<CreateMode>().unwrap_err().expected,
            names(
                CreateMode::value_variants()
                    .iter()
                    .filter_map(|v| v.to_possible_value())
                    .collect()
            )
        );
        assert_eq!(
            "".parse::<Clobber>().unwrap_err().expected,
            names(
                Clobber::value_variants()
                    .iter()
                    .filter_map(|v| v.to_possible_value())
                    .collect()
            )
        );
    }

    // --- the mount-root floor ---------------------------------------------------------------

    #[cfg(not(windows))]
    #[test]
    fn mount_roots_are_recognized() {
        for p in [
            "/Volumes",
            "/Volumes/Backup",
            "/mnt",
            "/mnt/usb",
            "/media",
            "/media/alice",
            "/media/alice/USBDRIVE",
            "/run/media",
            "/run/media/alice",
            "/run/media/alice/USBDRIVE",
        ] {
            assert!(is_mount_root(Path::new(p)), "should be a mount root: {p}");
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn paths_below_a_mount_point_are_not_mount_roots() {
        for p in [
            "/Volumes/Backup/proj",
            "/Volumes/Backup/proj/sub",
            "/mnt/usb/data",
            "/media/alice/USBDRIVE/data",
            "/run/media/alice/USBDRIVE/data",
            "/Users/alice/Backups/proj",
            "/tmp/whatever",
        ] {
            assert!(
                !is_mount_root(Path::new(p)),
                "should NOT be a mount root: {p}"
            );
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn an_output_file_named_for_a_mount_point_is_refused() {
        // Writing the file would create the mount point, as a file: refused in every mode.
        for p in ["/Volumes/NoSuchVolume_cc_out", "/mnt/no_such_mount_cc_out"] {
            let p = Path::new(p);
            assert!(!p.exists(), "the test needs {p:?} absent");
            for mode in [CreateMode::None, CreateMode::Final, CreateMode::All] {
                for clobber in [Clobber::Allow, Clobber::Deny] {
                    let got = prepare_output_file(p, mode, clobber);
                    assert!(
                        matches!(&got, Err(PathError::MountRoot(q)) if q == p),
                        "{p:?} {mode} {clobber}: {got:?}"
                    );
                }
            }
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn dotdot_is_normalized_before_the_floor_check() {
        // /Volumes/../etc resolves to /etc, which is not a mount root.  Where `/Volumes` is
        // absent, as on Linux, that `..` follows a level that does not exist, and the floor
        // refuses it (bug-0045).
        assert!(!is_mount_root_abs(Path::new("/Volumes/../etc")));
        assert_eq!(
            is_mount_root(Path::new("/Volumes/../etc")),
            cfg!(not(target_os = "macos"))
        );
    }

    // --- the floor compares what a path means from the current directory, not its text ------
    //
    // `is_mount_root` used to match a relative path's components as though they began at `/`.
    // Both directions were wrong: `media/uploads` (an everyday project dir) was refused, and a
    // relative path that really did resolve into /Volumes was NOT refused — a floor bypass.  The
    // matcher is now split, so most of this is testable without touching the process-global cwd.

    #[cfg(not(windows))]
    #[test]
    fn the_matcher_takes_absolute_paths_only() {
        // is_mount_root_abs is the raw comparison; it is only ever handed absolute paths.
        assert!(is_mount_root_abs(Path::new("/Volumes/Backup")));
        assert!(!is_mount_root_abs(Path::new("/Users/alice/media/uploads")));
        // The bug in miniature: as TEXT these look like mount roots; as absolute paths they are
        // ordinary project directories, and that is what the OS will create.
        assert!(!is_mount_root_abs(Path::new(
            "/Users/alice/proj/media/uploads"
        )));
        assert!(!is_mount_root_abs(Path::new("/Users/alice/proj/mnt")));
        assert!(!is_mount_root_abs(Path::new(
            "/Users/alice/proj/Volumes/Backup"
        )));
    }

    #[test]
    #[serial_test::serial] // set_current_dir is process-global
    fn relative_paths_are_resolved_against_cwd_before_the_floor_check() {
        // Direction 1 — false refusal.  A project-relative media/ dir is not a mount root.
        let d = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::set(d.path());

        assert!(!is_mount_root(Path::new("media/uploads")));
        assert!(!is_mount_root(Path::new("mnt")));
        assert!(!is_mount_root(Path::new("Volumes/Backup")));
        assert!(!is_mount_root(Path::new("run/media/x")));

        // And the contract functions stop refusing it: the parent (`media`) is genuinely missing,
        // so Final reports THAT — not a bogus mount root.
        assert!(matches!(
            check_destination(
                Path::new("media/uploads"),
                &DestPolicy::Create(CreateMode::Final)
            ),
            Err(PathError::MissingParent { .. })
        ));
    }

    #[test]
    #[serial_test::serial]
    fn a_relative_all_chain_is_not_refused_as_an_empty_mount_root() {
        // Regression guard for the trap the fix sets.  A relative path's ancestor walk ends at the
        // EMPTY path, `absolute("")` is an error, and `is_mount_root` fails closed on an error — so
        // a floor_check_chain that absolutized per-component (instead of once, up front) would
        // refuse EVERY relative All chain as MountRoot("").  Verified failing before the fix.
        let d = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::set(d.path());

        check_destination(
            Path::new("media/uploads"),
            &DestPolicy::Create(CreateMode::All),
        )
        .expect("a relative All chain in an ordinary cwd must be allowed");
        ensure_destination(
            Path::new("media/uploads"),
            &DestPolicy::Create(CreateMode::All),
        )
        .expect("and the live half must create it");
        assert!(d.path().join("media/uploads").is_dir());
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[serial_test::serial]
    fn a_relative_path_resolving_into_volumes_is_floored() {
        // Direction 2 — the floor BYPASS, the dangerous half of comparing text.  A cwd of /Volumes
        // plus a relative destination used to launder a mount-point creation past the floor.
        // check_destination only checks — nothing is created, so this is safe to run.
        let Some(_cwd) = CwdGuard::try_set(Path::new("/Volumes")) else {
            return; // /Volumes unavailable: nothing to prove here
        };

        assert!(is_mount_root(Path::new("NoSuchVolume_cli_contract_test")));
        assert!(matches!(
            check_destination(
                Path::new("NoSuchVolume_cli_contract_test"),
                &DestPolicy::Create(CreateMode::All),
            ),
            Err(PathError::MountRoot(_))
        ));
        // `Final` too: its bare-name arm must come after the floor, or a bare name here passes.
        assert!(matches!(
            check_destination(
                Path::new("NoSuchVolume_cli_contract_test"),
                &DestPolicy::Create(CreateMode::Final),
            ),
            Err(PathError::MountRoot(_))
        ));
        // Nothing was created by the check.
        assert!(!Path::new("/Volumes/NoSuchVolume_cli_contract_test").exists());
    }

    // --- the floor knows `/Volumes` by identity, not by spelling ------------------------------
    //
    // The list matched the spelling `Volumes`, so any other name for that directory passed, and
    // `Final` or `All` would create a mount point through it.  These are checks: nothing is
    // created under `/Volumes`.

    /// Assert that `mount_point`, another name for a `/Volumes/<name>`, is refused as a mount root
    /// by `is_mount_root`, by `Final`, and by `All` building below it.
    #[cfg(target_os = "macos")]
    fn assert_floored(mount_point: &Path) {
        assert!(is_mount_root(mount_point), "{}", mount_point.display());
        let final_ = check_destination(mount_point, &DestPolicy::Create(CreateMode::Final));
        assert!(
            matches!(&final_, Err(PathError::MountRoot(p)) if p == mount_point),
            "Final: {final_:?}"
        );
        let all = check_destination(
            &mount_point.join("proj"),
            &DestPolicy::Create(CreateMode::All),
        );
        assert!(
            matches!(&all, Err(PathError::MountRoot(p)) if p == mount_point),
            "All: {all:?}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_floor_knows_volumes_under_another_spelling() {
        // A case-insensitive volume answers to `/volumes`, and the firmlink
        // `/System/Volumes/Data/Volumes` is the same directory.
        use std::os::unix::fs::MetadataExt;
        // Only a proven absence, or a different directory, skips a spelling; any other stat error
        // fails the test, so it cannot pass having proven nothing.
        let identity = |p: &str| match std::fs::metadata(p) {
            Ok(md) => Some((md.dev(), md.ino())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => panic!("cannot stat {p}: {e}"),
        };
        let volumes = identity("/Volumes").expect("/Volumes exists on macOS");
        let mut proved = 0;
        for spelling in ["/volumes", "/VOLUMES", "/System/Volumes/Data/Volumes"] {
            if identity(spelling) != Some(volumes) {
                eprintln!("SKIPPED {spelling}: not /Volumes on this machine, so nothing to prove");
                continue;
            }
            assert!(is_mount_root(Path::new(spelling)), "{spelling}");
            assert_floored(&Path::new(spelling).join("NoSuchVolume_cli_contract_test"));
            proved += 1;
        }
        // The firmlink exists on every macOS since 10.15, so at least one spelling is proved.
        assert!(proved > 0, "no spelling of /Volumes was checked");
        assert!(matches!(
            std::fs::symlink_metadata("/Volumes/NoSuchVolume_cli_contract_test"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_floor_knows_volumes_through_a_link_to_it() {
        // A create through a link to `/Volumes` makes its directory there.  Only the link is
        // created, in a temp dir.
        let d = tempfile::tempdir().unwrap();
        let vols = d.path().join("vols");
        std::os::unix::fs::symlink("/Volumes", &vols).unwrap();
        assert_floored(&vols.join("NoSuchVolume_cli_contract_test"));
        assert!(matches!(
            std::fs::symlink_metadata("/Volumes/NoSuchVolume_cli_contract_test"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_identity_check_takes_a_file_spelled_with_a_separator_as_the_file() {
        // `stat("file/")` fails with ENOTDIR, which would refuse a path the bare name clears.
        let d = tempfile::tempdir().unwrap();
        let file = d.path().join("file");
        std::fs::write(&file, b"x").unwrap();
        assert!(!is_mount_root(&file));
        for suffix in ["/", "/."] {
            let mut spelled = file.as_os_str().to_owned();
            spelled.push(suffix);
            assert!(!is_mount_root(Path::new(&spelled)), "{suffix}");
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[serial_test::serial] // set_current_dir is process-global
    fn the_floor_refuses_a_path_it_cannot_tell_from_volumes() {
        // In a cwd near PATH_MAX (1024 on macOS), a relative name is provably absent, yet its
        // absolute path is too long to stat, so the floor's identity check cannot answer.  It
        // refuses, and says why rather than claim a mount point.  Under `Final` the identity check
        // is what refuses; under `All` the chain's `probe_dir` on the absolute path refuses first,
        // with the same errno, so only the `Final` half pins the identity check.
        let tmp = tempfile::tempdir().unwrap();
        let mut deep = tmp.path().canonicalize().unwrap();
        while deep.as_os_str().len() < 800 {
            deep.push("d".repeat(200));
        }
        std::fs::create_dir_all(&deep).unwrap();
        let _cwd = CwdGuard::set(&deep);
        let name = "n".repeat(1030 - deep.as_os_str().len());
        let dest = Path::new(&name);
        assert!(is_mount_root(dest));
        for mode in [CreateMode::Final, CreateMode::All] {
            let got = check_destination(dest, &DestPolicy::Create(mode));
            assert!(
                matches!(&got, Err(PathError::Undeterminable { path, source })
                    if path == &deep.join(&name)
                        && source.raw_os_error() == Some(libc::ENAMETOOLONG)),
                "{mode}: {got:?}"
            );
        }
        assert!(matches!(
            std::fs::symlink_metadata(dest),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound
        ));
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial]
    fn the_floor_refuses_when_the_current_directory_cannot_be_read() {
        // A deleted cwd cannot be resolved, so a relative path in it cannot be proven safe: the
        // floor refuses it, under `Final` as under `All`, rather than let the create find out.
        let d = tmp("deleted_cwd");
        let gone = d.join("gone");
        std::fs::create_dir(&gone).unwrap();
        let cwd = CwdGuard::set(&gone);
        std::fs::remove_dir(&gone).unwrap();
        let dest = Path::new("newdir");
        assert!(is_mount_root(dest));
        // Refused as what it is, a current directory that cannot be resolved, never as a mount
        // point, and never as a fact about `newdir`, which was proven absent.
        for mode in [CreateMode::Final, CreateMode::All] {
            let got = ensure_destination(dest, &DestPolicy::Create(mode));
            assert!(
                matches!(&got, Err(PathError::Undeterminable { path, .. }) if path == Path::new(".")),
                "{mode}: {got:?}"
            );
        }
        drop(cwd);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn an_empty_output_path_is_undeterminable_in_every_mode() {
        // The empty path names nothing, and the floor cannot resolve it.
        let p = Path::new("");
        for mode in [CreateMode::None, CreateMode::Final, CreateMode::All] {
            for clobber in [Clobber::Allow, Clobber::Deny] {
                let got = prepare_output_file(p, mode, clobber);
                assert!(
                    matches!(&got, Err(PathError::Undeterminable { path, .. }) if path == p),
                    "{mode} {clobber}: {got:?}"
                );
            }
        }
    }

    // --- ensure_destination: the behavior table ---------------------------------------------

    #[test]
    fn existing_dir_is_ok_in_every_mode() {
        let d = tmp("exists");
        for mode in [CreateMode::None, CreateMode::Final, CreateMode::All] {
            assert!(ensure_destination(&d, &DestPolicy::Create(mode)).is_ok());
        }
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn none_refuses_a_missing_dir_and_creates_nothing() {
        let d = tmp("none");
        let dest = d.join("proj");
        let err = ensure_destination(&dest, &DestPolicy::Create(CreateMode::None)).unwrap_err();
        assert!(matches!(err, PathError::MissingDestination(_)));
        assert!(!dest.exists(), "must not create anything under None");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn final_creates_the_leaf_when_the_parent_exists() {
        let d = tmp("final_ok");
        let dest = d.join("proj");
        ensure_destination(&dest, &DestPolicy::Create(CreateMode::Final)).unwrap();
        assert!(dest.is_dir());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn final_refuses_when_the_parent_is_missing() {
        let d = tmp("final_noparent");
        let dest = d.join("missing").join("proj");
        let err = ensure_destination(&dest, &DestPolicy::Create(CreateMode::Final)).unwrap_err();
        assert!(matches!(err, PathError::MissingParent { .. }));
        assert!(!dest.exists());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn all_creates_the_whole_chain() {
        let d = tmp("all");
        let dest = d.join("a").join("b").join("c");
        ensure_destination(&dest, &DestPolicy::Create(CreateMode::All)).unwrap();
        assert!(dest.is_dir());
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// Both halves of the contract, so a test asserts one verdict for the dry run and the live run.
    type Half = fn(&Path, &DestPolicy) -> Result<(), PathError>;
    const HALVES: [(&str, Half); 2] =
        [("check", check_destination), ("ensure", ensure_destination)];

    #[test]
    fn a_file_blocking_an_all_chain_is_named_by_check_and_ensure_alike() {
        // A file partway up the chain was never named: the check took it for the chain's base and
        // said Ok while the live run failed in `create_dir_all`, and later both answered "cannot
        // determine" (ENOTDIR at the leaf).  The leaf's probe now names the file first, on every
        // platform, before the chain walk reaches it.
        let d = tmp("all_file_blocker");
        let blocker = d.join("blocker");
        std::fs::write(&blocker, b"i am a file").unwrap();
        let dest = blocker.join("sub").join("dest");
        for (half, run) in HALVES {
            let got = run(&dest, &DestPolicy::Create(CreateMode::All));
            assert!(
                matches!(&got, Err(PathError::NotADirectory(p)) if p == &blocker),
                "{half}: {got:?}"
            );
        }
        assert_eq!(std::fs::read(&blocker).unwrap(), b"i am a file");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_link_in_an_all_chain_is_followed_or_named_by_check_and_ensure_alike() {
        // A dangling link was taken for absent, so the check said Ok and the live run failed with
        // "File exists" naming a path below the link; a link to a file got "cannot determine".
        use std::os::unix::fs::symlink;
        let d = tmp("all_link_blocker");
        let gone = d.join("gone");
        let dang = d.join("dang");
        symlink(&gone, &dang).unwrap();
        let file = d.join("real.txt");
        std::fs::write(&file, b"x").unwrap();
        let flink = d.join("flink");
        symlink(&file, &flink).unwrap();
        let real = d.join("real");
        std::fs::create_dir(&real).unwrap();
        let ldir = d.join("ldir");
        symlink(&real, &ldir).unwrap();
        let all = DestPolicy::Create(CreateMode::All);
        for (half, run) in HALVES {
            let got = run(&dang.join("sub").join("dest"), &all);
            assert!(
                matches!(&got, Err(PathError::DanglingSymlink(p)) if p == &dang),
                "{half}: {got:?}"
            );
            let got = run(&flink.join("sub").join("dest"), &all);
            assert!(
                matches!(&got, Err(PathError::NotADirectory(p)) if p == &flink),
                "{half}: {got:?}"
            );
            // A link to a directory is still a usable base.
            run(&ldir.join("sub").join("dest"), &all).unwrap();
        }
        assert!(
            matches!(std::fs::symlink_metadata(&gone), Err(e) if e.kind() == std::io::ErrorKind::NotFound),
            "the dangling link's target was created"
        );
        assert!(real.join("sub").join("dest").is_dir());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn final_reports_a_file_parent_as_a_file_not_as_missing() {
        // A parent that exists as a file was `MissingParent`, "does not exist", and later "cannot
        // determine" (ENOTDIR at the leaf); neither is the file that is there.  The leaf's probe
        // now names the file first, on every platform, before the parent probe reaches it.
        let d = tmp("final_file_parent");
        let parent = d.join("parent");
        std::fs::write(&parent, b"file").unwrap();
        let dest = parent.join("dest");
        for (half, run) in HALVES {
            let err = run(&dest, &DestPolicy::Create(CreateMode::Final)).unwrap_err();
            assert!(
                matches!(&err, PathError::NotADirectory(p) if p == &parent),
                "{half}: {err:?}"
            );
            assert!(!err.to_string().contains("does not exist"), "{half}: {err}");
        }
        assert_eq!(std::fs::read(&parent).unwrap(), b"file");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn final_reports_a_link_parent_as_what_it_is() {
        // A dangling link in the parent's place was `MissingParent`, "does not exist"; a link to a
        // file got "cannot determine".
        use std::os::unix::fs::symlink;
        let d = tmp("final_link_parent");
        let gone = d.join("gone");
        let dang = d.join("dang");
        symlink(&gone, &dang).unwrap();
        let file = d.join("real.txt");
        std::fs::write(&file, b"x").unwrap();
        let flink = d.join("flink");
        symlink(&file, &flink).unwrap();
        let real = d.join("real");
        std::fs::create_dir(&real).unwrap();
        let ldir = d.join("ldir");
        symlink(&real, &ldir).unwrap();
        let final_ = DestPolicy::Create(CreateMode::Final);
        for (half, run) in HALVES {
            let got = run(&dang.join("dest"), &final_);
            assert!(
                matches!(&got, Err(PathError::DanglingSymlink(p)) if p == &dang),
                "{half}: {got:?}"
            );
            let got = run(&flink.join("dest"), &final_);
            assert!(
                matches!(&got, Err(PathError::NotADirectory(p)) if p == &flink),
                "{half}: {got:?}"
            );
            // A link to a directory is still a usable parent.
            run(&ldir.join("dest"), &final_).unwrap();
        }
        assert!(
            matches!(std::fs::symlink_metadata(&gone), Err(e) if e.kind() == std::io::ErrorKind::NotFound),
            "the dangling link's target was created"
        );
        assert!(real.join("dest").is_dir());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    #[serial_test::serial] // set_current_dir is process-global
    fn final_accepts_a_bare_relative_destination() {
        // `Path::new("newdir").parent()` is the empty path, the current directory, and `Final` once
        // refused it as a missing parent, while `All` and `./newdir` were accepted.
        let d = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::set(d.path());
        let final_ = DestPolicy::Create(CreateMode::Final);
        check_destination(Path::new("newdir"), &final_).unwrap();
        assert!(!d.path().join("newdir").exists(), "the check created it");
        ensure_destination(Path::new("newdir"), &final_).unwrap();
        assert!(
            d.path().join("newdir").is_dir(),
            "the live run created nothing"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn the_floor_blocks_even_all_and_touches_no_filesystem() {
        // A nonexistent mount point: even All must refuse, without creating anything.  /Volumes is
        // root-owned so this is safe regardless, but the floor rejects before any write is attempted.
        let dest = Path::new("/Volumes/NoSuchTestVolume_cli_contract/proj");
        let err = ensure_destination(dest, &DestPolicy::Create(CreateMode::All)).unwrap_err();
        assert!(matches!(err, PathError::MountRoot(_)));
        assert!(!Path::new("/Volumes/NoSuchTestVolume_cli_contract").exists());
    }

    // --- the marker guard + policy validation ------------------------------------------------

    #[test]
    fn marker_present_passes_marker_absent_fails() {
        let d = tmp("marker");
        let policy = DestPolicy::Marked(marker_name("~Bak~"));
        assert!(matches!(
            ensure_destination(&d, &policy).unwrap_err(),
            PathError::MarkerMissing { .. }
        ));
        std::fs::create_dir(d.join("~Bak~")).unwrap();
        assert!(ensure_destination(&d, &policy).is_ok());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn marked_policy_never_creates_a_missing_dir() {
        let d = tmp("marked_noexist");
        let dest = d.join("proj");
        let err = ensure_destination(&dest, &DestPolicy::Marked(marker_name("~Bak~"))).unwrap_err();
        assert!(matches!(err, PathError::MissingDestination(_)));
        assert!(!dest.exists(), "a marked policy must never create the dir");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn from_flags_rejects_a_marker_with_a_creating_mode() {
        // marker + none (or the default) -> Marked
        assert_eq!(
            DestPolicy::from_flags(CreateMode::None, Some(marker_name("~Bak~"))).unwrap(),
            DestPolicy::Marked(marker_name("~Bak~"))
        );
        // marker + final|all -> a usage error (exit 2), not a silently-dropped mode
        for mode in [CreateMode::Final, CreateMode::All] {
            let err = DestPolicy::from_flags(mode, Some(marker_name("~Bak~"))).unwrap_err();
            assert_eq!(err, PolicyError::MarkerWithCreate);
            assert_eq!(err.exit_code(), crate::exit::USAGE);
        }
        // no marker -> Create(mode)
        assert_eq!(
            DestPolicy::from_flags(CreateMode::All, None).unwrap(),
            DestPolicy::Create(CreateMode::All)
        );
    }

    // --- input paths ------------------------------------------------------------------------

    #[test]
    fn require_input_path_names_a_missing_path_and_creates_nothing() {
        let d = tmp("input");
        let missing = d.join("nope");
        assert!(matches!(
            require_input_path(&missing).unwrap_err(),
            PathError::MissingInput(_)
        ));
        assert!(!missing.exists());
        assert!(require_input_path(&d).is_ok());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn require_marker_refuses_what_require_input_dir_refuses_and_a_missing_marker() {
        // `require_marker` stands in for `require_input_dir` on an input that takes a marker, so
        // it must refuse a missing directory and a file as that does, and then the marker.
        let d = tmp("input_marker");
        let audio = marker_name("audio");
        let file = d.join("afile");
        std::fs::write(&file, b"x").unwrap();
        let kind = |r: Result<(), PathError>| r.map_err(|e| std::mem::discriminant(&e));
        for p in [d.join("nope"), file] {
            let got = require_input_dir(&p);
            assert!(got.is_err(), "{}: {got:?}", p.display());
            assert_eq!(
                kind(require_marker(&p, &audio)),
                kind(got),
                "{}",
                p.display()
            );
        }
        assert!(require_input_dir(&d).is_ok());
        assert!(matches!(
            require_marker(&d, &audio).unwrap_err(),
            PathError::MarkerMissing { .. }
        ));
        std::fs::remove_dir_all(&d).unwrap();
    }

    // --- "I could not look" is never "it is not there" ---------------------------------------
    //
    // Every test here asserts on the CLAIM, not on the exit code or the mere presence of an Err.
    // Most of these sites already returned an error before the fix — with a false message — so
    // `is_err()` would pass both before and after and prove nothing.

    #[cfg(unix)]
    #[test]
    fn require_input_path_does_not_claim_absence_for_a_path_it_could_not_read() {
        if skipped_as_root() {
            return;
        }
        let f = LockedDir::new();
        // The file is right there, and readable.  The only thing we lack is permission to stat it.
        let err = require_input_path(&f.locked.join("real.txt")).unwrap_err();
        assert_undeterminable(&err, "does not exist");
    }

    #[cfg(unix)]
    #[test]
    fn require_input_dir_does_not_claim_absence_for_a_dir_it_could_not_read() {
        if skipped_as_root() {
            return;
        }
        let f = LockedDir::new();
        let err = require_input_dir(&f.locked.join("dest")).unwrap_err();
        assert_undeterminable(&err, "does not exist");
    }

    #[cfg(unix)]
    #[test]
    fn a_link_whose_target_cannot_be_read_is_not_reported_as_dangling() {
        // The dangling check is itself three-state: a link in a READABLE directory pointing at a
        // target under a locked one used to report "its target does not exist" for a target that
        // does.  The collapse simply moved one level deeper if the target probe stayed a bool.
        if skipped_as_root() {
            return;
        }
        let f = LockedDir::new();
        let link = f.tmp.path().join("link-into-locked");
        std::os::unix::fs::symlink(f.locked.join("real.txt"), &link).unwrap();

        let err = require_input_path(&link).unwrap_err();
        assert_undeterminable(&err, "target does not exist");
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_loop_is_undeterminable_not_dangling() {
        // ELOOP is not a missing target.  This one needs no root guard: a loop stops root too.
        let d = tempfile::tempdir().unwrap();
        let (a, b) = (d.path().join("a"), d.path().join("b"));
        std::os::unix::fs::symlink(&b, &a).unwrap();
        std::os::unix::fs::symlink(&a, &b).unwrap();

        let err = require_input_path(&a).unwrap_err();
        assert!(
            matches!(err, PathError::Undeterminable { .. }),
            "expected Undeterminable, got {err:?}"
        );
        let msg = err.to_string();
        assert!(!msg.contains("does not exist"), "loop misreported: {msg}");
    }

    #[cfg(unix)]
    #[test]
    fn check_destination_refuses_a_destination_it_could_not_inspect_in_every_mode() {
        // Sites 1-3.  `dest` EXISTS inside the locked dir — that is what makes the EACCES arrive at
        // the probe under test rather than at some earlier step failing for its own reason (the
        // masking abort).  Before the fix: None claimed "does not exist"; Final and All returned
        // Ok(()) and left ensure_destination to fail on its own — the dry run out-promising the
        // live run, which is exactly what these two functions exist to rule out.
        if skipped_as_root() {
            return;
        }
        let f = LockedDir::new();
        let dest = f.locked.join("dest");

        for mode in [CreateMode::None, CreateMode::Final, CreateMode::All] {
            let err = check_destination(&dest, &DestPolicy::Create(mode))
                .expect_err("check must not bless a destination it could not inspect");
            assert_undeterminable(&err, "does not exist");
        }
        let err = check_destination(&dest, &DestPolicy::Marked(marker_name("marker"))).unwrap_err();
        assert_undeterminable(&err, "does not exist");
    }

    #[cfg(unix)]
    #[test]
    fn check_and_ensure_still_agree_when_the_destination_cannot_be_inspected() {
        // The other half of sites 2-3: check and ensure must reach the same verdict, and now do.
        if skipped_as_root() {
            return;
        }
        let f = LockedDir::new();
        let dest = f.locked.join("dest");
        for mode in [CreateMode::None, CreateMode::Final, CreateMode::All] {
            let policy = DestPolicy::Create(mode);
            assert!(check_destination(&dest, &policy).is_err());
            assert!(
                ensure_destination(&dest, &policy).is_err(),
                "ensure disagreed with check for {mode:?}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn require_marker_does_not_claim_a_present_marker_is_missing() {
        // Site 4.  The marker is right there; the claim "is missing its required marker" is false,
        // and this is the wrong-disk guard, so a false claim here is expensive.
        if skipped_as_root() {
            return;
        }
        let f = LockedDir::new();
        let err = require_marker(&f.locked, &marker_name("marker")).unwrap_err();
        assert_undeterminable(&err, "missing");
    }

    #[cfg(unix)]
    #[test]
    fn reject_symlink_fails_closed_on_a_path_it_could_not_lstat() {
        // Site 5, the highest-value item in the sweep: the crate's official "output must land at a
        // real path" guard used to APPROVE the path it was asked to distrust, returning Ok(()) for
        // a link it could not see.  That is "could not look" read as "not there", in the crate
        // written to prevent it.
        if skipped_as_root() {
            return;
        }
        let f = LockedDir::new();
        let err = reject_symlink(&f.locked.join("link"))
            .expect_err("cannot-inspect must never be approved as not-a-symlink");
        assert!(
            matches!(err, PathError::Undeterminable { .. }),
            "expected Undeterminable, got {err:?}"
        );

        // A proven-absent path still passes through: it is not a symlink, and the must-exist
        // checks own that report.
        assert!(reject_symlink(&f.tmp.path().join("nope")).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn output_file_deny_does_not_skip_its_refusal_for_a_file_it_could_not_stat() {
        // Site 7.  Before the fix this returned Ok(()): the file could not be stat'd, so `exists()`
        // said false, so Deny never fired — and the parent check passed, because the locked dir
        // itself stats fine from outside.  A no-clobber guard that silently approves is worse than
        // no guard, because the caller composed it in order to stop checking.
        if skipped_as_root() {
            return;
        }
        let f = LockedDir::new();
        let err = prepare_output_file(&f.locked.join("real.txt"), CreateMode::None, Clobber::Deny)
            .expect_err("Deny must not pass a file it could not inspect");
        assert!(
            matches!(err, PathError::Undeterminable { .. }),
            "expected Undeterminable, got {err:?}"
        );
    }

    #[test]
    fn a_chain_print_names_the_errno_once() {
        // The variant exists to name the real cause, and its message does; a chain print (what
        // anyhow's `{:#}` does) must not print it a second time from `Error::source`.
        use std::error::Error;
        let errno = || std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        for err in [
            PathError::Undeterminable {
                path: PathBuf::from("/x"),
                source: errno(),
            },
            PathError::Io {
                path: PathBuf::from("/x"),
                source: errno(),
            },
        ] {
            let mut chain = err.to_string();
            let mut cause = err.source();
            while let Some(c) = cause {
                chain = format!("{chain}: {c}");
                cause = c.source();
            }
            assert_eq!(
                chain.to_lowercase().matches("permission denied").count(),
                1,
                "{chain}"
            );
            assert!(err.source().is_none());
            assert_eq!(err.exit_code(), crate::exit::TOOL_ERROR);
        }
    }

    // --- output files -----------------------------------------------------------------------

    #[test]
    fn output_file_deny_refuses_an_existing_file() {
        let d = tmp("clobber");
        let f = d.join("out.pdf");
        std::fs::write(&f, b"old").unwrap();
        assert!(matches!(
            prepare_output_file(&f, CreateMode::None, Clobber::Deny).unwrap_err(),
            PathError::Clobber(_)
        ));
        // Allow lets it through (the file is not written by this call).
        assert!(prepare_output_file(&f, CreateMode::None, Clobber::Allow).is_ok());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn output_file_reports_a_dangling_link_at_its_path() {
        // stat sees nothing at a dangling link, so it once passed as absent under both policies,
        // and the write then landed, or failed, at a target nothing had checked.
        let d = tmp("outfile_dangling");
        let link = d.join("out.pdf");
        std::os::unix::fs::symlink(d.join("no_such_dir").join("real.pdf"), &link).unwrap();
        for clobber in [Clobber::Allow, Clobber::Deny] {
            let err = prepare_output_file(&link, CreateMode::None, clobber).unwrap_err();
            assert!(
                matches!(&err, PathError::DanglingSymlink(p) if p == &link),
                "{clobber:?}: expected DanglingSymlink({}), got {err:?}",
                link.display()
            );
        }
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn ensure_destination_does_not_promise_a_directory_that_vanished_after_its_check() {
        // A race no test can time: the check passed, then the directory went.  The step after
        // the check must report it, not return the Ok that promises a directory.
        let d = tmp("vanished");
        let gone = d.join("gone");
        let file = d.join("afile");
        std::fs::write(&file, b"x").unwrap();
        // The report's own trigger: a destination linked onto a volume that has since unmounted.
        let link = d.join("link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(d.join("unmounted").join("x"), &link).unwrap();
        for policy in [
            DestPolicy::Marked(marker_name("m")),
            DestPolicy::Create(CreateMode::None),
        ] {
            let err = create_after_check(&gone, &policy).unwrap_err();
            assert!(
                matches!(&err, PathError::MissingDestination(p) if p == &gone),
                "{policy:?}: expected MissingDestination({}), got {err:?}",
                gone.display()
            );
            let err = create_after_check(&file, &policy).unwrap_err();
            assert!(
                matches!(&err, PathError::NotADirectory(p) if p == &file),
                "{policy:?}: expected NotADirectory({}), got {err:?}",
                file.display()
            );
            if cfg!(unix) {
                let err = create_after_check(&link, &policy).unwrap_err();
                assert!(
                    matches!(&err, PathError::DanglingSymlink(p) if p == &link),
                    "{policy:?}: expected DanglingSymlink({}), got {err:?}",
                    link.display()
                );
            }
        }
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn ensure_destination_after_its_check_fails_closed_on_a_directory_it_cannot_inspect() {
        if skipped_as_root() {
            return;
        }
        let fx = LockedDir::new();
        let dest = fx.locked.join("dest");
        for policy in [
            DestPolicy::Marked(marker_name("m")),
            DestPolicy::Create(CreateMode::None),
        ] {
            assert_undeterminable(
                &create_after_check(&dest, &policy).unwrap_err(),
                "does not exist",
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_destination_gone_after_its_check_meets_the_floor_before_any_create() {
        // The check passes a destination that exists without asking the floor, so one that then
        // vanished (an unmount) was created with no floor at all: as root, the mount point itself.
        // With the fix reverted, only the permissions on the parents stop that create, so the test
        // runs only where they would: it is skipped wherever a parent is writable, root included.
        use std::os::unix::ffi::OsStrExt;
        let writable = |p: &Path| {
            let nearest = p.ancestors().find(|a| std::fs::symlink_metadata(a).is_ok());
            let c = std::ffi::CString::new(nearest.unwrap().as_os_str().as_bytes()).unwrap();
            // SAFETY: `c` is a valid NUL-terminated path; access(2) only reads it.
            unsafe { libc::access(c.as_ptr(), libc::W_OK) == 0 }
        };
        let mount_point = Path::new("/Volumes/cli_contract_no_such");
        let base = Path::new("/mnt/cli_contract_no_such");
        if writable(mount_point) || writable(base) {
            eprintln!(
                "SKIPPED: a parent is writable here, so a regression would create a mount point"
            );
            return;
        }
        let absent = |p: &Path| matches!(std::fs::symlink_metadata(p), Err(e) if e.kind() == std::io::ErrorKind::NotFound);
        assert!(
            absent(mount_point),
            "{} is not provably absent",
            mount_point.display()
        );
        assert!(absent(base), "{} is not provably absent", base.display());
        // The destination is the mount point, gone after a check that saw it; and a destination
        // below a base that is gone by create time.  Collect every answer before judging any, so
        // the side effect is checked even when a regression returned Ok.
        let results = [
            (
                CreateMode::Final,
                create_after_check(mount_point, &DestPolicy::Create(CreateMode::Final)),
            ),
            (
                CreateMode::All,
                create_after_check(mount_point, &DestPolicy::Create(CreateMode::All)),
            ),
        ];
        let below = create_after_check(&base.join("sub"), &DestPolicy::Create(CreateMode::All));
        assert!(
            absent(mount_point) && absent(base),
            "a mount point is not provably absent after the calls"
        );
        for (mode, result) in results {
            assert!(
                matches!(&result, Err(PathError::MountRoot(p)) if p == mount_point),
                "{mode:?}: {result:?}"
            );
        }
        assert!(
            matches!(&below, Err(PathError::MountRoot(p)) if p == base),
            "{below:?}"
        );
    }

    #[test]
    fn all_does_not_build_again_a_base_gone_since_its_check() {
        // A race no test can time: the check found the base, then it went (an unmount).
        // `create_dir_all` creates from the leaf upward, so it built the base again; as root, a
        // mount point.  Only the approved levels may be created, and the first now has no parent.
        // This calls the create step directly, since no test can time the race; what pins `All`'s
        // use of it in `create_after_check` is clippy's dead-code lint (CI runs `-D warnings`),
        // which fires if that arm goes back to `create_dir_all`.
        let d = tmp("all_base_gone");
        let base = d.join("base");
        std::fs::create_dir(&base).unwrap();
        let dest = base.join("sub").join("dest");
        let levels = floor_check_chain(&dest).unwrap();
        assert_eq!(levels, [base.join("sub"), dest.clone()]);
        std::fs::remove_dir(&base).unwrap();
        let got = create_levels(&dest, &levels);
        assert!(
            matches!(std::fs::symlink_metadata(&base), Err(e) if e.kind() == std::io::ErrorKind::NotFound),
            "the base was built again"
        );
        assert!(
            matches!(&got, Err(PathError::Io { path, source })
                if path == &dest && source.kind() == std::io::ErrorKind::NotFound),
            "{got:?}"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn all_takes_a_level_made_since_its_check_as_what_it_is() {
        // A directory made at a level since the walk is what the create would have made.
        let d = tmp("all_level_made");
        let dest = d.join("sub").join("dest");
        let levels = floor_check_chain(&dest).unwrap();
        std::fs::create_dir(d.join("sub")).unwrap();
        create_levels(&dest, &levels).unwrap();
        assert!(dest.is_dir());
        // A file is not, and is named.
        let leaf = d.join("leaf");
        let levels = floor_check_chain(&leaf).unwrap();
        std::fs::write(&leaf, b"x").unwrap();
        let got = create_levels(&leaf, &levels);
        assert!(
            matches!(&got, Err(PathError::NotADirectory(p)) if p == &leaf),
            "{got:?}"
        );
        assert_eq!(std::fs::read(&leaf).unwrap(), b"x");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(windows)] // `absolute` resolves `..` there, so the chain has no `..` level to refuse
    #[test]
    fn all_builds_a_chain_through_a_dotdot_on_windows() {
        let d = tmp("all_dotdot");
        let all = DestPolicy::Create(CreateMode::All);
        ensure_destination(&d.join("a").join("..").join("b"), &all).unwrap();
        assert!(d.join("b").is_dir());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(not(windows))] // `absolute` keeps `..` here, as the OS resolves it: through the levels
    #[test]
    fn all_refuses_a_dotdot_after_a_level_it_would_build() {
        // bug-0040: building `a/..`, the parent of the file `a/../a`, makes `a` a directory where
        // the file goes, and building `a`, the parent of the file `a/..`, makes that path name
        // `d`.  bug-0041 is the same `..` hiding `/Volumes` from the floor (see tests/blackbox.rs).
        // Every caller is refused, by the same path, and nothing is built.
        let d = tmp("all_dotdot");
        let all = DestPolicy::Create(CreateMode::All);
        let level = d.join("a").join("..");
        let dir = level.join("b");
        let file = level.join("a");
        let refused = |r: Result<(), PathError>| {
            assert!(
                matches!(&r, Err(PathError::DotDotAfterMissing(p)) if p == &level),
                "{r:?}"
            );
        };
        refused(check_destination(&dir, &all));
        refused(ensure_destination(&dir, &all));
        refused(check_output_file(&file, CreateMode::All, Clobber::Allow));
        refused(prepare_output_file(&file, CreateMode::All, Clobber::Allow));
        for mode in [CreateMode::Final, CreateMode::All] {
            refused(check_output_file(&level, mode, Clobber::Allow));
            refused(prepare_output_file(&level, mode, Clobber::Allow));
        }
        assert_eq!(
            std::fs::read_dir(&d).unwrap().count(),
            0,
            "something was built"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn all_creates_each_level_by_its_bare_name() {
        // The walk keeps a destination's trailing separator, and `mkdir("link/")` creates a
        // dangling link's target on macOS, so a link made at the leaf since the check was followed.
        let d = tmp("all_bare_level");
        let leaf = d.join("leaf");
        let mut spelled = leaf.clone().into_os_string();
        spelled.push("/");
        let spelled = PathBuf::from(spelled);
        let levels = floor_check_chain(&spelled).unwrap();
        let ghost = d.join("ghost");
        std::os::unix::fs::symlink(&ghost, &leaf).unwrap();
        let got = create_levels(&spelled, &levels);
        assert!(
            matches!(std::fs::symlink_metadata(&ghost), Err(e) if e.kind() == std::io::ErrorKind::NotFound),
            "the dangling link's target was created"
        );
        assert!(
            matches!(&got, Err(PathError::DanglingSymlink(p)) if p == &leaf),
            "{got:?}"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn output_file_follows_a_link_that_resolves() {
        // A link to a file is written through, so Deny refuses it as the file it reaches; a link
        // to a directory puts a directory in the file's place.
        let d = tmp("outfile_link_ok");
        std::fs::write(d.join("real.pdf"), b"old").unwrap();
        let to_file = d.join("out.pdf");
        std::os::unix::fs::symlink(d.join("real.pdf"), &to_file).unwrap();
        assert!(prepare_output_file(&to_file, CreateMode::None, Clobber::Allow).is_ok());
        assert!(matches!(
            prepare_output_file(&to_file, CreateMode::None, Clobber::Deny).unwrap_err(),
            PathError::Clobber(p) if p == to_file
        ));
        let to_dir = d.join("dir.pdf");
        std::os::unix::fs::symlink(&d, &to_dir).unwrap();
        for clobber in [Clobber::Allow, Clobber::Deny] {
            assert!(matches!(
                prepare_output_file(&to_dir, CreateMode::None, clobber).unwrap_err(),
                PathError::NotAFile(p) if p == to_dir
            ));
        }
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn output_file_default_requires_the_parent_dir() {
        let d = tmp("outfile_parent");
        let f = d.join("missingdir").join("out.pdf");
        assert!(matches!(
            prepare_output_file(&f, CreateMode::None, Clobber::Allow).unwrap_err(),
            PathError::MissingDestination(_)
        ));
        // Final creates the one missing dir.
        assert!(prepare_output_file(&f, CreateMode::Final, Clobber::Allow).is_ok());
        assert!(d.join("missingdir").is_dir());
        std::fs::remove_dir_all(&d).unwrap();
    }

    // --- specific errors: a file where a directory is expected, and vice versa ---------------

    #[test]
    fn a_file_where_a_directory_is_expected_is_a_specific_error() {
        let d = tmp("filedir");
        let dest = d.join("dest");
        std::fs::write(&dest, b"i am a file").unwrap();
        // Under every policy the answer is "a file is there", never "does not exist" or a raw I/O error.
        for policy in [
            DestPolicy::Create(CreateMode::None),
            DestPolicy::Create(CreateMode::Final),
            DestPolicy::Create(CreateMode::All),
            DestPolicy::Marked(marker_name("~Bak~")),
        ] {
            assert!(
                matches!(
                    ensure_destination(&dest, &policy),
                    Err(PathError::NotADirectory(_))
                ),
                "policy {policy:?} should report NotADirectory for a file in the way"
            );
        }
        assert!(dest.is_file(), "the file must be left untouched");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn require_input_dir_distinguishes_a_file_from_a_missing_path() {
        let d = tmp("inputfile");
        let missing = d.join("nope");
        assert!(matches!(
            require_input_dir(&missing).unwrap_err(),
            PathError::MissingInput(_)
        ));
        let f = d.join("afile");
        std::fs::write(&f, b"x").unwrap();
        assert!(matches!(
            require_input_dir(&f).unwrap_err(),
            PathError::NotADirectory(_)
        ));
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// Every entry point, given a path one and two levels below `blocker` (`require_marker`,
    /// `blocker` itself as the directory), must answer with the error `names` accepts.
    fn assert_every_entry_point_names(blocker: &Path, names: impl Fn(&PathError) -> bool) {
        let below = blocker.join("x");
        let deeper = below.join("y");
        let mut verdicts: Vec<(String, Result<(), PathError>)> = vec![
            ("require_input_path".into(), require_input_path(&below)),
            ("require_input_dir".into(), require_input_dir(&below)),
            (
                "require_marker".into(),
                require_marker(blocker, &marker_name("m")),
            ),
            ("reject_symlink".into(), reject_symlink(&below)),
        ];
        for mode in [CreateMode::None, CreateMode::Final, CreateMode::All] {
            for clobber in [Clobber::Allow, Clobber::Deny] {
                let got = prepare_output_file(&below, mode, clobber);
                verdicts.push((format!("prepare_output_file {mode} {clobber}"), got));
            }
        }
        for policy in [
            DestPolicy::Create(CreateMode::None),
            DestPolicy::Create(CreateMode::Final),
            DestPolicy::Create(CreateMode::All),
            DestPolicy::Marked(marker_name("m")),
        ] {
            for (half, run) in HALVES {
                for path in [&below, &deeper] {
                    let what = format!("{half} {policy:?} {}", path.display());
                    verdicts.push((what, run(path, &policy)));
                }
            }
        }
        let wrong: Vec<String> = verdicts
            .into_iter()
            .filter(|(_, got)| !matches!(got, Err(e) if names(e)))
            .map(|(what, got)| format!("{what}: {got:?}"))
            .collect();
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    #[test]
    fn every_entry_point_names_a_file_above_the_path_it_was_given() {
        // Unix refuses the lookup below a file (ENOTDIR), and six entry points answered "cannot
        // determine" about the path below it.  Windows answers NotFound: "does not exist", or a
        // pass, and under `None` and `Marked` the file was named on Unix only.
        let d = tmp("file_above");
        let file = d.join("file");
        std::fs::write(&file, b"i am a file").unwrap();
        assert_every_entry_point_names(
            &file,
            |e| matches!(e, PathError::NotADirectory(p) if p == &file),
        );
        assert_eq!(std::fs::read(&file).unwrap(), b"i am a file");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_path_of_any_length_is_answered_not_a_stack_overflow() {
        // The walk above a path that is not there recursed once per level, so a long path
        // overflowed the stack and aborted the process instead of returning an error.
        let d = tmp("long");
        let mut long = d.clone();
        for _ in 0..8_000 {
            long.push("a");
        }
        let got = std::thread::Builder::new()
            .stack_size(512 << 10)
            .spawn(move || require_input_path(&long))
            .unwrap()
            .join()
            .unwrap();
        assert!(got.is_err(), "{got:?}");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn every_entry_point_names_a_dangling_link_above_the_path_it_was_given() {
        // The lookup below a dangling link answers NotFound, which read as absence: "does not
        // exist", or a pass, except where `Final`'s parent probe or `All`'s chain reached the link.
        use std::os::unix::fs::symlink;
        let d = tmp("dangling_above");
        let gone = d.join("gone");
        let dang = d.join("dang");
        symlink(&gone, &dang).unwrap();
        assert_every_entry_point_names(
            &dang,
            |e| matches!(e, PathError::DanglingSymlink(p) if p == &dang),
        );
        assert!(
            matches!(std::fs::symlink_metadata(&gone), Err(e) if e.kind() == std::io::ErrorKind::NotFound),
            "the dangling link's target was created"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn output_file_rejects_a_directory_in_its_place() {
        let d = tmp("outfile_isdir");
        let target = d.join("out.pdf");
        std::fs::create_dir(&target).unwrap(); // a directory where a file should be written
        assert!(matches!(
            prepare_output_file(&target, CreateMode::None, Clobber::Allow).unwrap_err(),
            PathError::NotAFile(_)
        ));
        std::fs::remove_dir_all(&d).unwrap();
    }

    // --- symlinks: followed by default; the caller can reject ----------------------------------

    #[cfg(unix)]
    #[test]
    fn symlink_to_a_directory_is_followed_by_default() {
        use std::os::unix::fs::symlink;
        let d = tmp("sl_dir");
        let real = d.join("real");
        std::fs::create_dir(&real).unwrap();
        let link = d.join("link");
        symlink(&real, &link).unwrap();
        assert!(ensure_destination(&link, &DestPolicy::Create(CreateMode::None)).is_ok());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn dangling_symlink_is_reported_specifically_not_as_missing() {
        use std::os::unix::fs::symlink;
        let d = tmp("sl_dangling");
        let link = d.join("link");
        symlink(d.join("nonexistent-target"), &link).unwrap();
        assert!(matches!(
            ensure_destination(&link, &DestPolicy::Create(CreateMode::None)).unwrap_err(),
            PathError::DanglingSymlink(_)
        ));
        assert!(matches!(
            require_input_dir(&link).unwrap_err(),
            PathError::DanglingSymlink(_)
        ));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn reject_symlink_refuses_a_link_but_allows_a_real_dir() {
        use std::os::unix::fs::symlink;
        let d = tmp("sl_reject");
        let real = d.join("real");
        std::fs::create_dir(&real).unwrap();
        let link = d.join("link");
        symlink(&real, &link).unwrap();
        assert!(matches!(
            reject_symlink(&link).unwrap_err(),
            PathError::IsSymlink(_)
        ));
        assert!(reject_symlink(&real).is_ok());
        // It asks about the path itself, not its ancestors: `link/sub` is not a link, whether or
        // not `sub` exists yet.
        assert!(reject_symlink(&link.join("sub")).is_ok());
        std::fs::create_dir(real.join("sub")).unwrap();
        assert!(reject_symlink(&link.join("sub")).is_ok());
        std::fs::remove_dir_all(&d).unwrap();
    }

    // --- check_destination: the dry-run half (validates, never creates) ------------------------

    #[test]
    fn check_destination_creates_nothing_where_ensure_would_create() {
        let d = tmp("check_no_create");

        // `final` on an absent leaf whose parent exists: a green verdict, and still no directory.
        let leaf = d.join("leaf");
        assert!(check_destination(&leaf, &DestPolicy::Create(CreateMode::Final)).is_ok());
        assert!(!leaf.exists(), "check_destination created the leaf");

        // `all` on an absent chain: a green verdict, and not one level of it created.
        let deep = d.join("a").join("b").join("c");
        assert!(check_destination(&deep, &DestPolicy::Create(CreateMode::All)).is_ok());
        assert!(!d.join("a").exists(), "check_destination created the chain");

        // ensure_destination, by contrast, does create — proving the two differ only in that.
        ensure_destination(&deep, &DestPolicy::Create(CreateMode::All)).unwrap();
        assert!(deep.is_dir());

        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn check_destination_agrees_with_ensure_destination_on_every_verdict() {
        let d = tmp("check_agrees");
        let existing = d.join("here");
        std::fs::create_dir(&existing).unwrap();

        // (path, policy, expected_ok) — check and ensure must reach the same verdict.
        let mut cases: Vec<(PathBuf, DestPolicy, bool)> = vec![
            (existing.clone(), DestPolicy::Create(CreateMode::None), true),
            (
                d.join("absent"),
                DestPolicy::Create(CreateMode::None),
                false,
            ),
            (
                d.join("absent"),
                DestPolicy::Create(CreateMode::Final),
                true,
            ),
            (d.join("a/b"), DestPolicy::Create(CreateMode::Final), false), // parent gone
            (d.join("a/b"), DestPolicy::Create(CreateMode::All), true),
        ];
        // Not a mount root on Windows, where `All` would create it.
        if cfg!(not(windows)) {
            cases.push((
                PathBuf::from("/Volumes/NotMounted"),
                DestPolicy::Create(CreateMode::All),
                false,
            ));
        }
        for (path, policy, expected_ok) in cases {
            let checked = check_destination(&path, &policy);
            assert_eq!(
                checked.is_ok(),
                expected_ok,
                "check verdict for {path:?} / {policy:?}"
            );
            let ensured = ensure_destination(&path, &policy);
            assert_eq!(
                ensured.is_ok(),
                expected_ok,
                "ensure disagreed with check for {path:?} / {policy:?}"
            );
        }
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn check_destination_still_enforces_the_floor_and_the_marker() {
        // The floor holds in check, exactly as in ensure — a dry run must refuse an unmounted volume.
        if cfg!(not(windows)) {
            let err = check_destination(
                Path::new("/Volumes/DefinitelyNotMounted"),
                &DestPolicy::Create(CreateMode::All),
            )
            .unwrap_err();
            assert!(
                matches!(err, PathError::MountRoot(_)),
                "expected MountRoot, got {err:?}"
            );
        }

        let d = tmp("check_marker");
        let err = check_destination(&d, &DestPolicy::Marked(marker_name("~Bak~"))).unwrap_err();
        assert!(
            matches!(err, PathError::MarkerMissing { .. }),
            "expected MarkerMissing, got {err:?}"
        );
        std::fs::create_dir(d.join("~Bak~")).unwrap();
        assert!(check_destination(&d, &DestPolicy::Marked(marker_name("~Bak~"))).is_ok());
        std::fs::remove_dir_all(&d).unwrap();
    }

    // --- check_output_file: the dry-run half of an output file (validates, never creates) -----

    /// Whether `path` is provably absent: `NotFound`, and nothing else.
    fn gone(path: &Path) -> bool {
        matches!(std::fs::symlink_metadata(path), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
    }

    #[test]
    fn check_output_file_creates_nothing_where_prepare_output_file_would_create_a_parent() {
        let d = tmp("check_outfile_no_create");
        // `final`, the file's parent missing and the parent's own parent there.
        let shallow = d.join("new").join("out.pdf");
        // `all`, two levels missing above the file.
        let deep = d.join("a").join("b").join("out.pdf");
        for clobber in [Clobber::Allow, Clobber::Deny] {
            check_output_file(&shallow, CreateMode::Final, clobber).unwrap();
            assert!(
                gone(&d.join("new")),
                "{clobber}: check_output_file created the parent"
            );
            check_output_file(&deep, CreateMode::All, clobber).unwrap();
            assert!(
                gone(&d.join("a")),
                "{clobber}: check_output_file created the chain"
            );
        }

        // prepare_output_file, by contrast, does create the parents, and never the file — proving
        // the two differ only in that.
        prepare_output_file(&shallow, CreateMode::Final, Clobber::Deny).unwrap();
        prepare_output_file(&deep, CreateMode::All, Clobber::Deny).unwrap();
        assert!(d.join("new").is_dir() && d.join("a").join("b").is_dir());
        assert!(
            gone(&shallow) && gone(&deep),
            "prepare_output_file created the file"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// The dry run of an output file must refuse `path` with an error `refusal` accepts, in every
    /// mode and under each of `clobbers`, and with the very error the live run gives.
    fn assert_check_output_file_refuses(
        path: &Path,
        clobbers: &[Clobber],
        refusal: impl Fn(&PathError) -> bool,
    ) {
        let mut wrong = Vec::new();
        for mode in [CreateMode::None, CreateMode::Final, CreateMode::All] {
            for &clobber in clobbers {
                let checked = check_output_file(path, mode, clobber);
                let prepared = prepare_output_file(path, mode, clobber);
                if !matches!(&checked, Err(e) if refusal(e))
                    || format!("{checked:?}") != format!("{prepared:?}")
                {
                    wrong.push(format!(
                        "{mode} {clobber}: check {checked:?}; prepare {prepared:?}"
                    ));
                }
            }
        }
        assert!(wrong.is_empty(), "{}: {wrong:#?}", path.display());
    }

    #[test]
    fn check_output_file_refuses_every_leaf_that_prepare_output_file_refuses() {
        let both = [Clobber::Allow, Clobber::Deny];
        let d = tmp("check_outfile_leaf");

        // A directory in the file's place, under either policy.
        let dir = d.join("dir.pdf");
        std::fs::create_dir(&dir).unwrap();
        assert_check_output_file_refuses(
            &dir,
            &both,
            |e| matches!(e, PathError::NotAFile(p) if p == &dir),
        );

        // An existing file under Deny; Allow lets it through, in the dry run as in the live run.
        let old = d.join("old.pdf");
        std::fs::write(&old, b"old").unwrap();
        assert_check_output_file_refuses(
            &old,
            &[Clobber::Deny],
            |e| matches!(e, PathError::Clobber(p) if p == &old),
        );
        assert!(check_output_file(&old, CreateMode::None, Clobber::Allow).is_ok());
        assert_eq!(std::fs::read(&old).unwrap(), b"old");

        // The empty path names nothing, and the floor cannot resolve it.
        let empty = Path::new("");
        assert_check_output_file_refuses(
            empty,
            &both,
            |e| matches!(e, PathError::Undeterminable { path, .. } if path == empty),
        );

        // An absent file named for a mount point: writing it would create one.
        if cfg!(not(windows)) {
            let vol = Path::new("/Volumes/NoSuchVolume_cc_check_out");
            assert!(gone(vol), "the test needs {vol:?} absent");
            assert_check_output_file_refuses(
                vol,
                &both,
                |e| matches!(e, PathError::MountRoot(p) if p == vol),
            );
        }

        // A dangling link: the write would create its target somewhere never validated.
        #[cfg(unix)]
        {
            let ghost = d.join("no_such_dir");
            let dangling = d.join("dangling.pdf");
            std::os::unix::fs::symlink(ghost.join("real.pdf"), &dangling).unwrap();
            assert_check_output_file_refuses(
                &dangling,
                &both,
                |e| matches!(e, PathError::DanglingSymlink(p) if p == &dangling),
            );
            assert!(gone(&ghost), "the dangling link's target was created");
        }
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn check_output_file_does_not_approve_a_file_it_could_not_inspect() {
        // Not under Deny, where approving would be the no-clobber guard failing open, nor under
        // Allow, where a directory or a dangling link at the path would have been refused.
        if skipped_as_root() {
            return;
        }
        let f = LockedDir::new();
        for leaf in ["real.txt", "nope.pdf"] {
            let p = f.locked.join(leaf);
            assert_check_output_file_refuses(
                &p,
                &[Clobber::Allow, Clobber::Deny],
                |e| matches!(e, PathError::Undeterminable { path, .. } if path == &p),
            );
        }
    }

    // --- a marker is a name, never a path -----------------------------------------------------

    /// Every shape `Path::join` would reinterpret: an absolute value replaces the directory, a
    /// `..` escapes it, a separator descends, and the empty string is the directory itself — each
    /// made the guard pass for every destination on earth.  None is a name.
    const NOT_A_NAME: [&str; 9] = ["", "/tmp", "/", "a/b", "a/", "../x", "..", ".", "./x"];

    #[test]
    fn a_marker_name_refuses_every_value_that_is_not_a_name() {
        for bad in NOT_A_NAME {
            let err = MarkerName::new(bad).expect_err(&format!("marker {bad:?} must be refused"));
            assert_eq!(err.marker, bad, "the error names the value as given");
            let msg = err.to_string();
            assert!(msg.contains(&format!("'{bad}'")), "marker {bad:?}: {msg}");
            // It may come from a flag of any spelling, or from no flag at all.
            assert!(!msg.contains("--require-marker"), "marker {bad:?}: {msg}");
            // Parsing one refuses it with the same error.
            assert_eq!(bad.parse::<MarkerName>(), Err(err), "marker {bad:?}");
        }
        // Genuine names — including ones with dots and odd punctuation — still work.
        for good in ["~Bak~", "a.b", ".marker", "spaced name"]
            .into_iter()
            .chain(cfg!(not(windows)).then_some("..."))
        {
            let name = MarkerName::new(good).expect("a name");
            assert_eq!((name.as_str(), name.to_string()), (good, good.to_string()));
            assert_eq!(good.parse::<MarkerName>().as_ref(), Ok(&name));
            assert_eq!(
                DestPolicy::from_flags(CreateMode::None, Some(name.clone())).unwrap(),
                DestPolicy::Marked(name),
                "marker {good:?} is a name"
            );
        }
        // A backslash is a separator on Windows alone, so "a\b" is a name everywhere else.
        let backslash = MarkerName::new("a\\b");
        if cfg!(windows) {
            assert!(
                matches!(backslash, Err(MarkerNameError { .. })),
                "{backslash:?}"
            );
        } else {
            assert_eq!(backslash.unwrap().as_str(), "a\\b");
        }
    }

    /// The README's flags: a mode, and a marker typed as a name.
    #[cfg(feature = "clap")]
    #[derive(clap::Parser, Debug)]
    struct FlagArgs {
        #[arg(long, value_enum, default_value_t)]
        create_destination: CreateMode,
        #[arg(long)]
        require_marker: Option<MarkerName>,
    }

    #[cfg(feature = "clap")]
    #[test]
    fn a_clap_flag_typed_as_a_marker_name_refuses_a_non_name_as_a_usage_error() {
        use clap::Parser;
        let err = FlagArgs::try_parse_from(["tool", "--require-marker", "a/b"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
        assert_eq!(err.exit_code(), i32::from(crate::exit::USAGE));
        let source = std::error::Error::source(&err).and_then(|e| e.downcast_ref());
        assert!(
            matches!(source, Some(MarkerNameError { fault, .. }) if *fault == "contains a path separator"),
            "{err:?}"
        );
        let args = FlagArgs::try_parse_from(["tool", "--require-marker", "~Bak~"]).unwrap();
        assert_eq!(args.require_marker, Some(marker_name("~Bak~")));
        assert_eq!(
            DestPolicy::from_flags(args.create_destination, args.require_marker),
            Ok(DestPolicy::Marked(marker_name("~Bak~")))
        );
    }

    #[cfg(feature = "clap")]
    #[test]
    fn a_clap_tool_reports_the_name_fault_before_the_create_contradiction() {
        // A non-name marker with a creating mode has two faults; the value fault is named,
        // because fixing the mode would only reveal it a run later.  The name is checked when the
        // `MarkerName` is made, so clap reports it, and `from_flags` never sees the pair.
        use clap::Parser;
        for mode in ["final", "all"] {
            let argv = [
                "tool",
                "--create-destination",
                mode,
                "--require-marker",
                "/tmp",
            ];
            let err = FlagArgs::try_parse_from(argv).unwrap_err();
            assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation, "{err}");
            let source = std::error::Error::source(&err).and_then(|e| e.downcast_ref());
            assert!(
                matches!(source, Some(MarkerNameError { fault, .. }) if *fault == "is absolute"),
                "{mode}: {err:?}"
            );
        }
    }

    #[test]
    fn a_marker_name_names_the_fault_of_each_shape() {
        // Each of these, joined to a directory, names some other path that exists, so a guard
        // that probed it would pass.  No marker name can hold one, so none reaches a probe.
        let cases = [
            // dir.join("") == dir
            ("".to_string(), "is empty"),
            // the root exists everywhere
            ("/".to_string(), "is absolute"),
            // absolute, exists
            (
                std::env::temp_dir().to_str().unwrap().to_string(),
                "is absolute",
            ),
            // escapes to a sibling
            ("../x".to_string(), "contains a path separator"),
            // descends to an entry
            ("./x".to_string(), "contains a path separator"),
            // trailing separator on an entry
            ("x/".to_string(), "contains a path separator"),
        ];
        for (bad, expected_fault) in &cases {
            let err = MarkerName::new(bad.as_str())
                .expect_err(&format!("marker {bad:?} must be refused"));
            assert_eq!(
                (err.marker.as_str(), err.fault),
                (bad.as_str(), *expected_fault),
                "marker {bad:?}"
            );
            let msg = err.to_string();
            assert!(msg.contains(&format!("'{bad}'")), "{msg}");
            assert!(
                msg.contains(&format!("the value {expected_fault}")),
                "{msg}"
            );
            assert!(msg.contains("never a path"), "{msg}");
        }
    }

    #[test]
    fn a_marker_name_error_says_what_a_marker_is() {
        let err = MarkerName::new("a/b").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("contains a path separator"), "{msg}");
        assert!(msg.contains("single file or directory name"), "{msg}");
        let err = MarkerName::new("").unwrap_err();
        assert!(err.to_string().contains("is empty"), "{err}");
        let err = MarkerName::new("/tmp").unwrap_err();
        assert!(err.to_string().contains("is absolute"), "{err}");
        let err = MarkerName::new("..").unwrap_err();
        assert!(err.to_string().contains("is '..'"), "{err}");
    }

    #[cfg(windows)]
    #[test]
    fn creating_under_an_absent_drive_fails_and_fabricates_nothing() {
        // Why the floor checks no list on Windows: a mount root there is a drive letter, and a
        // create under an absent one fails rather than fabricating it.
        let absent = |root: &String| matches!(std::fs::metadata(root), Err(e) if e.kind() == std::io::ErrorKind::NotFound);
        let drive = ('D'..='Z')
            .rev()
            .map(|letter| format!("{letter}:\\"))
            .find(absent)
            .expect("no drive letter from D to Z is free");
        let dest = Path::new(&drive).join("cli_contract_test").join("x");
        // The platform fact the ruling rests on, asked of the OS directly: the crate's own chain
        // check now refuses the absent root before any create is tried.
        assert!(
            std::fs::create_dir_all(&dest).is_err(),
            "created {}",
            dest.display()
        );
        assert!(absent(&drive), "{drive} exists after the create");
        let err = ensure_destination(&dest, &DestPolicy::Create(CreateMode::All)).unwrap_err();
        assert!(
            matches!(&err, PathError::MountRoot(p) if p == Path::new(&drive)),
            "{err:?}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_server_that_cannot_be_reached_is_undeterminable_not_missing() {
        // Windows reports an unreachable server (53) as `NotFound`; it is not proof of absence.
        // The reserved `.invalid` name fails at once, without a network timeout.
        let p = Path::new(r"\\nosuchserver.invalid\share\x");
        let got = require_input_path(p);
        assert!(
            matches!(&got, Err(PathError::Undeterminable { source, .. }) if source.raw_os_error() == Some(53)),
            "{got:?}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_last_name_is_probed_as_the_os_reads_it() {
        // A name `join` would re-parse is probed as written: in a `\\?\` path the OS takes `/`
        // literally, and `C:` or `a:s` as the last name would otherwise replace the parent.
        let d = tmp("last_name");
        let absent = |p: &Path| matches!(std::fs::symlink_metadata(p), Err(e) if e.kind() == std::io::ErrorKind::NotFound);
        let v = |tail: &str| PathBuf::from(format!(r"\\?\{}\{tail}", d.display()));
        let got = require_input_path(&v("absent/.."));
        assert!(
            matches!(&got, Err(PathError::Undeterminable { .. })),
            "{got:?}"
        );
        std::fs::create_dir(d.join("sub")).unwrap();
        let got = ensure_destination(&v("sub/new"), &DestPolicy::Create(CreateMode::Final));
        assert!(
            matches!(&got, Err(PathError::Undeterminable { .. })),
            "{got:?}"
        );
        assert!(absent(&d.join("sub").join("new")), "created sub\\new");
        let got = require_input_path(&PathBuf::from(format!(r"{}\gone\C:", d.display())));
        assert!(
            got.is_err(),
            "a trailing C: was probed as the current directory: {got:?}"
        );
        // An NTFS stream on a one-letter file: `a:s`, never drive A:.
        let stream = PathBuf::from(format!(r"{}\a:s", d.display()));
        std::fs::write(&stream, b"keep").unwrap();
        let got = prepare_output_file(&stream, CreateMode::None, Clobber::Deny);
        assert!(matches!(&got, Err(PathError::Clobber(_))), "{got:?}");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn an_absent_drive_root_is_refused_as_a_volume_that_is_not_there() {
        // `create_dir_all` answers Ok for a path with no parent, so `All` once reported an absent
        // `Z:\` as a directory that exists: from the check, and from the create after a check that
        // saw the drive, when it went in between.
        let drive = ('D'..='Z')
            .rev()
            .map(|letter| format!("{letter}:\\"))
            .find(|root| {
                matches!(std::fs::metadata(root), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
            })
            .expect("no drive letter from D to Z is free");
        let root = Path::new(&drive);
        let all = DestPolicy::Create(CreateMode::All);
        for err in [
            check_destination(root, &all).unwrap_err(),
            ensure_destination(root, &all).unwrap_err(),
            create_after_check(root, &all).unwrap_err(),
            prepare_output_file(&root.join("out.pdf"), CreateMode::All, Clobber::Allow)
                .unwrap_err(),
            check_output_file(&root.join("out.pdf"), CreateMode::All, Clobber::Allow).unwrap_err(),
        ] {
            assert!(
                matches!(&err, PathError::MountRoot(p) if p == root),
                "{err:?}"
            );
        }
        // The root itself as an output file has no parent to ensure, and is no place for a file.
        let bare = drive.trim_end_matches('\\');
        for p in [root, Path::new(bare)] {
            for mode in [CreateMode::None, CreateMode::Final, CreateMode::All] {
                for got in [
                    prepare_output_file(p, mode, Clobber::Allow),
                    check_output_file(p, mode, Clobber::Allow),
                ] {
                    assert!(
                        matches!(&got, Err(PathError::MissingDestination(q)) if q == p),
                        "{p:?} {mode}: {got:?}"
                    );
                }
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn the_windows_floor_refuses_none_of_the_unix_spellings() {
        // The ruling itself: on Windows the Unix list would refuse only ordinary folders.  These
        // are checks, so nothing is created.
        for p in [
            r"C:\Volumes",
            r"C:\Volumes\Backup",
            r"D:\mnt\usb",
            r"C:\media\photos\2026",
            r"C:\run\media\alice\X",
        ] {
            assert!(
                !is_mount_root(Path::new(p)),
                "{p} is refused as a mount root"
            );
        }
        let dest = Path::new(r"C:\Volumes\cli_contract_no_such\x");
        check_destination(dest, &DestPolicy::Create(CreateMode::All)).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn a_verbatim_path_keeps_its_literal_text_when_its_separator_is_dropped() {
        // The OS takes a `\\?\` path literally, so `..` there is not resolved; rebuilding the
        // whole path would probe a different object than every later call uses.
        let p = Path::new(r"\\?\C:\x\..\Windows\");
        assert_eq!(
            without_trailing_separator(p),
            Path::new(r"\\?\C:\x\..\Windows")
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_drive_prefix_is_not_a_marker_name() {
        // The last arm of marker_name_fault: a bare drive and a drive-relative path are neither
        // absolute nor separated, yet neither is one normal component.
        for bad in ["C:", "C:x"] {
            let err = MarkerName::new(bad).unwrap_err();
            assert_eq!(
                err.fault, "is not a single path component",
                "marker {bad:?}: {err:?}"
            );
        }
    }

    // --- a symlinked marker: followed when it resolves, reported as broken when it does not ----

    #[cfg(unix)]
    #[test]
    fn require_marker_reports_a_dangling_marker_link_as_broken_not_missing() {
        // An entry named as the marker IS present, so "is missing its required marker" would be
        // a false statement of fact; the honest report is the broken link itself (ruled
        // 2026-09-04).
        let d = tmp("marker_dangling");
        let link = d.join("m");
        std::os::unix::fs::symlink(d.join("gone"), &link).unwrap();
        let err = require_marker(&d, &marker_name("m")).unwrap_err();
        assert!(
            matches!(&err, PathError::DanglingSymlink(p) if p == &link),
            "expected DanglingSymlink({}), got {err:?}",
            link.display()
        );
        assert!(err.to_string().contains("broken symbolic link"), "{err}");
        assert!(!err.to_string().contains("missing"), "{err}");
        // The policy entry points agree, live and dry-run.
        let policy = DestPolicy::Marked(marker_name("m"));
        assert!(matches!(
            ensure_destination(&d, &policy).unwrap_err(),
            PathError::DanglingSymlink(_)
        ));
        assert!(matches!(
            check_destination(&d, &policy).unwrap_err(),
            PathError::DanglingSymlink(_)
        ));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn require_marker_follows_a_marker_link_that_resolves() {
        // A link to a real target is the marker: the entry proves the disk, as before.
        let d = tmp("marker_link_ok");
        std::fs::write(d.join("real"), b"").unwrap();
        std::os::unix::fs::symlink(d.join("real"), d.join("m")).unwrap();
        assert!(require_marker(&d, &marker_name("m")).is_ok());
        // ...and a link to a directory outside `dir` still counts: the entry is inside.
        let outside = tmp("marker_link_outside");
        std::os::unix::fs::symlink(&outside, d.join("out")).unwrap();
        assert!(require_marker(&d, &marker_name("out")).is_ok());
        std::fs::remove_dir_all(&d).unwrap();
        std::fs::remove_dir_all(&outside).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn require_marker_reports_an_unresolvable_marker_link_as_undeterminable() {
        // A link the probe cannot resolve is neither present nor absent: a self-loop (ELOOP)
        // must never be read as dangling or missing — nor, worse, as present.
        let d = tmp("marker_loop");
        let link = d.join("m");
        std::os::unix::fs::symlink(&link, &link).unwrap();
        let err = require_marker(&d, &marker_name("m")).unwrap_err();
        assert!(
            matches!(err, PathError::Undeterminable { .. }),
            "expected Undeterminable, got {err:?}"
        );
        let msg = err.to_string();
        assert!(!msg.contains("missing") && !msg.contains("broken"), "{msg}");
        assert!(msg.contains("symbolic links"), "must name ELOOP: {msg}");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn require_marker_reports_a_marker_link_into_an_unreadable_dir_as_undeterminable() {
        // The link itself lstat's fine; its target sits under a chmod-000 directory, so
        // following it is EACCES.  "I could not look" is not "it is not there".
        if skipped_as_root() {
            return;
        }
        let f = LockedDir::new();
        let d = tmp("marker_link_eacces");
        std::os::unix::fs::symlink(f.locked.join("real.txt"), d.join("m")).unwrap();
        let err = require_marker(&d, &marker_name("m")).unwrap_err();
        assert_undeterminable(&err, "missing");
        assert_undeterminable(&err, "broken");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn require_marker_still_says_missing_when_nothing_is_there() {
        let d = tmp("marker_absent");
        assert!(matches!(
            require_marker(&d, &marker_name("m")).unwrap_err(),
            PathError::MarkerMissing { .. }
        ));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn a_marker_of_only_dots_and_spaces_is_not_a_name() {
        // Windows strips trailing dots and spaces, so each of these names the directory itself,
        // and the guard would pass in an empty one.  Elsewhere `...` is an ordinary name.
        for v in ["...", "....", " ", ". ", ". .", ".. ", "... "] {
            let got = MarkerName::new(v);
            assert!(
                matches!(&got, Err(MarkerNameError { .. })),
                "MarkerName::new {v:?}: {got:?}"
            );
            assert_eq!(v.parse::<MarkerName>(), got, "parsing {v:?}");
        }
    }

    #[test]
    fn require_marker_on_a_missing_directory_says_the_directory_is_missing() {
        let d = tmp("marker_no_dir");
        let gone = d.join("gone");
        let got = require_marker(&gone, &marker_name("m"));
        assert!(
            matches!(&got, Err(PathError::MissingInput(p)) if *p == gone),
            "{got:?}"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }
}

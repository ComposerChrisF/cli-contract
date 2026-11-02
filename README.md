# cli-contract

Shared conventions for a family of command-line tools: the **caller-asserted path contract**,
canonical **exit codes**, and **BrokenPipe** handling.  One implementation, tested once, so a fix —
or a bug — can never live in one tool and not another.

The reasoning below is the part a client most needs to internalize; the per-item API reference lives
in the module docs.

## Why this exists

Three failures kept recurring across independently-written CLIs:

1. A tool silently created a mistyped output path — or an unmounted backup volume — and reported
   success.  A confident, plausible, wrong result.
2. Every tool spelled its exit codes differently, so an agent or a script could not branch on them
   mechanically.
3. `tool | head` panicked (exit 101) instead of ending cleanly — and for a tool doing real work, it
   truncated that work partway through.

`is_mount_root` is the sharpest reason to centralize rather than copy: a safety-critical,
platform-specific check is exactly where an independent per-tool reimplementation is subtly wrong in
one tool and right in the rest.

## 1.  The path contract

**Principle: a path the invocation asserted, that does not match the filesystem, is exit 1 — the
tool refuses to guess.**  This is symmetric for inputs and outputs, because an output’s _parent
directory_ is itself a caller-asserted location: writing to `dir/file` asserts `dir/` exists,
exactly as naming an input asserts the input exists.

The errors are **specific**, not generic: a missing path, a file sitting where a directory was
required (`NotADirectory`), and a directory sitting where an output file was required (`NotAFile`)
are distinct variants, each naming the offending path — the message states the actual fact on disk,
never a bare report of absence when something is in fact in the way.

**Absence is claimed only on proof.**  Every probe here answers three ways — present, provably
absent (`NotFound`, and nothing else), or **undeterminable** — never the two ways a `bool` can.  A
path that could not be inspected (permission denied, an I/O error, a symlink loop, a network share
or mount that cannot be reached) comes back as `Undeterminable`, naming the real errno; it is never
reported as missing, and a guard refuses it rather than approving it.  “I could not look” is not
“it is not there”: the first
sends you to `chmod`, the second sends you hunting a typo that does not exist — and next to a
delete, that same one-step collapse is how a tool talks itself into destroying data it merely failed
to read.

### Inputs

```rust,ignore
require_input_path(path)?;      // missing -> exit 1, and never created
require_input_dir(dir)?;        // must exist, and be a directory
require_marker(dir, &marker)?;  // in its place: must exist, be a directory, AND contain the marker
```

A missing caller-asserted input is never auto-created: creating it would launder a typo into
permanent world state and manufacture the silent-wrong-output failure (empty input, plausible empty
output, exit 0).  An input directory that must be a specific one takes a marker (below):
`require_marker` refuses a missing directory or a file just as `require_input_dir` does, so it is
called instead of `require_input_dir`, not after it.

### Output directories — must-exist by default

The default is **must-exist**, not create-if-missing, and it matches Unix convention (`cp` and `>`
both require the parent).  Creation is an opt-in spectrum, exposed as a flag:

| `CreateMode` | behavior |
|---|---|
| `None` (default) | create nothing; the directory must already exist, else exit 1 |
| `Final` | create the one final directory, only if _its_ parent already exists |
| `All` | create the whole missing chain, one level at a time below the deepest directory that exists |

Take the mode and an optional marker (below) as flags: with the `clap` feature as here, or under any
other argument parser through their `FromStr`.

```rust,ignore
#[derive(clap::Parser)]
struct Args {
    #[arg(long, value_enum, default_value_t)]
    create_destination: CreateMode,
    #[arg(long)]
    require_marker: Option<MarkerName>, // a value that is not a name is a usage error (exit 2)
}

let policy = DestPolicy::from_flags(args.create_destination, args.require_marker)?;
ensure_destination(&dest, &policy)?;
```

### `--dry-run`: check without creating

A mutating tool that previews its work needs both halves of that call.  It must still refuse an
unmounted volume (the floor does where the mount point goes away with its volume, and a marker does
elsewhere) — a dry run reporting a plausible backup onto a disk that is not there is the exact
false confidence this contract exists to kill — yet it must not create the directory it is only
pretending to write into.  So `ensure_destination` is **check-then-create**, and `check_destination`
is the check alone: the same contract verdicts, zero filesystem writes.  An output file has the same
pair: `prepare_output_file` checks the file and then ensures its parent directory, creating the
missing levels the mode permits, and `check_output_file` is the check alone.

```rust,ignore
if args.dry_run { check_destination(&dest, &policy)? } else { ensure_destination(&dest, &policy)? }

// An output file, whose parent is created as `mode` permits:
if args.dry_run {
    check_output_file(&out, mode, clobber)?
} else {
    prepare_output_file(&out, mode, clobber)?
}
```

The two halves of each pair run the same contract checks, so a dry run never disagrees with the
live run about the _contract_ — only an environmental failure at creation time (permissions, a full
disk, a race, a name the filesystem refuses) can separate them.

### The mount-root floor — unconditional

**No mode, not even `All`, will create a mount point the floor recognizes** (what it does not
recognize is under Known limitations).  A mount point’s existence _is_ the signal
that the volume is mounted, so creating one is never what anyone wanted — the canonical disaster is
`sudo` fabricating `/Volumes/Backup` on the boot disk and a backup silently landing there.  That
signal holds where the mount point goes away at unmount, as on macOS and under udisks2 on Linux.
An `/etc/fstab`, systemd, bind, or hand-made mount point outlives its volume: at the mount point
itself, which still exists, every mode approves and only a marker refuses; below it, `Final`
builds a destination whose parent exists, and `All` the whole chain, on the disk underneath.  `is_mount_root` refuses the
mount roots under `/Volumes` (macOS) and `/mnt`, `/media`, `/run/media` (Linux, best-effort); paths
_below_ an existing mount point are created normally, except that on Linux the list’s depths assume
volumes at `/mnt/<name>`, `/media/<user>/<label>`, and `/run/media/<user>/<label>`, so in a volume
mounted directly at `/mnt`, `/media/<name>`, or `/run/media/<name>` nothing new, neither a directory
nor a file, can be made at its top level.  A new output file named for a mount point is refused too, in every mode:
writing it would create the mount point, as a file.  On Unix,
`/Volumes` is recognized by identity, its device and inode, not only by its spelling: on a
case-insensitive volume `/volumes/Backup` is `/Volumes/Backup`, and so are the firmlink
`/System/Volumes/Data/Volumes/Backup` and `Backup` under a symbolic link to `/Volumes`, so all
are refused.  The Linux roots are matched by spelling only.  On Windows
the floor checks no list: the mount roots there are drive letters and UNC shares, which a
directory create cannot fabricate (under an absent drive it fails), and the Unix list would only
refuse ordinary folders, such as `C:\media\photos`.  Under `All`, an absent drive or share root is
refused as `MountRoot`: the volume that is not there.  A volume mounted into an empty NTFS folder
is not detected, just as a Unix mount outside the listed roots is not.

A **relative** destination is resolved against the current directory before the floor is applied,
because that is what the OS resolves when it creates the path — the floor compares what the path
means from the current directory, not its text.  (So `media/uploads` in a project is an ordinary
directory, while a bare `Backup` with a cwd of `/Volumes` is refused.)  Resolution is lexical: it
touches no filesystem and does not resolve symlinks, so the list is matched against the literal
path, including `..`, which after a symbolic link the OS resolves from the link’s target instead;
only the identity check for `/Volumes` and the check for a `..` below look through a link, and the
identity check still sees the directory the OS will create in.  So a `..` after a level that does
not exist yet is refused: under `All`, as `PathError::DotDotAfterMissing`; in every mode, for an
output file whose own path ends in such a `..`; and by `is_mount_root`, which answers `true` for a
path with one.  Until that level is built, the OS has not resolved where the `..` leads, and
nothing there can be checked.  If the current directory cannot be read at all, or the identity check’s
stat of `/Volumes`, the path, or its parent fails for any reason but absence, the floor
**fails closed** and refuses — it gates creation, so an unprovable path is never granted the
dangerous branch.

### The marker guard

A marker refuses a directory that does not contain a named file/dir — “is this the disk or folder I
actually mean?”, the automatic form of a hand-rolled `[ -d "$VOL/~Bak~" ]` guard.  It works on any
folder-valued path, input or output.

**A marker implies must-exist.**  You cannot pre-place a marker in a directory you are also creating,
so `--require-marker` together with `--create-destination=final|all` is a contradiction:
`DestPolicy::from_flags` rejects it as a **usage error (exit 2)** rather than silently dropping the
mode.

**A marker is a name, never a path** — exactly one path component, held as a `MarkerName`.  An
empty value, an absolute path, a value containing a separator, `.`/`..`, or, on Windows, a value
made only of dots and spaces or a drive prefix (`C:`, `C:x`) is refused when a `MarkerName` is
made, by `MarkerName::new` or by parsing one, as a `MarkerNameError` — so no `DestPolicy::Marked`
and no call to `require_marker` can hold one.  The error carries no exit code, because the crate
cannot tell where a value came from; the caller can.  For a flag typed `Option<MarkerName>`, clap
reports a bad value as a **usage error (exit 2)**; a caller parsing the tool’s own config file maps
the error to exit 1, since the file is the tool’s input.  The reason is what `Path::join` does with
a non-name: an absolute value _replaces_ the directory, `..` escapes it, and the empty string names
the directory itself — so each of those made the guard test some other path and pass for every
destination on earth, silently.  On Windows a name made only of dots and spaces does the same,
because Windows drops a name’s trailing dots and spaces when it opens the path.

**A marker that is a symbolic link is followed.**  A link to something that exists is the marker —
the entry is inside the directory, which is what proves the disk.  A _dangling_ link is reported as
`DanglingSymlink` (exit 1), never as a missing marker: an entry of that name is present, so “missing”
would be a false statement of fact by this crate’s own standard.  (The marker is
checked _in the directory itself_.  Must-exist proves only that a destination is there, not which
disk holds it: a replacement disk with the same folders, or a copy left on the disk under an
unmounted volume, passes it, and only the marker refuses.  See `ROADMAP.md` for the
marker-on-an-ancestor question.)

### Symlinks — followed by default; the caller decides

By default the checks **follow** symlinks (a link to a directory is used as that directory), matching
OS convention — with the edges made honest: a **dangling** link is reported as `DanglingSymlink`, not
as a missing path, and a link to a file is `NotADirectory`.  At an output **file** path, a link to
an existing file is written through, and `Clobber::Deny` refuses it as the existing file it is.  A
dangling link there is `DanglingSymlink`, since a write through it would land, or fail, at a
location nothing validated.

A trailing separator is dropped before any check, though the OS reads `link/` and `link/.` as the
link’s target: a link spelled that way is still the link, so `reject_symlink("link/")` refuses it,
and a file spelled `file/` is checked as `file`.

But a symlinked destination can silently redirect output off the path you named — the mount-root
floor looks through a link only to recognize `/Volumes` — so the crate hands the decision to the
caller instead of guessing.
Everything composes from primitives; there is no fixed policy enum:

- **Follow** (default) — call the functions; a link to a directory is used.
- **Reject** — `reject_symlink(&dest)?` before `ensure_destination` refuses a destination that is
  itself a link (`IsSymlink`).  It asks whether the destination itself is a link, not whether its
  ancestors are: `link/sub` passes when `link` leads to a directory, and lands through `link`.  A
  destination that cannot exist, because a file or a dangling link stands above it, is refused
  with the error that names that entry.  A caller that needs more proves the disk with the marker
  guard, or checks each ancestor itself, in a walk that refuses on every error but the one that
  proves an ancestor absent: `NotFound`, and nothing else.
- **Prove the disk** — a marker is looked for in the destination as the OS resolves it, through any
  link, so a link that leads somewhere unintended is refused there: the marker is not found.  A
  link into an unmounted volume is dangling, and `ensure_destination` already refuses it as
  `DanglingSymlink`; a link onto a mount point that outlived its volume is not, and only a marker
  catches it.

## 2.  Exit codes

```text
0  success / clean (including dry-run and a legitimate no-op)
1  tool error (I/O, a missing input path, a parse failure of the tool's own input)
2  usage error (bad command-line arguments)
3  findings (the tool ran and found problems: validation failures, drift, staleness)
4  setup needed (missing config or prerequisite state; not initialized)
```

Constants live in `exit`, and `exit::HELP_BLOCK` is ready to hand to clap’s `after_long_help` so
every tool documents the table identically.  **Findings never exit 1** — that is what lets a caller
distinguish “act on the findings” from “retry or debug the tool”.

## 3.  BrokenPipe — read both cases; the wrong one destroys data

Rust ignores `SIGPIPE`, so writing to a closed pipe (`tool | head`) returns `EPIPE`, which
`println!` unwraps into a panic (exit 101).  There are **two** correct fixes, and choosing the wrong
one for a mutating tool truncates its work:

- **Read-only / enumerating tool** — a listing or report, where the output _is_ the product: call
  `reset_sigpipe()` once at the top of `main`.  A broken pipe then terminates the process quietly
  via the signal (the shell sees exit 141), the standard behavior of a Unix tool in a pipeline.
  Nothing is lost, because there was nothing to do but print.  Windows has no `SIGPIPE`, so there
  `reset_sigpipe()` does nothing and a read-only tool still panics on a closed pipe (exit 101); a
  tool that must not writes with `writeln!` and, on an error `is_broken_pipe` recognizes, stops
  with exit 0.

- **Mutating tool that streams progress** — a backup that prints each copied file, where the output
  is _incidental_: a closed progress pipe must **not** abort the operation.  Do **not** call
  `reset_sigpipe()` here; it would kill the job on the first write after the reader left, turning an
  exit-101 truncation into an exit-141 truncation.  Instead leave `SIGPIPE` ignored and write
  progress through `ignore_broken_pipe(writeln!(out, ...))`, which swallows a broken pipe so the work
  runs to completion (“suppress and finish”) while still surfacing any real I/O error.

```rust,ignore
// read-only enumerator: the output is the product, so dying when no one reads is fine.
cli_contract::reset_sigpipe();
for line in listing {
    println!("{line}");
}

// mutating tool with progress: the work must finish even if the pipe closes.
for file in to_copy {
    copy(file)?;                                       // never interrupted by a closed pipe
    ignore_broken_pipe(writeln!(out, "{}", file.display()))?; // a closed pipe just stops output
}
```

The one-line takeaway: `reset_sigpipe()` is right when losing the output loses nothing; a tool that
is _doing_ something while it prints must suppress-and-finish instead.

## Using it

```toml
[dependencies]
cli-contract = "0.6"
# or, to take CreateMode and Clobber as clap value enums:
cli-contract = { version = "0.6", features = ["clap"] }
```

Runnable versions of the snippets above are in `examples/`: `inputs`, `destination` (the
directory half of the `--dry-run` snippet), and `broken_pipe`.  For instance,
`cargo run --example destination -- --create-destination all --dry-run /tmp/demo/out`.

### Features

| Feature | Default | What it adds |
|---|---|---|
| `clap` | off | `clap::ValueEnum` for `CreateMode` and `Clobber`, so a clap tool takes them as `value_enum` flags |

Without the feature, both types still implement `FromStr` and `Display` with the same spellings
(`none`, `final`, `all`; `allow`, `deny`), for any other argument parser.  A string that names none
of the values of the type being parsed is a `ParseValueError`, which is a usage error (exit 2) on
the command line and a tool error (exit 1) in a config file.

## Known limitations

Each of these is known, and stated where the contract describes it; they are listed here so a reader
weighing the crate finds them in one place.

- **On Linux the mount roots are matched by spelling.**  A path that reaches `/mnt`, `/media`, or
  `/run/media` under another name is not seen: through a symbolic link, a bind mount, `..` after a
  link (which the OS resolves from the link’s target), or a system alias such as `/var/run` or
  `/proc/self/root`.  Run as root, `Final` and `All` can then create a mount point.  macOS’s
  `/Volumes` is recognized by identity and has no such gap.
- **A mount point that outlives its volume is not detected.**  An `/etc/fstab`, systemd, bind, or
  hand-made mount point stays after the unmount.  Every mode approves the mount point itself, and
  `Final` and `All` can build below it, on the disk underneath; only a marker refuses.  A marker
  cannot yet be combined with a creating mode.
- **The Linux list over-refuses.**  Its depths assume volumes at `/mnt/<name>`,
  `/media/<user>/<label>`, and `/run/media/<user>/<label>`, so in a volume mounted directly at
  `/mnt`, `/media/<name>`, or `/run/media/<name>` nothing new can be made at its top level.  It
  fails closed.
- **A name the filesystem refuses can separate the dry run from the live run**, since the check
  below a missing level sees only `NotFound`.
- **An output file path that ends in a separator is approved.**  `out.pdf/` and `out.pdf/.` are
  checked as `out.pdf`, so `check_output_file` and `prepare_output_file` approve them, though no
  file can be written under that spelling; the write then fails.
- **On Windows:** there is no `SIGPIPE`, so `reset_sigpipe` does nothing; a volume mounted into an
  empty NTFS folder is not detected; and the tests do not stage symbolic links, permissions, or a
  closed pipe there.

## Status

Everything above is built and tested on Linux and macOS, the `/Volumes` identity check on macOS
alone.  On Windows it is built and tested,
except for the cases the tests stage on Unix only: symbolic links, permissions, a closed pipe, a
deleted current directory, and a file spelled with a trailing separator.  `ROADMAP.md`, in the
repository, tracks candidate additions: config discovery and `--show-config`, `--help` section
builders, a findings accumulator, and a dry-run convention.

## Contributing

Bug reports are welcome as GitHub issues: say what you called, with which path, on which platform,
what it returned, and what you expected.  A way around one of the safety checks is a security
report instead; see `SECURITY.md`.  The maintainer keeps the project’s own bug records, and may close
an issue once its report is filed there.

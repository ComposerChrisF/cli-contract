# bug-0050: `is_mount_root`’s fails-closed sentence promises a stat of the path that Linux never makes

Filed on 3-Nov-2026 by the review of the bug-0046 fix (rust-reviewer’s F4).  It is older than that fix and is not part of it.

Provenance `ai`: raised by rust-reviewer.  Cross-model verdicts: Fable agreed, as a fault older than the diff under review; Sol agreed, and named it a bug with basis `doc-drift`.  Read by hand against `src/paths.rs`.

**Claim:** `is_mount_root`’s rustdoc and the README both say the floor fails closed when `/Volumes`, the path, or its parent cannot be stat’d for any reason but absence.  On Linux, where `/Volumes` does not exist, `is_volumes_or_a_child_of_it` returns `Ok(false)` at that absence before it stats the path or its parent.  So a path whose stat would fail there (`/etc/hosts/x`, `ENOTDIR`) answers `false`, not `true`.  The behaviour is safe: on Linux the mount roots are matched by spelling, and the identity check has nothing to compare against.  The sentence promises more than the code does.

**Location:** `src/paths.rs:967-969` (the Fails-closed paragraph of `is_mount_root`), `README.md:151-153`

**Evidence:**
- rustdoc: “so it does when `/Volumes`, the path, or its parent cannot be stat’d for any reason but `NotFound`”.
- README: “If the current directory cannot be read at all, or the identity check’s stat of `/Volumes`, the path, or its parent fails for any reason but absence, the floor **fails closed** and refuses”.
- `is_volumes_or_a_child_of_it`: `let Some(volumes) = identity(Path::new("/Volumes"))? else { return Ok(false); };` comes before the loop that stats the path and its parent.
- plan-0015’s Problem already records the split: “on macOS `is_mount_root("/etc/hosts/x")` is `true` … while on Linux, which has no `/Volumes` to compare against, it is `false`”.

**Severity:** docs

**Basis:** `doc-drift`.  The code is the intended side: on Linux the identity check has no `/Volumes` to find, and plan-0015 states the platform split on purpose.  The remedy is the two sentences alone, saying that the path and its parent are stat’d only where `/Volumes` exists.

**Reproducer:** on Linux, `assert!(cli_contract::is_mount_root(Path::new("/etc/hosts/x")))` fails, though the rustdoc’s sentence says it holds.

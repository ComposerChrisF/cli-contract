# bug-0045: The public `is_mount_root` still answers `false` for a `..` after a level not yet built that hides `/Volumes`

Filed by the nightly review on 1-Nov-2026.

**Spec question: Chris to rule.**  The repository’s contract does not settle that this is a defect: its basis is `inferred` (the evidence, and what follows if either side is right, is below).  Job 2 does not fix it until Chris rules.  To have it fixed as stated, he removes this paragraph and sets the basis on every **Basis:** line below that is not `spec` or `house-rule` to `ruled`, with why; if the code is the right side, he sets them to `doc-drift`, and the fix is to the text alone.

Closed bug-0041 has the same cause, but its report and its fix named only check_destination and check_output_file, and the fix left the public `is_mount_root` and `mount_root` floor unguarded.  So this is a site the fix missed, filed as new because that report is closed.  plan-0015 covers only folding Undeterminable into a bool, not this.  The basis is inferred: it is a spec question whether the floor should fail closed here or document the gap.

From the nightly change review of `f540451065a3fa1e7cff9c26f0c8c6fb9569a7f1..9ad890c8c5520f3a1973e33ad86ce06d420907ad` on night `2026-11-01`.  Provenance `ai`: raised by fable; cross-model verdicts: no second model answered; reproduced by the harness, which ran the test below and saw it fail.  Ledger fingerprint `d87419dfc3d4`.

**Claim:** The fix refuses a `..` after a missing level in floor_check_chain and check_output_leaf, but not in `mount_root`, which `is_mount_root` wraps and the README calls 'the sharpest reason to centralize': the safety-critical floor a caller composes its own check from. For `to_usr/zz/../../Volumes/X` with `to_usr` a link to `/usr` and `zz` absent, the lexical pass sees `[tmp, d, Volumes, X]`, and the identity probes of the path and its parent both get NotFound, which `is_volumes_or_a_child_of_it`'s doc treats as proof ('an absent path or parent proves that it is not it'), so `is_mount_root` returns `false` while `check_destination(.., All)` on the same path now returns DotDotAfterMissing. A caller that gates its own `create_dir_all` on `is_mount_root`, run as root, creates `/Volumes/X`: bug-0041 through the public primitive.

**Location:** `src/paths.rs:971`

**Evidence:** src/paths.rs:971-974: `pub fn is_mount_root(path) -> bool { !matches!(mount_root(path), Ok(false)) }`; src/paths.rs:979-982 `mount_root` = lexical list || identity, no `..` pass; src/paths.rs:1003-1006 (rustdoc): 'an absent path or parent proves that it is not it'; src/paths.rs:1118-1121 (the fix's own comment): 'the probes above saw `NotFound` where the create would find, say, `/Volumes` (bug-0040, bug-0041)'. README.md:21: '`is_mount_root` is the sharpest reason to centralize rather than copy: a safety-critical…'. plan-0015 covers only the Undeterminable-to-bool fold, not this.

**Severity:** contract

**Basis:** `inferred`: The contract is split. `is_mount_root`'s rustdoc says the floor 'fails closed' and the README says 'an unprovable path is never granted the dangerous branch', and the fix's commit says a `..` after a missing level 'cannot be checked before the create', which makes the path unprovable; yet the same rustdoc says it returns `true` only for a failure 'for any reason but NotFound', and here the probes return NotFound. If the fail-closed reading is intended: `mount_root` should answer `true` (or Undeterminable, after plan-0015) when a `..` follows a missing level, with the test above, and the fix was incomplete at one site that shares its fault (minimal-fix.md: 'every other site that shares it'). If `is_mount_root` is meant as the spelling-plus-identity check only and callers must use the contract functions: its rustdoc and README Known limitations should say that a `..` after a level not yet built is not seen by `is_mount_root`, only by check_destination/ensure_destination/check_output_file/prepare_output_file. Either way something changes; which is for Chris.

**Reproducer:** `cargo test is_mount_root_refuses_a_dotdot_after_a_missing_level_that_hides_volumes` failed on the reviewed tree.  The test is `bugs/bug-0045/reproducer.patch`; apply it with `git apply`.


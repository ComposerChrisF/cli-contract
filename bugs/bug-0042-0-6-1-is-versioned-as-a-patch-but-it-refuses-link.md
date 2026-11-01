# bug-0042: 0.6.1 is versioned as a patch, but it refuses link-free `..` destinations such as `new/../new` that 0.6.0 built correctly, and TODO.md’s publish item does not record the semver question

Filed by the nightly review on 1-Nov-2026.

**Ruled by Chris, 1-Nov-2026 (7:59am):** the code is the right side.  He answered ask `row:App/Rust/cli-contract/0024` “Y: Publish now as 0.6.1”, against “M: Make it 0.7.0, with the narrowing documented”.  The basis below is now `doc-drift`, and the fix is to the text alone: `TODO.md`’s publish line records the ruling.  The reproducer asserts the side that was ruled out, so it is not applied.

No open report or plan covers this, and neither does closed bug-0040 or bug-0041: those reports asked for the refusal, not for a decision on how to version it.  The question is recorded only in commit 9ad890c’s F5.  The basis is inferred, so this is a spec question for Chris.

From the nightly change review of `f540451065a3fa1e7cff9c26f0c8c6fb9569a7f1..9ad890c8c5520f3a1973e33ad86ce06d420907ad` on night `2026-11-01`.  Provenance `ai`: raised by opus:rust-reviewer; cross-model verdicts: fable agree, sol agree; reproduced by the harness, which ran the test below and saw it fail.  Ledger fingerprint `43661ccd5924`.

**Claim:** The new `..` check in `floor_check_chain` refuses every `..` that follows a missing level.  That includes destinations that 0.6.0 built correctly.  For example, `ensure_destination("new/../new", All)` used to create `new`, get EEXIST on `new/..` and `new/../new`, and return Ok with the right directory in place.  It now returns `DotDotAfterMissing`.  Cargo treats 0.6.1 as compatible with 0.6.0, so `cargo update` gives every dependent tool the new exit-1 refusal without warning.  The commit message says this question (its F5: is it a fix or a narrowing that needs 0.7.0?) goes to the maintainer with the publish.  But TODO.md’s `Publish 0.6.1` item does not mention it, so the only record is a commit message.

**Location:** `src/paths.rs:1118-1127, Cargo.toml:3, CHANGELOG.md:7-15, TODO.md:13-14`

**Evidence:** src/paths.rs:1124-1126: `let dotdot = |p: &&Path| matches!(p.components().next_back(), Some(Component::ParentDir)); if let Some(p) = to_create.iter().rev().copied().find(dotdot) { return Err(PathError::DotDotAfterMissing(...)) }`.  CHANGELOG 0.6.1: “A destination such as `new/../new`, which was built before, is refused too: spell it without the `..`.” CHANGELOG header: “this project adheres to Semantic Versioning”.  Commit 9ad890c: “F5, whether refusing a link-free `new/../new` is a fix or a narrowing that calls for 0.7.0, goes to the maintainer with the publish.” TODO.md:13: “Publish 0.6.1 ...  It waits on the maintainer.” (it does not mention the semver question).

**Severity:** contract

**Basis:** `doc-drift`, ruled by Chris (above), on the ask’s reasoning: a narrowing that only refuses, made to close a safety hole, ships as a patch, so that every dependent on `"0.6"` picks it up.  As filed, `inferred`: CHANGELOG.md:5 says “this project adheres to Semantic Versioning”, and the 0.6.1 entry says `new/../new` “is refused too”.  The repository does not say whether a bug fix that also refuses inputs which worked counts as a patch.  If the patch reading is right: the release can ship as is, and only the CHANGELOG line warns callers.  If the 0.7.0 reading is right: shipping 0.6.1 lets `cargo update` change exit 0 to exit 1 for any dependent that builds a `..` path under `All`, so the version should become 0.7.0 before publishing.  A narrower code fix (refuse only a `..` that climbs above the existing base, plus the output-file case) would avoid the narrowing but is more complex, so the broad refusal is the simpler fix.  Either way, the open question belongs on TODO.md’s publish line, not only in a commit message.

**Reproducer:** `cargo test paths::tests::semver_patch_keeps_building_a_link_free_dotdot_destination_that_0_6_0_built` failed on the reviewed tree.  The test is `bugs/bug-0042/reproducer.patch`; apply it with `git apply`.


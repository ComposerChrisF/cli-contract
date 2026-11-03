# bug-0048: Two tests in `tests/blackbox.rs` quote README sentences that the 0.6.2 fix rewrote

Filed by the nightly review on 3-Nov-2026.

Confirmed at tests/blackbox.rs:164 and 201-203: the quoted wording is no longer in the README. It has the same origin as 598ff9292ebd (commit 49244f8), but it is a separate report because correcting the README's line 203 leaves these test comments wrong, and correcting the comments leaves that line wrong.

From the nightly change review of `689040d3b001eded0f8096c4a9069eacd2bbae85..d7e4a64f0e41b7de64e12a8b4878e2ebd11c0cb9` on night `2026-11-03`.  Provenance `ai`: raised by opus:rust-reviewer; cross-model verdicts: fable agree, sol agree; reproduced by the harness, which ran the test below and saw it fail.  Ledger fingerprint `617e26e2c9fa`.

**Claim:** The header of tests/blackbox.rs says every test cites the sentence it pins. Commit 49244f8 rewrote two of the cited README sentences and left the quotes in the test comments unchanged. Those quotes now match no text in the README, and one of them states the narrower claim the fix made false. The tests themselves still pass, so only the comments are wrong.

**Location:** `tests/blackbox.rs:164, tests/blackbox.rs:201-203`

**Evidence:** tests/blackbox.rs:3: "Every test here cites the sentence it pins." tests/blackbox.rs:164: README: "only the identity check for `/Volumes` looks through a link". tests/blackbox.rs:202-203: "So under `All` a `..` after a level that does not exist yet is refused, as `PathError::DotDotAfterMissing`". The README now reads, at 146: "only the identity check for `/Volumes` and the check for a `..` below look through a link", and at 147-148: "So a `..` after a level that does not exist yet is refused: under `All`, as `PathError::DotDotAfterMissing`; ...". The diff of README.md in 49244f8 shows the old wording removed.

**Severity:** docs

**Basis:** `doc-drift`: Passages: tests/blackbox.rs:164 and 202-203, quoted above, under the file's own rule at line 3. The code and the current README are plainly the intended side, because 49244f8 changed that wording deliberately for the bug-0045 fix. The remedy is the two comments alone, with no change to any assertion.

**Reproducer:** `cargo test the_floor_tests_quote_sentences_the_readme_contains` failed on the reviewed tree.  The test is `bugs/bug-0048/reproducer.patch`; apply it with `git apply`.


# bug-0052: `tests/blackbox.rs:84` quotes the README's floor sentence without its qualifier “the floor recognizes”, and the quote guard added for bug-0048 cannot see that comment

Filed by the nightly review on 5-Nov-2026.

Same kind of fault as closed bug-0048, but a different site that report did not name: that fix corrected two tests' doc comments, and its guard checks only those two, so this `//` comment is untouched. The mismatch dates from f540451, before the 0.6.2 rewrite, so this is an uncovered site and not a regression of that fix.

From the nightly change review of `d7e4a64f0e41b7de64e12a8b4878e2ebd11c0cb9..1669ced80d6575821935cb0c98bac4e1dbd6f6d8` on night `2026-11-05`.  Provenance `ai`: raised by opus:rust-reviewer; cross-model verdicts: fable agree, sol agree; reproduced by the harness, which ran the test below and saw it fail.  Ledger fingerprint `fff0d1ca662f`.

**Claim:** The floor section's comment quotes the README as "No mode, not even `All`, will create a mount point.", but README.md:116 reads "will create a mount point the floor recognizes", followed by a pointer to Known limitations. The quote drops the qualifier that makes the promise true. This is the fault bug-0048 reported, at a third site. The guard test added by b02782a checks only two tests by name, and only their doc comments, so it does not reach this `//` comment.

**Location:** `tests/blackbox.rs:84, README.md:116, tests/blackbox.rs:660`

**Evidence:** tests/blackbox.rs:84: `// README: "No mode, not even `All`, will create a mount point."`. README.md:116: `**No mode, not even `All`, will create a mount point the floor recognizes** (what it does not recognize is under Known limitations)`. Scope of the audit: a Grep for `README[:,]` in tests/ (every hit read by hand against README.md) and a separate search for `will create a mount point`. Each other quote found that way matched. A comment citing the README in another form was not covered. A scripted check of all quotes was denied in this session. `git log -S` puts the README wording and this quote both at f540451, so the mismatch predates the range.

**Severity:** docs

**Basis:** `doc-drift`: Passage: tests/blackbox.rs header, "Every test here cites the sentence it pins", and the quote at line 84. The README is plainly the intended side: its sentence points to Known limitations, which list mount points the floor does not recognize (README.md:303-311), so the unqualified quote would be a false promise. The remedy is the comment alone.

**Reproducer:** `cargo test the_floor_section_comment_quotes_what_the_readme_says` failed on the reviewed tree.  The test is `bugs/bug-0052/reproducer.patch`; apply it with `git apply`.


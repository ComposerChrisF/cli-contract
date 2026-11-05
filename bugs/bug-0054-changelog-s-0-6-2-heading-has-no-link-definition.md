# bug-0054: CHANGELOG's `[0.6.2]` heading has no link definition, though 0.6.0 and 0.6.1 have theirs and the tag exists

Filed by the nightly review on 5-Nov-2026.

No open or closed report covers this, and the file's foot was read: the definitions stop at `[0.6.1]`. It is independent of the heading's date (d17b4a9358c1), since adding the definition leaves the date wrong.

From the nightly change review of `d7e4a64f0e41b7de64e12a8b4878e2ebd11c0cb9..1669ced80d6575821935cb0c98bac4e1dbd6f6d8` on night `2026-11-05`.  Provenance `ai`: raised by fable; cross-model verdicts: no second model answered; reproduced by the harness, which ran the test below and saw it fail.  Ledger fingerprint `3996c680d31f`.

**Claim:** The 0.6.2 heading uses reference-link syntax, `## [0.6.2] - 2026-11-02`, but the definitions at the foot of the file stop at `[0.6.1]`, so the heading renders as literal brackets and links nowhere. The 0.6.1 fix commit (9ad890c) added its definition with its entry; the 0.6.2 fix commit (49244f8, before this range) did not, and fcd7ebe, which records the release and the tag in this range, left it out too. It is on the same heading as R2, so one edit can carry both. The copy in the published 0.6.2 crate cannot change.

**Location:** `CHANGELOG.md:7, CHANGELOG.md:54-55`

**Evidence:** A Grep of CHANGELOG.md for `^\[0\.|^## \[` returns line 7 `## [0.6.2] - 2026-11-02`, line 13 `## [0.6.1] - 2026-11-01`, line 23 `## [0.6.0] - 2026-10-31`, line 54 `[0.6.1]: https://github.com/ComposerChrisF/cli-contract/releases/tag/cli-contract-v0.6.1`, and line 55 `[0.6.0]: .../cli-contract-v0.6.0`; there is no `[0.6.2]:` line. `git tag -l` shows cli-contract-v0.6.2, created 2026-11-04. `git log -S'releases/tag/cli-contract-v0.6.1' -- CHANGELOG.md` names 9ad890c alone. Whether the tag's page on GitHub resolves was not checked; the URL form is the one the two existing definitions use.

**Severity:** docs

**Basis:** `doc-drift`: Passage: CHANGELOG.md:5, "The format is based on Keep a Changelog", and line 7's `[0.6.2]`, which is a link reference with nothing to resolve to. The released state is plainly the intended side: the tag cli-contract-v0.6.2 exists, TODO.md:19 records the release, and the two earlier entries each define their link in the same form. No reading makes an undefined reference intended. The remedy is one line of text, `[0.6.2]: https://github.com/ComposerChrisF/cli-contract/releases/tag/cli-contract-v0.6.2`, above the `[0.6.1]` definition; no behavior changes. The omission is older than the range (49244f8).

**Reproducer:** `cargo test every_changelog_version_heading_has_a_link_definition` failed on the reviewed tree.  The test is `bugs/bug-0054/reproducer.patch`; apply it with `git apply`.


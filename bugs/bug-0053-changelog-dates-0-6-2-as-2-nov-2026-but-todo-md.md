# bug-0053: CHANGELOG dates 0.6.2 as 2-Nov-2026, but TODO.md and the tag put its release on 4-Nov-2026

Filed by the nightly review on 5-Nov-2026.

No open or closed report covers this; closed bug-0044 concerns the 0.6.1 entry's Windows wording, not a date. It shares a heading with the missing link definition (3996c680d31f), but correcting the date does not add the definition, so the two are separate problems.

From the nightly change review of `d7e4a64f0e41b7de64e12a8b4878e2ebd11c0cb9..1669ced80d6575821935cb0c98bac4e1dbd6f6d8` on night `2026-11-05`.  Provenance `ai`: raised by opus:rust-reviewer; cross-model verdicts: fable agree, sol agree; reproduced by the harness, which ran the test below and saw it fail.  Ledger fingerprint `d17b4a9358c1`.

**Claim:** fcd7ebe records 0.6.2 as on crates.io on 4-Nov-2026, and the tag was made that afternoon, but the CHANGELOG heading still gives the date of the fix commit. The 0.6.1 entry was deliberately corrected to its publication day, and 0.6.2 did not get the same correction. The wrong date is already in the 0.6.2 tagged tree, which the crate's `include` list ships CHANGELOG.md from, so a repository fix reaches the tarball only with the next release.

**Location:** `CHANGELOG.md:7, TODO.md:19`

**Evidence:** CHANGELOG.md:7: `## [0.6.2] - 2026-11-02`. TODO.md: `**0.6.2 is on crates.io** (4-Nov-2026), tagged `cli-contract-v0.6.2``. The tag object's tagger timestamp is 1793846712 -1000, which is 4-Nov-2026 at 4:45pm. Commit 0c19404: "Date the 0.6.1 CHANGELOG entry 1-Nov-2026, the day it is published". crates.io itself was not queried; the publication date comes from TODO.md and the tag.

**Severity:** docs

**Basis:** `doc-drift`: Passage: CHANGELOG.md:7, `## [0.6.2] - 2026-11-02`. The release record is plainly the intended side: the 4-Nov-2026 tag and TODO.md agree, and 0c19404 sets the convention that the entry carries the day of publication. The remedy is the heading's date alone.

**Reproducer:** `cargo test changelog_dates_each_release_the_day_it_was_published` failed on the reviewed tree.  The test is `bugs/bug-0053/reproducer.patch`; apply it with `git apply`.


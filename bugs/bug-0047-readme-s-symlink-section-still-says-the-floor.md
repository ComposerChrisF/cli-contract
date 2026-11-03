# bug-0047: README's symlink section still says the floor looks through a link only to recognize `/Volumes`, contradicting README.md:146 and the 0.6.2 `..` check

Filed by the nightly review on 3-Nov-2026.

Confirmed at README.md:203 against README.md:146. Closed bug-0045 made the code change that caused this drift but does not cover the sentence, and bug-0046 concerns dangling links in the code and does not cover it either. A bug-0046 fix may touch the same section, but fixing this sentence does not depend on it.

From the nightly change review of `689040d3b001eded0f8096c4a9069eacd2bbae85..d7e4a64f0e41b7de64e12a8b4878e2ebd11c0cb9` on night `2026-11-03`.  Provenance `ai`: raised by opus:rust-reviewer; cross-model verdicts: fable agree, sol agree; reproduced by the harness, which ran the test below and saw it fail.  Ledger fingerprint `598ff9292ebd`.

**Claim:** The 0.6.2 fix added a second check to the floor that follows links: `dotdot_after_missing` stats each level with `probe_target`. The fix updated README.md:146 and the rustdoc to say so, but left the symlink section's older sentence. The README now contradicts itself on what the floor looks through. `is_mount_root`'s rustdoc also sends readers to that section for symlink behavior.

**Location:** `README.md:202-204`

**Evidence:** README.md:202-203: "the mount-root floor looks through a link only to recognize `/Volumes`". README.md:146: "only the identity check for `/Volumes` and the check for a `..` below look through a link". src/paths.rs:960-962: "only the identity check for `/Volumes`, and the check for a `..` after a level that does not exist, look through a link ... see the README's symlink section". src/paths.rs:1038-1039: "Each probe is [`probe_target`]'s, which follows links, as a create does."

**Severity:** docs

**Basis:** `doc-drift`: Passage: README.md:203, quoted above. The code is plainly the intended side. Commit 49244f8 and CHANGELOG 0.6.2 add this check on purpose, on Chris's ruling recorded in the deleted bug-0045 report ("the floor fails closed"). The test `is_mount_root_refuses_a_dotdot_after_a_missing_level_that_hides_volumes` pins it, and README.md:146 was updated to match. The remedy is the sentence alone.

**Reproducer:** `cargo test the_readme_symlink_section_admits_the_dotdot_check_looks_through_a_link` failed on the reviewed tree.  The test is `bugs/bug-0047/reproducer.patch`; apply it with `git apply`.


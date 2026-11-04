# bug-0049: bug-0046 publishes a maintainer folder path in a roster address, which no planned leak-gate rule would catch

Filed by the nightly review on 3-Nov-2026.

**Ruled by Chris, 4-Nov-2026 (11:03am):** a roster address is a path on his drive (option A).  It stays out of public text, and the leak gate refuses it.  He added that a short form naming only the repository and the ask’s number is fine, but that the path from his home tree reveals too much of his disk layout.  The basis below is now `ruled`.

From the nightly change review of `689040d3b001eded0f8096c4a9069eacd2bbae85..d7e4a64f0e41b7de64e12a8b4878e2ebd11c0cb9` on night `2026-11-03`.  Provenance `ai`: raised by opus:cli-agent-first-auditor; cross-model verdicts: fable cannot-determine, sol cannot-determine; reproduced by the harness, which ran the test below and saw it fail.  Ledger fingerprint `30c8d63f718a`.

**Claim:** Commit d7e4a64 records the ruling with the ask’s roster address, `row:` followed by this repository’s folder path from the maintainer’s own tree, in a repository whose leak workflow says `bugs/` is published.  The deleted bug-0045 report carried the same form, and it remains in history.  plan-0019, filed in this same range, records a ruling that paths on the maintainer’s drive never go public, but its new generic rule matches only `~/`, `$HOME/`, `${HOME}/`, and `env!("HOME")`, so it would not see this form.  The address also resolves to nothing for a public reader.

**Location:** `bugs/bug-0046-is-mount-root-answers-false-for-a-dangling-link-into.md:5, plans/plan-0019-one-shared-leak-gate-which-also-reads-commit-and.md:9`

**Evidence:** bug-0046 line 5 gave the address after “He answered ask”.  plan-0019 gap 3: “The maintainer has ruled that paths on his drive never go public.”  .github/workflows/leak.yml: “TODO.md, bugs/, and plans/ are published too wherever no .leak-gate-exclude keeps them out”; no `.leak-gate-exclude` exists in the tree.  The gate was not run in this session (execution was denied) and the private list is not available here, so whether that list covers this form is unknown.  This report’s own title, claim, and evidence quoted the address in full until the ruling.

**Severity:** contract

**Basis:** `ruled`: Chris’s answer above.

**Fix:** bug-0046 records its ruling by date and option alone, as this report now does.  The shared leak gate refuses a roster address that spells a path, in a file and in a message; this repository takes that rule when plan-0019’s step 2 replaces `scripts/leak-gate` with the pinned shared copy, and the commit that does so closes this report.  The form stays in history, in the deleted bug-0045 report and in two commit messages (37ca584 and 189cc1a), since public `main` is only ever fast-forwarded.

**Reproducer:** `cargo test published_text_holds_no_roster_address_with_a_drive_path` failed on the reviewed tree.  The test is `bugs/bug-0049/reproducer.patch`; apply it with `git apply`.

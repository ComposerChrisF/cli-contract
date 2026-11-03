# bug-0049: bug-0046 publishes a maintainer folder path in a roster address (`row:App/Rust/cli-contract/0027`), which no planned leak-gate rule would catch

Filed by the nightly review on 3-Nov-2026.

**Spec question: Chris to rule.**  The repository’s contract does not settle that this is a defect: its basis is `inferred` (the evidence, and what follows if either side is right, is below).  Job 2 does not fix it until Chris rules.  To have it fixed as stated, he removes this paragraph and sets the basis on every **Basis:** line below that is not `spec` or `house-rule` to `ruled`, with why; if the code is the right side, he sets them to `doc-drift`, and the fix is to the text alone.

Spec question for Chris (basis `inferred`): does a roster row count as a path on his drive? plan-0019's home-relative rule matches only `~/`, `$HOME/`, `${HOME}/`, and `env!("HOME")`, so it does not cover this form. Fixing bug-0046 changes `is_mount_root` and would not settle the question, and the same form from deleted bug-0045 stays in history.

From the nightly change review of `689040d3b001eded0f8096c4a9069eacd2bbae85..d7e4a64f0e41b7de64e12a8b4878e2ebd11c0cb9` on night `2026-11-03`.  Provenance `ai`: raised by opus:cli-agent-first-auditor; cross-model verdicts: fable cannot-determine, sol cannot-determine; reproduced by the harness, which ran the test below and saw it fail.  Ledger fingerprint `30c8d63f718a`.

**Claim:** Commit d7e4a64 records the ruling with the ask address `row:App/Rust/cli-contract/0027`. That text holds a folder path from the maintainer's own tree, in a repository whose leak workflow says `bugs/` is published. The deleted bug-0045 report carried the same form in `row:App/Rust/cli-contract/0026`, and it remains in history. plan-0019, filed in this same range, records a ruling that paths on the maintainer's drive never go public, but its new generic rule matches only `~/`, `$HOME/`, `${HOME}/`, and `env!("HOME")`, so it would not see this form. The address also resolves to nothing for a public reader.

**Location:** `bugs/bug-0046-is-mount-root-answers-false-for-a-dangling-link-into.md:5, plans/plan-0019-one-shared-leak-gate-which-also-reads-commit-and.md:9`

**Evidence:** bug-0046 line 5: “He answered ask `row:App/Rust/cli-contract/0027`”. plan-0019 gap 3: “The maintainer has ruled that paths on his drive never go public.” .github/workflows/leak.yml: “TODO.md, bugs/, and plans/ are published too wherever no .leak-gate-exclude keeps them out”; no `.leak-gate-exclude` exists in the tree. The gate was not run in this session (execution was denied) and the private list is not available here, so whether that list covers this form is unknown.

**Severity:** contract

**Basis:** `inferred`: The contract does not say whether a roster row counts as a path on the drive. If the ruling covers it: drop the address from bug-0046, record the ruling by date alone, add a rule or list entry for `row:` paths, and decide whether to accept the form left in history, since rewriting public `main` is the maintainer's call. If a row is only an opaque roster key: nothing needs fixing in the code, but plan-0019 or the bug-report conventions should say that such an address may be published, so that later reviews stop raising it.

**Reproducer:** `cargo test published_text_holds_no_roster_address_with_a_drive_path` failed on the reviewed tree.  The test is `bugs/bug-0049/reproducer.patch`; apply it with `git apply`.


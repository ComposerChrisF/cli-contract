# bug-0055: TODO.md's open-work list omits plan-0019 and bugs 0049, 0050, and 0051, and does not say whether the list is meant to be complete

Filed by the nightly review on 5-Nov-2026.

**Spec question: Chris to rule.**  The repository’s contract does not settle that this is a defect: its basis is `inferred` (the evidence, and what follows if either side is right, is below).  Job 2 does not fix it until Chris rules.  To have it fixed as stated, he removes this paragraph and sets the basis on every **Basis:** line below that is not `spec` or `house-rule` to `ruled`, with why; if the code is the right side, he sets them to `doc-drift`, and the fix is to the text alone.

plan-0019, bug-0049, and bug-0051 describe the leak-gate work but none addresses TODO.md's listing, and TODO.md names none of these reports. The basis is `inferred`, so this is filed as a spec question for Chris: must the list be complete, or should it state its scope?

From the nightly change review of `d7e4a64f0e41b7de64e12a8b4878e2ebd11c0cb9..1669ced80d6575821935cb0c98bac4e1dbd6f6d8` on night `2026-11-05`.  Provenance `ai`: raised by opus:rust-reviewer; cross-model verdicts: fable agree, sol agree; reproduced by the harness, which ran the test below and saw it fail.  Ledger fingerprint `2b29e3227338`.

**Claim:** TODO.md opens as "The order of open work", and its `## Open plans` section lists seven of the eight files in plans/. plan-0019 has never appeared in TODO.md, although 1669ced says step 2 can now pin to the published gate and bug-0049 and bug-0051 both wait on that step. bug-0050 is unlisted as well, while bug-0046 has its own line. A reader taking TODO.md as the order of work would not find that the leak-gate switch is ready, or that two filed bugs depend on it.

**Location:** `TODO.md:3, TODO.md:37, plans/plan-0019-one-shared-leak-gate-which-also-reads-commit-and.md:27-28`

**Evidence:** `git log -S'plan-0019' -- TODO.md` returns nothing. The `Open plans` section lists plan-0010, 0011, 0012, 0014, 0015, 0017, and 0018. plan-0019 step 1: "so step 2 can now pin to it". bug-0049 and bug-0051 each say the commit for plan-0019's step 2 closes them. No line in TODO.md mentions any of these reports.

**Severity:** docs

**Basis:** `inferred`: TODO.md says "The order of open work" and "Open plans", but it does not state whether the list must cover every plan and bug, and the house rule names `bugs/` and `plans/` as the source for what is open. If TODO.md is meant to be complete, lines for plan-0019 and these bugs are missing, and Chris should place them because he owns the order. If leak-gate work is deliberately kept out, the code needs no change, but TODO.md should state that scope so its list does not read as complete, per strategy-defaults' "Every enumeration states its scope".

**Reproducer:** `cargo test todo_names_every_open_plan_and_bug` failed on the reviewed tree.  The test is `bugs/bug-0055/reproducer.patch`; apply it with `git apply`.


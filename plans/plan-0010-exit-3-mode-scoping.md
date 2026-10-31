# Plan: Mode-Scope Exit 3 — Findings Belong to Reporting Modes, Not Working Modes

Re-filed at the public launch (31-Oct-2026) from the predecessor repository, where it was filed on 9-Sep-2026.

## Problem

`exit::FINDINGS` and `exit::HELP_BLOCK` present 3 as the code for findings, unconditionally.  When this plan was filed they said nothing about **who 3 is for**, and that omission is load-bearing:

- Exit 3’s value is that it lets a caller distinguish “act on the findings” from “retry or debug the tool”.  That value exists **only for a caller that can act on the distinction**: an agent that reads `--help`, or an orchestrator written against this table.
- **To a generic runner, 3 is not a third answer; it is an unanswerable question.**  `make`, a shell script, a CI step, or a build orchestrator can branch only on zero or non-zero.  Handed a 3, it must guess whether the thing it needed was done.  Either guess is a guess.

A table stated without its audience invites a tool author to reach for 3 whenever the word “findings” fits, without asking whether the caller of _that mode_ can use it.

### The principle

> **Assign exit 3 only in modes whose _purpose_ is to report: `check`, `validate`, `--dry-run`, `status`.  In a mode that _does work_, a condition that blocks the work is a tool error (1), and a condition that does not block it is not a finding at all.**
>
> A generic runner invokes only working modes, and therefore never sees a 3.  **If a generic runner would see a 3, the assignment is wrong.**  That is the diagnostic, and it is the useful half.

### The case that produced it

A tool’s dependency-scan mode emits a list of input files for a generic build runner, which treats any non-zero exit from a dependency program as a hard rule failure and does not special-case 3.  The scan silently dropped file references it could not resolve, where its spec said to record them.  The proposed fix recorded the reference, marked the list incomplete, and exited 3, reasoning from the rule that a result the tool could not determine must be loud and non-zero.  An audit of the caller stopped it: the change would have turned every input carrying a stale reference from a working build into a failed one.

The decision that followed is the origin of this plan.  A generic runner branches on zero or non-zero because that _is_ the standard when one knows nothing about the tool one is running.  Exit 3 is meant for a caller such as an agent, for whom 0 would hide a caveat and a non-zero code prompts a look.  The build runner has no such choice: what would 3 _mean_ to it?  So the scan makes a best effort and exits 0, with success defined to include imperfect information.

Two corrections fell out of it, and both generalize:

1. **The condition was not a finding.**  An unresolvable reference is the _normal output_ of a dependency scan, not a defect the scan discovered.  The mis-assignment was the bug; the caller’s inability to interpret it was only the symptom that exposed it.
2. **The non-zero half of the loudness rule is scoped to destruction.**  Its subject is a probe gating a delete or an overwrite.  What generalizes from it is **loudness**: name the thing, and put it in the output.  The non-zero exit does not carry over to a tool that destroys nothing.  Misreading that is exactly what produced the wrong proposal.

Tools built on this table had already found the principle independently.  One says it in its own `--help`: 3 is for `--check` alone, and an update treats the same validation problems as a tool error, since they block the write.  The same condition is 3 in the reporting mode and 1 in the working mode, decided by what the mode is for.  Another reaches the same place from the other side: it documents its skip-because-newer case as a legitimate no-op, not a finding, and uses no 3 at all.

## Proposed Change

> **Status, 30-Oct-2026: steps 1 and 2 landed before the launch.**  `exit::FINDINGS`’s rustdoc carries the principle, the diagnostic, and the destruction-scoping note.  `HELP_BLOCK` is unchanged.  Step 3 is open.

1. **Document the principle on `exit::FINDINGS`**, where an implementer will read it: the principle, the diagnostic, and the note that the non-zero half of the loudness rule is scoped to destruction.  _Done._
2. **Decide whether `HELP_BLOCK` changes.**  It is emitted verbatim into every adopting tool’s `--help`, so any edit is user-visible text in every tool, and must be worth it.  The candidate was qualifying line 3 as “findings (a reporting mode ran and found problems: …)”.  _Decided: unchanged._  The principle lives in the rustdoc, and a tool whose modes split the code says so in its own `--help`, as the tool above does.
3. **Decide what, if anything, enforces the assignment.**  `exit.rs`’s tests pin the numbers; nothing pins the _assignment_, and nothing in this crate can pin it mechanically, since it is a property of each consumer.  A test is likely the wrong tool, and the enforcement probably lies outside the crate, in each tool’s own `--help` exit table and its review.  Weigh it rather than assume a test.

## Implementation Notes

- An implementer reading `exit::FINDINGS` should learn who 3 is for, and the “would a generic runner see this?” diagnostic, without reading this plan.  The landed rustdoc does; keep it so if `exit.rs` changes.
- **Considered and rejected:** a caller-side switch to “collapse 3 to 0 or 1 for generic runners”.  It is recorded here so it is not re-proposed.
  - **No correct universal meaning.**  `3 → 0` asserts that findings are success; `3 → 1` asserts that they are failure.  Which is right depends on the finding, so the switch moves the ambiguity into the flag rather than resolving it.
  - **It is caller-side configuration of callee semantics.**  The caller would assert “treat your findings as success” while being, by construction, the party unable to evaluate them.
  - **Nothing to collapse.**  Where it was checked, the tools a generic build runner invokes already return 3 only from reporting modes the runner never calls, _because_ of the principle this plan writes down.
  - The legitimate adjacent need, “report findings but do not block”, is a **policy** question, and deserves a policy flag named for what it does (`--warn-only`), not an exit-code remapper.
- When this plan closes, the rejected switch and its reasons graduate into the `exit` module docs beside the principle, so they outlive the plan.

## Why Not a Workaround

The per-tool workaround is what every tool did before: work it out again, correctly or not, from a table that does not mention the constraint.  One tool got it right and wrote it into its own `--help`; another got it wrong, and was one caller audit away from breaking a working build.  That spread is the argument for the contract crate owning the principle—the same argument the mount-root floor is centralized on, and the reason this crate exists.

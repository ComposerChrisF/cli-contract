//! Canonical process exit codes.
//!
//! A small, opinionated convention this crate proposes: a tool that redefines these locally and
//! inconsistently makes it impossible for a caller (a script, an agent, a CI gate) to branch on the
//! code mechanically.  Centralizing them here — one meaning per code — is what makes that branch
//! reliable across every tool that adopts them.  The one load-bearing rule is that **findings never
//! exit 1**: exit 1 must always mean "the tool itself failed", which is what lets a caller
//! distinguish "act on the findings" from "retry or debug the tool".

/// Success / clean — including a dry-run and a legitimate no-op.
pub const OK: i32 = 0;
/// The tool itself failed: I/O, a missing caller-asserted path, a parse failure of its own input.
///
/// This is also the caller-claim/world-mismatch code, in either direction (a missing input that
/// should exist, a missing output directory in must-exist mode, an existing target under
/// no-clobber) — see [`crate::paths`].
pub const TOOL_ERROR: i32 = 1;
/// Usage error — bad command-line arguments (clap's default; never repurpose it).
pub const USAGE: i32 = 2;
/// Findings — the tool ran correctly and found problems (validation failures, drift, staleness).
/// Findings never exit 1.
///
/// **Exit 3 belongs to modes whose purpose is to report**: `check`, `validate`, `--dry-run`,
/// `status`.  In a mode that does work, a condition that blocks the work is a tool error
/// ([`TOOL_ERROR`]), and a condition that does not block it is not a finding at all, but usually
/// the mode's own normal output.  A dependency scan that meets a reference it cannot resolve, for
/// example, lists it and exits 0.
///
/// The reason is the audience.  3 is worth having because it separates "act on the findings" from
/// "retry or debug the tool", and only a caller that can act on that difference can use it: an
/// agent that reads `--help`, or an orchestrator written against this table.  A generic runner
/// (`make`, a shell script, a CI step) branches only on zero or non-zero, so handed a 3 it must
/// guess whether the thing it needed was done.  Hence the diagnostic: **a generic runner invokes
/// only working modes, so it should never see a 3; if it would, the assignment is wrong.**
///
/// The same reasoning scopes the rule that a result the tool could not determine must be loud.
/// Where a probe gates a delete or an overwrite, "I could not tell" stops the run with a non-zero
/// exit.  In a tool that destroys nothing, what carries over is the loudness, naming the thing in
/// the output, and not the non-zero exit.  That is about a result found during the run.  A
/// caller-asserted input the tool cannot read stops the run, and is a tool error (exit 1) in any
/// tool, as the path contract says.
pub const FINDINGS: i32 = 3;
/// Setup needed — no config/ledger/manifest; not initialized; missing prerequisite state.
pub const SETUP: i32 = 4;

/// The canonical exit-code table, ready to hand to clap's `after_long_help` so every tool documents
/// the same semantics verbatim.
pub const HELP_BLOCK: &str = "\
EXIT CODES:
  0  success / clean (including dry-run and a legitimate no-op)
  1  tool error (I/O, a missing input path, a parse failure of the tool's own input)
  2  usage error (bad command-line arguments)
  3  findings (the tool ran and found problems: validation failures, drift, staleness)
  4  setup needed (missing config or prerequisite state; not initialized)";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_the_canonical_table() {
        assert_eq!((OK, TOOL_ERROR, USAGE, FINDINGS, SETUP), (0, 1, 2, 3, 4));
    }

    #[test]
    fn help_block_documents_every_code() {
        for line in [
            "0  success",
            "1  tool error",
            "2  usage error",
            "3  findings",
            "4  setup",
        ] {
            assert!(HELP_BLOCK.contains(line), "help block missing: {line}");
        }
    }
}

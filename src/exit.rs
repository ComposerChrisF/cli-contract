//! Canonical process exit codes.
//!
//! A small, opinionated convention this crate proposes: a tool that redefines these locally and
//! inconsistently makes it impossible for a caller (a script, an agent, a CI gate) to branch on the
//! code mechanically.  Centralizing them here — one meaning per code — is what makes that branch
//! reliable across every tool that adopts them.  The one load-bearing rule is that **findings never
//! exit 1**: exit 1 must always mean "the tool itself failed", which is what lets a caller
//! distinguish "act on the findings" from "retry or debug the tool".
//!
//! The codes are `u8`, which [`std::process::ExitCode`] takes through `From<u8>`;
//! `std::process::exit` takes one with `.into()`.
//!
//! [`HELP_BLOCK`] documents the five for `--help`.  A tool with something to add under one of them
//! (the usage errors it checks itself), codes of its own above them, or a mode that gives a code a
//! meaning of its own builds its table with [`ExitTable`] instead.

use std::collections::BTreeMap;

/// Success / clean — including a dry-run and a legitimate no-op.
pub const OK: u8 = 0;
/// The tool itself failed: I/O, a missing caller-asserted path, a parse failure of its own input.
///
/// This is also the caller-claim/world-mismatch code, in either direction (a missing input that
/// should exist, a missing output directory in must-exist mode, an existing target under
/// no-clobber) — see [`crate::paths`].
pub const TOOL_ERROR: u8 = 1;
/// Usage error — bad command-line arguments (clap's default; never repurpose it).
pub const USAGE: u8 = 2;
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
pub const FINDINGS: u8 = 3;
/// Setup needed — no config/ledger/manifest; not initialized; missing prerequisite state.
pub const SETUP: u8 = 4;

/// The canonical exit-code table, ready to hand to clap's `after_long_help` so every tool documents
/// the same semantics verbatim.  It is what [`ExitTable::canonical`] renders.
pub const HELP_BLOCK: &str = "\
EXIT CODES:
  0  success / clean (including dry-run and a legitimate no-op)
  1  tool error (I/O, a missing input path, a parse failure of the tool's own input)
  2  usage error (bad command-line arguments)
  3  findings (the tool ran and found problems: validation failures, drift, staleness)
  4  setup needed (missing config or prerequisite state; not initialized)";

/// The five canonical codes with their meanings, as [`HELP_BLOCK`] words them.
const CANONICAL: [(u8, &str); 5] = [
    (
        OK,
        "success / clean (including dry-run and a legitimate no-op)",
    ),
    (
        TOOL_ERROR,
        "tool error (I/O, a missing input path, a parse failure of the tool's own input)",
    ),
    (USAGE, "usage error (bad command-line arguments)"),
    (
        FINDINGS,
        "findings (the tool ran and found problems: validation failures, drift, staleness)",
    ),
    (
        SETUP,
        "setup needed (missing config or prerequisite state; not initialized)",
    ),
];

/// The highest code a tool may return.  Above it the shell reports conditions of its own: 126, not
/// executable; 127, not found; 128 + n, killed by signal n.
const HIGHEST: u8 = 125;

/// A tool's exit-code table for `--help`: the five canonical codes, with what the tool has to add
/// under them; the codes the tool adds above them; and the table of each mode that has codes of
/// its own to explain.
///
/// A tool declares each extra code once, as a constant beside its table, so the code it returns
/// and the code its `--help` documents are the same value.  The canonical five take their meanings
/// from this crate, so every tool words them alike; [`canonical`](Self::canonical) renders as
/// exactly [`HELP_BLOCK`].
///
/// ```
/// use cli_contract::exit::{ExitTable, FINDINGS, TOOL_ERROR, USAGE};
///
/// const LOCKED: u8 = 5;
///
/// let help = ExitTable::canonical()
///     .detail(USAGE, "also --fix with --check, which the tool refuses itself")
///     .code(LOCKED, "lock held by another run")
///     .mode("--check", &[(FINDINGS, "drift found"), (TOOL_ERROR, "the check could not run")])
///     .render();
/// assert_eq!(help, "\
/// EXIT CODES:
///   0  success / clean (including dry-run and a legitimate no-op)
///   1  tool error (I/O, a missing input path, a parse failure of the tool's own input)
///   2  usage error (bad command-line arguments)
///      also --fix with --check, which the tool refuses itself
///   3  findings (the tool ran and found problems: validation failures, drift, staleness)
///   4  setup needed (missing config or prerequisite state; not initialized)
///   5  lock held by another run
///
/// EXIT CODES (--check):
///   1  the check could not run
///   3  drift found");
/// ```
///
/// A table is written from literals, so a code it cannot hold is a mistake in the tool's source,
/// and the methods that add one panic.  They run when the table is built, which for a table handed
/// to clap is every run of the tool, so the tool's first test that runs it finds the mistake.
///
/// A code the tool returns without declaring it here is not caught by the table itself.  With the
/// `testing` feature, each of its contract helpers checks the code of its own run against the
/// table, and no other run's.
#[derive(Debug, Clone)]
pub struct ExitTable {
    codes: BTreeMap<u8, String>,
    modes: Vec<(String, BTreeMap<u8, String>)>,
}

impl ExitTable {
    /// The five canonical codes and nothing else.  Rendered, it is [`HELP_BLOCK`].
    #[must_use]
    pub fn canonical() -> Self {
        Self {
            codes: CANONICAL
                .iter()
                .map(|&(code, meaning)| (code, meaning.to_owned()))
                .collect(),
            modes: Vec::new(),
        }
    }

    /// Adds lines under one of the canonical five, for what the code covers in this tool: the
    /// usage errors it checks itself beyond clap's (two flags that contradict each other), which a
    /// tool names under 2, or that the tool never returns the code.  The canonical meaning stays
    /// first and unchanged, and each call adds its lines below the ones before.
    ///
    /// # Panics
    ///
    /// If `code` is not from 0 to 4.  A code of the tool's own takes all its lines in
    /// [`code`](Self::code).
    #[must_use]
    pub fn detail(mut self, code: u8, text: impl Into<String>) -> Self {
        assert!(
            code <= SETUP,
            "exit code {code} is not canonical, so its lines go in code()"
        );
        let meaning = self
            .codes
            .get_mut(&code)
            .expect("canonical() holds 0 to 4, and no method removes a code");
        meaning.push('\n');
        meaning.push_str(&text.into());
        self
    }

    /// Adds a code of the tool's own to the main table.
    ///
    /// # Panics
    ///
    /// If `code` is from 0 to 4, whose meanings are canonical; above 125, where the shell's own
    /// codes are; or already in the table.
    #[must_use]
    pub fn code(mut self, code: u8, meaning: impl Into<String>) -> Self {
        assert!(
            code > SETUP,
            "exit code {code} is canonical, so its meaning is fixed"
        );
        assert!(
            code <= HIGHEST,
            "exit code {code} is above {HIGHEST}, where the shell's own codes are"
        );
        let added = self.codes.insert(code, meaning.into()).is_none();
        assert!(added, "exit code {code} is already in the table");
        self
    }

    /// Adds the table of a mode that has codes of its own to explain, rendered after the main table
    /// under the heading `EXIT CODES (<name>):`.  It lists the codes the mode has something to say
    /// about: one of the tool's own, which may be listed here alone when only this mode returns
    /// it, or a canonical one with what it means in that mode, such as `--check` exiting 3 for
    /// drift.  A mode may give a canonical code a meaning of its own, but not 1 or 2: 1 is a tool
    /// error in every mode, since findings never exit 1, and clap exits with 2 before any mode
    /// runs, so a mode lists 2 only for usage errors of its own.
    ///
    /// # Panics
    ///
    /// If `codes` is empty, a code is above 125, a code appears twice in `codes`, or the table
    /// already has a mode of this name.
    #[must_use]
    pub fn mode(mut self, name: impl Into<String>, codes: &[(u8, &str)]) -> Self {
        let name = name.into();
        assert!(
            self.modes.iter().all(|(have, _)| *have != name),
            "the table already has mode {name}"
        );
        assert!(!codes.is_empty(), "mode {name} lists no codes");
        let mut table = BTreeMap::new();
        for &(code, meaning) in codes {
            assert!(
                code <= HIGHEST,
                "exit code {code} of mode {name} is above {HIGHEST}, where the shell's own codes are"
            );
            let added = table.insert(code, meaning.to_owned()).is_none();
            assert!(added, "exit code {code} is twice in mode {name}");
        }
        self.modes.push((name, table));
        self
    }

    /// Whether the table lists `code`: one of the canonical five, one of the tool's own, or one a
    /// mode lists.
    #[cfg(feature = "testing")]
    pub(crate) fn lists(&self, code: u8) -> bool {
        self.codes.contains_key(&code)
            || self
                .modes
                .iter()
                .any(|(_, codes)| codes.contains_key(&code))
    }

    /// The table as `--help` text, for clap's `after_long_help`: the main table, then each mode's
    /// in the order they were added.  Each section is sorted by code, and the codes are aligned on
    /// the widest in the whole table, so a code of 10 or more moves the canonical rows right too, a
    /// column for each digit past the first.  A meaning that spans lines, in any section, has each
    /// later line indented under the first.  Like [`HELP_BLOCK`], it ends without a newline.
    #[must_use]
    pub fn render(&self) -> String {
        let width = self
            .codes
            .keys()
            .chain(self.modes.iter().flat_map(|(_, codes)| codes.keys()))
            .map(|code| code.to_string().len())
            .fold(1, usize::max);
        let mut out = String::from("EXIT CODES:");
        render_section(&mut out, &self.codes, width);
        for (name, codes) in &self.modes {
            out.push_str(&format!("\n\nEXIT CODES ({name}):"));
            render_section(&mut out, codes, width);
        }
        out
    }
}

/// One line per code, `  <code>  <meaning>`, with the codes right-aligned to `width` and each later
/// line of a meaning indented under its first.
fn render_section(out: &mut String, codes: &BTreeMap<u8, String>, width: usize) {
    let indent = format!("\n{:1$}", "", width + 4);
    for (code, meaning) in codes {
        let meaning = meaning.replace('\n', &indent);
        out.push_str(&format!("\n  {code:>width$}  {meaning}"));
    }
}

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

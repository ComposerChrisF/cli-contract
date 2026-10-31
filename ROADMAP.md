# cli-contract — Roadmap

What the first releases deliberately leave out (they cover the path contract, the exit codes, and
BrokenPipe), and the design questions carried forward.

## Where the reasoning lives

The crate’s own docs carry the contract and its justification: why must-exist is the default, why
an output’s parent is a caller-asserted location, why the mount-root floor exists, and the
alternatives rejected (create-if-missing as the default, silently ignoring a marker, booleans
instead of an enum).  Today that is the README and the rustdoc; a `DESIGN.md` can gather it if it
outgrows them.

## Candidate additions

The framing: **an agent-first CLI, as code**.  A CLI that scripts and AI agents drive needs
documented exit codes, machine-readable output, and safe defaults, and a crate can make a tool
follow those conventions by construction.  Beyond the path, exit, and pipe modules, the candidates,
roughly by how often a tool would reuse them:

- **Config discovery + `--show-config`** (high value): the walk-up `.toolname.toml` discovery with
  documented precedence, reimplemented in nearly every tool and easy to get subtly wrong.  A
  `discover_config(name)` plus a `--show-config` renderer.
- **`--help` section builders**: assemble `after_long_help` from standard blocks (the exit-code
  table, a config-discovery block, a suppression block) so `--help` reads uniformly across tools.
  `exit::HELP_BLOCK` is the first such block.
- **Findings accumulator → exit 3 + `--json`**: a small type that collects findings and yields exit 3
  (never 1) with schema-stable JSON, so “findings never exit 1” holds structurally.
- **`--dry-run` convention**: a flag plus a “would do X” reporting helper for mutating tools.
- **stdout/stderr discipline**: thin wrappers that keep data on stdout and findings/errors on stderr,
  making the convention hard to violate.
- **Adopt/bootstrap**: an “audible first-run seed” helper for tool-owned config/state.
- **Status/freshness**: a convention for a cheap `status` subcommand an orchestrator can gate on.

Not all of these belong in one crate.  Some are better expressed as docs, or as a project template,
than as code: decide per item whether it is a helper (code) or a rule (doc).

## Open design question carried forward

- **Marker location.**  The marker is currently checked _in the destination directory itself_
  (`ensure_destination` / `DestPolicy::Marked`).  A real-world backup guard may instead check a
  marker on the _volume root_, with the destination deeper.  Must-exist alone does not identify
  the disk: a replacement disk with the same folders, or a copy left on the disk under an unmounted
  volume, passes it.  So the marker is the wrong-disk guard, and today it goes in each destination.
  If that proves awkward, or explicit “bless the volume” semantics turn out to matter, revisit
  checking the marker on a named ancestor (the “which ancestor?” question is why it was not
  done in v0.1).

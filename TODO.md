# TODO

The order of open work.  The detail lives in each plan under `plans/`; a bug’s report lives under
`bugs/` until the commit that fixes it deletes it.  This repository was cut from a private
predecessor at 0.5.24, so IDs continue from `bugs/FLOOR` and `plans/FLOOR`.

## The release

- [ ] **Publish 0.6.0 to crates.io.**  The release commit:
  - turns the `clap` feature off by default, with the README’s Features line and the manifest’s
    comment on the interim default;
  - adds `#[non_exhaustive]` to `SymlinkInfo` and `DestPolicy`;
  - makes the exit constants and both `exit_code()` methods `u8`;
  - and drops `publish = false`.

  After publishing, check that docs.rs renders the page with the clap impls visible, and that the
  `.crate`’s normalized `Cargo.toml` says `default = []`.  Then set the semver baseline, so every
  later release is checked against 0.6.0.

## Open plans

- [ ] [plan-0010](plans/plan-0010-exit-3-mode-scoping.md) — **mode-scope exit 3**: findings codes
  belong to reporting modes (`check`, `validate`, `--dry-run`, `status`), never to modes that do
  work, since a generic runner can branch only on zero or non-zero.  The principle is in
  `exit::FINDINGS`’s rustdoc, and `HELP_BLOCK` stays unchanged; what enforces the assignment is
  open.

- [ ] [plan-0011](plans/plan-0011-stderr-writer-that-never-panics.md) — **a stderr writer that
  never panics**: `errln!` / `errprint!`, which ignore every stderr write error and evaluate to
  `()`, because `eprintln!` exits 101 when stderr’s reader has gone, and a mid-run notice then
  stops a mutating run partway.

- [ ] [plan-0012](plans/plan-0012-linux-mount-roots-by-identity.md) — **the Linux mount roots by
  identity**, as `/Volumes` is recognised by device and inode, so a symlinked or bind-mounted
  `/mnt` cannot pass the floor.  The README lists the gap under Known limitations.

- [ ] [plan-0013](plans/plan-0013-check-output-file-the-dry-run-half.md) — **`check_output_file`**,
  the dry-run half of `prepare_output_file`, which creates parent directories under `Final` and
  `All`, so a dry run over output files writes to disk.  Additive.  Listed under Known
  limitations.

- [ ] [plan-0014](plans/plan-0014-a-create-below-a-mount-point-that-outlives-its-volume.md) —
  **a create below a mount point that outlives its volume**: on Linux an fstab, systemd, bind, or
  hand-made mount point stays after the unmount, so `Final` and `All` build the backup on the root
  filesystem one level down.  A marker on an ancestor, or proof of a live mount.  Listed under
  Known limitations.

- [ ] [plan-0015](plans/plan-0015-the-floors-three-state-answer-in-public.md) — **the floor’s
  three-state answer, in public**: `is_mount_root` folds “could not tell” into `true`, so a caller
  cannot say why it refused.  Make the private `Result<bool, PathError>` form public.  Additive.

- [ ] [plan-0016](plans/plan-0016-a-public-marker-name-check.md) — **a public marker-name check**,
  so a tool can refuse an input directory’s marker as a usage error without building a throwaway
  `DestPolicy`.  Additive.

## Smaller items

- [ ] **The leak gate trusts a lowered floor.**  It reads `bugs/FLOOR` and `plans/FLOOR` from the
  index, so one commit can lower a floor and cite the IDs that newly pass.  A floor is never
  lowered, so the gate could compare the index’s floor with `HEAD`’s and stop on a lower one.
  That adds to its contract, so it wants a plan.

- [ ] **Nits from the pre-launch audit**, none a broken promise:
  - `Undeterminable`’s rustdoc says “see the module docs”, which say nothing about it;
  - `require_input_dir` probes the directory before it checks the marker’s name;
  - `create_after_check`’s bare `is_dir()` is correct, but reads as the bool collapse the module
    forbids;
  - the README cites `ROADMAP.md`, which the package leaves out;
  - `floor_check_chain` re-walks the missing ancestors once per level, O(k²);
  - the marked paths probe the directory twice;
  - `examples/destination` cannot take a directory whose name begins with a dash;
  - the leak gate takes an empty `LEAK_GATE_PATTERNS` as unset, so it reads the default list;
  - `--require-private` exits 4 before the findings summary and exit 3.

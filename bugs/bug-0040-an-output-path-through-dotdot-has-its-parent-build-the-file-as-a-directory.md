# bug-0040: An output file path through `..` can have its own parent chain build the file’s path as a directory

**Severity:** low.  The failure is loud and loses no data, but it breaks a promise the rustdoc makes.

## What happens

Given the output path `a/../a`, with `a` absent, under `CreateMode::All`:

```text
check:   Ok(())
prepare: Ok(())
a is a dir now: true
write: Err(IsADirectory)
again: Err(NotAFile("a/../a"))
```

`check_output_file` and `prepare_output_file` both approve.  `prepare_output_file` then builds the parent, `a/..`, which creates `a`: the file’s own path, as a directory.  The caller’s write fails with `IsADirectory`, and a second call refuses the path as `NotAFile`.

## The promise it breaks

`prepare_output_file`’s rustdoc says it enforces “the clobber policy on the file itself, then” ensures its parent directory, and “returns once the parent is ready and the clobber check passes”.  Here the parent’s creation itself puts a directory where the file goes, so by the time it returns `Ok`, its own check would refuse the path as `NotAFile`.  `check_output_file` returns the same `Ok`, as the dry run of a call that cannot leave the file writable.

## Why

The leaf is classified before the parent is built, against the path as spelled.  `a/../a` resolves to `a`, which is absent, so the leaf passes.  The parent chain `a/..` approves `a` as a level to create, and the OS resolves `..` after that link-free `a`, so the file’s path is now a directory.  A destination directory spelled the same way (`x/../x`) ends correctly, since the directory it wanted is the one built; only a file collides with its own parent chain.

## Reproducer

`bugs/bug-0040/repro-main.rs`: a binary that depends on this crate by path, run with a scratch directory as its argument.  The property test `check_and_prepare_agree_on_every_output_file_shape_mode_clobber_and_spelling` in `tests/check_equals_ensure.rs` also reaches it, and skips its “check approves again after prepare” assertion for exactly this case.

## Fix direction

Refuse, in the shared leaf check, an output path whose own resolved location is a level the parent chain would create: compare the lexically absolute leaf with the levels `approved_levels` returns.  Both halves then refuse it the same way, with no write.  A plainer alternative is to refuse any output path whose parent contains `..`, which is wider than the fault.  Either needs a test that fails with the fix reverted.

**Amended 31-Oct-2026:** bug-0041 has the same cause, a `..` after a level not yet built, which no check can resolve before the create.  There it hides `/Volumes` from the floor.  The leaf comparison above would fix only this report, so the fix is bug-0041’s instead: under `All`, refuse a chain one of whose missing levels is a `..`.  `a/../a`’s parent chain, `a/..`, has one.

**Amended 31-Oct-2026, from review:** the same fault reaches an output file whose own last component is the `..`, under `Final` as well as `All`.  With `x` present and `x/a` absent, `x/a/..` is approved by both halves, since the chain is given only the parent `x/a`.  `prepare_output_file` builds `x/a`, the path then names `x`, a directory, and the write fails with `IsADirectory`.  So the leaf check refuses an absent leaf that ends in `..` too.

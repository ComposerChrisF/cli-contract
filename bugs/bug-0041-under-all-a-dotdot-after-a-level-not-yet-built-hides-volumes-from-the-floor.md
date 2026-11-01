# bug-0041: Under `All`, a `..` after a level not yet built hides `/Volumes` from the floor

**Severity:** medium.  The mount-root floor is the contract’s one unconditional promise, and here it approves a mount point.  Reaching it takes a spelling nobody types by hand, and root to create in `/Volumes`.

## What happens

On macOS, with `to_usr` a symbolic link to `/usr`, under `CreateMode::All`:

```text
check_destination(to_usr/../Volumes/cli_contract_bb, All): Err(MountRoot(…))
check_destination(to_usr/zz/../../Volumes/cli_contract_bb, All): Ok(())
check_output_file(to_usr/zz/../../Volumes/cli_contract_bb, All, Allow): Ok(())
```

Once `zz` exists, the second and third paths are `/Volumes/cli_contract_bb`: the OS resolves `to_usr` to `/usr`, `zz/..` back to `/usr`, and the second `..` to `/`.  The first path, with no missing level before its `..`, is refused as it should be.  The live run builds `to_usr/zz`, then `/Volumes/cli_contract_bb`: a mount point, as a directory or, for the output file, as a file.  Without root the `/Volumes` create fails for permissions, but through a link to a writable directory the levels before it are built first.

## The promise it breaks

The README’s floor section: “only the identity check for `/Volumes` looks through a link, and it still sees the directory the OS will create in”.  Its Known limitations: “macOS’s `/Volumes` is recognized by identity and has no such gap”.

## Why

`floor_check_chain` walks the spelled path up to the deepest directory that exists, then checks each missing level.  A level after `zz/..` cannot be resolved while `zz` is absent.  Its stat answers `NotFound`, and so does its parent’s, which the identity check takes as proof the level is neither `/Volumes` nor a child of it (“an absent path or parent proves that it is not it”).  That `NotFound` is true only until `zz` is built.  Then `..` resolves from wherever `zz` really is, and the rest of the path from there, through any link it meets.  The lexical list sees `<workspace>/Volumes/cli_contract_bb`, which is not a mount root.  So the check judges a spelling that the OS has not resolved yet, and cannot until the create.

bug-0040 has the same cause, seen by an output file’s own check rather than by the floor.

## Reproducer

`bugs/bug-0041/repro-main.rs`: a binary that depends on this crate by path, run with a scratch directory holding `to_usr`, a link to `/usr`.  It only checks, and creates nothing.

## Fix direction

Under `All`, refuse a chain one of whose missing levels is a `..`.  That is exactly a `..` after a level not yet built, and nothing about where it leads can be checked first.  It closes bug-0040 too, since that file’s parent chain, `a/..`, has one.  The alternative, simulating the OS’s resolution through the levels to be built and the links beyond them, puts a path resolver in the floor’s path.  Either needs a test that fails with the fix reverted.

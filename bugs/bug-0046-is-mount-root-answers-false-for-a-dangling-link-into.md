# bug-0046: `is_mount_root` answers `false` for a dangling link into an absent `/Volumes/<name>`, though `mkdir` on the link, spelled with a trailing separator, creates that mount point

Filed by the review of the bug-0045 fix on 2-Nov-2026.

**Spec question: Chris to rule.**  The contract does not settle that this is a defect: its basis is `inferred` (the evidence, and what follows if either side is right, are below).  It is not fixed until Chris rules.  To have it fixed as stated, he sets the basis below to `ruled`, with why; if the code is the right side, he sets it to `doc-drift`, and the fix is to the text alone.

It was there before 0.6.1, and the fix for bug-0045 does not touch it: that fix is about a `..` after a level that does not exist, and this path has none.  plan-0015 (the floor’s three-state answer in public) and plan-0017 (a three-state symlink-facts primitive) do not cover it either.

Provenance `ai`: raised by the reviewer of the bug-0045 fix.  Reproduced by hand on macOS (below).

**Claim:** with `link` a symbolic link to `/Volumes/X`, and `/Volumes/X` absent, `is_mount_root(link)` and `is_mount_root("link/")` both answer `false`.  But on macOS `mkdir("link/")` creates the link’s target, and so does `std::fs::create_dir_all("link/")`, whose first step is that `mkdir`.  So a caller that gates its own create on `is_mount_root`, run as root, makes `/Volumes/X`: the mount point the floor exists to keep from being made.  Only a spelling that makes the lookup follow the link (`link/`, `link/.`) does so: `mkdir("link")` fails with `EEXIST`, and so does `create_dir_all` on `link` or on a path below it.  The contract functions refuse the same path: `check_destination` under `Final` and `All` returns `DanglingSymlink`.

**Location:** `src/paths.rs`, `is_volumes_or_a_child_of_it` (and `is_mount_root`, which reads it)

**Evidence:**
- A scratch program against the working tree, on macOS, with `link` → `/Volumes/cli_contract_f7_absent`: `is_mount_root(link) = false`, `is_mount_root(link/) = false`; `check_destination(.., Create(Final))` and `Create(All)`, for both spellings: `Err(DanglingSymlink(..))`.
- In a scratch folder, with `link` → an absent `absent_target`: `mkdir link` fails with “File exists”, and `mkdir link/` succeeds and creates `absent_target`.  Python’s `os.mkdir("l2/")` does the same.  `create_levels`’s own comment says it too: “`mkdir("link/")` creates a dangling link’s target”.
- A scratch program, with each link pointing to an absent `<name>_target`: `create_dir_all("a/sub")` and `create_dir_all("b")` fail with `AlreadyExists` and make nothing, and `create_dir_all("c/")` returns `Ok` and makes `c_target`.
- `is_volumes_or_a_child_of_it`’s rustdoc: “Each probe follows links, as a create does, and answers three ways.  …  an absent path or parent proves that it is not it.”  Here the followed probe of the path answers `NotFound`, because the link’s target is absent; the parent is a real directory that is not `/Volumes`; so the answer is `false`.

**Severity:** contract

**Basis:** `inferred`.  The contract is split.
- **For refusing:** `is_mount_root`’s rustdoc says it “Fails closed”, and that the floor “must examine the components the create will actually walk”; the README says “an unprovable path is never granted the dangerous branch”.  A create through `link/` walks to `/Volumes/X`.
- **For the code as it is:** the same rustdoc limits its look through links to the identity check for `/Volumes` (“any other symlink redirection stays the caller’s policy; see the README’s symlink section”), and the identity probe’s rustdoc counts an absent path as proof that it is not `/Volumes` or a child of it.

If the fail-closed reading is right: `is_mount_root` should answer `true` for a dangling link whose target is a mount root, which means reading the link’s target (`read_link`, resolved against the link’s directory) and judging it as the floor judges a path, or refusing every dangling link.  If the code is right: `is_mount_root`’s rustdoc and the README’s Known limitations should say that a dangling link is not followed, and that a caller should refuse one first (with `reject_symlink`, or the contract functions, which already refuse it).

**Reproducer:** add this to `tests/blackbox.rs`; it fails on the current tree.

```rust
/// bug-0046.  A dangling link into an absent `/Volumes/<name>`: `mkdir("link/")` creates the
/// target, so the public floor must not call it safe.
#[cfg(target_os = "macos")]
#[test]
fn is_mount_root_refuses_a_dangling_link_into_volumes() {
    let d = tempfile::tempdir().unwrap();
    let link = d.path().join("link");
    std::os::unix::fs::symlink("/Volumes/cli_contract_bb_absent", &link).unwrap();
    let slashed = PathBuf::from(format!("{}/", link.display()));
    for p in [&link, &slashed] {
        assert!(is_mount_root(p), "is_mount_root({}) = false", p.display());
    }
}
```

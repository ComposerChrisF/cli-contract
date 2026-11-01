# bug-0044: CHANGELOG 0.6.1 says that on Windows the `..` refusal meets only verbatim paths, but the output-file refusal also meets ordinary Windows paths

Filed by the nightly review on 1-Nov-2026.

This is doc drift in the CHANGELOG's Windows sentence, which is true of floor_check_chain only.  No open report or plan covers it, and the fix is to CHANGELOG.md alone.

From the nightly change review of `f540451065a3fa1e7cff9c26f0c8c6fb9569a7f1..9ad890c8c5520f3a1973e33ad86ce06d420907ad` on night `2026-11-01`.  Provenance `ai`: raised by opus:cli-agent-first-auditor; cross-model verdicts: fable agree, sol dispute; reproduced by the harness, which ran the test below and saw it fail.  Ledger fingerprint `326ba20646c6`.

**Claim:** The 0.6.1 entry says that on Windows `..` is resolved before the file system sees it, so the refusal meets only a verbatim (`\\?\`) path. That is true of the chain-floor pass, because floor_check_chain works on `absolute()`'s output. The leaf check in check_output_leaf has no cfg and reads `path.components()` of the caller's own spelling. So an ordinary `x\a\..` with x absent is probed (Windows resolves it to x, which is absent) and refused as DotDotAfterMissing on Windows too.

**Location:** `CHANGELOG.md:15`

**Evidence:** CHANGELOG.md:15: "On Windows `..` is resolved before the file system sees it, so the refusal meets only a verbatim (`\\?\`) path, where `..` is a literal name that no create can make."  src/paths.rs:912: `Presence::Absent if matches!(path.components().next_back(), Some(Component::ParentDir))`. There is no cfg, and it runs on the unabsolutized `path`.

**Severity:** docs

**Basis:** `doc-drift`: adjudicated by Opus (The sentence's subject, "the refusal", sits under a headline that covers both refusals, so a Windows reader is told that the output-file refusal meets only verbatim paths, which is false. Sol's reading, that the sentence is about destinations only, is possible but cites nothing showing the code is wrong, and both readings lead to the same fix to the text.): CHANGELOG.md:15 is the second paragraph of the bullet whose headline (line 11) names both refusals: the chain refusal under All and "In every mode, so is an output file whose own path ends in such a `..`". It says: "On Windows `..` is resolved before the file system sees it, so the refusal meets only a verbatim (`\\?\`) path." That is true only of floor_check_chain, which works on `absolute()`'s output; the code comment at src/paths.rs:1122-1123, which the sentence follows almost word for word, is about the chain. check_output_leaf (src/paths.rs:912) has no cfg and reads `path.components()` of the path as the caller gave it. Rust's components() keeps a trailing ParentDir, and on Windows `fs::metadata` resolves `x\a\..` to the absent `x`, so the leaf reads as Absent. An ordinary `x\a\..` is therefore refused on Windows too, through check_output_file or prepare_output_file in any CreateMode. This follows from the code; it was not run on Windows. The code is plainly the intended side. The variant doc (paths.rs:368-369) and README.md:147-149 state the leaf refusal "in every mode" with no platform exception, and the refusal is correct on Windows: building `x\a` makes `x\a\..` name the directory `x`, the very failure that bug-0040 describes. The fix is to the text alone: confine the Windows sentence to the chain refusal.

**Reproducer:** `cargo test changelog_does_not_confine_the_output_file_dotdot_refusal_to_verbatim_paths` failed on the reviewed tree.  The test is `bugs/bug-0044/reproducer.patch`; apply it with `git apply`.


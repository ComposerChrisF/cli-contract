# bug-0051: the leak gate misses a match on a line holding a byte that is not UTF-8

Filed on 4-Nov-2026, from the review of the shared gate’s message checks.  The same fault was found there, in the text scan, and traced back to this repository’s copy.

Provenance `ai`: found by Fable in the cross-check of that review, then reproduced by hand in this repository’s copy on macOS.

**Claim:** in a UTF-8 locale, `git grep` and macOS’s `grep` match nothing on a line that holds a byte that is not valid UTF-8, even with `-a`.  `scripts/leak-gate` runs its home-path, `.claude`, and private-name searches in the caller’s locale.  So a private name or a home path on such a line, in a tracked file or in a tracked path, passes, and the gate exits 0.  The bug and plan ID search already runs with `LC_ALL=C` and is not affected.  The maintainer’s own locale is `en_US.UTF-8`, and the private layer runs only there.

**Location:** `scripts/leak-gate`, `grep_index` and `grep_paths`, and the calls to them for the home path, the `.claude` path, and the private names

**Evidence:** in a scratch repository on macOS, a tracked `a.txt` holding `Fix <the byte 0xFF> secretname`, with a list naming `secretname`:
- `LC_ALL=C git grep --cached -i -E '(^|[^[:alnum:]])(secretname)([^[:alnum:]]|$)'` finds the line; under `en_US.UTF-8` and `C.UTF-8` it finds nothing.
- `scripts/leak-gate` with that list exits 0 under `en_US.UTF-8`.
- `/usr/bin/grep -a -c -i -E` over the same line gives 1 under `C` and 0 under `C.UTF-8`.

**Severity:** contract

**Basis:** `spec`.  The header: “Two layers … matched as whole words … in every tracked file’s contents and in its path”, and “a new file is scanned by default, so the gate fails closed”.  A match that is never seen fails open.

**Fix:** the shared gate runs each locale-sensitive search twice, in the caller’s locale and with `LC_ALL=C`, and joins the matches; a test there pins it.  This repository takes that fix when plan-0019’s step 2 replaces `scripts/leak-gate` with the pinned shared copy, and the commit that does so closes this report.

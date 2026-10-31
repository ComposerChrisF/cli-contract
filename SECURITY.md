# Security

cli-contract is a safety crate: its checks stand between a tool and a write to the wrong place.  A way around one of them is a security report, not an ordinary bug, unless the README already lists it under _Known limitations_.  For example:

- a path the mount-root floor should refuse, which `ensure_destination` creates anyway;
- a guard that approves a path it could not inspect;
- a marker check that passes for a directory without the marker;
- a symlink that leads a check to approve one location while the write lands in another.

## Reporting a vulnerability

Please report it privately, through the **Report a vulnerability** form on this repository’s Security tab, not in a public issue.  Say which platform and filesystem it happens on, which function you called with which path, what it returned, and what was on disk.

## Supported versions

Fixes land in the latest release.

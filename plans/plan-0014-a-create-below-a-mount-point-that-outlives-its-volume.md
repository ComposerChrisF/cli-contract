# Plan: A Create Below a Mount Point That Outlives Its Volume

Re-filed at the public launch (31-Oct-2026) from the predecessor repository, where it was filed on 30-Oct-2026.

## Problem

The floor refuses to create a mount point, because on macOS, and under udisks2 on Linux, the mount point’s existence is the signal that the volume is mounted: the directory goes away at unmount.  Plenty of mount points stay.  An `/etc/fstab` entry under systemd, a hand-made mount point, and a bind mount all leave their directory behind.  Below one of those, `CreateMode::Final` and `All` create on the root filesystem, and a backup lands on the boot disk one level down from where the floor looks.

It was found before the launch, as root in an Ubuntu 26.04 VM:

```text
$ sudo systemctl stop mnt-fsbackup.mount; findmnt /mnt/fsbackup
findmnt: /mnt/fsbackup is not a mount point
[euid 0] ensure_destination(/mnt/fsbackup/2026-10-30-final, final) = Ok(())
$ findmnt -n -o TARGET,FSTYPE -T /mnt/fsbackup/2026-10-30-final
/ ext4
```

`None` refuses it, and a marker would, but a marker cannot be combined with a creating mode (`DestPolicy::from_flags` refuses the pair), so a tool that makes a dated folder on a backup disk has no guard.  The README states the gap, in § The mount-root floor and under Known limitations; it is to be fixed after the launch.  This plan is the guard.

## Proposed Change

Two candidates, not exclusive:

1. **A marker on an ancestor.**  The open question in `ROADMAP.md`: a marker checked on a named ancestor (the volume root), with the destination created below it.  The marker proves the disk; the creating mode builds the dated folder.  The open part is the API for naming the ancestor.
2. **Proof of a live mount.**  A policy, or a primitive, that requires the base of the chain to sit on a different device from its parent (`st_dev` differs), so an unmounted mount point (on the same device as its parent) is refused.  In the VM this distinguished a tmpfs (`/mnt dev=64769`, `/mnt/t dev=54`); it does not distinguish a bind mount on the same filesystem, which needs `/proc/self/mountinfo` (inferred, not tested).  macOS needs no new mechanism for `/Volumes`, but a hand-made mount point elsewhere has the same gap.

## Implementation Notes

- Either is additive: a new `DestPolicy` variant, or a new function beside `check_destination`.  `DestPolicy` is `#[non_exhaustive]` from 0.6.0, so a new variant is not a breaking change.
- Candidate 2, as stated, holds only when the base of the chain is the mount point itself.  A tool that creates a folder deeper on a mounted volume has a base below the mount point, on the same device as its parent, so the check would refuse a legitimate create.  It must instead find the nearest ancestor whose parent is on another device and require that one to be the expected mount, which brings back candidate 1’s “which ancestor?” question.
- Tests: on Linux a tmpfs needs root, so the identity test may be manual, or use an existing mount such as `/proc` or `/dev/shm` as the “live” case.
- A tool’s `--help` names the policy, so the flag spelling (`--require-mounted`, or `--require-marker` with an ancestor) is part of the design.

## Why Not a Workaround

Each tool would write its own `st_dev` comparison or its own ancestor check around `ensure_destination`, and they would drift; that is what the crate exists to prevent.  Rejecting this plan is a reasonable exit too: `None` already refuses, and a tool that needs a creating mode on such a disk can document `None` plus a pre-made folder.

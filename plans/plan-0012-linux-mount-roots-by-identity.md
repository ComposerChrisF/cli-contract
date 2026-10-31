# Plan: The Linux Mount Roots by Identity, as `/Volumes` Is

Re-filed at the public launch (31-Oct-2026) from the predecessor repository, where it was filed on 29-Oct-2026.

## Problem

The floor knows macOS’s `/Volumes` by identity: it compares device and inode, so `/volumes` on a case-insensitive volume, the `/System/Volumes/Data/Volumes` firmlink, and a symbolic link to `/Volumes` are all refused.  The Linux roots, `/mnt`, `/media`, and `/run/media`, are still matched by spelling alone.  So under `CreateMode::All`, run as root, each of these creates `/mnt/usb`:

- `m/usb/backup`, where `m` is a symbolic link to `/mnt`;
- the same path reached through a bind mount of `/mnt`;
- `L/../usb`, where `L` is a symbolic link to `/mnt/x`: the floor resolves `..` lexically, to a sibling of `L`, while the OS resolves it from the link’s target, to `/mnt`.  The exhaustive test that `check_destination` and `ensure_destination` agree (`tests/check_equals_ensure.rs`) found this one.  An identity check on the parent catches it with no change of its own, as the `/Volumes` check already does on macOS.

The same class needs no link of the user’s own.  Every systemd system ships `/var/run → /run` and `/var/lock → /run/lock`, and every process has `/proc/self/root → /`.  Run as root in an Ubuntu 26.04 VM, each of these returned `Ok(())` and created the mount point:

- `ensure_destination(/var/run/media/<user>/USB/backup, all)` created `/run/media/<user>/USB/backup`;
- `ensure_destination(/var/run/../mnt/newvol2, final)` created `/mnt/newvol2`;
- `ensure_destination(/proc/self/root/mnt/newvol3, all)` created `/mnt/newvol3`.

The same VM confirmed the three spellings above: `L/../newvol` with `L → /mnt/x`, a link `m → /mnt`, and a bind mount of `/mnt` at `/srv/mntalias` each created a directory directly under `/mnt`.

The README’s Known limitations section lists this, and `is_mount_root`’s docs say that the Linux roots are matched by spelling only; it is to be fixed after the launch.  The crate calls the floor unconditional, but its Linux list “best-effort, less canonical”.  This plan asks whether best-effort should cover linked, aliased, and bind-mounted spellings.

## Proposed Change

Extend `is_volumes_or_a_child_of_it` into a check against every root on the list, and resolve the base of the chain before the spelling list is applied:

1. **Compare identities.**  Stat each of `/Volumes`, `/mnt`, `/media`, and `/run/media` that exists.  Compare a path, its parent, and, where the list allows deeper roots (`/media/<user>/<label>`, `/run/media/<user>/<label>`), its grandparent against their identities, with the same depth limits as the spelling list.  Where a root exists, identity sees through its aliases, a bind mount included: in the VM, `/mnt` and the bind mount `/srv/mntalias` were both `dev=64769 ino=1668`.
2. **Resolve the base.**  Comparing the roots’ identities misses a root that is absent.  With `/run/media` removed (staged; it exists from boot on Ubuntu), `ensure_destination(/var/run/media/<user>/USB/backup, all)` created `/run/media/<user>/USB/backup`, while `/run/media/…` spelled directly is refused.  So resolve the chain’s deepest existing ancestor (`/var/run` to `/run`), and apply the spelling list to that resolved base plus the missing tail.  That covers links, `..` after a link, the system aliases, and absent roots; a bind mount, which resolution does not see through, is step 1’s.  Inferred, not built.

Keep it three-state: a root that is absent matches nothing by identity, and any other error refuses.  Keep Windows as it is: the floor there checks no list (see `is_mount_root`).

## Implementation Notes

- Each check then costs up to one stat per root plus two or three per path, still negligible beside a create.
- Tests on Linux need no root: a symbolic link in a temp directory to `/mnt`, checked with `check_destination` only, asserts `MountRoot` and creates nothing.  So can `/var/run/../mnt/<x>`, with no fixture link, where `/var/run` exists.  CI’s `ubuntu-latest` job runs them, and they are `cfg(target_os = "linux")`.
- A bind mount cannot be staged without root, so a test for it may be manual, or skipped with a reason.
- The sentence “The Linux roots are matched by spelling only” changes, in the README § The mount-root floor and in `is_mount_root`’s docs, and the first item under Known limitations goes.

## Why Not a Workaround

The floor is the crate’s reason to exist: one implementation, tested once.  A tool cannot add identity matching around `ensure_destination` without reimplementing the chain walk, and a caller-side check would drift from the crate’s.  Rejecting this plan is a reasonable exit: the crate calls the Linux list best-effort, and the README states the limit honestly.

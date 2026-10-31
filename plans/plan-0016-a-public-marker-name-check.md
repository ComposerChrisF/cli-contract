# Plan: A Public Marker-Name Check

Re-filed at the public launch (31-Oct-2026) from the predecessor repository, where it was filed on 30-Oct-2026.

## Problem

A tool that takes a marker for an **input** directory (`require_input_dir(dir, Some(marker))`) gets a non-name marker back as `PathError::MarkerNotAName`, exit 1, because the crate cannot tell there whether the value came from argv or from a config file.  The README says to “validate at the flag boundary to get the usage code”, but the only public check is `DestPolicy::from_flags`, a constructor for a **destination** policy.  So the crate’s own `examples/inputs.rs` builds a throwaway `DestPolicy::from_flags(CreateMode::None, Some(marker))` and reads its error to exit 2.  The other route, copying the private `marker_name_fault` with its Windows arms (a name made only of dots and spaces, a drive prefix), is the per-tool reimplementation the crate exists to prevent.

It was raised by a code audit before the launch.

## Proposed Change

```rust
/// Refuse a marker that is not a single name, as a usage error ([`PolicyError::MarkerNotAName`],
/// exit 2).  [`DestPolicy::from_flags`] calls it; so can a tool validating an input marker.
pub fn check_marker_name(marker: &str) -> Result<(), PolicyError>;
```

`from_flags` calls it in place of its own `marker_name_fault` check, and `examples/inputs.rs` uses it in place of the throwaway policy.

## Implementation Notes

- `PolicyError::MarkerNotAName`’s message names `--require-marker` and “the destination”.  An input tool spells its flag otherwise, so it renders the variant itself, as the `PolicyError` rustdoc already says; the example does that today.
- Additive, so any 0.6.x.

## Why Not a Workaround

The workaround exists (the throwaway policy) and works, but it reads as a destination check on an input, and it is easy to miss.  A function named for the check is where a reader of the API looks for it.

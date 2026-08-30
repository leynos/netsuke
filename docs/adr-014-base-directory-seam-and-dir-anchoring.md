# Architecture decision record (ADR): base-directory seam and `-C` anchoring

## Status

Accepted.

## Date

2026-08-27

## Context and problem statement

Manifest resolution and glob expansion previously read the process working
directory (`std::env::current_dir`) deep inside the library, and tests mutated
that working directory (`CwdGuard`) or coordinated process-global environment
state (`EnvLock`) to influence resolution. Under the AGENTS.md environment
mandate (see ADR-008) that ambient coupling is unacceptable: parallel test
execution cannot isolate a mutable process CWD, and a library's correct result
should not depend on where the invoking process happens to sit.

Separately, the CLI contract for `-C/--directory` needed a precise statement:
the flag anchors automatic project discovery and manifest lookup, but an
explicit `--config` (or `NETSUKE_CONFIG`) selector is resolved against the
shell's original working directory and is deliberately independent of `-C`.

## Decision

- **Capture the working directory at the composition boundary, once, as data.**
  The command-line entry points read `std::env::current_dir()` and pass the
  value onward as an explicit base directory; manifest workspace resolution and
  glob expansion accept that base as a parameter and never read the process CWD
  themselves. `expand_glob`/`glob_paths` thread the base through to
  `strip_base`, which removes it from matches to restore pattern-relative
  spellings.
- **An ambient fallback exists only where no manifest root is available**, and
  that read is confined to the composition boundary, not to resolution
  internals.
- **Explicit selectors are independent of `-C`.** A relative `--config <PATH>`
  or `NETSUKE_CONFIG` resolves against the shell's original working directory.
  `-C` scopes automatic project discovery and manifest lookup; it never
  re-anchors an explicit selector. See ADR-004 for the selection machinery and
  `src/cli/discovery.rs` for the implementation.
- **In-process environment mutation is banned and gated.** `clippy.toml` and
  `test_support/clippy.toml` disallow `std::env::set_var`, `remove_var`, and
  `set_current_dir` across the workspace targets, and `make lint` runs Clippy
  with those restrictions. `Command::env`/`Command::env_clear`/
  `Command::current_dir` — the child-process configuration builders — remain
  the sanctioned route and are deliberately not disallowed.

## Consequences

- Manifest and glob resolution is deterministic: results depend on the injected
  base, not on where the test or process was launched.
- Tests no longer need `EnvLock`/`CwdGuard`; `test_support/src/env_lock.rs` and
  `cwd_guard.rs` were deleted, and suites pass explicit base directories.
- A contributor who reintroduces in-process mutation immediately fails the
  Clippy stage of `make lint` (disallowed-methods) with a reason string telling
  them what to do instead.
- Explicit `--config` behaviour is documented identically in the user guide,
  the design document, and this ADR, fixing a stale passage that claimed
  `-C`-anchoring.

## Alternatives considered

- **Threading a CWD value through every query.** Rejected: ADR-008's taxonomy
  prefers capture-once-at-the-boundary over parameter threading everywhere.
- **Allowing sanctioned sites for `set_current_dir` in tests.** Rejected: that
  would recreate the very coupling the seam removes.

## Implementation references

- Base seam: [`src/manifest/glob/mod.rs`](../src/manifest/glob/mod.rs)
  (`expand_glob`, `glob_paths`, `strip_base`) and
  [`src/manifest/workspace.rs`](../src/manifest/workspace.rs)
  (`resolve_absolute_workspace_root`).
- Composition boundary: `src/runner/mod.rs` and `src/runner/help_query.rs`.
- Explicit-selector independence:
  [`src/cli/discovery.rs`](../src/cli/discovery.rs); ADR-004.
- Gate: `make lint` runs Clippy with `clippy.toml` and
  `test_support/clippy.toml`, which contain the disallowed-method policy.

## Addendum — 2026-08-30

The accepted decision above remains the historical rationale for the seam. Its
current implementation has these clarified contracts:

- Manifest parsing supplies the manifest directory or workspace root to
  `glob_paths(pattern, base)` and `expand_glob(pattern, base)`. Relative
  patterns, including parent-relative patterns, resolve from that injected root
  and retain their pattern-relative result spelling; absolute patterns remain
  absolute.
- Explicit `--config` and `NETSUKE_CONFIG` selectors remain independent of
  `-C/--directory`: relative selectors resolve from the process working
  directory and absolute selectors remain unchanged. `-C` anchors automatic
  project discovery and manifest lookup.
- The environment-mutation enforcement is Clippy-only. `clippy.toml` and
  `test_support/clippy.toml` reject the forbidden process-global mutation
  methods across workspace targets; child-process configuration through
  `Command::env`, `Command::env_clear`, and `Command::current_dir` remains
  allowed.

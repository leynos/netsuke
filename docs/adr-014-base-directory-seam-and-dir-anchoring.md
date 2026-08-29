# Architecture decision record (ADR): Base-directory seam and `-C` anchoring

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
the flag anchors automatic project discovery, manifest lookup, and relative
explicit configuration selectors.

## Decision

- **Capture the working directory at the composition boundary, once, as data.**
  The command-line entry points read `std::env::current_dir()` and pass the
  value onward as an explicit base directory; manifest workspace resolution and
  glob expansion accept that base as a parameter and never read the process CWD
  themselves. `expand_glob`/`glob_paths` thread the base through to
  `strip_base`, which removes it from relative matches to restore
  pattern-relative spellings, including `..` segments. Absolute patterns do
  not use or strip the base.
- **An ambient fallback exists only where no manifest root is available**, and
  that read is confined to the composition boundary, not to resolution internals.
- **Explicit selectors remain independent of `-C`.** A relative `--config
  <PATH>` or `NETSUKE_CONFIG` resolves from the process working directory,
  even when `-C/--directory` is supplied; absolute selectors remain unchanged.
  `-C` continues to anchor automatic project discovery. See ADR-004 for the
  selection machinery and `src/cli/discovery.rs` for the implementation.
- **In-process environment mutation is banned and gated.** `clippy.toml` and
  `test_support/clippy.toml` disallow `std::env::set_var`, `remove_var`, and
  `set_current_dir`; `scripts/check-env-mutation.sh` greps `src/`, `tests/`, and
  `test_support/` for the same spellings and is wired into `make lint`.
  `Command::env`/`Command::env_clear`/`Command::current_dir` — the
  child-process configuration builders — remain the sanctioned route and are
  deliberately not matched.

## Consequences

- Manifest and glob resolution is deterministic: results depend on the injected
  base, not on where the test or process was launched.
- Tests no longer need `EnvLock`/`CwdGuard`; `test_support/src/env_lock.rs` and
  `cwd_guard.rs` were deleted, and suites pass explicit base directories.
- A contributor who reintroduces in-process mutation immediately fails `make
  lint` (grep gate) and `cargo clippy` (disallowed-methods) with a reason string
  telling them what to do instead.
- Explicit `--config` behaviour is documented identically in the user guide,
  the design document, and this ADR: `-C` anchors relative selectors while
  absolute selectors retain their spelling.

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
- Explicit-selector anchoring:
  [`src/cli/discovery.rs`](../src/cli/discovery.rs); ADR-004.
- Gate: [`scripts/check-env-mutation.sh`](../scripts/check-env-mutation.sh),
  `clippy.toml`, `test_support/clippy.toml`.

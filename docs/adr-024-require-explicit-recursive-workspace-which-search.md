# Architecture decision record (ADR): require explicit recursive `which` search

## Status

Accepted.

## Date

2026-09-04

## Context and problem statement

`which` and `command_available` resolve names used by manifests, including
names later rendered into Ninja commands. The previous default recursively
walked the workspace after an empty or unset `PATH` missed. A trusted manifest
could therefore resolve a checkout-controlled executable without explicitly
crossing that trust boundary.

## Decision

For executable discovery from trusted manifests, the decision requires an
explicit `cwd_mode="workspace-recursive"` opt-in. It rejects default recursive
workspace discovery and recursive `always` behaviour, preserving a predictable
PATH-only default and visible trust boundaries. Manifests relying on the old
fallback must migrate.

`auto` searches only `PATH` entries and yields no directories when `PATH` is
empty or unset. `always` prepends only the workspace root/current directory,
`never` excludes it, and `workspace-recursive` retains the bounded recursive
walker after its flat `PATH` pass misses.

## Rationale

- A recursive walk can select a file controlled by a less-trusted checkout
  contributor, whereas PATH entries and direct paths are explicit inputs.
- Separating flat PATH search, current-directory search, and recursive
  discovery makes every search root reviewable in the manifest.
- Retaining the existing recursive implementation behind a named mode preserves
  its skip-list, symlink, executability, canonicalization, cache, and `all`
  contracts for callers that intentionally need it.

## Consequences

- Existing manifests depending on the empty-PATH fallback must select
  `cwd_mode="workspace-recursive"` deliberately.
- `NETSUKE_WHICH_WORKSPACE` is a kill-switch only for the explicit recursive
  mode.
- ADR-005's typed resolver error boundary remains unchanged: absence is still
  `false` for `command_available` and a diagnostic for `which`.

## Alternatives considered

- **Keep recursive discovery as the empty-PATH default.** Rejected because it
  makes workspace control sufficient to alter a trusted command lookup.
- **Make `always` recursive.** Rejected because its name promises a flat
  current-directory addition, not a wider and security-relevant tree walk.
- **Remove recursive discovery.** Rejected because some trusted workspaces
  intentionally need it and the existing walker has bounded, tested behaviour.

## Implementation references

- Typed error boundary:
  [ADR-005](adr-005-typed-which-resolve-error.md)
- Resolver design:
  [Executable discovery filter](netsuke-design.md#executable-discovery-filter-which)
- User contract: [users' guide](users-guide.md)

## Addendum — 2026-09-19: Bounded search-domain telemetry

The resolver now reports which search domain a resolution used. The four modes
previously produced indistinguishable series, so an operator could not tell
whether `workspace-recursive` had been requested, or whether recursive lookup
contributed to an outcome at all.

The label is `cwd_mode`, drawn from the closed set `auto`, `always`, `never`,
and `workspace_recursive`. That vocabulary is a telemetry spelling rather than
the template spelling: a manifest writes `workspace-recursive`, and the label is
`workspace_recursive`.

The label is carried on the existing `netsuke_stdlib_which_cache_total` and
`netsuke_stdlib_which_resolution_total` counters, and on the
`stdlib.which.resolve` span. The cache and resolver metric names are unchanged.
The change is additive at the label level, but it is a deliberate compatibility
change to the series shape, so a scraper that assumed a fixed label set for
these two counters must be updated.

Redaction rules are unchanged. No command name, path, workspace name, `PATH`
value, `PATHEXT` value, or other environment value is recorded on any span,
event, or metric label. Only the mode, the outcome, and the bounded error
category leave the process. Resolver behaviour and its search semantics are
unchanged by this addendum, and the four `CwdMode` contracts recorded above
still hold.

The contract lives in `src/stdlib/which/telemetry.rs`, which owns both counter
names and every label vocabulary; the implementation is in
`src/stdlib/which/cache.rs`, and the recorder admission rule is in
`src/observability_recorder.rs`.

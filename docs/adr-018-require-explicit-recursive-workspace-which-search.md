# Architecture decision record (ADR): require explicit recursive `which` search

## Status

Accepted.

## Date

2026-09-04.

## Context and problem statement

`which` and `command_available` resolve names used by manifests, including
names later rendered into Ninja commands. The previous default recursively
walked the workspace after an empty or unset `PATH` missed. A trusted manifest
could therefore resolve a checkout-controlled executable without explicitly
crossing that trust boundary.

## Decision

In the context of executable discovery from trusted manifests, facing an
implicit checkout-controlled command-resolution path, we decided for an explicit
`cwd_mode="workspace-recursive"` opt-in, and against default recursive
workspace discovery or giving `always` recursive behaviour, to achieve a
predictable PATH-only default and visible trust boundaries, accepting that
manifests relying on the old fallback must be migrated.

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

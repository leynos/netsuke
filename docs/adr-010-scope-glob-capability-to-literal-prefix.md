# Architecture decision record (ADR): Scope the glob metadata capability to the literal prefix

## Status

Accepted.

## Date

2026-08-09

## Context and problem statement

Manifest glob expansion (`src/manifest/glob/mod.rs`) matches a caller-supplied
pattern such as `src/**/*.c` and returns the matching file paths. Matching
itself runs through the `glob` crate, which walks the filesystem directly.
Filtering the walk's results down to regular files, however, goes through a
metadata check, and that check was routed through a `cap_std::fs::Dir`
capability handle rather than a raw filesystem call.

The handle's authority was disproportionate to what any single pattern could
need. `glob_paths` opened it at `/` for an absolute pattern and at `.` for a
relative one, so the capability covered the entire filesystem root or the
entire working directory regardless of how narrow the pattern's own literal
component was. A pattern such as `src/**/*.c` names only the `src/` subtree,
yet the metadata check could resolve any path reachable from the root or the
working directory. Issue #173 asked for this ambient authority to be reviewed
against the least-privilege principle the capability was meant to enforce.

## Decision

Open the metadata capability at the pattern's longest literal directory prefix
instead of at a fixed root:

- **Literal prefix extraction.** `walk::literal_dir_prefix` scans the
  normalized pattern up to the first glob metacharacter (`*`, `?`, `[`, or `{`)
  and trims the result back to the last path separator, yielding the deepest
  directory the pattern names without wildcards. For `src/**/*.c` this is
  `src/`. A pattern with no literal directory component, such as `*.c`, yields
  `.`, keeping the working-directory scope for patterns that cannot narrow it.
  The scan steps over bracketed literal escapes such as `[*]`, so
  `src/[*]x/generated/*.c` reaches `src/[*]x/generated/` rather than stopping
  at the first `[`.
- **`GlobRoot` couples the handle and the prefix.** `walk::open_root_dir`
  opens a `cap_std::fs::Dir` at the literal prefix one component at a time,
  refusing symbolic links, and wraps it together with the lexical prefix in a
  `GlobRoot`. Every subsequent metadata lookup relativizes the matched path
  against the prefix (`GlobRoot::relativise`) before resolving it through the
  handle, so the capability only ever sees paths inside the subtree it was
  opened at.
- **A missing or non-directory prefix yields no capability at all.**
  `open_root_dir` returns `Ok(None)` when the prefix does not exist or is not a
  directory (`walk::prefix_is_unopenable`), and `glob_paths` returns an empty
  match set in that case, mirroring the empty result the matcher would produce
  anyway. `diagnostics::record_unopenable_prefix` records the outcome so a
  degraded expansion remains observable.
- **The matcher still walks ambiently.** The `glob` crate's own traversal is
  unchanged; only the metadata check used to filter directories out of its
  results is capability-scoped. Narrowing the capability's opening point
  therefore narrows what the metadata check can resolve, not what the walk
  itself can see on disk.

## Rationale

- **Least privilege follows the pattern's own scope.** A pattern can only
  ever match inside its literal prefix, so opening the capability there gives
  the metadata check exactly the authority the pattern could use, rather than
  the authority of the whole filesystem root or working directory.
- **Relativization keeps the capability boundary honest.** Resolving a
  matched path against the capability requires rebasing it onto the prefix first
  (`GlobRoot::relativise`); a path that does not start with the prefix cannot
  be looked up at all, so the capability cannot be handed an absolute or
  differently rooted path by accident.
- **An unopenable prefix is an empty result, not an error.** A pattern whose
  literal prefix does not exist can match nothing, so treating a missing or
  non-directory prefix as an empty expansion (rather than a hard failure)
  matches the semantics a caller already expects from a glob that matches no
  files.
- **A symbolic-link prefix is rejected.** Opening each literal directory
  component without following links prevents a pattern such as `src/link/*.c`
  from gaining the authority of `link`'s target, while retaining the lexical
  prefix in `GlobRoot` for later match relativization.
- **Symbolic-link handling is explicit and bounded.**
  `GlobRoot::metadata_relative` classifies a failed metadata lookup as a
  skipped link only when `is_unresolvable_link` identifies `PermissionDenied` or
  `NotFound` and `traverses_symlink` confirms a final or intermediate symbolic
  link. Other failures, including symlink loops, still propagate. This
  classification is paired with bounded skipped-match recording via
  `diagnostics::record_unreachable_symlink`.

## Consequences

- **The matcher's own traversal remains ambient.** Scoping the metadata
  capability does not scope the `glob` crate's directory walk; that crate still
  reads the filesystem directly rather than through `cap_std`. Replacing it
  with a capability-native matcher would close this remaining gap but was out
  of scope for this change (see Alternatives considered).
- **A match reached through an escaping symbolic link is silently skipped.**
  If a matched path resolves through a symlink whose target leaves the literal
  prefix, the capability cannot follow it. `GlobRoot::metadata` treats this the
  same as a dangling link: the match is dropped from the results rather than
  failing the whole expansion, and the drop is recorded via
  `diagnostics::record_unreachable_symlink`. A symlink loop is treated
  differently and still fails the expansion, because a cycle describes a broken
  tree rather than a file that is simply unreachable through the capability.
- **An escaping symlink and a permission-denied symlink are
  indistinguishable.** `cap_std` reports both an out-of-prefix resolution and a
  genuine permission failure inside the prefix as
  `io::ErrorKind::PermissionDenied`. The capability cannot tell these apart, so
  a link that is legitimately unreadable inside the prefix is skipped alongside
  one that escapes it, rather than being reported as a distinct error.
- **Parent-relative patterns now work.** A pattern such as `../*.txt`
  previously reached the working-directory handle as a `../…` lookup and was
  rejected as a sandbox escape. Because the capability is now opened at the
  pattern's own literal prefix (which can itself be `../`), a parent-relative
  pattern gets a capability rooted at that parent directory instead of being
  rejected outright.
- **Diagnostics stay bounded.** `diagnostics.rs` records the unopenable-prefix
  and unreachable-symlink outcomes as low-cardinality counters
  (`netsuke_manifest_glob_expansions_total`,
  `netsuke_manifest_glob_entries_skipped_total`) labelled only by a closed set
  of outcome and reason strings. Tracing replaces every caller-controlled path
  field — patterns, prefixes, and sampled relative matches — with the stable
  `<redacted>` marker. Errors may retain the original caller input so invalid
  patterns can be explained precisely.

## Alternatives considered

- **Keep the ambient root.** Rejected. Opening the handle at `/` or `.`
  regardless of the pattern's own scope gives the metadata check no
  least-privilege benefit at all; the capability wraps the check in `cap_std`
  API surface without constraining what it can resolve.
- **Always open at the working directory.** Rejected. This breaks absolute
  patterns, which need a handle rooted above the working directory to resolve
  at all, and it does not help parent-relative patterns such as `../*.txt`,
  which would still need to escape the working-directory handle to resolve.
- **Replace the `glob` crate with a capability-native matcher.** Rejected for
  this change. This would close the remaining gap where the matcher's own
  traversal reads the filesystem ambiently, but it is a materially larger
  change than scoping the existing metadata check, and is named here as the way
  to close that boundary rather than attempted as part of this decision.

## Implementation references

- Capability scoping and symbolic-link handling:
  [`src/manifest/glob/walk.rs`](../src/manifest/glob/walk.rs)
- Expansion entry point and capability composition:
  [`src/manifest/glob/mod.rs`](../src/manifest/glob/mod.rs)
- Bounded diagnostics for unopenable prefixes and skipped matches:
  [`src/manifest/glob/diagnostics.rs`](../src/manifest/glob/diagnostics.rs)
- Developer guide:
  [`docs/developers-guide.md`](developers-guide.md#manifest-glob-module-boundary)

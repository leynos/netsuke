# Architectural decision record (ADR) 0012: Bound dyndep sidecar retention

## Status

Accepted.

## Date

2026-08-15

## Context and problem statement

Serial dependency ordering uses immutable, content-addressed Ninja dyndep
sidecars beneath `.netsuke/dyndep`. A changed manifest therefore produces new
`.dd` files without changing or overwriting files that an existing generated
Ninja manifest references. Without a cleanup policy, obsolete sidecars would
accumulate indefinitely. Cleanup must also avoid removing a sidecar that a
concurrent Netsuke command is still consuming.

The policy must preserve every sidecar in the current generated bundle, remove
stale temporary files left by interrupted atomic writes, and bound obsolete
storage deterministically. It must define the failure boundary for `clean` and
make the compatibility consequence of retaining an old `generate --output`
manifest explicit.

## Decision

Netsuke will retain immutable, content-addressed dyndep sidecars. Each
sidecar-capable `build`, `generate`, or `clean` command materializes every
sidecar in its current bundle before writing or invoking the generated Ninja
file. Publication and cleanup use a capability-scoped, exclusive lease for the
`.netsuke/dyndep` directory. The lease remains held through Ninja consumption
for `build` and `clean`, or through generated-output consumption for `generate`.

While that lease is held, Netsuke removes stale `.tmp` files and applies the
following deterministic policy to obsolete `.dd` files:

- every sidecar in the current bundle is retained;
- at most 32 obsolete `.dd` files are retained; and
- at most 1 MiB of obsolete `.dd` bytes is retained.

Obsolete files are considered in deterministic path order. A sidecar's content
is never changed in place. `build` and `generate` prune after materialization.
`clean` prunes only after `ninja -t clean` succeeds; a failed clean does not
prune sidecars.

## Rationale

- **Content addressing preserves active bundles.** A matching sidecar can be
  reused and a mismatching file is corruption, not permission to overwrite it.
- **The lease protects consumption.** Publication, temporary-file cleanup,
  and pruning share one directory lease, so cleanup cannot remove files while
  another serial command is using its bundle.
- **Fixed budgets are predictable.** File-count and byte limits provide a
  bounded cache without relying on filesystem timestamps or an age-based policy.
- **`clean` keeps failure evidence.** Deferring cleanup until successful
  `ninja -t clean` avoids deleting historical state when the requested clean
  did not complete.

## Consequences

An old arbitrary manifest written by `generate --output` may lose referenced
sidecars after a later Netsuke command applies retention. Such a manifest must
be regenerated before use when its sidecars have been pruned. Generated
manifests should therefore be treated as command outputs paired with the
current sidecar cache, not as permanently self-contained artefacts.

The policy does not use sidecar age, and it does not make sidecars mutable.
There is no guarantee that an obsolete sidecar remains available merely because
its manifest was generated successfully in an earlier command.

## Alternatives considered

### Retain sidecars by age

Rejected. Wall-clock age is not deterministic and does not bound storage.

### Keep every content-addressed sidecar

Rejected. Immutable files would accumulate without bound as manifests change.

### Mutate or overwrite existing sidecars

Rejected. A content-addressed path must continue to identify one byte sequence,
and overwriting it could change the graph seen by an existing manifest.

### Prune without a directory lease

Rejected. Publication and cleanup could race with a command that is consuming
the current bundle, removing a required sidecar between materialization and
Ninja or output consumption.

## Implementation references

- Runner publication boundary:
  [`src/runner/dyndep/publication.rs`](../../src/runner/dyndep/publication.rs)
- Atomic sidecar materialization:
  [`src/runner/process/dyndep/files/mod.rs`](../../src/runner/process/dyndep/files/mod.rs)
- Verified generated-state directory:
  [`src/runner/process/dyndep/directory/mod.rs`](../../src/runner/process/dyndep/directory/mod.rs)
- Retention and lease implementation:
  [retention implementation](../../src/runner/process/dyndep/retention/mod.rs)
- User contract:
  [user's guide](../users-guide.md#run-direct-dependencies-serially)
- Serial dyndep architecture:
  [ADR-0011](adr-0011-use-ninja-dyndep-for-serial-dependency-ordering.md)

## Amendment — 2026-11-12: Reject generated-state aliases

The original decision assumed `.netsuke` and `.netsuke/dyndep` were ordinary
directories. They are project-controlled state, so a manifest can ship either
component as a symbolic link or a Windows junction. A capability rooted at the
workspace resolves such an alias, and retention would then treat unrelated
workspace files as disposable cache entries and delete them.

Netsuke now establishes the generated-state directory through
`src/runner/process/dyndep/directory/mod.rs`. The module walks `.netsuke` and
`.netsuke/dyndep` one component at a time, refuses any entry that is a symbolic
link, junction, other reparse point, or non-directory, and returns a verified
handle. Publication, locking, and retention address bare entry names through
that handle. The refusal is checked on the entry before it is opened, so the
diagnostic names the aliased component, and again on the opened handle, so an
entry swapped in between cannot smuggle an alias past the check. Unix opens with
`O_NOFOLLOW`; Windows opens the reparse point itself and rejects every reparse
attribute, not only the tags `std` reports as symlinks.

The lease remains advisory: it coordinates cooperating Netsuke processes and
does not defend against a process that ignores it. The `--directory` root may
itself be a symbolic link, because it is the caller's chosen capability root
rather than generated state. A project that previously relied on an aliased
`.netsuke` directory must replace it with a real directory; the command now
fails with a localized error and deletes nothing.

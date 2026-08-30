# Architecture decision record (ADR): Bound dyndep sidecar retention

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
  [`src/runner/dyndep_publication.rs`](../src/runner/dyndep_publication.rs)
- Atomic sidecar materialization:
  [`src/runner/process/dyndep_files.rs`](../src/runner/process/dyndep_files.rs)
- Retention and lease implementation:
  [retention implementation](../src/runner/process/dyndep_retention.rs)
- User contract: [user's guide](users-guide.md#run-direct-dependencies-serially)
- Serial dyndep architecture:
  [ADR-011](adr-011-use-ninja-dyndep-for-serial-dependency-ordering.md)

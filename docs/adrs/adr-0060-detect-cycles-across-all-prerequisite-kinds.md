# ADR-0060: Detect cycles across all prerequisite kinds

## Status

Accepted.

## Date

2026-10-24

## Context and problem statement

Netsuke validates the complete intermediate representation (IR) before it
generates or runs Ninja. The cycle detector follows `sources` and `deps`, but
it previously skipped `order_only_deps` on the mistaken ground that an
order-only prerequisite affects scheduling without affecting freshness. A
target still has to wait for every order-only prerequisite. If those edges
close a cycle, the affected targets cannot be scheduled.

That exclusion let `netsuke generate` emit a build file that Ninja rejects when
the cyclic targets are requested. Ninja 1.11.1 rejected the probed order-only
cycle, and GNU Make 4.4.1 reported and dropped its corresponding circular
prerequisite. The Ninja 1.13.1 manual confirms that order-only dependencies
must be built first, while a change to them alone does not rebuild the
dependent. These are separate scheduling and freshness properties.

The graph also has two related rules that need to stay explicit. Every explicit
or implicit output names the same producing edge, so a dependency on any alias
reaches that edge. `dependency_order: serial` controls scheduling of an
action's direct `deps` list; it does not add an edge to the manifest graph.

## Decision drivers

- Detect graph errors before emitting Ninja or starting an external process.
- Apply the same cycle rule to every prerequisite that must be scheduled first.
- Treat all output aliases as names for their one producing build edge.
- Keep serial scheduling metadata out of dependency analysis.
- Preserve deterministic cycle paths and unresolved-dependency reporting.
- Show the typed cycle diagnostic in human CLI output.
- Keep the public API and dependency set unchanged.

## Options considered

### Keep excluding order-only dependencies and document the exclusion

This preserves the current Netsuke behaviour but leaves the failure to Ninja
for a requested cycle. It also makes `generate` accept an IR graph that can
produce an invalid Ninja build file.

### Include order-only dependencies in cycle detection

This makes the pre-generation check account for every prerequisite that must be
ready before a build edge can run. It is the selected option.

### Include order-only dependencies but only warn

A warning preserves `generate`'s success status while still emitting a file
Ninja can reject. It also leaves command success dependent on whether the
affected cycle is requested.

### Warn for one release, then reject

A tiered rollout carries the same invalid-output interval as warning-only
behaviour. The project is pre-1.0, and the release-admission canary manifests
and repository examples contain no known affected cycle.

### Match Ninja's requested-target traversal scope

Ninja may ignore a cycle it is not asked to build. Netsuke's `generate` command
has no requested targets, and `sources` and `deps` are already checked across
the whole manifest. Restricting only order-only validation to a requested
subgraph would make the compiler's validation scope inconsistent.

## Decision outcome

The cycle detector traverses `sources`, `deps`, and `order_only_deps`. A cycle
through any of those prerequisite kinds is reported during IR validation,
including when no default target requests the affected nodes. A cycle in an
unrequested part of the graph might not prevent a particular Ninja invocation,
but Netsuke deliberately rejects it before generating a build file.

Human CLI output renders the localized `CircularDependency` cause directly. The
runner's generic graph-building context otherwise masks the cycle path in human
mode. The tracing event records only the fixed `circular_dependency` category;
the manifest-derived cycle path is written to human stderr, not logs. JSON
output retains the stable outer graph-building message and the typed cycle
message in its `causes` array; its schema is unchanged.

Dependencies naming explicit or implicit outputs resolve to their producing
edge. The cycle path retains the alias used by the dependency, and aliases do
not create extra build steps. An unresolved prerequisite remains a missing
dependency and does not create a cycle edge.

`dependency_order` remains scheduling policy for a direct `deps` list. It does
not add a cycle-analysis arc. This allows an earlier listed dependency to
depend on a later listed dependency; Ninja schedules the real prerequisite
first, as the serial-ordering integration test demonstrates.

The detector change remains private, and `IrGenError` and the `netsuke::ir` API
keep their existing shape. The CLI error boundary now selects the typed cycle
cause for human output; JSON rendering remains unchanged.
`missing_dependencies` now also includes unresolved order-only paths, because
those are traversed with the other prerequisite classes.

The detector continues to walk output aliases and resolve each alias through
the canonical edge index. Its worst-case lookup cost is proportional to the
sum, over edges, of the number of aliases times the number of dependencies. An
edge-keyed traversal could remove that multiplier, but would touch Polonius-
and Kani-sensitive code and is deferred until profiling justifies that change.

## Known risks and limitations

- A manifest with a latent cycle in an unrequested target now fails IR
  generation. This is stricter than Ninja's behaviour when it is never asked to
  build that target, and is intentional so all prerequisite classes have one
  whole-manifest validation rule.
- `missing_dependencies` may contain additional order-only paths compared
  with earlier versions. The report type is crate-private; the JSON diagnostic
  schema and `IrGenError` shape remain unchanged.
- Alias traversal repeats dependency lookups for each output alias. The current
  bound is acceptable for the graph sizes under test; edge-keyed state remains
  deferred.

## Consequences

- The detector, property tests, BDD scenarios, end-to-end tests, and Kani
  harnesses cover order-only participation, output aliases, and serial-order
  invariance.
- Human CLI cycle failures show the localized cycle path even when the runner
  wraps graph-construction errors in a generic context.
- The users' guide describes all dependency classes and documents that the
  whole manifest is checked before Ninja starts.
- The v0.1.0 migration guide calls out newly rejected latent cycles. The
  v0.1.1 guide's compatibility promise is unchanged because v0.1.0 is still
  unreleased.
- The `missing_dependencies` content change is recorded in the changelog.

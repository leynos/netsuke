# Architecture decision record (ADR): Use Ninja dyndep for serial dependency ordering

## Status

Accepted.

## Date

2026-08-11

## Context and problem statement

Manifest authors need an explicit way to run the direct `deps` of an action or
target in declaration order. The existing Ninja dependency classes preserve
freshness and scheduling constraints, but ordinary implicit dependencies are
all visible to the scheduler and may run concurrently.

The implementation must retain a single Ninja invocation so a shared dependency
runs once, propagate a failing early dependency to stop later work through the
annotated path, and leave unrelated branches available for normal concurrent
scheduling. It must also leave the Intermediate Representation (IR)
backend-agnostic and make generated Ninja output executable in every command
path.

## Decision

**Y-statement:** In the context of declaration-ordered direct dependencies,
and facing the forces of shared-work reuse, failure short-circuiting,
unrelated-branch concurrency, backend-neutral IR, and executable generated
output, we decided to use staged Ninja dyndep sidecars for
`dependency_order: serial`, accepting the Ninja 1.10 floor, reserved generated
state beneath `.netsuke`, and a path-scoped ordering guarantee.

In the context of a `dependency_order: serial` manifest `deps` list, Netsuke
will use staged Ninja dyndep sidecars to reveal one direct dependency at a time
and will materialize those sidecars atomically beneath `.netsuke/dyndep` before
writing or invoking the main build file.

`dependency_order` is a closed `parallel`/`serial` enum on the shared action
and target AST shape. It is copied to `BuildEdge`, where it remains a logical
graph annotation. Only the Ninja generator lowers a serial list containing two
or more direct dependencies into synthetic phony gates beneath
`.netsuke/serial` and content-addressed dyndep sidecars beneath
`.netsuke/dyndep`.

The main generated build file declares `ninja_required_version = 1.10` only
when staged serial lowering is present. The generator exposes a complete bundle
containing main-file text and every required sidecar. String-only generation
rejects a graph requiring sidecars instead of returning an incomplete file.

The serial guarantee is deliberately path-scoped: each direct dependency in the
annotated list becomes schedulable only after its predecessor succeeds. A later
dependency independently reachable through another requested path remains free
to run through that other path.

## Rationale

- **One scheduler preserves shared work.** The generated gates stay inside one
  Ninja invocation, so Ninja continues to deduplicate a repeated or diamond
  dependency.
- **Dyndep controls visibility.** A later real dependency is absent from the
  relevant graph path until its sidecar is revealed, unlike an order-only edge
  whose transitive inputs are already visible to Ninja.
- **The IR remains portable.** Gates, sidecar paths, and Ninja version syntax
  are backend mechanics rather than manifest graph concepts.
- **Bundle ownership prevents incomplete output.** Treating sidecars as part of
  the generated artefact makes every runner path materialize them before Ninja
  loads the main file.
- **Content addressing makes state reusable.** Existing matching sidecars are
  safely reused; mismatching content is corruption and is reported rather than
  overwritten.

## Consequences

- Serial lists with zero or one dependency use ordinary Ninja lowering; no
  relative order needs enforcing and no dyndep version floor is emitted.
- User graph paths in outputs, inputs, implicit dependencies, and order-only
  dependencies cannot use `.netsuke/serial` or `.netsuke/dyndep`, because those
  names are reserved generated state.
- `build`, `clean`, and `generate` each materialize sidecars relative to the
  effective Ninja working directory. `clean` may leave the immutable,
  content-addressed sidecars in place.
- `src/ninja_gen/dyndep.rs` owns staging and naming. The runner's
  `dyndep_files` module owns capability-scoped, atomic persistence. Neither
  module may broaden the path-scoped guarantee with a global scheduler.
- Tests must continue to use real Ninja for ordered starts, failure
  short-circuiting, shared-work reuse, and unrelated-branch concurrency.

## Alternatives considered

### Order-only phony gate chain

Rejected. Ninja eagerly schedules already-visible transitive inputs, so an
order-only chain can order gate completion without preventing later real
dependencies from starting early.

### Ninja pool with depth one

Rejected. A pool provides mutual exclusion, not declaration order, and would
serialize unrelated work outside the annotated dependency list.

### Recursive Ninja or Netsuke invocation per dependency

Rejected. Separate child schedulers lose the enclosing build's memoization and
can execute a shared dependency more than once.

### A Netsuke-owned global scheduler

Rejected for this feature. It would change the execution architecture and
global reachability semantics rather than implement the requested scoped
manifest policy. It requires a separately approved design.

## Implementation references

- Manifest and IR contract: [`src/ast.rs`](../src/ast.rs),
  [`src/ir/graph.rs`](../src/ir/graph.rs), and
  [`src/ir/from_manifest.rs`](../src/ir/from_manifest.rs)
- Ninja bundle generation:
  [`src/ninja_gen/dyndep.rs`](../src/ninja_gen/dyndep.rs)
- Atomic sidecar materialization:
  [`src/runner/process/dyndep_files.rs`](../src/runner/process/dyndep_files.rs)
- User contract: [user's guide](users-guide.md#run-direct-dependencies-serially)
- Implementation history:
  [issue #552 ExecPlan](execplans/issue-552-support-serial-dependency-ordering-for-actions-and-targets.md)

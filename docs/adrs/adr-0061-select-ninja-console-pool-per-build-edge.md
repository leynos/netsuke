# Architectural decision record (ADR) 0061: Select Ninja's console pool per build edge

## Status

Accepted. Actions and targets may set `console: true`; Netsuke carries that
choice on `BuildEdge` and writes `pool = console` on the generated build edge.
The option defaults to `false` and does not change `Action` identity.

## Date

2026-10-25.

## Context and problem statement

Ninja buffers a command's output until it exits unless the edge uses Ninja's
special `console` pool. A manifest currently cannot select that pool. Long
developer gates can therefore appear idle while they run, even when their
commands are waiting on a lock or making progress.

The IR shares actions when their resolved recipes and file sets match. Making
console selection part of `Action` would make a scheduling choice affect action
identity and could prevent otherwise identical edges from sharing an action.
The pool is also an edge-level Ninja binding, so the graph edge is its natural
owner.

## Decision drivers

- Preserve action hashing and rule sharing when only one edge needs streamed
  output.
- Keep scheduling validation common to direct generation and dyndep bundles.
- Reject combinations that Ninja cannot represent without silently dropping a
  requested guarantee.
- Explain the serialization and output-forwarding limits to manifest authors.

## Decision outcome

- Add a default-false `console` Boolean to manifest actions and targets, and
  carry it as `BuildEdge.console`.
- Emit an edge-level `pool = console` binding only for selected user edges.
  When an action rule declares `pool = console`, emit an empty edge-level
  `pool =` binding on its ordinary edges so they do not inherit console
  scheduling. Synthetic serial gates and dyndep sidecars do not inherit either
  binding.
- Keep `Action.pool` and action hashing unchanged. A pre-existing
  `Action.pool: console` is accepted as redundant; a different custom pool
  conflicts with console selection and is rejected by the generator.
- Reject console selection on dependency-only aggregates. Reject console plus
  a named contention class while continuing to allow console selection by
  itself.
- Validate these rules before both direct Ninja generation and dyndep-bundle
  generation, preserving the existing missing-action diagnostic path.

Adding the field to the public `Target` and `BuildEdge` structs breaks
downstream Rust callers that construct either type with a struct literal. Both
structs expose public fields and neither is `#[non_exhaustive]`, so an existing
literal fails to compile until it names `console`. The package currently
declares version `0.1.0-beta4`, and this source break is accepted for the next
pre-release API: callers must provide `console`. The default only preserves
manifest compatibility through Serde.

Ninja runs console-pool edges one at a time in one invocation. Output from
other edges is held while a console edge runs. Netsuke forwards Ninja output as
it reads it outside JSON mode, but it does not allocate a PTY; recipes see a
pipe and their own buffering can still delay visible output.

## Consequences

Manifest authors can opt long or interactive gates into prompt output without
changing reusable rules or action identity. They also accept reduced edge
concurrency while any console edge is running. The option cannot provide
interactive terminal semantics, and it does not bypass buffering inside a
recipe.

The contention-class RFC must reject only the combination of console and a
contention class, not console selection in general. Generator validation also
guards against incompatible custom pools supplied through existing IR callers.

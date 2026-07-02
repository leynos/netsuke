# Architectural decision record (ADR) 004: Bound Kani Intermediate Representation (IR) harnesses to small N

## Status

Accepted.

Accepted: 2026-06-12. Netsuke will verify manifest-to-Intermediate
Representation (IR) safety properties with small bounded Kani harnesses and
delegate larger graph coverage to the future Proptest layer. Extended on
2026-06-23 to cover cycle canonicalization with a private production-owned
kernel proved over small integer cycles, plus path-wrapper coverage outside the
kernel proof.

## Date

2026-06-23.

## Context and problem statement

Roadmap item `4.2.1` asks for Kani harnesses over the manifest-to-IR lowering
path and cycle detector. The roadmap also describes coverage "up to 10 nodes,
depth limit 20 edges". That bound is too large for Kani against the current
production representation, which uses `HashMap`, owned strings, `Utf8PathBuf`,
and serde-backed action hashing.

The project needs a verification boundary that is useful now, cheap enough to
run locally, and honest about what remains for the later randomised and
property-testing layers.

## Decision drivers

- Keep `make kani-full` suitable for local use.
- Avoid verification-only public API.
- Keep verification compatibility code production-owned and private to the IR
  module.
- Keep proofs close to the IR modules they verify.
- Preserve ordinary `make check-fmt`, `make lint`, and `make test` behaviour.
- Record any narrowed harness entry points as part of the proof contract.

## Options considered

### Option A: encode the roadmap bound directly in Kani

This would model graphs up to 10 nodes and 20 edges inside Kani.

It was rejected because the verifier lowers the real `HashMap` and hashing
implementation. The proof budget is spent in collection, serde, and hashing
internals before the IR invariant is reached.

### Option B: use small Kani bounds with Proptest hand-off

This verifies focused safety properties with one to three graph nodes in Kani
and records larger graph coverage as a future Proptest obligation.

It is accepted because it gives deterministic proofs over the most important
branching logic while keeping the full Kani suite within the local runtime
budget.

### Option C: introduce a private Kani collection compatibility layer

This adds a `cfg(kani)` `IrHashMap` implementation that preserves the small map
operations used by the IR modules while replacing `HashMap` hashing with a
fixed-capacity deterministic array.

It is accepted with constraints. The layer is private to `src/ir`, preserves
the ordinary public API through a `not(kani)` type alias, and may only
implement the operations needed by production IR code under proof. It is not a
harness-side model of cycle detection or manifest lowering.

### Option D: introduce public verification ports

This would add traits or alternate collections to the public or crate-visible
IR surface so Kani could run against adapter implementations.

It was rejected because it would widen the production module surface and add a
second implementation of the data-flow mechanics being verified.

## Decision outcome

Netsuke accepts small bounded Kani harnesses for roadmap item `4.2.1`.

- Manifest-to-IR harnesses use fixed, minimal manifests.
- Cycle harnesses target self-edge, two-node, and, if solver budget permits,
  three-node cycles.
- The larger-N graph property is handed off to the future Proptest roadmap
  item `4.3.1`.
- Kani harnesses are declared from the production modules they verify and
  stored in sibling `*_verification.rs` files.
- Harness helpers stay private unless reuse pressure justifies a narrower
  internal abstraction.
- `cfg(kani)` compatibility code must support production code paths rather than
  replacing those paths in harnesses.

The duplicate-output harness drives the private production `find_duplicates`
helper in `src/ir/from_manifest.rs`, rather than the full
`BuildGraph::from_manifest` path. This keeps the proof focused on duplicate
discovery after direct attempts through manifest lowering reached serde-backed
action hashing before duplicate assertions became tractable. Under `cfg(kani)`,
duplicate-output messages use the real message key without formatted arguments
so the proof does not execute localization formatting internals.

The rule-selection harnesses drive the private `resolve_rule` helper for
empty-rule, multiple-rules, and missing-rule shapes. The proof boundary covers
the production dispatch and error construction while avoiding full manifest
lowering and action registration.

The cycle harnesses drive `cycle::contains_cycle`, a `cfg(kani)` production
entry point that shares the `CycleDetector` traversal with `cycle::analyse`.
Direct attempts through the full `cycle::analyse` report path proved the
self-cycle case but exceeded the local Kani budget for two-node cycles once
cycle-path allocation and canonicalization were included. The boolean entry
point is the production testability repair for that finding: it verifies the
same traversal decision without constructing the human-facing report payload.

Roadmap item `4.2.2` applies the same small-bound decision to cycle
canonicalization. Direct `Utf8PathBuf` proofs reached the local 8 GiB cap
before N=3 and the ID-retaining salvage encoding reached that cap at N=2 once
the full property set was asserted. Netsuke therefore owns the rotation and
closure algorithm in a private generic `canonicalize_cycle_by` kernel. Kani
proves that kernel over distinct symbolic `u8` cycles for N=2, N=3, and N=4,
while `canonicalize_cycle(Vec<Utf8PathBuf>)` remains the production wrapper
using the path comparator. A small direct adapter harness checks the
wrapper/kernel connection for two-node path cycles, and Proptest continues to
exercise path-bearing canonicalization up to the larger randomized bounds.

## Known risks and limitations

- The duplicate-output harness does not prove that full manifest lowering
  reaches the duplicate-output branch; existing unit and behavioural tests keep
  that integration path covered, and `find_duplicates` remains the production
  duplicate-discovery helper.
- The duplicate-output harness does not prove rendered Fluent output for that
  error variant. It proves the message key is selected and the semantic payload
  is preserved.
- The cycle harnesses prove the production cycle detector's boolean traversal,
  not the full report-building path. Existing unit tests keep cycle path
  canonicalization and missing-dependency reporting covered until roadmap item
  `4.3.1` expands generated graph coverage.
- The cycle-canonicalization harnesses prove the private production kernel over
  small `u8` cycles, not all direct `Utf8PathBuf` inputs. The wrapper adapter
  harness and Proptest suite are the documented connection back to the
  path-bearing production instantiation.
- The `cfg(kani)` `IrHashMap` implementation is a private compatibility layer,
  not a public collection port. If future Kani versions model `HashMap`
  efficiently enough for these harnesses, the compatibility layer should be
  removed.
- Graph coverage beyond one to three nodes remains a tracked future obligation
  for Proptest.
- If the production IR representation changes away from `HashMap` or owned
  strings, the Kani bound should be re-evaluated instead of copied forward.
- The sibling-file harness layout is a project-local constraint caused by the
  400-line source-file limit, not a general Kani requirement.

## Related documents

- [`docs/developers-guide.md`](developers-guide.md)
- [`docs/execplans/4-2-1-kani-harnesses-for-manifest-to-ir-safety-checks.md`](execplans/4-2-1-kani-harnesses-for-manifest-to-ir-safety-checks.md)
- [`docs/execplans/4-2-2-kani-harnesses-for-cycle-canonicalization.md`](execplans/4-2-2-kani-harnesses-for-cycle-canonicalization.md)
- [`docs/formal-verification-methods-in-netsuke.md`](formal-verification-methods-in-netsuke.md)
- [`docs/roadmap.md`](roadmap.md)

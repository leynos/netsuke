# Architectural decision record (ADR) 004: Bound Kani IR harnesses to small N

## Status

Accepted.

Accepted: 2026-06-12. Netsuke will verify manifest-to-IR safety properties with
small bounded Kani harnesses and delegate larger graph coverage to the future
Proptest layer.

## Date

2026-06-12.

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
- Avoid verification-only public API or collection ports.
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

### Option C: introduce verification-only ports

This would add traits or alternate collections so Kani could run against a
simpler implementation.

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

The duplicate-output harness verifies the private duplicate-output error
constructor in `src/ir/from_manifest.rs`, rather than the full
`BuildGraph::from_manifest` path or the target-map lookup. This keeps the proof
focused on variant construction and duplicate reporting after direct attempts
through manifest lowering and `HashMap<Utf8PathBuf, _>` lookup exceeded the
solver budget in path and map hashing. Under `cfg(kani)`, this constructor also
uses the real duplicate-output message key without formatted arguments so the
proof does not execute localization formatting internals.

The rule-selection harness follows the same boundary. It verifies private
constructors for empty-rule, multiple-rules, and missing-rule errors, rather
than the full `resolve_rule` dispatch. Direct attempts through `resolve_rule`
pulled in default `HashMap` random-state initialisation, `StringOrList` vector
conversion, and string sorting/drop internals. Existing Rust behavioural tests
continue to cover the integration path from manifest shape to those errors.

## Known risks and limitations

- The duplicate-output harness no longer proves that full manifest lowering or
  target-map lookup reaches the duplicate-output branch; existing unit and
  behavioural tests keep that integration path covered.
- The duplicate-output harness does not prove rendered Fluent output for that
  error variant. It proves the message key is selected and the semantic payload
  is preserved.
- The rule-selection harness no longer proves that full `resolve_rule` dispatch
  reaches each branch; existing behavioural tests keep that path covered.
- Graph coverage beyond one to three nodes remains a tracked future obligation
  for Proptest.
- If the production IR representation changes away from `HashMap` or owned
  strings, the Kani bound should be re-evaluated instead of copied forward.
- The sibling-file harness layout is a project-local constraint caused by the
  400-line source-file limit, not a general Kani requirement.

## Related documents

- [`docs/developers-guide.md`](developers-guide.md)
- [`docs/execplans/4-2-1-kani-harnesses-for-manifest-to-ir-safety-checks.md`](execplans/4-2-1-kani-harnesses-for-manifest-to-ir-safety-checks.md)
- [`docs/formal-verification-methods-in-netsuke.md`](formal-verification-methods-in-netsuke.md)
- [`docs/roadmap.md`](roadmap.md)

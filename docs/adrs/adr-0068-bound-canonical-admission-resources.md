# Architectural decision record (ADR) 0068: Bound canonical admission resources

## Status

Accepted.

## Date

2026-10-30

## Context and problem statement

[RFC 0006 §6.7](../rfcs/rfc-0006-ansible-inspired-template-standard-library-expansion.md)
defines canonical value equality as RFC 8785 byte-identity. The
`src/stdlib/canonical_value/` adapter materializes a value into a private
snapshot tree before serializing it, and it allocates one snapshot subtree per
collection *occurrence*. `MiniJinja` values are reference-counted, so two
members of one collection may be clones of a single host object. A host value
is therefore a directed acyclic graph, not necessarily a tree.

That difference is the defect reported in
[#972](https://github.com/leynos/netsuke/issues/972) (finding
`af4acf2f-2f4f-43a0-ada6-b1b351ef4a80`, CWE-400). A depth-`d` chain in which
each level holds two clones of one child is `d + 1` host objects but
`2^(d + 1) - 1` occurrences. Admission of such a value performed unbounded
allocation and traversal work, and a depth-64 fixture (65 objects) requests
`2^65 - 1` occurrences, which no host can satisfy. The failure arrived as an
allocation abort or an effectively unbounded run rather than as a typed refusal.

`ManifestBudget` ([ADR-0050](adr-0050-bound-manifest-template-evaluation.md))
does not cover this traversal. Its MiniJinja fuel bounds expression evaluation,
is refunded for unused reservations, and is not charged by host callbacks;
deriving a canonical key is a post-evaluation traversal of an already-built
value. A manifest value can therefore be evaluated within its fuel allowance
and still expand without limit during admission.

## Decision

Add a crate-private `budget` module to `src/stdlib/canonical_value/` and thread
one budget through every admission traversal.

- `AdmissionLimits` carries `depth`, `nodes`, and `bytes`, each a maximum.
  `AdmissionBudget` carries those limits plus the used node and byte counters.
  Both are visible only inside the module; neither is a public configuration
  surface.
- The defaults are 128 edges below the snapshot root, 100,000 snapshot nodes,
  and 8 MiB of admitted string and member-name bytes.
- The budget is **occurrence-based**. The root is charged one node, and each
  enumerated item or pair is charged one node as it is pulled. Recursion does
  not charge a child a second time, so a completed admission charges exactly
  the nodes it allocated. This is what bounds a shared graph by the expansion
  it requests rather than by its object count.
- Depth is a bound, not a counter. It is checked when a value is admitted,
  before a container enumerates, so it also caps the recursion depth of
  snapshot serialization and teardown. A cycle ends as a `Depth` refusal; the
  adapter still does not detect cycles as such.
- String scalars and mapping member names charge their UTF-8 byte length
  before the owning copy is made. Member names are charged after the
  non-string-key check and before insertion into the sorting `BTreeMap`.
- Charges use checked arithmetic, a charge that lands exactly on a ceiling
  succeeds, and a charge that would exceed it leaves the counter unchanged.
- Enumeration is bounded with `take(remaining + 1)`. The extra pull turns an
  over-long or endless enumeration into a `Nodes` refusal before the reported
  length check, and the refusal is raised before the snapshot vector is
  allocated or the sorting map is built.
- One budget serves one public operation. `canonical_key` creates a fresh
  budget; `canonical_eq` shares one budget across its left and right operands,
  keeping the left-refusal short-circuit; `stable_deduplicate` shares one
  budget across every input, keeping the earliest refusal and producing no
  partial result.
- Refusal reporting is
  `CanonicalValueError::ResourceLimitExceeded { resource, limit, path }`, with
  the new public `CanonicalResource` enum naming `Depth`, `Nodes`, or `Bytes`.
  `kind()` returns `None` for the variant, and no Fluent key is added: roadmap
  6.1.4 owns localized diagnostics.
- Refusal precedence is: bounded enumeration and its node charge, then the
  reported-length check, then the non-string-key check, then the duplicate-name
  check, then member-name bytes and child admission in sorted name order. A
  mapping therefore refuses a non-string key or a duplicate name before any
  value-level resource refusal can fire.

## Rationale

Occurrence charging is the only accounting that matches the work admission
performs. Counting distinct host objects would under-count the expansion being
defended against, and `MiniJinja` exposes no public object-identity hook with
which to count them in the first place.

Bounding depth separately keeps recursion finite even for an input whose node
count would otherwise be small, and checking it before enumeration means a
refusal happens before the subtree that crosses the bound is built. Charging
bytes before copying means an oversized string is rejected without being
duplicated, and it makes member names part of the same allowance as values.

The limits are not configurable because they are a safety bound on a library
surface rather than operator policy. The supported input domain sits far below
every default — the property generator reaches depth 3 and the deepest fixture
that is expected to be admitted reaches depth 12 — so no supported input
changes behaviour. A caller needing a different ceiling needs a different API,
not a different number, and making the ceiling configurable would require the
policy ownership, validation, and localization that roadmap 6.1.4 and the CLI
policy layer own.

## Consequences

A shared acyclic graph within the defaults is admitted and has the same
canonical key as an independently allocated expansion of itself; a depth-4
graph is 31 occurrences and well inside the node ceiling. Inputs that would
expand past 100,000 occurrences, reach beyond 128 edges, or admit more than 8
MiB of strings and member names are refused with a typed error before the
overflowing subtree exists.

`canonical_eq` can refuse an operation whose operands each pass `canonical_key`
independently, because one budget serves both. This is intended: the ceiling
bounds the work of one public operation, not one value, and a caller that wants
per-value admission should call `canonical_key` per value.

Admitted nodes and bytes, together with JSON escaping expansion, transitively
bound the serializer's output; no separate output cap is added. Internal
buffering inside `serde_json_canonicalizer` 0.3.2 was not verified as part of
this decision, and neither was MiniJinja fuel governance; both are recorded
here as unverified rather than assumed.

The refusal message is stable English prose until roadmap 6.1.4 supplies
localized diagnostics. Cycles and unstable host enumeration remain caller
preconditions: an infinite enumeration is refused as `Nodes` and a cycle as
`Depth`, but neither is diagnosed as the condition it is.

## Alternatives considered

- **Charge distinct host objects rather than occurrences.** Rejected because
  the adapter cannot observe object identity through the public `MiniJinja`
  API, and because object count would not bound the expansion the traversal
  actually materializes.
- **Reuse `ManifestBudget`.** Rejected because its fuel is refunded and is not
  charged by host callbacks, so it cannot bound a post-evaluation traversal,
  and because sharing it would let an unrelated earlier expression consume the
  allowance for a later key comparison.
- **Check size after materialization.** Rejected because the check would run
  only after the expansion being defended against had already been allocated.
- **Make the ceilings configurable.** Rejected as public surface for a safety
  bound, with the policy and diagnostic ownership belonging to roadmap 6.1.4
  and the CLI policy layer.
- **Detect cycles explicitly.** Rejected because no public identity hook
  exists; the depth bound already terminates a cyclic input with the same typed
  refusal family.

## Implementation references

- [`src/stdlib/canonical_value/budget.rs`](../../src/stdlib/canonical_value/budget.rs)
- [`src/stdlib/canonical_value/admission.rs`](../../src/stdlib/canonical_value/admission.rs)
- [`src/stdlib/canonical_value/error.rs`](../../src/stdlib/canonical_value/error.rs)
- [`src/stdlib/canonical_value/mod.rs`](../../src/stdlib/canonical_value/mod.rs)
- [`src/stdlib/canonical_value/tests/budget.rs`](../../src/stdlib/canonical_value/tests/budget.rs)
- [`src/stdlib/canonical_value/tests/diagnostics.rs`](../../src/stdlib/canonical_value/tests/diagnostics.rs)
- [`tests/features/canonical_value.feature`](../../tests/features/canonical_value.feature)

## References

- [#972](https://github.com/leynos/netsuke/issues/972): bound expansion of
  shared collection graphs during canonical admission.
- [RFC 0006 §6.7](../rfcs/rfc-0006-ansible-inspired-template-standard-library-expansion.md):
  canonical value equality.
- [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785): JSON Canonicalization
  Scheme.
- [ADR-0050](adr-0050-bound-manifest-template-evaluation.md): the manifest
  evaluation budget this decision deliberately does not extend.

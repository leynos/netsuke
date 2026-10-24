# Architectural decision record (ADR) 0066: Contention class schema

## Status

Proposed. This decision remains pending review and explicit acceptance.

## Date

2026-10-24

## Context and problem statement

RFC 0024 proposes named contention classes that lower to Ninja pools. The
schema must constrain concurrent executable edges without introducing a second
scheduler, conflating dependency order with resource limits, or suggesting that
a pool controls the worker threads launched by one command. Public class names
also cannot be emitted directly as Ninja identifiers.

## Decision Drivers

- Reuse Ninja's existing edge scheduler and pool machinery.
- Keep one class and one scalar annotation sufficient for the initial surface.
- Preserve unannotated scheduling and legacy command syntax.
- Make limits stable, bounded, and explainable to operators.

## Requirements

### Functional requirements

- Declare named classes under `contention_classes` with a positive capacity.
- Allow at most one scalar `contention` reference per executable action or
  target.
- Reject class annotations on dependency-only aggregates and rules in the
  initial version.
- Report requested and effective capacity with provenance.

### Technical requirements

- Cap schema capacity independently of CLI `jobs`.
- Combine declared capacity with a trusted operator ceiling using ADR-0062.
- Lower public names through a typed identity and safe internal encoding.
- Reject incompatible use of Ninja's `console` pool rather than weaken either
  guarantee.

## Options considered

### Option A: One bounded class per executable edge

Resolve a public class name to an internal identity and lower that identity to
one Ninja pool. The complete action edge holds its slot for the full command
sequence.

### Option B: Serialize dependency aggregates

Dependencies express ordering and data requirements, not reusable contention
policy. Serializing an aggregate can block unrelated work while failing to
constrain other actions using the same resource.

### Option C: Add a general multi-resource scheduler

A new ready queue and multiple resource locks would duplicate Ninja and
increase the correctness and recovery burden.

## Decision outcome / proposed direction

The root `contention_classes` mapping contains declarations with `capacity`. An
executable action or target may contain one scalar `contention` reference.
Lists, missing references, unknown fields, duplicate declarations, reserved
internal names, and annotations on dependency-only aggregates fail validation.
Rule annotations are unsupported in the first version and receive an explicit
diagnostic; rules do not silently supply class defaults.

Capacity is an integer from 1 through the schema-owned maximum 1024. Reject
zero because Ninja treats pool depth zero as unlimited. Also reject negative,
fractional, Boolean, unbounded, and out-of-range values. The effective capacity
is the minimum of the declared value and the trusted
`contention_capacity_ceiling`; inspection reports both values and provenance.
The ceiling is independent of the CLI `jobs` limit.

Resolve public names to typed internal class identities before backend
emission. Never pass an unchecked name into Ninja text. Roadmap item 22.1.2
uses ADR-0014's escaping seam and the reserved `netsuke_pool_` prefix with an
injective, declaration-order-independent encoding, then fixes and
property-tests the concrete encoding. Emit a pool only for a used class, and
hold the edge slot across the complete multi-command action.

An edge that needs both a contention class and Ninja's special `console` pool
is rejected. No current surface requests the console pool, but a future request
cannot silently discard either guarantee. Pools constrain eligible edges within
one Ninja invocation; they do not impose dependency order, fairness,
cross-invocation exclusion, or limits on subprocess worker counts.

## Goals and non-goals

- Goals:
  - Lower named contention to existing Ninja pools.
  - Preserve dependencies and internal worker settings as separate concepts.
  - Make the single-invocation boundary explicit.
- Non-goals:
  - Add multiple resource acquisition, weighted tokens, or a second scheduler.
  - Provide cross-host or cross-invocation locking.
  - Require contention annotations for ordinary commands.

## Migration plan

1. Roadmap item 22.1.1 implements declarations, references, capacity bounds,
   and operator ceilings.
2. Item 22.1.2 lowers typed class identities to Ninja pools and adds the
   persisted scheduling metadata through the shared codec.
3. Item 22.2.1 proves measured edge concurrency and cancellation behaviour;
   22.2.2 documents inspection and the invocation-only limit.

## Known risks and limitations

- One Ninja pool does not coordinate separate invocations or an external
  process that accesses the same resource.
- A pool is not a CPU, memory, or subprocess-worker quota.
- Rule-level inheritance may not match recipe resolution; the initial schema
  rejects it until that contract is settled.

## Outstanding decisions

- Roadmap item 22.1.2 owns the concrete class-name encoding and its
  declaration-order-independence tests; merge an ADR amendment before its code
  merges if the encoding contract changes.
- A later rule-default form requires an explicit recipe-inheritance decision
  and ADR amendment before implementation; 22.1.1 rejects it initially.
- Multi-resource allocation and cross-host scheduling need separate proposals
  and have no implementation owner in this feature.

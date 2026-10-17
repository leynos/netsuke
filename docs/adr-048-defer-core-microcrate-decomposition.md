# Architectural decision record (ADR) 048: Defer core microcrate decomposition

## Status

Proposed.

## Date

2026-10-03.

## Context and problem statement

[RFC 0031][decomposition] proposes a cohesive `netsuke-core` as a shared
compilation boundary. Further splitting authored syntax, resolved graph types,
lowering helpers, hashing, and recipe-shell primitives could reduce some
invalidation, but would also introduce public interfaces and publication edges
before measurements establish their value.

The current AST/IR relationship is not the desired semantic contract.
[RFC 0026][semantics] and [ADR-035][semantic-adr] already require resolved
operations to exclude authored recipe states, bind their interpreter, and report
typed diagnostic facts. Physical colocation must not defer those corrections.

## Decision drivers

- A small initial extraction must remain reviewable and measurable.
- The resolved model needs stronger invariants regardless of crate count.
- Rust privacy, inherent implementations, and trait coherence constrain type
  movement; public unchecked constructors are not an acceptable shortcut.
- Tiny packages still incur release, feature, dependency, and compatibility
  costs, even when their source volume is small.

## Proposed direction

Extract a cohesive core when RFC 0031's gates pass, but defer separate AST,
IR-types, hashing, and shell microcrates. Keep authored and resolved types in
clearly owned modules initially. Parser/evaluation implementation and application
orchestration do not belong in core merely because they use those types.

Preserve the semantic direction: resolved operation definitions must not retain
`ast::Recipe`; lowering may consume both representations. Only checked
construction paths may produce valid executable operations, under
[ADR-054][construction]. A graph-building convenience must not weaken that rule.

Keep only genuinely shared recipe/shell and action-identity primitives in core.
An implementation-head dependency inventory decides ownership, not the current
name of a module. A feature-private digest helper stays with its feature.

The target semantic model has no localisation dependency. If a bounded first
extraction retains existing `LocalizedMessage` use temporarily, record that
exception and its retirement dependency on RFC 0026. During that transition,
semantic-diagnostic mapping stays in the application; `netsuke-l10n` must not
import core and thereby create a cycle. Do not call the transitional package
presentation-independent or authorize new rendering dependencies.

This decision defers additional physical subdivisions. It does not defer the
initial shared boundary, semantic hardening, or the compiler library needed by
independent linting and manifest testing.

## Alternatives considered

Creating all microcrates immediately would settle API ownership before actual
consumers and benchmark evidence exist. Leaving every shared type in the
application would prevent independent component builds. Treating physical
colocation as permission for unresolved semantic states would contradict the
existing model proposal.

## Consequences

A central model edit may still invalidate many consumers. That is an expected
trade-off, not a failed promise that every change becomes local. Stable sibling
implementations can still benefit from the initial extraction.

Some future type movement may remain necessary. Compatibility re-exports can
preserve paths where appropriate, but cannot erase coherence or construction
obligations. The core must not become a destination for every shared helper.

## Verification and revisit criteria

Reopen a particular subdivision only when an implementation-head inventory
identifies a concrete independent consumer or compatibility boundary, or
[RFC 0030][benchmark] locates a practical compilation bottleneck that the split
can address. Record the intended dependency graph, public API cost, and
before/after evidence before accepting it.

Acceptance requires no dependency cycle, preserved checked construction and
canonical graph invariants, focused tests, package-consumer checks, and either
measured workflow benefit or a separately reviewed semantic benefit worth its
measured cost. Absence of that evidence means retaining the cohesive core.

## Outstanding decisions

The extraction inventory determines exact type and algorithm placement. This
record does not reserve additional registry names or require speculative
packages before their consumers exist.

[decomposition]: rfcs/0031-staged-crate-decomposition.md
[semantics]: rfcs/0026-hexagonal-domain-hardening.md
[semantic-adr]: adr-035-semantic-compiler-boundaries.md
[construction]: adr-054-preserve-validated-operation-construction.md
[benchmark]: rfcs/0030-build-performance-benchmark-extension.md

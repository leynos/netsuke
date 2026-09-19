# ADR-030: Enforce architecture through complementary checks

## Status

Proposed.

## Date

2026-09-19.

## Context and problem statement

Documentation and ordinary lints do not reliably prevent architectural drift.
Import rules alone cannot distinguish a valid resolved operation from a renamed
AST variant. Enforcement also fails when an empty scan or widened exception
silently produces a clean result. [RFC 0027][rfc] defines the proposed checks.

[rfc]: rfcs/0027-executable-architecture-contract.md

## Decision drivers

- Enforce semantic responsibilities without abandoning feature colocation.
- Detect newly introduced dependency and effect-boundary violations.
- Make omitted analysis and migration debt visible.
- Keep diagnostics actionable for maintainers and implementation agents.
- Test enforcement itself and bound its operational cost.

## Proposed direction

Combine a repository-local Rust dependency checker, existing effect lints,
type/API restrictions, shared port contracts, and production integration tests.
Borrow Wildside's in-memory checker decomposition, Corbusier's ownership and
contract discipline, and Hecate's declarative policy and explainable exceptions,
with the adaptations and source references recorded in RFC 0027.

Separate source inventory, bounded dependency analysis, policy evaluation, and
rendering. Classify model and lowering separately. Missing roots, unsupported
protected-boundary forms, unclassified internal modules, and unresolved origins
must never become implicit permission. Keep the analysis subset explicit rather
than claiming compiler-complete resolution from syntax inspection.

Introduce a reviewed exception ledger for existing debt, not an automatically
generated allow-list. Scope exceptions by rule and stable source/target
identity, with reasons, remediation owners, and removal conditions. Fail on
stale, broadened, or malformed exemptions. Remove each exception with its
remediation.

Qualify every rule with positive and negative fixtures and test gate failure
propagation through the real command. Retain separate doctest execution and
existing effect-policy compiler probes. Require review for policy, exception,
source-root, marker, and gate-wiring changes.

## Alternatives considered

Documentation alone cannot detect regressions. Directory-only checking ignores
Netsuke's mixed compiler/model responsibilities. A bespoke complete Rust
resolver is too large an initial dependency. A crate split may eventually
strengthen well-understood boundaries but cannot replace deciding what those
boundaries mean. Paralegal remains a separate experiment, not the baseline
enforcement tool.

## Consequences and migration

Phase 28 of the [architecture roadmap](roadmap-hexagonal-hardening.md)
introduces the policy, debt ledger, checker, executable probes, and qualified CI
gate. Phases 26 and 27 can retire debt before the full checker is complete. Do
not block those transformations on optional semantic analysis.

Build/cache installation follows existing runner limits: at most 4 vCPU and 8
GiB for builds and 1 vCPU and 2 GiB for non-build checks. Record measured cold
and warm cost before required CI adoption; do not add recurring work or silently
increase runner size. Documentation acceptance alone installs no gate.

## Known risks and limitations

Syntax analysis has explicit macro, generated-code, and name-resolution limits.
Types do not prove adapter behaviour, and negative examples do not prove that a
policy captures every possible violation. Maintainer review remains necessary,
especially when a contributor can change both the policy and its tests.

## Outstanding decisions

Approve the precise source inventory, permitted value dependencies, exception
review ownership, and measured gate budget before enabling strict enforcement.

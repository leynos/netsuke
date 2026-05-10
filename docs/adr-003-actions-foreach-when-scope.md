# Architecture Decision Record (ADR): Keep `foreach` and `when` scoped to manifest targets

## Status

Accepted

## Date

2026-05-08

## Context and problem statement

Netsuke manifests support `foreach` and `when` control keys during manifest
target expansion. The implementation in `src/manifest/expand.rs` walks the
top-level `targets` sequence, expands each target into zero or more concrete
targets, and removes the control keys before downstream deserialization.

The open question is whether the same control keys should also be supported on
individual actions inside a target. Action-level support would make a single
target able to expand or filter its recipe steps independently of target
generation.

## Decision outcome

Netsuke supports `foreach` and `when` at target level only. Action-level
`foreach` and `when` are not part of the manifest contract.

## Rationale

Target-level expansion keeps the manifest model easy to reason about: one
control pass creates the concrete target list, then later stages validate and
render ordinary target and action data. Allowing actions to expand themselves
would introduce a second expansion phase with different ordering, dependency,
and error-reporting rules.

Keeping the scope target-only also preserves clear diagnostics. Errors from
`foreach` and `when` always point to target generation, not to a partially
rendered recipe body whose surrounding target may or may not survive
filtering.

## Alternatives considered

- **Support action-level `foreach` and `when`.** Rejected because it adds a
  second expansion surface, complicates validation order, and makes it harder
  to explain whether target guards or action guards run first.
- **Support action-level `when` only.** Rejected because it still introduces a
  separate filtering phase and creates an asymmetric contract with target
  expansion.
- **Treat action-level control keys as ordinary action data.** Rejected because
  silently accepting likely-mistyped control keys would make manifests harder
  to debug if action-level support is ever added later.

## Consequences

- Documentation and tests should describe `foreach` and `when` as target-level
  controls.
- Future action-level repetition should be designed as a separate manifest
  feature with its own ADR before implementation.
- Validators MUST reject action-level `foreach` and `when` keys. Validators
  are required to produce an error, not merely a warning, when these control
  keys appear at action level so mistyped control keys are not silently
  accepted.

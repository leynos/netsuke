# Architectural decision record (ADR) 053: Separate findings from runtime authority

## Status

Proposed.

## Date

2026-10-03.

## Context and problem statement

Maturity policies and semantic linting can report quality findings with
configurable severity. Runtime policies constrain effects such as network
access, executable selection, environment reads, and scoped deletion. Treating
both as suppressible findings would allow a presentation preference to weaken
an authority boundary.

[RFC 0025][maturity] retains opt-in, scoped enhancement and trust-aware policy.
[RFC 0034][ownership] proposes a `netsuke-policy` component without making it
depend on concrete execution, state, or cleanup engines.

## Decision drivers

- Suppressing a quality finding must never grant an effect permission.
- Effective policy must preserve source trust and operator restrictions.
- Every relevant effect boundary must enforce the applicable decision.
- Shared observations must have one semantic owner, not competing analysers.

## Proposed direction

Represent maturity/quality evaluation and runtime authority decisions as distinct
contracts, even if the same policy component houses their pure composition
logic. A configurable lint severity affects reporting and the relevant quality
gate. It cannot convert a denied capability into an allowed one.

Keep source provenance and trust in effective-policy composition. Apply the
existing feature-specific authority rules, retaining the strongest applicable
restriction. Do not use an unqualified last-wins merge or duplicate the generic
configuration precedence engine. Do not let untrusted project declarations
increase operator-granted authority.

The policy component consumes typed semantic observations or validated operation
summaries. It does not call the linter to discover them or depend on concrete
network, executor, state, or artefact implementations. Compiler-owned inventory
and provenance remain authoritative.

Production adapters enforce decisions where effects occur. A check during
planning does not eliminate redirect checks, executable/working-directory
validation, or replay-time authority validation. Retain feature-specific
boundedness and redaction requirements. Missing policy information must follow
a documented failure or restricted-result contract, not silently select host
capabilities.

Keep finding suppression, severity, diagnostic localization, and JSON rendering
outside the permission-granting path. Decision evidence should identify the
relevant policy source and denied capability without exposing secrets.

These Rust boundaries make ownership auditable but do not by themselves create
an operating-system sandbox or defend against arbitrary native code with ambient
privileges. State the enforcement scope and test the actual adapters.

## Alternatives considered

A unified suppressible finding type is convenient for reporting but unsafe as
an authorization result. Policy implemented independently in each feature can
diverge on trust precedence. A policy crate that imports every effect engine
recreates a large dependency closure and risks cycles.

## Consequences

Some operations need both a quality result and an authority decision. Their
messages can share presentation machinery, but their semantics remain distinct.
Boundary tests supplement import checks because a valid crate graph does not
prove that an adapter performs its required check.

## Verification and revisit criteria

Negative tests must show that disabling a rule, lowering its severity, changing
output mode, or selecting an alternate renderer cannot enable a forbidden
effect. Cover project/operator conflicts, absent provenance, redirects,
persisted-plan replay, and partial-failure paths where applicable.

Positive controls cover permitted operations under the same effective policy.
Dependency checks reject concrete engine imports into policy. Shared inventory
contracts ensure linting and maturity evaluation do not reimplement compilation.

Any future proposal to combine finding and authority representations must prove
that suppression cannot change permission; convenience alone cannot reopen the
separation. A new policy language or sandbox requires a separate proposal.

## Outstanding decisions

Feature RFCs retain exact authority lattices, failure classifications, and syntax.
Implementation review determines the smallest shared decision vocabulary without
flattening distinct resource policies into an untyped bag of options.

[maturity]: rfcs/0025-progressive-enhancement-and-maturity-policies.md
[ownership]: rfcs/0034-forthcoming-component-ownership.md

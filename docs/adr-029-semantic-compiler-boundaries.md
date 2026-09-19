# ADR-029: Own semantic compiler and execution boundaries

## Status

Proposed.

## Date

2026-09-19.

## Context and problem statement

The build model still stores authored recipe variants, while shell-dependent
lowering relies on callers preserving interpreter selection. Application
orchestration and semantic failures also retain CLI or presentation concerns.
These are semantic and ownership weaknesses, not evidence that every function
needs an interface. [RFC 0026](rfcs/0026-hexagonal-domain-hardening.md)
specifies the baseline, migration, and acceptance obligations.

Canonical edge storage already exists on the inspected baseline and must remain
intact. Existing process request types, capability-bearing directories, dyndep
leases, environment seams, and deterministic generation are assets to preserve.

## Decision drivers

- Successful compilation must exclude unresolved executable states.
- A lowered recipe must retain the interpreter that gives its quoting meaning.
- Application callers and tests must not construct CLI state to request a build.
- Semantic error facts must remain independent of their rendered wording.
- New abstractions must preserve authority and reuse existing contracts.

## Proposed direction

Adopt model-owned resolved operations, explicit dependency-only semantics, and
an enforced shell-bound recipe or plan. Keep AST-to-model conversion in the
compiler, which may depend on both representations. Restrict construction and
mutation so the backend can trust successful lowering.

Map inbound CLI/configuration state into application-owned requests. Introduce a
narrow execution port with a production Ninja adapter and a recording/failing
implementation. Preserve temporary artefact and lease ownership through the
preparation/execution lifecycle. Do not substitute arbitrary filesystem paths
for capabilities or create a general process framework.

Keep semantic diagnostics as typed facts. Render Fluent and structured output at
presentation boundaries and classify recipe-validation failures before they
become parser display strings. Preserve diagnostic and manifest compatibility.

Continue [ADR-008](adr-008-environment-seam-taxonomy.md)'s proportionate seams,
[ADR-014](adr-014-backend-text-escaping-seam.md)'s backend escaping ownership,
and [ADR-006](adr-006-adopt-polonius-nightly-toolchain.md)'s compiler contract.
No universal effects interface, folder-first rewrite, or broad crate split is
part of this decision. Pure transformations remain functions.

## Alternatives considered

Retaining AST variants avoids migration but leaves successful lowering weak.
Merely moving the enum changes imports without improving its states. A full
backend/plugin abstraction or layered directory rewrite adds scope without
establishing the missing invariants. Prefer narrow semantic changes.

## Consequences and migration

Phases 26 and 27 of the [architecture roadmap](roadmap-hexagonal-hardening.md)
characterize behaviour, migrate recipes and shell binding, separate diagnostics,
and expose the application contract. Types, behavioural/property tests, and
adapter integration tests establish the resulting guarantees. The structural
checker provides a no-new-violations policy during migration.

Provenance and resource-fetching extensions retain the existing composition,
linter, and URL-provider owners; they are conditional rather than immediate
port-extraction mandates. The proposal adds no v0.1.0 release gate and no
Netsukefile annotations. Legacy simple manifests remain supported.

## Known risks and limitations

Rule delegation, declaration precedence, action identity, and diagnostic shape
need characterization before refactoring. A source-compatible-looking type move
can still change emitted output or rebuild behaviour. No static dependency rule
proves correct quoting, successful publication, or subprocess behaviour.

## Outstanding decisions

Settle the characterized rule/declaration policy and smallest shell-bound
representation before their implementation. Review execution/preparation lease
ownership against the existing dyndep contract. Acceptance of this record does
not mark any migration task complete.

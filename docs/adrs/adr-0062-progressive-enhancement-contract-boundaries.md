# Architectural decision record (ADR) 0062: Progressive-enhancement contract boundaries

## Status

Proposed. This decision remains pending review and explicit acceptance.

## Date

2026-10-24

## Context and problem statement

RFCs 0021 to 0025 add optional capabilities that compose across manifest
loading, structured commands, configuration, reporting, and persisted plans. If
each RFC owns its own parser, version signal, or runner path, the features will
duplicate common machinery and make compatibility depend on implementation
order. The proposals also need to remain useful one at a time: a basic build
must not acquire a state, input, cleanup, or policy prerequisite merely because
another task uses one.

Several decisions therefore cross the boundary of any single schema proposal:
the feature identifiers used for admission and reporting, ownership of the
structured command operation union, the shallow-end contract, persisted-format
coordination, and one rule for combining operator ceilings.

## Decision Drivers

- Give each optional capability one stable identifier and one accountable
  implementation owner.
- Keep command parsing, encoding, and execution in the structured-command
  owner.
- Preserve legacy manifests and let each feature remain optional.
- Coordinate persisted formats without coupling them to manifest minors.
- Ensure project configuration cannot raise trusted operator limits.

## Requirements

### Functional requirements

- Distinguish feature identifiers from maturity rule identifiers.
- Report reader capabilities by feature identifier through existing query and
  lint surfaces.
- Register every structured-command discriminator and reject unregistered or
  unsupported forms.
- Keep optional top-level declarations empty by default, silent when absent,
  and local to the nodes that use their feature.
- Apply one monotone merge rule to numeric ceilings and Boolean denials.

### Technical requirements

- The design-document schema registry is the open list of feature identifiers.
- The command AST, parser, action-plan codec, and runner dispatch remain owned
  by RFC 0001 and phase 12.
- Reuse OrthoConfig and existing diagnostics, metadata, and JSON envelopes.
- Do not introduce a feature-private parser, persisted format, or command
  execution path.

## Options considered

### Option A: Record shared contracts once and keep schema decisions feature-local

One cross-cutting ADR fixes identifiers, operation ownership, compatibility,
persisted-format coordination, and operator ceilings. Separate ADRs settle the
syntax owned by states, inputs, artefacts, contention, and maturity. Each
feature continues to use the shared parser, codec, runner, and reporting
surfaces.

### Option B: Let each feature define its own integration contract

Each RFC could choose its own discriminator handling, version allocation,
configuration merge, and persistence rules. This permits local decisions but
creates competing owners and makes cross-feature composition a sequence of
special cases.

### Option C: Require a global feature and policy declaration

A manifest-wide list could enable capabilities and enforce annotations
uniformly. That would make optional facilities contagious and duplicate the
schema-version opt-in.

## Decision outcome / proposed direction

### Feature identifiers and capability reporting

Feature identifiers and maturity rule identifiers are separate namespaces. The
registry in `docs/netsuke-design.md` is the open list of feature identifiers.
Its initial optional entries are `structured-commands`, `named-shells`,
`includes`, `bundles`, `host-facts`, `inputs`, `contention`, `states`,
`artefacts`, and `maturity`. A future identifier is reserved there before its
syntax is implemented.

Capability reporting is keyed by feature identifier. Roadmap item 20.2.1 owns
the JSON shape; phase 5 owns its `context` surface; `netsuke check` reports
maturity findings through the existing lint envelope in ADR-0056. A policy that
requests a rule whose required feature is unavailable fails with a version or
capability remedy. A reader must not report an unimplemented feature as
available merely because its identifier is registered.

### Structured command operation union

Phase 12 owns one closed operation union: its abstract syntax tree (AST),
parser, action-plan codec, and runner dispatch. Its registered discriminators
are `invoke`, `rule`, and `script` from RFC 0001, `require_state`,
`ensure_state`, and `prepare_state` from RFC 0021, and `clean_owned` from RFC
0023. Each mapping contains exactly one registered discriminator.

The four state and cleanup operations may also be the direct value of
`command:` because no recipe-level form covers them. State preparation and
external probes reuse the structured command-block type. Feature RFCs own their
variants' validation and semantics; they do not add a private parser, codec, or
runner. Roadmap task 12.1.2 reserves these discriminators and rejects their use
until the owning feature is admitted.

### Shallow end and persisted formats

For every schema-1 release, optional top-level declarations default to empty.
Their absence produces no message at any verbosity, and using a feature on one
node does not require unrelated declarations elsewhere. The unchanged
hello-world fixture remains the acceptance baseline for this contract; its
fixtures belong to roadmap item 20.1.2.

Persisted formats are versioned independently of the manifest schema. Action
plans are versioned by the variants they contain, and a runner rejects a plan
variant it cannot represent. State records use a versioned runtime namespace
separate from dyndep sidecars. An unknown record version is no evidence of
readiness. When an identity is declared, missing evidence yields `not_ready`
only after the resource can be inspected successfully; otherwise the result is
`unknown`. Lease identity stays outside the versioned record namespace so two
Netsuke versions cannot prepare the same path concurrently, and retention
covers stale namespaces. ADR-0012 continues to own dyndep sidecars. The JSON
output envelope evolves additively within version 1 under roadmap item 20.2.1.

### Operator ceilings

A numeric ceiling set by a trusted configuration layer is a maximum. The
untrusted project `.netsuke.toml` layer may lower it but cannot raise it. The
effective value is the minimum, and diagnostics or inspection retain both the
effective value and its provenance. Boolean denials follow ADR-0021's
tightening rule: a less trusted layer may add a denial but cannot remove one.

Ceiling names follow `<feature>_<quantity>_ceiling[_<unit>]`; Boolean denials
follow `<feature>_<capability>_deny`. Declare and merge these settings through
the existing OrthoConfig seams and the configuration contracts in ADR-0021,
ADR-0026, and ADR-0050.

## Goals and non-goals

- Goals:
  - Reuse shared command, configuration, reporting, and persistence boundaries.
  - Keep optional features independently adoptable and identifiable.
  - Make operator restrictions monotone across configuration layers.
- Non-goals:
  - Define the feature-specific schema fields settled by ADR-0063 to ADR-0067.
  - Add a new capability command, report envelope, parser, or runner.
  - Version persisted formats with `netsuke_version`.

## Migration plan

1. Record feature identifiers and reserved discriminators in the design
   registry before implementing their feature-specific variants.
2. Extend the phase-12 AST, codec, and runner once per admitted discriminator.
3. Add capability reporting and shared operator configuration through roadmap
   item 20.2.1 and phase 5, then let feature owners expose their data through
   those surfaces.
4. Preserve the existing action-plan and JSON envelope version contracts as
   variants are added.

## Known risks and limitations

- A registered identifier does not mean a binary implements or reports it.
  Capability output must reflect the exact reader build.
- The public registry requires coordinated edits with the admission change; a
  stale entry can mislead policy authors, so the later contract test binds it
  to the reader.
- A correct ceiling merge does not make an untrusted project configuration
  safe to execute; it only prevents that layer from weakening trusted policy.

## Outstanding decisions

- Roadmap item 20.2.1 must settle the feature-report JSON shape and shared
  OrthoConfig field declarations before those interfaces are implemented.
- Phase 5 owns the `context --json` surface; the existing `netsuke check`
  owner retains maturity findings.
- Roadmap item 12.1.2 or a later admission change must implement raw-document
  checks for feature minima and reserved syntax as specified by ADR-0061.

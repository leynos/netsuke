# Architectural decision record (ADR) 0064: Typed task input schema

## Status

Proposed. This decision remains pending review and explicit acceptance.

## Date

2026-10-24

## Context and problem statement

RFC 0022 proposes optional public inputs so a manifest can validate one
user-facing value without turning every variable into a public parameter. The
initial type and constraint vocabulary must compose with bundle parameters and
must not create a second profile system, execute defaults, or silently convert
legacy command strings to structured commands.

## Decision Drivers

- Validate declared inputs before template evaluation or command execution.
- Promote one existing value without annotating unrelated variables.
- Reuse the parameter contract already proposed by RFC 0003.
- Preserve provenance and redaction through existing configuration surfaces.

## Requirements

### Functional requirements

- Add optional root `inputs`; ordinary `vars` remain valid and unchanged.
- Require `type`; allow `default`, `description`, `expose`, and applicable
  constraints.
- Use one value vocabulary for root inputs and bundle parameters.
- Provide repeatable `--input NAME=VALUE` for explicit caller values.

### Technical requirements

- Keep definitions and defaults literal data, not Jinja expressions.
- Distinguish Boolean values from integers and reject integer overflow.
- Treat a typed path as data, not a filesystem capability or permission grant.
- Use existing canonical CLI metadata, configuration, diagnostics, and
  redaction seams.

## Options considered

### Option A: Add opt-in typed inputs over the shared parameter contract

Authors declare only public values that need validation. The input namespace
appears only when the manifest declares inputs, and callers use the canonical
repeatable option.

### Option B: Treat every variable as a public input

This removes declaration work but exposes internal implementation details and
turns harmless variables into a public interface.

### Option C: Define a second Netsuke-only type language

A separate input type system would duplicate RFC 0003's bundle parameter
contract and make a shared conformance corpus harder to maintain.

## Decision outcome / proposed direction

The root `inputs` mapping is optional. Each definition contains required
`type`, optional `default`, `description`, and `expose`, plus applicable
constraints. No default means the input is required. Defaults and declarations
are literal data; computed internal values remain in `vars` after resolution.
An input's absence does not create a Jinja `inputs` namespace; declaring the
feature creates the namespace explicitly.

The initial closed type vocabulary is `string`, `bool`, `integer`, `path`,
`sequence<string>`, `sequence<path>`, and `mapping<string, string>`. A string
with `choices` represents an enum; there is no separate `enum` or `argv` type.
`minimum` and `maximum` constrain integers. `choices` is a nonempty,
duplicate-free list of values of the declared scalar type. The initial explicit
exposure value is `non-secret`.

Integer inputs use the signed 64-bit range and reject overflow. Booleans are
not integers, and fractional values are not coerced. Strings remain strings,
including empty strings. Null is not an omitted optional value. Path validation
checks encoding and lexical shape without creating files or granting filesystem
authority.

`--input NAME=VALUE` is repeatable, registered once in ADR-0016's canonical
command metadata, and attached to every subcommand that loads a manifest. It is
not a top-level global. Resolution and source provenance use the existing
configuration and profile surfaces; phase 5 owns profile precedence.

## Goals and non-goals

- Goals:
  - Give each selected public value one typed declaration and source.
  - Preserve ordinary variables and one-value promotion.
  - Share type validation with bundle parameters.
- Non-goals:
  - Require unrelated values to be declared or validated.
  - Add executable defaults, custom validator code, or automatic aliases.
  - Define profile precedence or a new help renderer in this ADR.

## Migration plan

1. Roadmap item 21.1.1 establishes the normalized parameter contract shared
   with RFC 0003.
2. Item 21.1.2 adds optional root inputs and immutable resolution before Jinja.
3. Item 21.2.1 connects explicit CLI and profile sources through existing
   metadata and provenance.
4. Item 21.3.1 demonstrates the worker-count canary before considering richer
   defaults or validators.

## Known risks and limitations

- A typed path value does not itself authorize access to the named path.
- The shared integer and path-normalization contracts must be settled before
  root inputs and bundle parameters use them together.
- Profile overlays can change the effective source; diagnostics and output
  must preserve source provenance and redaction.

## Outstanding decisions

- Roadmap item 20.2.1, coordinated with 17.1.3, owns shared type validation
  and path grammar; merge an ADR amendment before that code merges if either
  contract changes.
- Roadmap item 5.3.1 owns profile overlay order before 21.2.1 implements input
  sources; merge an ADR amendment before its code merges if precedence changes.
- Roadmap item 21.3.1 owns the evidence gate for dependent defaults or custom
  validators. Add a separate accepted decision before implementing either.

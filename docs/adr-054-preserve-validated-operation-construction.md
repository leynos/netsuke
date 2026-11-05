# Architectural decision record (ADR) 054: Preserve validated operation construction

## Status

Proposed.

## Date

2026-10-03.

## Context and problem statement

[RFC 0026][semantics] requires successful lowering to exclude unresolved recipe
states and bind the interpreter used to lower an executable. Crate extraction
changes Rust privacy boundaries: `pub(crate)` does not make an item available
to a sibling package, and Rust does not provide an implicit friend-crate
exception for a compiler.

Making fields or unchecked constructors public to complete a mechanical move
would defeat the semantic invariant. Serialization and mutable access can
reopen the same hole even when the primary constructor validates its input.

## Decision drivers

- Every production construction and mutation path must preserve invariants.
- Published Rust APIs must remain safe for callers outside the repository.
- Persisted data is not evidence of current filesystem or process authority.
- Tests and package boundaries must not introduce production escape hatches.

## Proposed direction

Keep invariant-defining types and their validation under one clear semantic
owner. Where compiler and model occupy separate crates, expose a checked
construction interface that performs the required model validation, or colocate
the invariant-establishing lowering kernel with the types behind a narrow API.
Choose the smallest arrangement that satisfies the semantic contract; do not
pretend sibling crates share module privacy.

A public checked constructor may be callable by other consumers. That is
acceptable only if every successful call establishes the same invariants as
compiler lowering. Merely naming an argument `Validated` or accepting a Boolean
assertion from the caller does not establish anything.

Distinguish authored/raw records, validated semantic operations, and runtime
authority-bearing resources. Resolve or reject rule references before a value
can masquerade as an executable. Keep dependency-only operations explicit,
retain interpreter binding, and prevent arbitrary replacement of command text,
interpreter, or other invariant-bearing fields after validation.

Deserializing a persisted plan first produces untrusted data. Validate its schema,
version, references, and semantic constraints through the same checked boundary.
Reacquire and revalidate current runtime authority before execution. A serialized
path, resource identifier, or former capability token cannot itself grant access.
Separate pure semantic validation from the effectful authority check so plan
inspection does not import the executor merely to examine data.

Keep failed validation as typed facts with useful provenance; localization and
formatting do not participate in deciding validity. Preserve canonical edge
identity, aliases, atomic insertion, and deterministic action identity while
changing type ownership.

Tests may construct internal states inside the owning crate to exercise its
validator. Do not expose unchecked constructors through a publicly selectable
Cargo feature or export a private-fixture bypass into published production code.
External consumer tests should exercise the checked public boundary.

## Alternatives considered

Public fields and unchecked constructors make invalid operations representable.
A marker token that any sibling can fabricate provides no protection. A public
unsafe escape hatch transfers an unneeded proof obligation to callers instead
of solving the ownership problem. Duplicated validation in compiler and executor
can drift; a shared checked semantic boundary plus execution-time authority
checks addresses different obligations without duplicating the compiler.

## Consequences

Some mechanical extractions must wait for a small ownership or validation change.
That cost is preferable to silently weakening a safety invariant. A separate
plan crate remains conditional under [RFC 0034][ownership]; type count alone
is not sufficient reason to extract it.

The claim concerns validated states and sanctioned execution paths, not an
operating-system sandbox against arbitrary code. Capability and adapter tests
remain necessary.

## Verification and revisit criteria

Inventory every constructor, deserializer, conversion, mutator, and generated
entry point before moving the types. Use API-negative or compile-fail tests for
forbidden direct construction/mutation and positive controls for legitimate
checked consumers. Runtime tests reject unresolved references, interpreter
mismatches, malformed persisted plans, stale authority, and invalid replacements.

Preserve relevant property tests and bounded proof harnesses with the owning
implementation. Exercise default and relevant feature configurations so a test
feature cannot accidentally export a bypass. A green application build alone
does not prove external callers cannot violate the contract.

Revisit the physical placement when concrete compiler/model consumers require
it, but retain the invariant as a non-negotiable acceptance condition.

## Outstanding decisions

Implementation review chooses checked-constructor versus colocated validation
kernel based on actual types and consumers. Persisted-plan feature RFCs own the
versioning/replay protocol; extraction must not invent that protocol by accident.

[semantics]: rfcs/0026-hexagonal-domain-hardening.md
[ownership]: rfcs/0034-forthcoming-component-ownership.md

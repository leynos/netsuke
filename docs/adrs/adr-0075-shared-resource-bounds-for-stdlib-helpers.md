# Architectural decision record (ADR) 0075: Share one resource-bounds module across stdlib helpers

## Status

Accepted. One standard-library module, `src/stdlib/resource_bounds/`, owns the
RFC 0006 table 3 ceilings, the checked cardinality and running-total
arithmetic, and the typed refusals that every bounded template helper uses. No
helper re-implements the arithmetic or invents a ceiling, and the module
produces no localized diagnostic.

## Date

2026-11-09.

## Context and problem statement

RFC 0006 §6.8 requires every parser, combinatorial helper, regular-expression
operation, and materialized output to reject unreasonable expansion *before*
allocating. Table 3 of that RFC fixes the defaults: 8 MiB of input, 128 levels
of nesting, 100000 output tuples for `product` and `combinations`, 10000 for
`permutations`, and 100000 matches for `regex_findall`. Eight child roadmap
tasks implement the consuming helpers, and each needs the same arithmetic: a
checked product over operand lengths, a checked binomial coefficient with a
symmetric reduction, a falling factorial, a linear scan that stops at the first
excess item, and a running byte total that never admits past its limit.

The roadmap step 6.1.3 asks for that arithmetic once, before the consumers
exist, rather than eight times afterwards. The risk it names is duplication
with drift: two helpers computing "the same" cardinality differently, or
choosing different ceiling constants, produce a standard library whose bounds
depend on which helper a template author happened to use.

Two constraints shape the answer. First, RFC 0006 assigns each refusal to a
consumer module's diagnostic code, so a shared `bounds` namespace would
contradict it; the shared module must not own the user-facing message. Second,
the arithmetic is the part whose correctness is hardest to audit by eye — an
off-by-one in the symmetric reduction, or a missed overflow, silently admits or
rejects the wrong request — so it should be the part with the strongest
evidence attached.

## Decision drivers

- One ceiling per resource kind, written once and read by every consumer.
- Refusals stay typed and inspectable, so a consumer can map one to its own
  module code without parsing text.
- Overflow must be reported distinctly from "measured and too large": the two
  call for different consumer responses, and RFC 0006 requires the diagnostic
  to name the computed cardinality where one exists.
- The arithmetic must be cheap enough to run before allocating, on the request
  path of an ordinary template evaluation.
- The correctness claim should be checkable by machine, because review by eye
  is exactly what fails on combinatorial overflow.
- The design must be reversible if a third pure numeric kernel appears.

## Requirements

### Functional requirements

- Admit or refuse `product`, `combinations`, and `permutations` requests by
  exact cardinality against the RFC 0006 ceilings.
- Admit or refuse a parse input by byte length.
- Track nesting depth, refusing the level past the ceiling.
- Track a running match count and a running output-byte total across a stream,
  refusing the first amount that would pass the limit and leaving the admitted
  total unchanged on refusal.
- Report overflow, ceiling excess, and identity/zero results as distinct,
  structured outcomes.

### Technical requirements

- No dependency on `minijinja` in the module, so the arithmetic is testable
  without a template environment and usable by any consumer.
- Refusals are `Copy` and carry the observed value and the ceiling.
- `RunningTotal` is neither `Copy` nor `Clone`, so a stream-wide total cannot
  be silently duplicated.
- The combinatorial kernels are small, pure functions that a formal model and
  a bounded model checker can both address.

## Options considered

### Option A: Per-group bounds in each helper module

Each consumer defines its own ceiling constant and its own arithmetic. This is
the smallest change per consumer and needs no shared module. It was rejected:
the same binomial reduction would exist in `collections`, the same running
total in `regex` and `path`, and the same product in `collections` and
`interchange`. A later correction to one copy leaves the others wrong, and the
duplication is precisely what roadmap 6.1.3 exists to prevent.

### Option B: One generic `BoundError` enum for every resource kind

A single error type with variants such as
`TooLarge { kind, observed, ceiling }` would unify the surface further. It was
rejected because it erases information each consumer needs: a combinatorial
request that overflowed carries no cardinality, a parse input carries a byte
length rather than a tuple count, and the precedence between overflow and
excess is part of the contract. Consumers would have to re-inspect `kind` to
recover what a precise type states directly, and the mapping to module
diagnostic codes becomes a match on a string-like tag rather than on a type.

### Option C: A shared module of precise, per-API refusals

One module owns the ceilings, the kernels, the depth and running-total state
machines, and one refusal type per resource family. Consumers map a refusal to
their own diagnostic code. This keeps the arithmetic single-sourced, keeps
refusals typed and `Copy`, and leaves localization where RFC 0006 puts it.

### Option D: A separate workspace crate

The bounds arithmetic could ship as its own small crate. It was rejected for
this step: the module has no independent consumer outside the standard library,
and a crate adds a workspace member, a version, and a publish surface for code
that eight in-tree consumers use. If a third pure numeric kernel later needs
the same treatment, splitting it out becomes the natural move; the shape of
this module does not obstruct that.

| Topic                        | A: per-group     | B: generic error | C: shared, precise | D: separate crate |
| ---------------------------- | ---------------- | ---------------- | ------------------ | ----------------- |
| Ceilings written once        | No               | Yes              | Yes                | Yes               |
| Consumer can map to its code | Yes, per copy    | Only after a tag | Yes, by type       | Yes, by type      |
| Overflow distinguishable     | Only if repeated | Yes, by tag      | Yes, by variant    | Yes, by variant   |
| Shared arithmetic            | No               | Yes              | Yes                | Yes               |
| Added workspace member       | No               | No               | No                 | Yes               |
| Reversible to a crate later  | n/a              | n/a              | Yes                | n/a               |

*Table 1: Comparison of the four placements for the bounds contract.*

## Decision outcome

Option C. `src/stdlib/resource_bounds/` holds:

- the six ceiling constants, re-exported from `src/stdlib`;
- `CardinalityRequest`, which computes an exact cardinality, reports the
  ceiling for its own kind, and admits or refuses before output exists;
- `AdmittedCount`, an approved tuple count that is explicitly not a memory
  budget;
- `CardinalityError`, `InputLengthExceeded`, `DepthExceeded`, and
  `StreamLimitExceeded`, each carrying the observed value and the ceiling;
- `NestingDepth` and `RunningTotal`, the two state machines;
- `admit_input`, `collect_matches`, and `try_collect_matches`.

The private arithmetic kernel in `kernel.rs` is the unit that the formal model
mirrors and the Kani harnesses drive. Overflow is reported before excess, a
ceiling is inclusive, and `product`'s identity and zero results are answered
before any ceiling comparison.

RFC 0006 table 3 gains an output-content row: materialized helper output is
bounded at 8 MiB and counted with checked arithmetic as it is produced. Six
accepted child RFCs already fix that value and the `output_too_large`
behaviour, no roadmap task owned it, and omitting it would have made the first
consumer write it locally for the rest to copy. The user approved the necessary
RFC amendment, and the row is added in this work rather than left to the pull
request.

## Goals and non-goals

- Goals:
  - Single-source the ceilings, arithmetic, and refusal types for the
    standard library.
  - Make every refusal inspectable without reading a message.
  - Give the combinatorial arithmetic a machine-checked correctness claim.
- Non-goals:
  - I/O byte limits, redirect limits, and the ADR-018 manifest budget, which
    have their own owners and different failure semantics.
  - Fluent keys, diagnostic codes, and `minijinja::Error` conversion, which
    belong to roadmap 6.1.4 and the consuming helper modules.
  - Exposing the ceilings as configuration. They are compile-time constants
    until a consumer needs otherwise.
  - A general-purpose numerics crate.

## Consequences

- Every bounded helper in `src/stdlib/` calls this module; a review of a new
  helper checks that it does, and that it passes its own input length and depth
  through the state machines rather than testing them inline.
- A ceiling change is a one-line change with one test update, not eight.
- The module carries two verification artefacts that must move with it: the
  Verus model under `verus/resource_bounds/` and the six Kani harnesses in
  `src/stdlib/resource_bounds/verification.rs`, each with a committed mutation
  patch under `docs/verification/mutations/`. A change to a kernel is expected
  to update all three.
- The module adds no user-visible behaviour, so no user guide changes.

## Known risks and limitations

- **The `serde_json` depth boundary differs by one.** `serde_json` 1.0.154
  starts `remaining_depth` at 128 and rejects when the decremented value
  reaches zero, so it admits 127 nested containers; this module admits 128 and
  rejects the 129th. `NestingDepth` guards only recursion Netsuke owns, and a
  consumer must check it before recursing. Reconciling the two is roadmap
  6.2.1's acceptance criterion ("128 levels pass and 129 fail identically for
  JSON and YAML"), and a consumer must never report a depth it did not measure.
- **The Verus model is a transcription, not the production code.** The proof
  establishes the arithmetic over the model's own definitions. Kani drives the
  production kernels, and the model's module comment states the gap; a silent
  divergence between the two is possible in principle and is what the Kani
  harnesses and their mutation patches exist to catch.
- **The output-content row is new to RFC 0006.** It was added under explicit
  user approval, and this ADR and table 3 must stay in step. If a later RFC
  revision renumbers or reshapes table 3, the constant and this record move
  together.
- **No user-facing message ships with the module.** A refusal that reached a
  template author today would render English developer text. No helper consumes
  the module yet, so no such path exists; the hand-off to roadmap 6.1.4 and the
  first consumer, 6.4.2, must land before one does.
- **Counts above 2^53 cannot be formatted exactly by Fluent.** In the deferred
  localization work, `FluentNumber` stores its value as `f64`, so a diagnostic
  that interpolates a computed cardinality above 2^53 can print an
  approximation. The ceiling comparison itself is exact; only the rendered
  number is at risk, and the consumer's message design must account for it.

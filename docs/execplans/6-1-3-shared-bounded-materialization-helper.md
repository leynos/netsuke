# Implement the shared bounded-materialization helper

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

Implementation must not begin until the user explicitly approves this plan.

## Purpose / big picture

Later standard-library helpers will parse untrusted text (`from_json`,
`from_yaml`, `b64decode`), build combinatorial build matrices (`product`,
`combinations`, `permutations`), recurse through nested mappings
(`combine(recursive=true)`), and collect regular-expression matches
(`regex_findall`). Each of them can be asked, by one short manifest line, to
produce more output than the machine can hold. RFC 0006 §6.8 requires every
such helper to reject unreasonable expansion before allocating, and to say
exactly why. This task delivers the one shared Rust helper that every later
helper uses to do that, so that ceilings, arithmetic, and refusal wording are
decided once rather than eight times.

After this change a Rust caller inside Netsuke can:

1. describe a combinatorial request by its operand lengths (never by its
   materialized output) and learn its exact cardinality, computed with checked
   arithmetic, or learn that the cardinality is too large to represent;
2. obtain an `AdmittedCount` token, which is the only way to learn a capacity
   the helper has approved, or receive a typed refusal naming both the computed
   cardinality and the ceiling;
3. check an input length, a nesting depth, and a growing match count against
   the RFC 0006 table 3 ceilings, receiving the same typed refusal shape.

No template filter, function, or test is registered by this task, so existing
manifests render exactly as before. The observable result is a public,
documented, tested, and partly proved Rust contract. A reviewer can see it
working by running the new unit, property, and behavioural suites, the Kani
harnesses, and the Verus proof, all of which are named in
`Validation and acceptance` with their expected output. For example, the
behavioural scenario "Over-ceiling combinatorial request is rejected naming
cardinality and ceiling" admits a product of operand lengths 316 and 317 and
observes the refusal
`combinatorial cardinality 100172 exceeds the ceiling of 100000 output tuples`
without any tuple being built.

This plan covers roadmap item 6.1.3 only. It targets a release after v0.1.0
final and does not widen the hardening release defined by issue #594.

## Constraints

- Follow RFC 0006 §6.8 and table 3. The ceilings are compile-time constants in
  this slice: input length 8 MiB (8,388,608 bytes), nesting depth 128, output
  tuples 100,000 for `product` and `combinations`, output tuples 10,000 for
  `permutations`, and match count 100,000. Do not make them configurable; RFC
  0006 §6.8 and open question 5 defer configuration until a consumer needs it.
- Cover exactly the four bound families named by roadmap 6.1.3: input length,
  nesting depth, checked combinatorial cardinality, and match counts. Alias
  expansion nodes belong to roadmap 6.2.2, and compiled-pattern size and the
  pattern cache belong to roadmap 6.5.1. Do not implement them here.
- Cardinality is computed from operand lengths only, with checked arithmetic,
  and compared with the ceiling before any output exists. The helper must not
  accept, iterate, or allocate the output it is bounding. Overflow is a typed
  refusal, never a wrap, a saturation presented as a count, or a panic (RFC
  0006 §6.6).
- Report an exact cardinality or the overflow refusal, never an estimate. When
  the cardinality is not representable in `u64`, report overflow; overflow
  takes precedence over the over-ceiling refusal (RFC 0015 §5.8). An empty
  operand yields cardinality zero even when other operands would overflow.
- The permutation ceiling is selected by the request kind inside the helper.
  A caller must not be able to check a permutation request against the product
  ceiling.
- The module is pure: no I/O, environment, clock, randomness, filesystem,
  subprocess, locale, or global state. It must not depend on `minijinja`; a
  consumer extracts lengths from template values and maps refusals into its own
  domain error.
- The refusal type is typed, `Copy`, and carries only integers and enumerated
  kinds, so constructing a refusal performs no heap allocation. Do not add
  Fluent keys, `netsuke::jinja::` codes, or an
  `impl From<BoundError> for minijinja::Error`. Roadmap 6.1.4 owns the
  diagnostic scaffolding, and the child RFCs assign each code to its consumer
  module (`collections`, `interchange`, `transform`, `pattern`), not to this
  helper.
- Add no dependency, including dev-dependencies. `proptest`, `rstest`,
  `rstest-bdd`, `googletest`, `pretty_assertions`, `insta`, and `thiserror` are
  already declared.
- `unsafe_code` is forbidden workspace-wide (`Cargo.toml`, `[workspace.lints]`).
  Do not plan or add a counting global allocator.
- Do not change the behaviour or registration of any existing helper,
  including `uniq`, `group_by`, and the canonical value module. Do not retrofit
  the depth bound into `src/stdlib/canonical_value/`; table 3 does not list it
  (see `Decision log`, D-07).
- Keep every Rust file under 400 lines. Every module starts with a `//!`
  comment; every function and method carries a `///` comment. Use `foo/mod.rs`
  layout for new multi-file modules; a test module with two or more files is
  `tests/mod.rs`.
- Verus remains optional, proof-kernel-only, and outside ordinary Cargo and
  continuous-integration gates. Do not use `assume`, `admit`, `external_body`,
  trusted bodies, or added axioms in the Verus proof.
- No source-API compatibility machinery: this is a new, pre-1.0 surface with no
  external consumer.

If satisfying the objective would violate a constraint, stop, record the
conflict in `Decision log`, and escalate.

## Tolerances (exception triggers)

- Scope: stop and escalate if implementation needs more than 30 changed files
  or more than 1,800 net added lines of Rust, excluding snapshots and the Verus
  proof file.
- Interface: stop if the public surface must differ from the names listed in
  `Interface and representation` below, or if any existing public signature
  must change.
- Dependencies: stop if any new crate, feature flag, or toolchain component is
  required.
- Semantics: stop if the exact-cardinality contract cannot be met for some
  admissible input without an estimate, or if any child RFC requirement turns
  out to conflict with the helper's refusal shape in a way this plan does not
  record.
- Kani: if a harness exceeds 20 minutes or the local 8 GiB memory cap, reduce
  its stated bound once and record the new bound; if the reduced harness is
  still intractable or its bound no longer reaches every outcome class, stop
  and escalate rather than deleting the harness silently.
- Verus: if three focused attempts fail to discharge a planned lemma, record
  the failing verification condition and the attempted decompositions in
  `Surprises & discoveries`, then escalate. Never weaken a lemma to make it
  pass, and never add an axiom.
- Iterations: if a gate still fails after three fix attempts for the same root
  cause, stop and escalate with the log path.
- Ambiguity: if a reviewer or a discovery shows that the diagnostic boundary
  chosen in D-03 is unacceptable, stop and present options rather than adding
  Fluent keys speculatively.

## Risks

- Risk: the binomial computation can overflow an intermediate product even when
  the final value fits, or can loop for a very long time on a huge `r`.
  Severity: high. Likelihood: medium. Mitigation: use the symmetric reduction
  `k = min(r, n - r)`, carry the running value in `u128`, and stop as soon as
  the running value exceeds `u64::MAX`. Because the running value is at most
  `u64::MAX` before each multiplication and the multiplier is at most
  `u64::MAX`, the intermediate product is below `2^128`. Because the running
  values are non-decreasing for `i < k <= n / 2`, an early stop is sound. Both
  claims are Verus lemmas (L5, L6) and Kani obligations (EP-V4).
- Risk: an empty operand placed after two huge operands is reported as overflow
  instead of cardinality zero. Severity: medium. Likelihood: high for a naive
  left fold. Mitigation: scan for a zero length before multiplying; witness
  `[2^40, 2^40, 0]` and a seeded-fault control in EP-V2.
- Risk: the child RFCs assume a shared 8 MiB output-content ceiling that RFC
  0006 table 3 does not list (RFC 0015 §5.8 calls it "the group's own rather
  than a table 3 row"). Severity: medium. Likelihood: high. Mitigation: the
  streaming primitive in this task is generic over its ceiling, so the first
  consumer that needs an output-bytes ceiling adds one `BoundKind` variant and
  one constant. This task does not invent the row; the gap is recorded for the
  RFC owner in `Surprises & discoveries`.
- Risk: `serde_json` enforces its own recursion limit, which admits 127 nested
  containers and rejects the 128th, while this helper admits 128 and rejects
  the 129th. Severity: medium. Likelihood: high once roadmap 6.2.1 lands.
  Mitigation: define the helper's depth precisely, document the off-by-one in
  the developers' guide, and hand the reconciliation to roadmap 6.2.1, which
  owns `from_json` (see D-06).
- Risk: Fluent formats numeric arguments through `f64`, so a cardinality above
  `2^53` would print rounded if a consumer passed it as a number. Severity:
  medium. Likelihood: medium once roadmap 6.1.4 localizes refusals. Mitigation:
  the developers' guide re-use policy tells consumers to pass counts to Fluent
  as decimal strings, and the English `Display` rendering in this task formats
  integers exactly.
- Risk: adding files under `src/stdlib/` changes the derived Kani proof scope
  (`tools/kani/proof-scope.toml`) and the workspace source scan
  (`tests/env_access_suppressions/`), which file-scoped gates do not notice.
  Severity: medium. Likelihood: high (roadmap 6.1.2 hit exactly this).
  Mitigation: run `make test-workflow-contracts` unconditionally at every
  milestone, and derive the scope file from source rather than editing it to
  match a failure message.
- Risk: the Verus model is a hand-written mirror, so it can drift from the
  production kernel. Severity: medium. Likelihood: medium. Mitigation: keep the
  production kernel in one small file whose loop structure matches the model
  step for step, drive the production code with Kani within explicit bounds,
  and check it against independent oracles with Proptest. Record the residual
  gap rather than claiming the proof covers production code.
- Risk: Kani bit-blasts `u128` multiplication and division, which may be slow.
  Severity: low. Likelihood: medium. Mitigation: divisors are concrete per
  unrolled iteration, the loop bound is small, and the Kani tolerance above
  governs any reduction.

## Progress

- [x] (2026-10-17 10:05Z) Renamed the branch to
  `6-1-3-shared-bounded-materialization-helper`; the remote branch does not
  exist yet and will be created when the plan is pushed.
- [x] (2026-10-17 10:40Z) Reconnaissance: stdlib layout, the 6.1.2 canonical
  value precedent, localization and diagnostic-code conventions, existing
  bounds, verification infrastructure, child RFC requirements, and ADR
  allocation.
- [x] (2026-10-17 11:20Z) Prior-art research: `serde_json` recursion limit,
  Fluent numeric formatting, and checked binomial computation.
- [x] (2026-10-17 12:30Z) Drafted this plan.
- [ ] Expert-panel design review and revision.
- [ ] Plan approved by the user.
- [ ] EP-M1: characterized toolchain and red tests.
- [ ] EP-M2: checked cardinality kernels and admission.
- [ ] EP-M3: input length, nesting depth, streaming count, and refusal
  rendering.
- [ ] EP-M4: Verus proof and Kani harnesses.
- [ ] EP-M5: documentation, ADR, roadmap, and final gates.

## Surprises & discoveries

- Observation: `serde_json` 1.0.154 starts `remaining_depth` at 128 and
  rejects when the decremented value reaches zero, so it admits 127 nested
  containers. Evidence: `src/de.rs` lines 63 and 1372–1382 in the registry copy
  of `serde_json-1.0.154`. Impact: this helper's "depth 128" and `serde_json`'s
  limit differ by one; see D-06 and the matching risk.
- Observation: Fluent's `FluentNumber` stores its value as `f64`, although it
  implements `From` for every integer width up to `u128`. Evidence: the
  `fluent_bundle::types::FluentNumber` API documentation. Impact: localized
  counts above `2^53` must be passed as strings.
- Observation: RFC 0006 table 3 has no output-content row, yet RFCs 0013,
  0014, 0015, 0016, 0018, and 0019 all rely on an 8 MiB output ceiling with an
  `output_too_large` code. Evidence: RFC 0015 §5.8 and the sibling §5.8
  sections. Impact: not in scope here; the streaming primitive is shaped so the
  row can be added without redesign.
- Observation: `unsafe_code = "forbid"` is set in `[workspace.lints]`, so a
  counting global allocator cannot be written in a test binary. Evidence:
  `Cargo.toml` line 266. Impact: "rejected before allocation" is established
  structurally and by unmaterializable witnesses (EP-V5), not by allocation
  counting.
- Observation: RFC 0015 §5.8 and roadmap 6.4.2 require cardinality and
  overflow to be distinct codes, with overflow taking precedence, and an exact
  count, never an estimate. Evidence: RFC 0015 lines 231–245 and 339–349.
  Impact: the refusal type has distinct `Exceeded` and `Overflow` variants.
- Observation: at planning time the highest ADR number allocated on any remote
  branch is 056 (`issue-592-…`), so the next free number is 057. Evidence:
  remote-branch sweep on 2026-10-17. Impact: the ADR in EP-M5 is provisionally
  ADR-057 and must be re-derived immediately before it is committed.

## Decision log

- D-01. Decision: one shared module, `src/stdlib/resource_bounds/`, owns every
  table 3 ceiling this task covers, the checked cardinality arithmetic, and one
  refusal type. Rationale: step 6.1 asks whether one shared contract can carry
  every later helper; five child RFCs reach these bounds, and duplicating the
  arithmetic would give five chances to get overflow precedence or zero
  absorption wrong. Date/Author: 2026-10-17, planning agent.
- D-02. Decision: the module is independent of `minijinja` and works on counts
  (`usize` lengths, `u64` cardinalities). Rationale: the bounded quantity is
  always a count that the consumer can read before materializing anything
  (`Value::len()`, `str::len()`, a recursion level, an iterator step). Keeping
  template values out makes the kernel pure, Kani-tractable, and mirrorable in
  Verus, and keeps the hexagonal dependency rule: the template adapter depends
  on the kernel, never the reverse. Date/Author: 2026-10-17, planning agent.
- D-03. Decision: refusals are a typed, `Copy`, unlocalized `BoundError` with
  an exact English `Display` rendering. This task adds no Fluent keys, no
  diagnostic codes, and no conversion into `minijinja::Error`. Rationale:
  roadmap 6.1.4 owns the domain-error and Fluent scaffolding, the child RFCs
  assign each code to its consumer module rather than to this helper, and the
  6.1.2 precedent took the same boundary. Adding a localized key now would mean
  editing 35 catalogues for a message no user can trigger yet. The roadmap
  success criterion "the diagnostic names both the computed cardinality and the
  ceiling" is met by the refusal carrying both values as typed fields and
  rendering both, pinned by snapshot tests. Alternative considered: add
  `stdlib.bounds.*` keys now. Rejected because it pre-empts 6.1.4's design and
  invents a module code no RFC names. Date/Author: 2026-10-17, planning agent.
- D-04. Decision: cardinalities are `u64`; intermediates for the binomial are
  `u128`. A cardinality above `u64::MAX` is reported as overflow. Rationale:
  every ceiling is far below `u64::MAX`, so a wider type would only move the
  overflow boundary. `u64` with a `u128` intermediate is the narrowest pair for
  which the binomial step provably cannot overflow (L5). Date/Author:
  2026-10-17, planning agent.
- D-05. Decision: `CardinalityRequest::admit` selects its own ceiling from the
  request kind; callers cannot pass a ceiling for combinatorial requests.
  Rationale: removes the defect class "permutations checked against the product
  ceiling". Date/Author: 2026-10-17, planning agent.
- D-06. Decision: nesting depth counts enclosing containers. The root value is
  at depth 0; entering a container moves to depth `d + 1`; depths 1 to 128 are
  admitted, and the attempt to enter depth 129 is refused with observed depth
  129. Rationale: this is the literal reading of "nesting depth 128". The
  one-level difference from `serde_json` is recorded for roadmap 6.2.1 to
  reconcile, because only that task knows whether `from_json` maps
  `serde_json`'s refusal or replaces it. Date/Author: 2026-10-17, planning
  agent.
- D-07. Decision: do not apply the depth bound to the canonical value
  snapshot in this task. Rationale: table 3 does not list canonical key
  derivation, and the 6.1.2 plan said only that 6.1.3 *owns* bounds, not that
  6.1.3 must apply them there. Applying them would change an existing public
  function's refusal set. Record it as a follow-up candidate in the developers'
  guide. Date/Author: 2026-10-17, planning agent.
- D-08. Decision: a streamed count that passes its ceiling uses a separate
  refusal variant, `StreamExceeded`, carrying only the ceiling. Rationale: a
  stream stops at the first item past the ceiling, so the true total is
  unknown; reporting `ceiling + 1` as "the count" would be an estimate, which
  RFC 0015 forbids for cardinalities and which would mislead here too.
  Date/Author: 2026-10-17, planning agent.
- D-09. Decision: `BoundKind` is exhaustive (no `#[non_exhaustive]`).
  Rationale: consumers map `(variant, kind)` to their own codes; an exhaustive
  enum forces every consumer to decide a code when a kind is added, instead of
  falling through a wildcard. All consumers are inside this crate. Date/Author:
  2026-10-17, planning agent.
- D-10. Decision: verification is layered. Verus proves the arithmetic
  contract over an abstract model for all inputs; Kani drives the production
  kernel within explicit bounds for panic freedom and admission soundness;
  Proptest compares the production kernel with independent oracles; named
  `rstest` witnesses pin boundaries and controls. Rationale: Verus cannot
  compile the production crate, and Kani cannot cover unbounded loops, so
  neither alone closes the gap. Date/Author: 2026-10-17, planning agent.
- D-11. Decision: a substantive ADR records the shared bounds contract,
  provisionally ADR-057. Rationale: the decision is hard to reverse once eight
  consumer tasks depend on it, and it answers step 6.1's central question.
  Date/Author: 2026-10-17, planning agent.

## Outcomes & retrospective

Not yet started. Complete this section at each milestone boundary and at
completion, comparing the result against `Purpose / big picture` and
reconciling every discovery with the artefacts in `Conformance basis`.

## Context and orientation

Netsuke is a build tool. It reads a YAML manifest called a `Netsukefile`,
expands embedded Jinja templates with the MiniJinja engine, and compiles the
result to a Ninja build file. Netsuke ships a template standard library, which
is a set of filters, functions, and tests available to every template. Its code
lives under `src/stdlib/`, in the main crate (package `netsuke-build`, library
name `netsuke`). `src/stdlib/mod.rs` declares private modules and re-exports a
public surface, for example
`pub use canonical_value::{CanonicalKey, CanonicalValueError, canonical_eq,
canonical_key, stable_deduplicate};`.
Registration of template helpers happens in `src/stdlib/register.rs` (which is
already at the 400-line cap) and `src/stdlib/register/query_helpers.rs`. This
task registers nothing, so neither file changes.

RFC 0006 (`docs/rfcs/0006-ansible-inspired-template-standard-library.md`) is
the parent design for the standard-library expansion. §6.8 and table 3 define
resource bounds; §6.6 says overflow is a diagnostic; §6.9 defines diagnostics,
which roadmap 6.1.4 implements. The child RFCs specify the consumers:

- RFC 0013 (`from_json`, `from_yaml`, `from_yaml_all`): input length 8 MiB and
  nesting depth 128, codes `interchange::length_exceeded` and
  `interchange::depth_exceeded`.
- RFC 0014 (`combine(recursive=true)`): nesting depth, code
  `transform::depth_exceeded`.
- RFC 0015 (`product`, `combinations`, `permutations`): exact checked
  cardinality, distinct `collections::cardinality_exceeded` and
  `collections::overflow` codes with overflow taking precedence, permutations
  under the lower ceiling.
- RFC 0016 (`regex_findall`): match count checked as it grows, code
  `pattern::match_limit`.
- RFC 0019 (`b64decode`): input length.

The term *materialization* in this plan means building a template value, such
as a list of tuples, in memory. A *ceiling* is the largest admitted value of a
bounded quantity. A *cardinality* is the number of output tuples a
combinatorial request would produce. *Admission* is the check that a quantity
is at most its ceiling.

The closest precedent is roadmap 6.1.2, the canonical value module in
`src/stdlib/canonical_value/` (`mod.rs`, `admission.rs`, `error.rs`, `node.rs`,
and a `tests/` subtree). It exposes a `thiserror` error enum without Fluent
keys, keeps its tests in `src/stdlib/canonical_value/tests/`, pins rendered
messages with `insta` snapshots under `src/snapshots/canonical_value/` via
`crate::snapshot_test_support::snapshot_settings`, has a behavioural feature at
`tests/features/canonical_value.feature` with steps in
`tests/bdd/steps/stdlib/canonical_value.rs`, and a standalone Verus proof at
`verus/canonical_value/main.rs`.

Existing repository bounds are unrelated to this task but worth not confusing
with it. `src/stdlib/config/types.rs` holds configurable byte limits for
`fetch`, file reads, and commands, and `src/manifest/budget/` holds the
manifest evaluation budget from ADR-018. Neither is a table 3 row.

Formal verification tools in this repository:

- Kani is a bounded model checker for Rust. Harnesses live beside the code in a
  `verification.rs` file gated by `#[cfg(kani)] mod verification;`, are listed
  in the Kani harness table in `docs/developers-guide.md`, and run through
  `make kani-full` (alias `make kani-ir`) and in the `kani-smoke` CI job. The
  derived module closure in `tools/kani/proof-scope.toml` must list every
  reachable source file; `tests/workflow_contracts/kani_proof_scope_test.py`
  recomputes it under `make test-workflow-contracts`.
- Verus is a deductive verifier for Rust-like code. Proofs live under
  `verus/<kernel>/main.rs`, outside Cargo, are run with
  `make verus VERUS_FLAGS='--proof-file verus/<kernel>/main.rs'`, and are not
  part of any routine gate. The pinned version is in `tools/verus/VERSION`.
  `docs/formal-verification-methods-in-netsuke.md` ("Optional Verus proof
  kernel") lists the accepted kernels.

Documentation and skills to consult while implementing:

- `docs/netsuke-design.md` §4.7 "Template Standard Library".
- `docs/terms-of-reference.md` goals G4 "Deterministic plans" and G8
  "Actionable failures".
- `docs/developers-guide.md` "Canonical value identity for template helpers"
  (the model for the new re-use policy section) and "Executable availability
  predicate" (the `ResolveError` boundary consumers will follow).
- `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/rust-doctest-dry-guide.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md`,
  `docs/ortho-config-users-guide.md` (no configuration surface is added; it is
  consulted only to confirm that the constants stay out of `OrthoConfig`), and
  `docs/rstest-bdd-users-guide.md`.
- `docs/formal-verification-methods-in-netsuke.md` and
  `docs/documentation-style-guide.md`.
- Skills: `execplans`, `rust-router`, `rust-types-and-apis`, `rust-errors`,
  `rust-unit-testing`, `proptest`, `kani`, `verus`, `rust-verification`,
  `hexagonal-architecture`, `codegraph-mcp`, `arch-decision-records`, and
  `en-gb-oxendict-style`.

## Conformance basis

- Baseline: `main` at `56c58027fcdfc4dd027834f9fc8bb2708898e8ed`.
- `docs/terms-of-reference.md` goals G4 (deterministic plans) and G8
  (actionable failures), and §2.3 "Trust and purity".
- `docs/roadmap.md` 6.1.3 (`EP-RQ-613`): cover input length, nesting depth,
  checked combinatorial cardinality, and match counts; cardinality rejected
  before allocation; the diagnostic names the computed cardinality and the
  ceiling.
- RFC 0006 §6.8 and table 3 (`EP-RQ-BOUNDS`): ceilings, check-before-allocate,
  constants rather than configuration.
- RFC 0006 §6.6 (`EP-RQ-OVERFLOW`): checked arithmetic; overflow is a
  diagnostic.
- RFC 0015 §§5.6 and 5.8 and roadmap 6.4.2 (`EP-RQ-TWO-CODES`): exact count or
  overflow, never an estimate; overflow precedence; distinct refusals; lower
  permutation ceiling; empty operand yields an empty result.
- RFC 0016 §5.8 (`EP-RQ-STREAM`): match count checked as it grows.
- RFC 0013 §5.8, RFC 0014 §5.8, RFC 0019 §5.8 (`EP-RQ-CONSUMERS`): length and
  depth consumers.
- RFC 0006 §6.9 and roadmap 6.1.4 (`EP-RQ-DIAG`): Fluent keys and codes are out
  of scope here (D-03).
- RFC 0006 §6.3 (`EP-RQ-DET`): purity and determinism.
- `docs/formal-verification-methods-in-netsuke.md` and the Verus policy in
  `docs/developers-guide.md`: this plan adds a third accepted Verus kernel.
- No existing ADR governs standard-library resource bounds. ADR-018 governs the
  separate manifest evaluation budget and is not changed.

Trace:

```plaintext
EP-RQ-613 + EP-RQ-BOUNDS + EP-RQ-OVERFLOW + EP-RQ-TWO-CODES
  -> EP-M2 checked kernels + AdmittedCount
  -> EP-V1, EP-V2, EP-V3, EP-V5 (tests::cardinality, tests::oracle, BDD)
  -> EP-M4 Verus (L1-L7) + Kani (EP-V4)
EP-RQ-613 + EP-RQ-STREAM + EP-RQ-CONSUMERS
  -> EP-M3 input length, NestingDepth, RunningCount
  -> EP-V6, EP-V7, EP-V8
EP-RQ-613 (diagnostic) + EP-RQ-DIAG
  -> EP-M3 BoundError rendering -> EP-V9 (insta snapshots, BDD)
EP-RQ-DET -> EP-V10 (module review, no minijinja or I/O imports)
All -> EP-M5 ADR-057, design §4.7, developers' guide re-use policy
```

## Interface and representation

The new module is `src/stdlib/resource_bounds/`. Its public names are
re-exported from `src/stdlib/mod.rs`, matching the canonical value precedent.
This is the complete public surface; anything else stays private to the module.

```rust
/// The quantity a ceiling limits, one per RFC 0006 table 3 row in scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundKind { InputBytes, NestingDepth, OutputTuples, PermutationTuples, MatchCount }

/// A table 3 ceiling. Constructible only through the associated constants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ceiling { kind: BoundKind, limit: u64 }

impl Ceiling {
    pub const INPUT_BYTES: Self;        // 8_388_608
    pub const NESTING_DEPTH: Self;      // 128
    pub const OUTPUT_TUPLES: Self;      // 100_000
    pub const PERMUTATION_TUPLES: Self; // 10_000
    pub const MATCH_COUNT: Self;        // 100_000
    pub const fn kind(self) -> BoundKind;
    pub const fn limit(self) -> u64;
}

/// Why a bounded operation was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum BoundError {
    /// An exactly computed quantity exceeds its ceiling.
    Exceeded { observed: u64, ceiling: Ceiling },
    /// A combinatorial cardinality exceeds `u64::MAX`; no count is reported.
    Overflow { ceiling: Ceiling },
    /// A streamed count passed its ceiling; the true total is unknown.
    StreamExceeded { ceiling: Ceiling },
}

/// A combinatorial request described by operand lengths, never by output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CardinalityRequest<'a> {
    Product { lengths: &'a [usize], repeat: u64 },
    Combinations { length: usize, r: usize },
    Permutations { length: usize, r: usize },
}

impl CardinalityRequest<'_> {
    /// Exact cardinality, or `None` when it exceeds `u64::MAX`.
    pub fn cardinality(&self) -> Option<u64>;
    /// The ceiling this request kind is checked against.
    pub const fn ceiling(&self) -> Ceiling;
    /// Admit the request or refuse it, before any output exists.
    pub fn admit(&self) -> Result<AdmittedCount, BoundError>;
}

/// A capacity the helper has approved; always at most its ceiling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedCount(usize);
impl AdmittedCount { pub const fn get(self) -> usize; }

/// Admit an input of `byte_length` bytes against `Ceiling::INPUT_BYTES`.
pub fn admit_input_length(byte_length: usize) -> Result<(), BoundError>;

/// The number of containers enclosing the current value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NestingDepth(u32);
impl NestingDepth {
    pub const fn root() -> Self;
    pub const fn level(self) -> u32;
    pub fn descend(self) -> Result<Self, BoundError>;
}

/// A count checked as it grows, for streams whose total is not known upfront.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RunningCount { ceiling: Ceiling, admitted: u64 }
impl RunningCount {
    pub const fn new(ceiling: Ceiling) -> Self;
    pub fn admit_one(&mut self) -> Result<(), BoundError>;
}

/// Collect at most `ceiling.limit()` items, pulling at most one more to detect excess.
pub fn collect_bounded<I: IntoIterator>(items: I, ceiling: Ceiling)
    -> Result<Vec<I::Item>, BoundError>;
```

The signatures above are a specification sketch: doc comments, attributes, and
exact `const` qualifiers are settled during implementation within the tolerance
on interface changes. `CardinalityRequest` borrows a slice of lengths and
returns owned values, so it is not a borrow-centric lookup API and needs no
`docs/polonius.md` entry; record that conclusion in the developers' guide
section.

The private arithmetic kernel lives in `src/stdlib/resource_bounds/kernel.rs`
and is the unit that the Verus model mirrors and Kani drives:

```rust
pub(super) fn checked_product(lengths: &[usize], repeat: u64) -> Option<u64>;
pub(super) fn checked_binomial(n: u64, r: u64) -> Option<u64>;
pub(super) fn checked_falling_factorial(n: u64, r: u64) -> Option<u64>;
pub(super) fn classify(cardinality: Option<u64>, ceiling: Ceiling)
    -> Result<u64, BoundError>;
```

The planned algorithms, stated so that the proof obligations are visible:

1. `checked_product`: if `repeat == 0`, return `Some(1)` (one empty tuple).
   Otherwise, if any length is zero, return `Some(0)`. Otherwise convert each
   length with `u64::try_from` (failure means overflow), fold with
   `checked_mul`, and on overflow return `None`. Raise the base to `repeat`: a
   base of 1 gives 1; a base of at least 2 with `repeat >= 64` gives `None`;
   otherwise use `checked_pow` with `repeat` converted to `u32`.
2. `checked_binomial`: if `r > n`, return `Some(0)`. Let `k = min(r, n - r)`.
   Carry `acc: u128 = 1`; for `i` in `0..k`, set
   `acc = acc * (n - i) / (i + 1)` and return `None` as soon as
   `acc > u64::MAX`. Return `acc` as `u64`.
3. `checked_falling_factorial`: if `r > n`, return `Some(0)`. Carry
   `acc: u64 = 1`; for `i` in `0..r`, set `acc = acc.checked_mul(n - i)?`.
4. `classify`: `None` gives `Overflow`; `Some(v)` with `v > limit` gives
   `Exceeded { observed: v, .. }`; otherwise `Ok(v)`.

The binomial and falling-factorial loops stop after at most about 65 iterations
for any input, because every step multiplies by at least 2 until the final
steps, so work is bounded independently of `n` and `r` (EP-V4).

Each `BoundKind` renders a fixed noun phrase, giving messages such as:

```plaintext
combinatorial cardinality 100172 exceeds the ceiling of 100000 output tuples
combinatorial cardinality exceeds 18446744073709551615 and cannot be represented; the ceiling is 100000 output tuples
input length 8388609 bytes exceeds the ceiling of 8388608 bytes
nesting depth 129 exceeds the ceiling of 128 levels
more than 100000 matches; the ceiling is 100000 matches
```

Exact wording is fixed by the snapshot tests in EP-M3 and may be polished
there; each message must name the observed value (where one exists) and the
ceiling.

Consumers will map refusals to their own module codes, for example
`collections` maps `Exceeded` with `OutputTuples` or `PermutationTuples` to
`netsuke::jinja::collections::cardinality_exceeded` and `Overflow` to
`netsuke::jinja::collections::overflow`. That mapping is written by the
consumer tasks, not here; the developers' guide records the table.

## Verification plan

### Behavioural contracts and invariants introduced

The work introduces these contracts, each traced to an obligation below:

1. Exactness: `cardinality()` equals the mathematical cardinality whenever that
   value is at most `u64::MAX`, and is `None` exactly when it exceeds
   `u64::MAX` (EP-V1).
2. Precedence and absorption: overflow precedes the ceiling check; a zero
   length or `r > n` gives zero regardless of other operands (EP-V2).
3. Admission soundness: `admit` returns `AdmittedCount(v)` exactly when the
   cardinality is at most the request kind's ceiling, and `v` equals it;
   `AdmittedCount::get()` never exceeds the ceiling (EP-V3).
4. Panic freedom and bounded work for every input (EP-V4).
5. Rejection before allocation (EP-V5).
6. Depth, stream, and input-length boundaries (EP-V6 to EP-V8).
7. Refusal rendering names the observed value and the ceiling (EP-V9).
8. Purity and determinism (EP-V10).

### Proof sketch for the arithmetic contract

This sketch states what must be shown, not how the solver will be driven.
Solver plumbing, auxiliary induction lemmas, trigger management, and
nonlinear-arithmetic helpers are deferred until implementation exposes the real
verification conditions. The lemmas below are hypotheses about the proof's
structure; implementation may merge, split, or drop them, and any change is
recorded in `Decision log`.

Abstract model, independent of the implementation. Over unbounded naturals:

- `spec_product(lens, repeat)` is the product of the sequence `lens` raised
  to `repeat`, with the empty product equal to 1.
- `spec_falling(n, r)` is 0 when `r > n`, and otherwise the product of
  `n - i` for `i` in `0..r`.
- `spec_binom(n, r)` is defined by Pascal's rule, deliberately not by the
  multiplicative formula the implementation uses: `binom(n, 0) = 1`;
  `binom(0, r + 1) = 0`; `binom(n + 1, r + 1) = binom(n, r) + binom(n, r + 1)`.
- `MAX` is `2^64 - 1`.

Representation relation: the executable kernels take `u64` (lengths arrive as
`usize` and are converted with `u64::try_from`; on every supported target
`usize` is at most 64 bits) and return `Option<u64>`. `Some(v)` represents the
natural `v`; `None` represents "greater than `MAX`".

Contract theorem (one per kernel). For all admissible inputs, the kernel returns
`Some(v)` if and only if the specification value equals `v` and `v <= MAX`,
and returns `None` if and only if the specification value exceeds `MAX`. For
`classify` composed with each kernel: the result is `Ok(v)` exactly when the
specification value is at most the limit, `Exceeded` exactly when it lies in
`(limit, MAX]`, and `Overflow` exactly when it exceeds `MAX`.

Preconditions: none beyond the types; every `u64` input and every slice length
is admissible. Postconditions: as in the contract theorem, plus termination.
Loop invariants: for the binomial loop, `acc == spec_binom(n, i)` and
`acc <= MAX` at the head of iteration `i`; for the falling-factorial loop,
`acc == spec_falling(n, i)` restricted to `i <= r <= n`; for the product fold,
`acc` equals the product of the prefix and all remaining factors are at least 1.

Lemma hypotheses:

1. L1, zero absorption: if some factor is 0 and `repeat >= 1`,
   `spec_product` is 0. This is why the zero scan precedes multiplication.
2. L2, monotone prefixes: if every factor is at least 1, every prefix product
   is at most the full product, so the first overflowing prefix proves the full
   product exceeds `MAX`.
3. L3, power bounds: for a base of at least 2 and `repeat >= 64`, the power
   exceeds `MAX`; for a base of 1 the power is 1; for `repeat == 0` it is 1.
4. L4, binomial symmetry and range: `binom(n, r) = binom(n, n - r)` for
   `r <= n`, and `binom(n, r) = 0` for `r > n`.
5. L5, exact multiplicative step:
   `binom(n, i) * (n - i) = binom(n, i + 1) * (i + 1)`, so the integer division
   in the loop is exact; and when `acc <= MAX` and `n - i <= MAX`,
   `acc * (n - i) < 2^128`.
6. L6, central monotonicity: `binom(n, i) <= binom(n, i + 1)` for
   `i < k <= n / 2`, so stopping when `acc > MAX` is sound.
7. L7, falling-factorial monotonicity: for `i < r <= n` every factor is at
   least 1, so the first overflowing prefix proves the result exceeds `MAX`.

Optional, if cheap once the above stands: a termination-measure lemma bounding
loop iterations by 65. If it is not cheap, bounded work stays a Kani and test
obligation (EP-V4).

These specifications pressure-tested the interface. They are the reason for:
`u64` cardinality with `u128` binomial intermediates (L5); the symmetric
reduction (L4 and L6); the zero scan before multiplication (L1); the separate
`Overflow` variant (the contract's third outcome); and a `repeat == 0` case
that is total rather than an error (L3; the consumer, not the kernel, rejects
`repeat < 1` per RFC 0006 §8.3).

Verus is warranted only here, because this arithmetic is contractual business
logic with an all-inputs guarantee that sampling cannot give. Depth, stream,
and input-length checks are single comparisons, and diagnostics are formatting;
they get conventional tests, not proof obligations.

### EP-V1 — exact cardinality

- Obligation: the contract theorem for `checked_product`,
  `checked_binomial`, and `checked_falling_factorial`.
- Method: Verus proof over the abstract model (all inputs), plus Proptest
  comparing the production kernel with independent oracles, plus named
  witnesses.
- Rationale: the guarantee is universal and the arithmetic is nonlinear, so
  only proof establishes it; the oracles connect the production code to the
  model's semantics on real executions.
- Domain: proof over all naturals representable as `u64` inputs. Proptest:
  product with 0 to 6 operands of length up to `2^20` and `repeat` up to 8,
  using a `u128` checked oracle where any `u128` overflow also means "above
  `MAX`"; binomial with `n <= 130` using an additive Pascal-triangle `u128`
  oracle (`binom(130, 65)` is below `2^128`); falling factorial with `n <= 40`
  using a `u128` checked product oracle.
- Artefacts: `verus/resource_bounds/main.rs`;
  `src/stdlib/resource_bounds/tests/oracle.rs` and
  `src/stdlib/resource_bounds/tests/properties.rs`;
  `src/stdlib/resource_bounds/tests/cardinality.rs` for witnesses.
- Evidence: `cargo nextest run --lib resource_bounds` fails before the kernels
  exist (red), then passes;
  `make verus VERUS_FLAGS='--proof-file verus/resource_bounds/main.rs'` reports
  every function verified and zero errors.
- Witnesses: `[4294967295, 4294967297]` gives exactly `u64::MAX`;
  `[4294967296, 4294967296]` overflows; `binom(67, 33) = 14226520737620288370`
  fits and `binom(68, 34)` overflows; `P(20, 20) = 2432902008176640000` fits and
  `P(21, 21)` overflows. Confirm each value against the oracle when
  implementing; do not trust these figures from the plan alone.
- Non-vacuity: the Proptest classifier records how many cases fall in each of
  the classes zero, admitted, exceeded, and overflow, and fails if any class is
  empty. Seeded faults that must be rejected: binomial without symmetric
  reduction computed in `u64` (fails at `binom(67, 33)`); the multiplicative
  step dividing before multiplying (fails exactness); a product that saturates
  instead of returning `None`. For Verus, check that the lemma antecedents are
  inhabited by exhibiting the witnesses above as concrete `proof` calls, and
  review the trusted base: only `vstd`, no `assume`, `admit`, or
  `external_body`.

### EP-V2 — overflow precedence and zero absorption

- Obligation: zero length or `r > n` gives `Some(0)` whatever the other
  operands are; overflow is reported only when the true value exceeds `MAX`.
- Method: named `rstest` witnesses plus L1 in Verus.
- Artefact: `src/stdlib/resource_bounds/tests/cardinality.rs`.
- Witnesses: product `[2^40, 2^40, 0]` gives 0; product `[1]` with repeat
  1,000,000 gives 1; product `[2]` with repeat 63 gives `2^63` (exceeded, not
  overflow); `[2]` with repeat 64 overflows; repeat 0 gives 1;
  `combinations(5, 6)` and `permutations(5, 6)` give 0.
- Non-vacuity: seeded fault "fold left without the zero scan" must fail the
  `[2^40, 2^40, 0]` witness; seeded fault "exponent loop without the base-of-1
  shortcut" must fail or time out on repeat 1,000,000.

### EP-V3 — admission soundness and ceiling selection

- Obligation: `admit` returns `AdmittedCount(v)` iff the cardinality `v` is at
  most `request.ceiling().limit()`; `Exceeded` carries the exact cardinality
  and the ceiling; permutations use `Ceiling::PERMUTATION_TUPLES` and the other
  kinds use `Ceiling::OUTPUT_TUPLES`.
- Method: `rstest` boundary table, Proptest over `classify` with arbitrary
  limits (in-crate, through the private kernel), and a Kani harness asserting
  `get() <= ceiling.limit()` for every admitted result within its bound.
- Artefacts: `src/stdlib/resource_bounds/tests/admission.rs`;
  `src/stdlib/resource_bounds/verification.rs`.
- Witnesses: product `[100000]` admitted; `[100001]` refused with observed
  100001 (RFC 0015's `range(100001) | product` example); `[316, 317]` refused
  with observed 100172; `combinations(100000, 99999)` admitted with 100000;
  `combinations(100001, 1)` refused; `permutations(10000, 1)` admitted;
  `permutations(10001, 1)` refused; `permutations(8, 8)` refused with observed
  40320 and ceiling 10000.
- Non-vacuity: seeded fault "permutations checked against `OUTPUT_TUPLES`" must
  fail the `permutations(8, 8)` witness; seeded fault `<` for `<=` must fail the
  `[100000]` witness.

### EP-V4 — production panic freedom and bounded work

- Obligation: no production kernel panics (arithmetic overflow, division by
  zero, failed conversion) for any input, and the binomial and
  falling-factorial loops execute at most 65 iterations.
- Method: Kani harnesses over the production kernel, plus a wall-clock-free
  unit witness that very large `n` and `r` return promptly.
- Rationale: Verus proves the model, not the production code; Kani checks the
  production code with overflow checks enabled.
- Domain and bounds: `checked_product` with up to 3 symbolic `usize` lengths
  and a symbolic `u64` `repeat`; `checked_binomial` and
  `checked_falling_factorial` with symbolic `u64` `n` and symbolic `r`
  restricted to at most 4 (unwind 6), plus a harness with symbolic `n` and `r`
  that asserts an iteration counter never exceeds 65 (unwind 67). If a harness
  breaches the Kani tolerance, reduce the bound once and record it.
- Artefact: `src/stdlib/resource_bounds/verification.rs`, wired by
  `#[cfg(kani)] mod verification;` in `src/stdlib/resource_bounds/mod.rs`, with
  rows added to the Kani harness table in `docs/developers-guide.md`.
- Evidence: the Kani run reports `VERIFICATION:- SUCCESSFUL` for each
  harness. Use the repository's Kani target (`make kani-full`), or the
  change-scoped form documented in ADR-039 if that is the established local
  entry point at implementation time.
- Non-vacuity: `kani::cover!` statements must show that the zero, admitted,
  exceeded, and overflow outcomes are each reachable within the bound; an
  unreachable cover fails the milestone. Seeded fault: computing the binomial
  step in `u64` must produce an overflow counterexample.
- Witness: `checked_binomial(u64::MAX, u64::MAX / 2)` and
  `checked_falling_factorial(10^18, 10^18)` return `None` in a unit test.

### EP-V5 — rejection before allocation

- Obligation: no output is allocated before admission, and a refusal performs
  no allocation.
- Method: structural argument backed by tests. `CardinalityRequest` contains
  only lengths, so the helper has no output to allocate; `BoundError` is `Copy`
  and holds only integers and enumerations, which a compile-time `Copy`
  assertion on `BoundError` (or an equivalent test) pins; `AdmittedCount` has a
  private field, so the only way to obtain an approved capacity is `admit`.
  Witnesses request cardinalities that cannot be materialized on any machine,
  such as `combinations(usize::MAX, usize::MAX / 2)`, and observe a prompt
  typed refusal; an implementation that allocated first would abort.
- Rationale: a counting global allocator would be stronger evidence, but it
  needs `unsafe` (forbidden) or a new dev-dependency (outside tolerance). The
  rejected option is recorded in `Decision log` if a reviewer asks for it.
- Artefact: `src/stdlib/resource_bounds/tests/admission.rs`.
- Non-vacuity: the unmaterializable witnesses would fail by aborting if the
  implementation built output before checking.

### EP-V6 — nesting depth boundary

- Obligation: from `NestingDepth::root()`, 128 successive `descend` calls
  succeed and the 129th returns
  `Exceeded { observed: 129, ceiling: Ceiling::NESTING_DEPTH }`; `level()`
  reports the depth reached.
- Method: `rstest` boundary cases at 0, 1, 127, 128, and 129, plus a Proptest
  property that `d` descents succeed iff `d <= 128`.
- Artefact: `src/stdlib/resource_bounds/tests/streams.rs`.
- Non-vacuity: seeded fault "compare with `<` instead of `<=`" must fail at 128.

### EP-V7 — streaming count

- Obligation: `collect_bounded(items, ceiling)` returns all items when there are
  at most `ceiling.limit()` of them, otherwise `StreamExceeded { ceiling }`; it
  pulls at most `ceiling.limit() + 1` items from the iterator in every case.
  `RunningCount::admit_one` succeeds exactly `limit` times, then refuses.
- Method: Proptest with a counting iterator wrapper over lengths around the
  ceiling (using a small in-crate ceiling through the private constructor that
  only tests can reach), plus a named witness collecting from an infinite
  iterator under `Ceiling::MATCH_COUNT`, which must refuse after pulling
  exactly 100,001 items.
- Artefact: `src/stdlib/resource_bounds/tests/streams.rs`.
- Non-vacuity: the infinite-iterator witness cannot pass for an implementation
  that collects first; seeded fault "pull one item fewer" must fail the
  exact-ceiling case.

### EP-V8 — input length boundary

- Obligation: `admit_input_length(n)` succeeds iff `n <= 8_388_608`; the refusal
  carries the exact length.
- Method: `rstest` cases at 0, 8,388,607, 8,388,608, 8,388,609, and
  `usize::MAX`.
- Artefact: `src/stdlib/resource_bounds/tests/streams.rs`.

### EP-V9 — refusal rendering

- Obligation: every `(variant, kind)` combination that can arise renders a
  message naming the observed value (when one exists) and the ceiling, with
  exact integer formatting up to `u64::MAX`.
- Method: `insta` snapshots, one per reachable combination, under
  `src/snapshots/resource_bounds/` via `snapshot_settings("resource_bounds")`,
  kept beside typed `googletest` assertions on the fields so a snapshot cannot
  pass for the wrong variant. Behavioural scenarios assert the cardinality and
  ceiling through the typed fields.
- Artefacts: `src/stdlib/resource_bounds/tests/diagnostics.rs`;
  `tests/features/resource_bounds.feature`;
  `tests/bdd/steps/stdlib/resource_bounds.rs`.
- Non-vacuity: a rendering that drops either number changes a snapshot.

### EP-V10 — purity and determinism

- Obligation: the module performs no I/O and reads no ambient state; repeated
  calls with the same arguments return equal results.
- Method: review of the module's `use` list (no `minijinja`, `std::env`,
  `std::fs`, `cap_std`, time, or randomness), plus a repeat-call Proptest
  assertion on the kernels. The existing environment-access contract in
  `tests/env_access_suppressions/` scans the new files automatically.
- Artefact: `src/stdlib/resource_bounds/tests/properties.rs`.

### External assumptions and residual gaps

- Axioms: Rust's documented semantics for `checked_mul`, `checked_pow`,
  `u64::try_from`, and `u128` arithmetic; `usize` is at most 64 bits on every
  supported target; Verus and `vstd` are sound for the constructs used; Kani's
  documented bounded semantics with overflow checks enabled.
- Residual gap: the Verus proof covers a hand-written model, not the compiled
  production kernel. The gap is narrowed by keeping
  `src/stdlib/resource_bounds/kernel.rs` structurally identical to the model,
  by Kani on the production kernel within bounds, and by Proptest against
  independent oracles. The proof file's module comment must state this gap.
- No end-to-end or command-line test is added: this task registers no template
  helper and changes no command-line behaviour. The consumer tasks (6.2.1,
  6.4.2, 6.5.3, and others) add end-to-end tests through manifests.

## Plan of work

### Stage A — understand and characterize (no production code)

Confirm the toolchain: `make check-build-tools`, the pinned Verus install
(`make install-verus` if `.verus/` is absent), and the Kani front end
(`make kani-check`). Record versions in `Progress`. Re-read RFC 0015 §5.8 and
RFC 0016 §5.8 to confirm nothing in them conflicts with the interface above.
Confirm that `snapshot_settings` accepts a new subject name without
configuration. Go/no-go: the toolchains work and no conflict is found;
otherwise record the issue and escalate.

### Stage B — red tests and behavioural specification

Create the module skeleton `src/stdlib/resource_bounds/mod.rs` declaring the
public types with their real signatures, and write the tests from EP-V1 to
EP-V9 against them. Kernel bodies return a deliberately wrong value (`None` or
`Ok(0)`), not `todo!()`, so the suite compiles and fails for the expected
reason. Observe the failures locally; do not commit a red tree, because every
commit must pass the gates. Write the feature file below. Go/no-go: each new
test fails for the documented reason, and existing suites still pass.

The behavioural specification, `tests/features/resource_bounds.feature`:

```gherkin
Feature: Bounded materialization for standard-library helpers

  Scenario: Over-ceiling combinatorial request is rejected naming cardinality and ceiling
    Given a product request over operand lengths 316 and 317
    When the request is admitted
    Then admission is refused with cardinality 100172 and ceiling 100000

  Scenario: An unrepresentable cardinality reports overflow rather than a count
    Given a product request over operand lengths 4294967296 and 4294967296
    When the request is admitted
    Then admission is refused as overflow against ceiling 100000

  Scenario: An empty operand admits an empty result even beside huge operands
    Given a product request over operand lengths 1099511627776, 1099511627776, and 0
    When the request is admitted
    Then the admitted count is 0

  Scenario: Permutations are checked against the lower ceiling
    Given a permutations request choosing 8 of 8 items
    When the request is admitted
    Then admission is refused with cardinality 40320 and ceiling 10000

  Scenario: Nesting one level past the ceiling is refused
    Given a nesting depth at the ceiling
    When the depth descends one more level
    Then descent is refused with depth 129 and ceiling 128

  Scenario: A match stream stops one item past the ceiling
    Given an unbounded stream of matches
    When the stream is collected under the match ceiling
    Then collection is refused after pulling 100001 items

  Scenario: An over-long input is refused before parsing
    Given an input of 8388609 bytes
    When its length is admitted
    Then admission is refused with length 8388609 and ceiling 8388608
```

Steps live in `tests/bdd/steps/stdlib/resource_bounds.rs` and are declared in
`tests/bdd/steps/stdlib/mod.rs`; they call the public API in process, as the
canonical value steps do. The world stores the last request, depth, or stream
counter and the last result.

### Stage C — implementation and verification together

Implement `kernel.rs` first, mirroring the algorithms in
`Interface and representation`, then `cardinality.rs` (`CardinalityRequest`,
`AdmittedCount`), `error.rs` (`BoundError`, `BoundKind` nouns), `ceiling.rs`
(`Ceiling` and constants), and `streams.rs` (`admit_input_length`,
`NestingDepth`, `RunningCount`, `collect_bounded`). Re-export the public names
from `src/stdlib/mod.rs`. Make the red tests pass one obligation at a time. In
parallel, write `verus/resource_bounds/main.rs` from the proof sketch and the
Kani harnesses in `verification.rs`. If the proof or a harness shows the
algorithm is wrong, fix the algorithm and its model together, then record the
change in `Decision log` and `Verification plan`.

### Stage D — refactor, documentation, and wider validation

Tidy the code without changing behaviour, run the seeded-fault controls listed
in each obligation (by temporarily editing, observing failure, and reverting
before committing; record each outcome in `Progress`), then write the
documentation in EP-M5 and run the full gates.

## Milestones and plateaus

Each milestone ends in a committed, gated, coherent repository state.

### EP-M1 — characterized toolchain and red tests

Assigned: Stage A and Stage B. End state: no production behaviour changes; the
plan's `Progress` records toolchain versions and observed red failures. Nothing
is committed except plan updates, because a red tree fails gates. Acceptance:
recorded failures for every new test, for the documented reason. Conformance
check: the interface still matches this plan. Recovery: discard the working
tree changes. Remaining gaps: everything else.

### EP-M2 — checked cardinality kernels and admission

Assigned: `EP-RQ-613` (cardinality), `EP-RQ-OVERFLOW`, `EP-RQ-TWO-CODES`. End
state: `kernel.rs`, `cardinality.rs`, `ceiling.rs`, `error.rs`, and the
re-exports exist; EP-V1 (tests), EP-V2, EP-V3, and EP-V5 pass. Update
`tools/kani/proof-scope.toml` by deriving it from source if
`make test-workflow-contracts` reports the new files. Acceptance:
`make check-fmt`, `make lint`, `make test`, `make doc-coverage`, and
`make test-workflow-contracts` pass. Conformance check: no `minijinja` import
in the module; refusal is `Copy`. Recovery: revert the milestone commit.
Remaining gaps: streams, depth, length, rendering, proofs, documentation.

### EP-M3 — input length, nesting depth, streaming count, and refusal rendering

Assigned: `EP-RQ-613` (length, depth, match counts), `EP-RQ-STREAM`,
`EP-RQ-CONSUMERS`, `EP-RQ-DIAG` boundary. End state: `streams.rs`, the
diagnostics snapshots, and the behavioural feature pass; EP-V6 to EP-V10 pass.
Acceptance: the same five gates as EP-M2. Conformance check: no Fluent key or
diagnostic code was added. Recovery: revert the milestone commit. Remaining
gaps: proofs and documentation.

### EP-M4 — Verus proof and Kani harnesses

Assigned: the proof sketch (L1 to L7) and EP-V4. End state:
`verus/resource_bounds/main.rs` verifies with no `assume`, `admit`, or
`external_body`; the Kani harnesses pass with every `kani::cover!` reached; the
seeded faults in EP-V1, EP-V2, and EP-V4 are observed and reverted. Acceptance:

```sh
make verus VERUS_FLAGS='--proof-file verus/resource_bounds/main.rs' 2>&1 \
  | tee /tmp/verus-netsuke-6-1-3-shared-bounded-materialization-helper.out
make kani-full 2>&1 | tee /tmp/kani-netsuke-6-1-3-shared-bounded-materialization-helper.out
```

The Verus run reports zero errors; the Kani run reports success for every new
harness. Then `make check-fmt`, `make lint`, `make test`, `make doc-coverage`,
and `make test-workflow-contracts` pass. Conformance check: the proof's module
comment states the model-versus-production gap; the trusted base is only
`vstd`. Recovery: revert the milestone commit; if a lemma cannot be proved
within tolerance, escalate rather than weaken it. Remaining gaps: documentation.

### EP-M5 — documentation, ADR, roadmap, and final gates

Assigned: documentation obligations from `AGENTS.md`. End state:

1. ADR, provisionally
   `docs/adr-057-shared-resource-bounds-for-stdlib-helpers.md`, using the
   documentation style guide's ADR format. Before creating it, re-derive the
   next free number by sweeping every remote branch (the command is in the
   repository memory and the 6.1.1 plan); take a number above the highest
   allocation. Index it in `docs/contents.md` and confirm every ADR index label
   matches its filename.
2. `docs/netsuke-design.md` §4.7: a "Resource bounds" subsection stating the
   ceilings, the exactness and precedence rules, and the ownership boundary,
   referencing the ADR.
3. `docs/developers-guide.md`: a "Resource bounds for template helpers"
   section after "Canonical value identity for template helpers", with the
   public surface, a **Re-use policy** (every bounded helper calls this module;
   no helper re-implements the arithmetic or invents a ceiling; consumers map
   `(variant, kind)` to their own codes; counts go to Fluent as strings), the
   consumer mapping table from the child RFCs, the depth definition and the
   `serde_json` difference, the canonical-value follow-up candidate, and the
   Polonius classification note. Add the Kani harness rows. Amend the Verus
   policy paragraph to list the third accepted kernel.
4. `docs/formal-verification-methods-in-netsuke.md` "Optional Verus proof
   kernel" and "Current repository state": add the resource-bounds kernel.
5. `docs/repository-layout.md`: add `src/stdlib/canonical_value/` (missing
   since 6.1.2) and `src/stdlib/resource_bounds/`.
6. `docs/users-guide.md` and `docs/stdlib-yaml-and-jinja-guide.md`: no change,
   because no user-visible behaviour changes; record that conclusion in
   `Outcomes & retrospective`.
7. `docs/roadmap.md`: mark 6.1.3 done (`- [x]`), keeping the text shape that
   `tests/rfc_stdlib_coverage/roadmap.rs` parses.

Acceptance: `make fmt`, then `make check-fmt`, `make lint`, `make test`,
`make doc-coverage`, `make markdownlint`, `make nixie`,
`make test-workflow-contracts`, and `make test-rfc-stdlib-coverage` pass.
Conformance check: every `Conformance basis` item is traced to evidence; set
`Status: COMPLETE` only after reconciling discoveries with upstream documents.

## Concrete steps

Run every command from the worktree root. Capture long output with `tee`:

```sh
make check-fmt 2>&1 | tee /tmp/check-fmt-netsuke-6-1-3-shared-bounded-materialization-helper.out
make lint 2>&1 | tee /tmp/lint-netsuke-6-1-3-shared-bounded-materialization-helper.out
make test 2>&1 | tee /tmp/test-netsuke-6-1-3-shared-bounded-materialization-helper.out
make doc-coverage 2>&1 | tee /tmp/doc-coverage-netsuke-6-1-3-shared-bounded-materialization-helper.out
make test-workflow-contracts 2>&1 | tee /tmp/workflow-contracts-netsuke-6-1-3-shared-bounded-materialization-helper.out
```

Run gates sequentially, never in parallel, and delegate full gate runs to the
`scrutineer` agent. A build waiting on `build-limits` admission is queued, not
hung. For the focused red-green loop:

```sh
cargo nextest run --lib resource_bounds 2>&1 | tee /tmp/focused-netsuke-6-1-3-shared-bounded-materialization-helper.out
cargo nextest run --test bdd_tests resource_bounds 2>&1 \
  | tee /tmp/bdd-netsuke-6-1-3-shared-bounded-materialization-helper.out
```

Commit after each green milestone with an imperative subject, for example
`Add checked cardinality kernels for stdlib bounds (6.1.3)`. Each step is
re-runnable: tests and proofs have no side effects beyond `target/` and `/tmp`
logs.

## Validation and acceptance

The feature is accepted when all of the following hold:

1. `cargo nextest run --lib resource_bounds` passes, and the Proptest class
   counts show every class reached.
2. `cargo nextest run --test bdd_tests resource_bounds` passes all seven
   scenarios; before the implementation they failed.
3. The `insta` snapshots render, among others:

   ```plaintext
   combinatorial cardinality 100172 exceeds the ceiling of 100000 output tuples
   ```

4. `make verus VERUS_FLAGS='--proof-file verus/resource_bounds/main.rs'`
   ends with a summary reporting zero errors.
5. `make kani-full` reports success for each new harness, with every cover
   reached.
6. `make check-fmt`, `make lint`, `make test`, `make doc-coverage`,
   `make markdownlint`, `make nixie`, `make test-workflow-contracts`, and
   `make test-rfc-stdlib-coverage` pass on the final commit.
7. `docs/roadmap.md` shows 6.1.3 as done, and the ADR is indexed.

## Idempotence and recovery

All steps are additive. To abandon a milestone, `git revert` its commit. The
seeded-fault controls are temporary edits that must be reverted before
committing; check `git diff` is empty for production files after each control.
If the ADR number collides with another branch after pushing, renumber this
branch's ADR in a separate commit, updating its heading, index entry, and every
reference, then re-check every ADR index label against its filename.

## Artefacts and notes

Planning-time evidence for the `serde_json` depth semantics:

```plaintext
serde_json-1.0.154/src/de.rs:63:            remaining_depth: 128,
serde_json-1.0.154/src/de.rs:1375:            $this.remaining_depth -= 1;
serde_json-1.0.154/src/de.rs:1376:            if $this.remaining_depth == 0 {
```

## Revision note

2026-10-17: initial draft, written from Wyvern reconnaissance of the stdlib,
the verification infrastructure, and the child RFCs. Pending expert-panel
review.

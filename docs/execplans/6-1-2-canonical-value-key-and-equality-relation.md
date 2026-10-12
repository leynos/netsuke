# Add canonical value equality for standard-library helpers

This ExecPlan is a living document. Keep `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` current as work proceeds.

Status: DRAFT

## Purpose / big picture

Standard-library helpers need one deterministic definition of value equality
before ordered set operations and duplicate detection can share a contract.
After this change, Rust callers can derive an opaque canonical key for a
supported MiniJinja value, compare two values by those keys, and deduplicate a
materialized slice while retaining references to first representatives in input
order. Unsupported values fail with a typed error naming their kind. No
template function or filter is registered in this step, so existing manifests
keep their current rendering behaviour; the observable result is a reusable,
tested Rust contract for later helpers.

This plan covers roadmap item 6.1.2 only. It targets a release after v0.1.0
final and does not widen the issue #594 hardening release. Implementation must
wait for explicit approval of this DRAFT.

## Constraints

- Follow RFC 0006 §6.7 and use the existing `serde_json_canonicalizer`
  dependency. Do not substitute MiniJinja's native equality or hash semantics.
- A key is opaque immutable RFC 8785 canonical JSON bytes. Object member order
  is insignificant; sequence order is significant. `none` is JSON `null`.
  Undefined, callables, `now()` objects, non-string mapping keys at any depth,
  and other unsupported representations are errors.
- Snapshot each admitted value once into a controlled JSON-compatible tree,
  retaining numeric representations, then serialize that tree with the existing
  canonicalizer. Do not use `serde_json::Value` if it rejects or coerces
  supported `i128`/`u128` values. The serializer must not re-read a custom
  object after admission.
- Enumerate each map once. Reject non-string keys with a deterministic typed
  category before visiting values; choose the rejected key kind
  deterministically if several are present, then sort accepted string keys
  before recursively snapshotting their values. This makes canonical bytes and
  the first reported invalid value independent of host-map iteration order.
- Admit scalar JSON values and finite, stable, acyclic Map/Seq representations,
  including well-behaved custom MiniJinja objects exposing those
  representations. Reject Plain, Iterable-only, Bytes, Undefined, Invalid,
  `now()`, and other unsupported kinds. Map/Seq iteration failure is a typed
  error. `ObjectRepr::Map`/`Seq` does not prove host iteration is finite,
  stable, pure, or acyclic; document this host precondition. MiniJinja exposes
  no public identity hook that would let this adapter promise cycle detection.
- Keep the pure canonical byte and index-deduplication kernels separate from
  MiniJinja traversal. The adapter invokes host-object iteration under the
  stated precondition and cannot police arbitrary host-code effects. Netsuke
  must add no I/O, process state, locale, clock, randomness, filesystem, or
  subprocess access.
- Deduplication receives a materialized `&[Value]`, validates every value
  (including later duplicates), returns references to first representatives,
  and produces no partial result if any value is invalid. Do not expose any
  membership-map iteration order. Do not consume arbitrary iterables or invent
  a size/depth limit; RFC 0006 §6.8 and roadmap 6.1.3 own those bounds.
- Expose only `CanonicalKey`, `CanonicalValueError`, `canonical_key`,
  `canonical_eq`, and `stable_deduplicate` from the `stdlib` module. Keep the
  implementation module, key bytes, and controlled snapshot tree private.
  `canonical_eq` must validate both operands before returning, including
  self-comparison; no MiniJinja-equality fast path is allowed.
- Keep the error typed and identify the rejected value kind. Do not add Fluent
  keys, diagnostic codes, localization conversion, or a generic error
  framework; roadmap 6.1.4 owns those concerns.
- Use the already declared `proptest`, `rstest-bdd`, `googletest`,
  `pretty_assertions`, and `insta` dependencies. Add no dependency.
- A Verus proof of the unbounded first-index kernel is required for feature
  acceptance. Invoke it explicitly through `make verus`; keep it outside
  ordinary Cargo, pull-request, and routine developer gates. Do not use
  `assume`, `admit`, trusted/external proof bodies, or equivalent shortcuts.
- Keep Rust files under 400 lines, document new modules/functions as required,
  and preserve the project's borrow-centric API style. No source-API
  compatibility layer is required for this new pre-1.0 surface.

## Tolerances (exception triggers)

- Stop if ordinary finite MiniJinja maps/sequences cannot be snapshotted
  without coercion, or if the supported domain must differ from RFC 0006 §6.7.
  Record concrete values and options before changing the contract.
- Stop if implementation requires arbitrary input bounds, arbitrary iterable
  consumption, cycle detection through private MiniJinja internals, a new
  crate, an unreviewed public surface, or a new trust boundary.
- Stop if a released external API requires compatibility machinery. The
  permitted public change is only the narrow additive foundation stated above.
- Stop if Verus cannot prove the inhabited abstract first-index kernel
  substantively, or proof completion depends on assuming the runtime bridge.
  Revise the proof plan rather than weakening the requirement to sampled tests.
- Preserve deterministic gate failures and logs; fix them without weakening
  gates or creating an isolated Cargo cache. For shared compiler-admission
  waits follow the repository's `build-limits` instructions.
- Escalate any material ambiguity about MiniJinja's supported value
  representations or current `uniq` semantics. Do not invent a template
  contract to resolve it.

## Risks

- Risk: MiniJinja containers and custom values use `Object`; Map/Seq
  representation does not establish finite, stable, pure, acyclic traversal.
  Severity: high. Likelihood: medium. Mitigation: traverse once, reject
  iteration failures and unsupported representations, document the host-value
  precondition, and test custom Map/Seq plus failing iterators. The adapter
  cannot detect cycles or constrain arbitrary host code without a separate
  identity/materialization contract.
- Risk: RFC 8785 number serialization uses IEEE-754 binary64 semantics and can
  deliberately collapse source values such as `2^53 + 1` and `2^53`. Severity:
  medium. Likelihood: high. Mitigation: assert boundary behaviour and
  signed-zero behaviour; do not claim integer-injective equality. Exercise
  `i64`, `u64`, `i128`, `u128`, and `f64` representations through the actual
  canonicalizer.
- Risk: recursive snapshots and retained unique keys can use stack and memory
  proportional to caller input. Severity: high. Likelihood: low in current
  manifest use. Mitigation: do not consume lazy iterables; state complexity as
  traversal/output bytes plus per-map key sorting and retained key bytes. Leave
  shared bounds to 6.1.3. No bound is claimed for arbitrary embedding inputs.
- Risk: Verus can prove the abstract key/index kernel without proving Rust or
  third-party serialization. Severity: high. Likelihood: medium. Mitigation:
  model source indices and abstract byte keys, review the trusted base, compare
  the implementation algorithm to the model, and use independent runtime tests
  for the MiniJinja/canonicalizer bridge.
- Risk: `docs/developers-guide.md` currently limits first Verus work to cycle
  canonicalization. Severity: medium. Likelihood: high. Mitigation: the plan's
  approved proof milestone explicitly amends that policy to add this narrow
  optional kernel while preserving its exclusion from default workflows.
- Risk: changing current `uniq` behaviour would alter existing manifests.
  Severity: high. Likelihood: medium. Mitigation: preserve `uniq` and
  `group_by` registrations and behaviour; a later consumer migration needs its
  own accepted contract.

## Progress

- [x] (2026-10-13) Inspect roadmap 6.1.2, RFC 0006 §6.7, dependent RFC
  constraints, `uniq`, the Verus Make targets, and required design/testing
  references.
- [x] (2026-10-13) Wyvern reconnaissance and three paired six-lens reviews
  examined API boundaries, numeric semantics, proof non-vacuity, and delivery
  gates. Findings are incorporated in this DRAFT; all six expert lenses
  completed final readback with no remaining blockers.
- [x] (2026-10-13) Draft proof sketch, runtime bridge, test strategy,
      milestones,
  tolerances, and conformance trace. No implementation has begun.
- [x] (2026-10-13) Planning-document gates passed: `make fmt`,
  `make check-fmt`, `make markdownlint` (including spelling), and `make nixie`.
  No runtime code changed; Rust lint/test/doc-coverage and the future Verus
  proof have not been run for this documentation-only planning change.
- [ ] Obtain explicit approval before implementation.
- [ ] Characterize MiniJinja values and confirm the approved API against actual
  constructors, iterator failures, numeric serializers, and pinned Verus tools.
- [ ] Implement canonical keys/equality and stable deduplication; discharge
  property, BDD, runtime-boundary, and Verus obligations.
- [ ] Update design and formal-verification guidance, run sequential gates, and
  mark roadmap 6.1.2 done only after all feature acceptance evidence passes.

## Surprises & discoveries

- Observation: current `uniq` uses `IndexSet<Value>` and follows MiniJinja's
  equality; RFC 0006 defines canonical JSON equality for later set helpers.
  Evidence: `src/stdlib/collections.rs`, `uniq_filter`. Impact: 6.1.2 adds a
  reusable foundation without migrating `uniq`.
- Observation: JCS uses IEEE-754 number semantics rather than source-integer
  identity. Evidence: RFC 8785 §§3.1, 3.2.2.3, and Appendix B, checked via
  Firecrawl and the [RFC text](https://www.rfc-editor.org/rfc/rfc8785/). Impact:
  `2^53 + 1` can share a key with `2^53`; negative and positive zero serialize
  alike. No arbitrary-precision integer-injectivity claim is valid.
- Observation: the repo already delegates optional Verus execution through
  `make verus`, but has no canonical-value proof file or `tools/verus/` pin
  files. Evidence: `Makefile`,
  `docs/formal-verification-methods-in-netsuke.md`, and the `tools/verus/`
  inventory. The pinned `prover-tools` invocation accepts `--repo-root` and
  `--proof-file`, verified by the planning agent. Impact: implementation must
  choose and record the upstream Verus release and add `tools/verus/VERSION`
  plus `tools/verus/SHA256SUMS` before installation.
- Observation: MiniJinja's public object representation does not establish
  finiteness, purity, or cycle freedom. Evidence: MiniJinja `ObjectRepr`,
  `try_iter`, and `try_iter_pairs` APIs. Impact: explicitly document the host
  precondition; do not promise cycle detection or adapter-enforced bounds.

## Decision log

- Decision: expose the narrow API `CanonicalKey`, `CanonicalValueError`,
  `canonical_key(&Value) -> Result<CanonicalKey, CanonicalValueError>`,
  `canonical_eq(&Value, &Value) -> Result<bool, CanonicalValueError>`, and
  `stable_deduplicate(&[Value]) -> Result<Vec<&Value>, CanonicalValueError>`
  from `src/stdlib`; keep the implementation module private. `CanonicalKey`
  implements `Eq`, `PartialEq`, and `Hash` over its private bytes and has no
  unchecked constructor. `CanonicalValueError` is a semantic `thiserror` enum
  covering unsupported kind, non-string map key, invalid traversal, non-finite
  number, and canonicalizer failure; structured access identifies the rejected
  kind. Validate the left operand first, then the right; an error terminates
  the operation, but no successful false/true result bypasses either operand.
  Deduplication reports the earliest invalid input index without partial
  output. Rationale: later helpers and embedding callers need a common pure
  contract; first representatives remain original values without cloning or
  reconstruction. `canonical_eq` must derive both keys before comparing.
  Date/Author: 2026-10-13, planning draft.
- Decision: snapshot accepted MiniJinja values exactly once into a private
  controlled tree with explicit Null, Bool, String, I128, U128, F64, Seq, and
  Map variants (or a semantically equivalent representation). Pass this tree
  directly to the canonicalizer. Enumerate maps once, reject non-string keys
  deterministically, and sort string keys before visiting values. Rationale:
  prevents coercive serializer conversions and avoids re-reading
  mutable/dynamic objects. Reject non-finite floats and unsupported kinds
  before serialization. Date/Author: 2026-10-13, planning draft.
- Decision: accept finite, stable, acyclic Map/Seq custom objects under a
  documented host-value precondition; reject Plain, Iterable-only, Bytes,
  Undefined, Invalid, `now()`, nested non-string map keys, and failed
  iterators. Do not claim the adapter detects cycles or arbitrary host effects.
  Rationale: MiniJinja uses objects for containers and exposes no public object
  identity needed to distinguish or cycle-check every implementation.
  Date/Author: 2026-10-13, planning draft.
- Decision: validate all input items and return no partial deduplication result
  if one is unsupported; preserve `uniq` and all template registrations.
  Rationale: deterministic error behaviour and no silent fast-path around
  invalid data; the roadmap requests a foundation, not an implicit filter
  migration. Date/Author: 2026-10-13, planning draft.
- Decision: require Verus for the unbounded first-index kernel, but keep it out
  of ordinary Cargo, CI, and routine gates. Extend both formal-verification
  guidance and the developers' guide's cycle-only Verus statement. Rationale:
  proof is part of this task's feature acceptance, while the existing project
  policy keeps Verus opt-in for normal workflows. Date/Author: 2026-10-13,
  planning draft.

## Outcomes & retrospective

Draft outcome: this plan defines the canonical domain, concrete Rust API,
first-wins semantics, host-object precondition, numerical limits, and
proof/test responsibilities. It deliberately leaves `uniq` unchanged. No
feature result is claimed yet. Wyvern reconnaissance and the
Pandalump/Wafflecat, Telefono/Doggylump, and Buzzy Bee/Dinolump review pairs
are complete. Review changed the initial key-only proof into source-index
refinement, made host-object preconditions explicit, fixed one-snapshot
admission and numeric semantics, and required both formal-policy documents to
be updated. At implementation closeout, record actual supported kinds, proof
boundary, test evidence, documentation changes, and residual resource risks;
only then mark roadmap item 6.1.2 complete.

## Context and orientation

Netsuke expands YAML manifests with MiniJinja and compiles them to a
deterministic Ninja graph. Standard-library implementations live under
`src/stdlib`. `src/stdlib/collections.rs` registers `uniq`, `flatten`,
`compact`, and `group_by`; `uniq` currently uses MiniJinja equality. The new
private module belongs under `src/stdlib`, with only the stated foundation
re-exported.

RFC 0006 §6.7 defines equality as byte equality of RFC 8785 canonical JSON
produced by `serde_json_canonicalizer`. RFC 0006 §6.8 defines shared input and
materialization bounds, which roadmap 6.1.3 implements. RFC 0006 §6.9 defines
typed diagnostics, with Fluent mapping in 6.1.4. RFCs 0013 and 0014 constrain
structured data and mapping conversions; in particular a lossy conversion
cannot establish source equality.

The repository's Makefile delegates Verus installation and execution through
the pinned `rust-prover-tools` source. There is no existing Verus model for
canonical values, so this plan adds a standalone model under `verus/`, outside
Cargo. The model uses abstract canonical byte keys and source indices; it does
not prove MiniJinja or the third-party canonicalizer.

## Conformance basis

- Baseline: repository state `b14bde201810a945b17ad484af3bd46749de2dad`.
- `docs/terms-of-reference.md`, Draft v0.2, last revised 2026-09-25:
  deterministic build graph, purity, and trust boundaries.
- `docs/netsuke-design.md`, “Generic collection filters”: current filter
  placement and `uniq` behaviour, which this step preserves.
- `docs/roadmap.md` 6.1.2 (`EP-RQ-612`): existing dependency, unsupported-kind
  errors, order-preserving helper, and property criteria.
- RFC 0006 §6.7 (`EP-RQ-JCS`): canonical equality, key sorting, sequence order,
  typed rejection, and first-wins deduplication. RFC 0006 §6.8 (`EP-RQ-BOUNDS`)
  leaves shared bounds to 6.1.3; §6.9 (`EP-RQ-DIAGNOSTICS`) leaves localized
  codes to 6.1.4.
- RFC 0006 §11.3 and `src/stdlib/collections.rs`: preserve `uniq` and
  `group_by` behaviour and registrations.
- RFCs 0013 §§5.5, 5.7, 8.1 and 0014 §5.7: serializer coercion, key domains,
  and numeric round-trip constraints.
- RFC 8785 §§3.1, 3.2.2.3, and Appendix B, plus the existing
  [`serde_json_canonicalizer` 0.3.2 API](https://docs.rs/serde_json_canonicalizer/0.3.2/serde_json_canonicalizer/):
  external canonicalization contract. Third-party internals are axioms;
  Netsuke-owned adapter behaviour is tested against the actual interface.
- `docs/formal-verification-methods-in-netsuke.md` and
  `docs/developers-guide.md` Verus policy: this approved work explicitly adds a
  narrow optional kernel without making it an ordinary gate.
- Required references: `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/rust-doctest-dry-guide.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md`,
  `docs/ortho-config-users-guide.md`, and `docs/rstest-bdd-users-guide.md`.
  These govern fixtures, doctests, injected dependencies, diagnostic
  boundaries, and BDD scenarios.
- Skills: `execplans`, `codegraph-mcp`, `hexagonal-architecture`, `rust-router`,
  `rust-verification`, `proptest`, `verus`, `firecrawl-mcp`, and
  `logisphere-experts`.

Trace:

```plaintext
RFC0006-6.7 + ROADMAP-6.1.2
  -> EP-M1 domain/API characterization
  -> EP-M2 canonical key and equality
  -> EP-M3 stable dedup + tests + Verus proof
  -> EP-M4 design/proof documentation + sequential gates
```

No change to the Terms of Reference or a new ADR is currently required. If the
implementation changes the published standard-library contract or establishes a
durable proof-driven architecture decision, update the design document and add
an ADR only when the decision warrants a separate record.

## Verification plan

The abstract model accepts a finite sequence of byte keys `K = [k0, ..., kn]`.
Define output as the increasing source-index sequence `R` where index `i` is
retained exactly when no earlier key `K[j]`, `j < i`, equals `K[i]`. The Rust
helper then returns `&input[R[t]]`. This specification does not claim JCS keys
distinguish source numbers that JCS serializes to the same IEEE-754 value.

### EP-V1 — equality is an equivalence relation

Statement: for admitted values, key equality is reflexive, symmetric, and
transitive. Method: Proptest over recursively generated values plus named
`rstest` cases. Domain: null, booleans, strings/Unicode, finite numbers, nested
arrays/maps, insertion-order permutations, signed zero, and JCS numeric
boundaries. Artefact: tests inside `src/stdlib/canonical_value/mod.rs`.

Evidence: `cargo nextest run --lib canonical_value` fails before implementation
for absent behaviour, passes after it, then `make test` passes. The generator
constructs valid values directly rather than filtering arbitrary values; report
classification counts for every material class. Include equivalent triples
constructed independently and unlike values. A negative control that retains
source map insertion order in the key must fail on reordered-map fixtures;
another that inverts equality must fail reflexivity. A reversed operand
comparison alone is not a sufficient fault because equality remains symmetric.

### EP-V2 — admission and canonical bytes match the contract

Statement: reordered object members produce equal keys, sequence order matters,
unsupported nested keys/kinds fail before coercion, and emitted bytes match the
canonicalizer on selected JCS fixtures. Method: finite `rstest` cases and real
MiniJinja/canonicalizer integration tests. Artefact:
`src/stdlib/canonical_value/mod.rs` tests.

Domain: nested integer/boolean map keys, Undefined, callable, `now()` object,
Plain/Iterable/Bytes/Invalid values, finite stable acyclic custom Map/Seq
objects, failing Map/Seq iterators, non-finite floats, `-0.0`/`0.0`,
`2^53 + 1`, minimum/maximum `i64`, `u64`, `i128`, `u128`, exponent boundaries,
and RFC 8785 Appendix B cases. The snapshot tree retains explicit Null, Bool,
String, I128, U128, F64, Seq, and Map representations, then serializes that
snapshot once.

Evidence: direct assertions check exact keys/error kinds. Require `2^53 + 1` and
`2^53` to share a JCS key, and negative/positive zero to share a key. Exercise
extreme integers through the direct canonicalizer interface to confirm accepted
range. A nested-key validation omission must fail on a non-string nested key; a
coercive integer-key conversion must fail on a collision fixture; re-reading a
mutating custom object must fail a one-snapshot test. No snapshot file is
warranted unless genuinely multivariant diagnostic rendering is added. No
CLI/end-to-end test is warranted because this step adds no user-facing template
operation.

Non-vacuity requires every rejected case to reach the adapter and every
admitted class to reach the actual canonicalizer. Hostile or cyclic custom
objects are outside the enforceable contract; document the finite, stable,
acyclic host-value precondition rather than claiming those inputs are detected.

### EP-V3 — stable deduplication returns first representatives

Statement: output references point to increasing least-occurrence indices;
every distinct key appears once; all input keys are represented; and later
equal values never replace their first representative. Method: Proptest against
an independent quadratic first-index oracle, plus a standalone Verus proof over
the abstract kernel and a runtime correspondence test. Artefacts:
`src/stdlib/canonical_value/mod.rs` and `verus/canonical_value/main.rs`.

The proof model processes `(index, key)` pairs left to right, maintains
abstract seen keys and retained source indices, and proves that the final index
sequence equals the specification's least-index sequence. Prove input-range,
strict index ordering, key uniqueness, coverage, and minimality. Do not claim
the proof establishes the MiniJinja-to-key conversion; the implementation must
mirror the model's append-or-skip algorithm, while runtime tests compare
returned reference identity with the independent source-index oracle.

At processed index `i`, establish `seen = {K[j] | 0 <= j < i}` and that
retained indices are exactly the least occurrences in that prefix, in
increasing order. Prove that each membership-test append/skip branch preserves
this representation relation and refines the independent least-index
specification. The runtime loop must use those same state transitions; review
that correspondence and test the actual Rust implementation. This is an
abstract algorithm refinement proof, not a claim that Verus verifies Rust's
`HashSet` internals or the entire production adapter.

Domain and witnesses: empty, singleton, all-distinct, all-equal, repeated
values at head/middle/tail, interleaved duplicates, reordered map duplicates,
order-distinct sequences, and generated mixed values. Generator classification
must show each shape is reached without excessive filtering. A negative control
that retains the last duplicate or emits membership-set iteration order must
fail. The runtime oracle computes each key then scans prior keys quadratically;
pointer identity verifies retained values are the original first input items.

Before running the proof, add the chosen upstream version to
`tools/verus/VERSION` and checksum data to `tools/verus/SHA256SUMS` according to
`docs/formal-verification-methods-in-netsuke.md`. Verify the pinned verifier
and its checksum, then run `make install-verus` if absent and:

```sh
make verus VERUS_FLAGS='--repo-root . --proof-file verus/canonical_value/main.rs'
```

The pinned `prover-tools` command was checked during planning and accepts these
flags. Proof acceptance requires satisfiable witnesses, reviewed trusted
assumptions, and no `assume`, `admit`, external body, contradiction, or theorem
equivalent to an assumption. Verus is required for feature completion, but
remains outside the ordinary gates.

### EP-V4 — public purity and ordering boundary

Statement: no public output order depends on membership storage; no Netsuke
environmental inputs enter the canonical kernels. Method: API review and repeat
tests over different map insertion orders. Artefacts: public exports in
`src/stdlib/mod.rs` and tests in `src/stdlib/canonical_value/mod.rs`.

Evidence: repeated calls return identical keys and first-wins references. Use a
witness with at least two duplicate classes whose first-occurrence order
differs from lexical/hash order. Complexity is proportional to snapshot
traversal and serialized bytes, key sorting per map, and retained unique key
bytes; no unbounded embedding-input resource guarantee is claimed.

### External assumptions and proof limits

The proof assumes the mathematical semantics of byte equality and its own
finite-sequence model. The runtime adapter relies on documented MiniJinja value
and object interfaces, the caller's finite/stable/acyclic host-object
precondition, and the documented RFC 8785 behaviour of
`serde_json_canonicalizer`. Do not attempt to prove third-party internals.
Proptest and direct fixtures verify the repository-owned snapshot/serialization
boundary. Kani is not warranted: the core contract applies to unbounded
sequence lengths rather than an interesting finite state bound.

## Plan of work

### Stage A — characterize the runtime domain

Inspect MiniJinja scalar/object representations and iteration APIs, and the
locked canonicalizer's serializer interface. Create an evidence-backed table of
admitted/rejected cases. Confirm that one snapshot can preserve numeric scalar
representations and avoid re-reading objects. Record that arbitrary custom
objects are admitted only under the finite, stable, acyclic host precondition.
Add `tools/verus/VERSION` and `tools/verus/SHA256SUMS` for a selected upstream
release using the existing pinned `prover-tools` path; verify its checksum and
the documented Make flags before proof work.

Go/no-go: proceed only if ordinary finite maps/sequences and JSON scalars have
a deterministic non-coercive route to JCS, unsupported values fail before
coercion, and the abstract proof boundary remains independent of MiniJinja.

### Stage B — write failing tests and BDD contract

Add unit/property cases in `src/stdlib/canonical_value/mod.rs`. Add
`tests/features/canonical_value.feature`, bindings in
`tests/bdd/steps/stdlib/canonical_value.rs`, and wire the steps/scenarios
through `tests/bdd/steps/stdlib/mod.rs`. Add isolated scenario state under
`tests/bdd/fixtures/` and connect it to `TestWorld` in
`tests/bdd/fixtures/mod.rs`; use no process-global mutable state.
`tests/bdd_tests.rs` already discovers `tests/features`, so retain that
entrypoint without redundant scenario registration. Describe the Rust boundary;
do not imply a Jinja filter exists. Scenarios:

```gherkin
Feature: Canonical value identity

  Scenario: Reordered maps have the same canonical key
    Given two string-keyed maps with identical members in different orders
    When their canonical keys are derived
    Then the keys are equal

  Scenario: Deduplication keeps the first equal value
    Given a materialized sequence with repeated canonical values
    When stable deduplication is applied
    Then the original first values appear once in first-appearance order

  Scenario: Unsupported values fail before coercion
    Given a value outside the canonical JSON domain
    When its canonical key is requested
    Then a typed error identifies its value kind
```

Run the red stage with `cargo nextest run --lib canonical_value` and
`cargo nextest run --test bdd_tests canonical_value`. Failure must be due to
missing/incorrect behaviour, not malformed feature wiring. Verify that each
focused filter selects nonzero tests; use the generated scenario names reported
by Nextest if its naming convention differs. Use `rstest` fixtures,
`googletest` matchers for predicates and typed errors, and `pretty_assertions`
for expected sequences/bytes. Keep canonical admission tests separate from the
index-selection oracle so a common conversion fault cannot bless both.

### Stage C — implement key, equality, stable dedup, and proof together

Add private `src/stdlib/canonical_value/mod.rs` with child modules
`admission.rs`, `error.rs`, and `tests/mod.rs` as needed to keep every Rust
file under 400 lines. The admission module owns MiniJinja traversal; the
private snapshot tree, canonical key, and index selection have no MiniJinja
dependency. Document permitted callers and composition in
`docs/netsuke-design.md` and `docs/developers-guide.md` before extracting
shared helpers. Recheck the repository for equivalent abstractions;
`src/hasher.rs::ActionHasher::hash` uses the canonicalizer but is an action
digest, not a reusable value key. Implement the public API from
`src/stdlib/mod.rs`. The snapshot tree must validate recursively, retain
numeric kinds, reject unsupported map keys/values, and be the only value passed
to JCS. Equality derives both keys before comparing. Stable dedup traverses the
full input in order, validates every value, records membership privately, and
returns original references at first occurrence. On failure return an error
without partial output.

Implement `verus/canonical_value/main.rs` alongside the algorithm. Prove the
abstract least-index model with source indices and constructive append/skip
steps. Review the trusted base and model correspondence. Keep Verus files
outside Cargo and ordinary CI.

Run focused tests for green. Run the Verus command above and require proof
success before declaring this plateau complete.

### Stage D — document and validate

Update `docs/netsuke-design.md` with the canonical foundation and unchanged
`uniq` semantics. Update `docs/formal-verification-methods-in-netsuke.md` and
`docs/developers-guide.md` to admit canonical deduplication as a second narrow
Verus kernel; the developers' guide currently says first Verus work must focus
on cycle canonicalization. Preserve its rule that Verus remains outside normal
Cargo and routine CI. Update `docs/developers-guide.md` with the host-object
precondition and reuse boundary if that practice will guide future helper
authors. No users' guide change is needed because no template helper is
registered. Add an ADR only if implementation reveals a durable decision beyond
RFC 0006.

After each coherent implementation plateau, run required gates sequentially:
`make check-fmt`, `make lint`, `make doc-coverage`, `make test`. When docs
change, first run `make fmt`, then run `make markdownlint` and `make nixie`. At
final acceptance run every listed gate and the proof. Capture each gate's
output under `/tmp` as repository instructions require; never overlap
formatting, lint, and test gates.

At each boundary trace `EP-RQ-612` and `EP-RQ-JCS` to evidence; confirm no
change to existing `uniq`/`group_by` registrations, no pull-forward of 6.1.3 or
6.1.4 scaffolding, no new dependency/trust boundary, and no exposed hash order.

## Milestones and plateaus

### EP-M1 — characterized domain and toolchain

Outcome: an evidence-backed supported-value matrix, concrete API, and pinned
Verus release/checksum exist. Requirements: clarify EP-RQ-612 and EP-RQ-JCS.
Acceptance: tests can construct each admitted/rejected kind; extreme numbers
and JCS boundaries are exercised; `prover-tools`/Make flags and verifier
checksum are confirmed. Conformance: RFC domain preserved; ordinary
maps/sequences usable; no coercion, new template name, dependency, wire format,
or unapproved trust boundary. Recovery: retain characterization only and revise
this plan if the contract cannot be met. Remaining: implementation/proof/docs.
Compatibility: none.

### EP-M2 — canonical key and equality

Outcome: typed keys and equality operate on supported values; errors occur
before coercion; existing template behaviour is unchanged. Requirements:
EP-RQ-JCS key semantics; advance EP-RQ-612. Acceptance: EP-V1/EP-V2 tests and
Rust BDD scenario pass, including invalid self-comparison and validation of
both operands. Conformance: recursive one-time snapshot; no error localization
or bounds pulled forward; no new dependency/registration; trace links current.
Recovery: retain red tests and revert incoherent implementation changes.
Remaining: stable dedup and proof. Compatibility: none.

### EP-M3 — first-index dedup and Verus proof

Outcome: helper returns original first references in order and the abstract
unbounded kernel is proved. Requirements: complete EP-RQ-612 and first-wins
EP-RQ-JCS. Acceptance: EP-V3 Proptest/oracle/BDD tests pass; seeded last-wins
or hash-order fault is rejected; Verus proves index
range/order/coverage/uniqueness/ minimality without shortcuts. Conformance:
runtime tests cover the bridge; proof is not described as proving serializer
correctness; Verus stays outside ordinary gates; trace links current. Recovery:
stop and revise the proof plan if assumptions or actual tool flags differ.
Remaining: docs/final gates. Compatibility: none.

### EP-M4 — documented delivery

Outcome: design and proof guidance explain the contract/limits; sequential
gates pass; roadmap item 6.1.2 is marked done. Acceptance: updated docs, all
gates and proof pass, and no user guide entry claims a nonexistent template
helper. Conformance: upstream assumptions reconciled; no hidden CI gate/new
dependency/ format; trace links current. Recovery: preserve last passing
commit, repair the failure, then rerun affected gates and all dependent gates.
Remaining: later helpers decide their own bounds, diagnostics, and integration.
Compatibility: none.

## Concrete steps

Run from the repository root:

1. Inspect current dependency and filter behaviour:

   ```sh
   rg -n 'serde_json_canonicalizer|uniq_filter|group_by_filter' Cargo.toml src/stdlib
   ```

2. Inspect MiniJinja/canonicalizer locked source APIs and construct direct
   fixtures for scalar kinds, custom Map/Seq, iterator failures, nested invalid
   keys, and numeric extremes. Record the admitted tree representation.

3. Red: add unit/Proptest tests to `src/stdlib/canonical_value/mod.rs`, BDD
   feature and bindings at the named paths; run:

   ```sh
   cargo nextest run --lib canonical_value
   cargo nextest run --test bdd_tests canonical_value
   ```

4. Green: implement the smallest canonical tree, key, equality and dedup
   behaviour. Rerun both focused commands. Refactor without changing behaviour
   and rerun them.

5. Verify pinned Verus version/checksum from `tools/verus/VERSION` and
   `tools/verus/SHA256SUMS`; install through `make install-verus` if required;
   then run:

   ```sh
   make verus VERUS_FLAGS='--repo-root . --proof-file verus/canonical_value/main.rs'
   ```

6. Run each full quality gate sequentially after each major plateau, with logs
   under `/tmp`:

   ```sh
   make check-fmt
   make lint
   make doc-coverage
   make test
   make markdownlint
   make nixie
   ```

   Markdown/Mermaid gates apply when docs change. Run all six at final
   acceptance. Add the Verus command to EP-M3 and final acceptance.

7. Review the final diff for the public export list, unchanged filters, proof
   assumptions, source file lengths, docs, and roadmap state before committing
   each coherent change. Use the commit-message skill, a message file, and
   gated atomic commits; do not commit an intentionally red test state.
   Preserve proof logs and red/green evidence in this plan as work progresses.

## Validation and acceptance

Red-Green-Refactor is available and required. Red tests fail because the
canonical key/dedup behaviour is absent or wrong. Green passes after the
minimal implementation. Refactor changes structure only and focused tests plus
plateau gates pass again. Do not leave expected-failure markers in the
completed suite.

The contract example uses a materialized input so references have clear
provenance:

```rust
let input = [map_a, map_b, reordered_map_a];
let retained = stable_deduplicate(&input)?;
assert_eq!(retained.len(), 2);
assert!(std::ptr::eq(retained[0], &input[0]));
assert!(std::ptr::eq(retained[1], &input[1]));
```

The final names should match the API in `Constraints`. Completion requires
equivalence properties over generated admitted values; first-index properties
against an independent quadratic oracle; typed unsupported-value errors before
coercion; successful unbounded Verus proof; runtime tests against MiniJinja and
the canonicalizer; unchanged template registrations; no public hash iteration
order; and all required sequential gates passing. Only then mark roadmap 6.1.2
done.

## Revision note

- 2026-10-13: Initial DRAFT from roadmap 6.1.2, RFC 0006 §6.7, repository
  reconnaissance, Firecrawl check of RFC 8785, Wyvern findings, and six-lens
  review. Defined the host-object boundary, numeric semantics, exact APIs and
  test paths, an index-based proof sketch, and the required documentation
  policy update. No implementation started; approval remains required.

- 2026-10-13: Final expert readback accepted the revised design. Recorded
  documentation-gate evidence, explicit prefix-state refinement, original-value
  identity assertions, module ownership, and existing BDD fixture/discovery
  wiring. The plan remains DRAFT; these checks do not establish feature
  implementation or discharge the future proof obligations.

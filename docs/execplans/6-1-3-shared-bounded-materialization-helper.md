# Implement the shared bounded-materialization helper

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: APPROVED

Approved by the user on 2026-11-01, with the D-03 and D-12 recommendations
accepted. The status moves to `IN PROGRESS` when EP-M1 starts, and to
`COMPLETE` only after EP-M5's reconciliation. These are the only values
`tests/execplan_status_contract_tests.rs` accepts.

## Purpose / big picture

Later standard-library helpers will parse untrusted text (`from_json`,
`from_yaml`, `b64decode`), build combinatorial build matrices (`product`,
`combinations`, `permutations`), recurse through nested mappings
(`combine(recursive=true)`), and collect matches (`regex_findall`, and
`glob(files_only=false)`). Each of them can be asked, by one short manifest
line, to produce more output than the machine can hold. RFC 0006 §6.8 requires
every such helper to reject unreasonable expansion before allocating, and to
say exactly why. This task delivers the one shared Rust helper that every later
helper uses to do that, so that ceilings, arithmetic, and refusal shapes are
decided once rather than eight times.

After this change a Rust caller inside Netsuke can:

1. describe a combinatorial request by its operand lengths, never by its
   materialized output, and learn its exact cardinality, computed with checked
   arithmetic, or learn that the cardinality is too large to represent;
2. obtain an `AdmittedCount` token, which is the only way to learn a tuple
   count the helper has approved, or receive a typed refusal naming both the
   computed cardinality and the ceiling;
3. check an input's byte length, a nesting depth, a growing match count, and a
   growing output byte total against fixed ceilings, each through its own
   precisely typed refusal.

No template filter, function, or test is registered by this task, so existing
manifests render exactly as before. The observable result is a public,
documented, tested, and partly proved Rust contract. A reviewer can see it
working by running the new unit, property, and behavioural suites, the Kani
harnesses, and the Verus proof, each named in `Validation and acceptance` with
its expected output. For example, the behavioural scenario "Over-ceiling
combinatorial request is rejected naming cardinality and ceiling" admits a
product of operand lengths 316 and 317 and observes the refusal
`combinatorial cardinality 100172 exceeds the ceiling of 100000 output tuples`
without any tuple being built.

This plan covers roadmap item 6.1.3 only. It targets a release after v0.1.0
final and does not widen the hardening release defined by issue #594.

## Constraints

- Follow RFC 0006 §6.8 and table 3. The ceilings are compile-time constants in
  this slice: input length 8 MiB (8,388,608 bytes), nesting depth 128, output
  tuples 100,000 for `product` and `combinations`, output tuples 10,000 for
  `permutations`, and match count 100,000. Do not make them configurable; RFC
  0006 §6.8 and its open question 5 defer configuration until a consumer needs
  it. Constants never enter `OrthoConfig` or `StdlibConfig`.
- Cover the four bound families named by roadmap 6.1.3 (input length, nesting
  depth, checked combinatorial cardinality, match counts) plus the output byte
  total that six child RFCs require, approved under D-12. Alias expansion nodes
  belong to roadmap 6.2.2; compiled-pattern size and the pattern cache belong
  to roadmap 6.5.1. Do not implement them here.
- Cardinality is computed from operand lengths only, with checked arithmetic,
  and compared with the ceiling before any output exists. The helper must not
  accept, iterate, or allocate the output it is bounding. Overflow is a typed
  refusal, never a wrap, a saturation presented as a count, or a panic (RFC
  0006 §6.6).
- Report an exact cardinality or the overflow refusal, never an estimate. When
  the cardinality is not representable in `u64`, report overflow; overflow
  takes precedence over the over-ceiling refusal (RFC 0015 §5.8). An empty
  operand yields cardinality zero even when other operands would overflow.
- The permutation ceiling is selected by the request kind inside the helper,
  and the stream ceilings by the stream kind. A caller must not be able to pair
  a quantity with the wrong ceiling.
- The module is pure: no I/O, environment, clock, randomness, filesystem,
  subprocess, locale, or global state. It must not depend on `minijinja`; a
  consumer extracts lengths from template values and maps refusals into its own
  domain error.
- Refusals are typed, `Copy`, and carry only integers and enumerated kinds, so
  constructing one performs no heap allocation. Do not add Fluent keys,
  `netsuke::jinja::` codes, or any conversion into `minijinja::Error`. Roadmap
  6.1.4 owns the diagnostic scaffolding, and the child RFCs assign each code to
  its consumer module (`collections`, `interchange`, `transform`, `pattern`,
  `path`), not to this helper. The English `Display` rendering is for
  developers and logs only (D-03).
- Add no dependency, including dev-dependencies. `proptest`, `rstest`,
  `rstest-bdd`, `googletest`, `pretty_assertions`, `insta`, and `thiserror` are
  already declared.
- `unsafe_code` is forbidden workspace-wide (`Cargo.toml`, `[workspace.lints]`).
  Do not plan or add a counting global allocator.
- Do not change the behaviour or registration of any existing helper,
  including `uniq`, `group_by`, and the canonical value module. Do not retrofit
  the depth bound into `src/stdlib/canonical_value/` (D-07). Do not migrate any
  existing limit (D-01).
- Keep every Rust file under 400 lines. Every module starts with a `//!`
  comment. Every function and method carries a `///` comment with an imperative
  summary. Each public function and method carries a runnable `# Examples`
  doctest using the public path `netsuke::stdlib::…`, following the precedent at
  `src/stdlib/canonical_value/error.rs`; `src/stdlib/resource_bounds/mod.rs`
  opens with a module-level example. Private kernel functions get `///`
  summaries without doctests. Use `foo/mod.rs` layout for the new multi-file
  module; its test subtree has several files, so it is `tests/mod.rs`.
- Verus remains optional, proof-kernel-only, and outside ordinary Cargo and
  continuous-integration gates. Do not use `assume`, `admit`, `external_body`,
  trusted bodies, or added axioms in the Verus proof. Do not put
  `#![allow(warnings)]` or `#![allow(clippy::all)]` in the proof file, which
  sits under a root the environment-access scan reads.
- Every Kani harness must own a committed mutation patch under
  `docs/verification/mutations/`; `EXEMPT_HARNESSES` in
  `tests/kani_mutation_evidence_tests.rs` stays empty.
- Never run Kani bare on a developer machine: always use the
  `systemd-run --user --scope` wrapper documented in "Kani scope wrapper" in
  `docs/developers-guide.md`.
- No source-API compatibility machinery: this is a new, pre-1.0 surface with no
  external consumer.

If satisfying the objective would violate a constraint, stop, record the
conflict in `Decision log`, and escalate.

## Tolerances (exception triggers)

- Scope: stop and escalate if implementation needs more than 40 changed files
  or more than 2,000 net added lines of Rust, excluding snapshots, mutation
  patches, and the Verus proof file.
- Interface: stop if the public surface must differ from the names listed in
  `Interface and representation`, or if any existing public signature must
  change.
- Dependencies: stop if any new crate, feature flag, or toolchain component is
  required.
- Semantics: stop if the exact-cardinality contract cannot be met for some
  admissible input without an estimate, or if a child RFC requirement turns out
  to conflict with a refusal shape in a way this plan does not record.
- Kani: every local run is capped at 8 minutes by the wrapper. If a single new
  harness takes more than 60 seconds or breaches the 8 GiB cap, reduce its
  stated bound once and record the new bound; if the reduced harness is still
  intractable, or its bound no longer reaches every `kani::cover!`, stop and
  escalate rather than deleting the harness.
- Verus, tiered: lemmas L4, L5, L5a, L6, and L8 (the binomial) must be proved.
  If three focused attempts fail on one of them, record the failing
  verification condition and the decompositions tried in
  `Surprises & discoveries`, then escalate. Lemmas L1, L2, L3, L7, and L9
  (product and falling factorial) may instead fall back to Kani plus named
  witnesses after three failed attempts, recorded in `Decision log`. Never
  weaken a lemma to make it pass, and never add an axiom.
- Iterations: if a gate still fails after three fix attempts for the same root
  cause, stop and escalate with the log path.
- File size: if `tests/bdd/fixtures/mod.rs` (281 lines at planning time) would
  exceed 400 lines, stop and move the new slots into a separate fixture module
  rather than compressing existing code.
- Ambiguity: if a reviewer or a discovery shows that the diagnostic boundary in
  D-03 or the output-bytes inclusion in D-12 is unacceptable, stop and present
  options rather than guessing.

## Risks

- Risk: the binomial computation can overflow an intermediate product even when
  the final value fits, or can loop for a long time on a huge `r`. Severity:
  high. Likelihood: medium. Mitigation: use the symmetric reduction
  `k = min(r, n - r)`; return overflow immediately when `k >= 34` (L8); carry
  the running value in `u128` and stop as soon as it exceeds `u64::MAX`. The
  running value is at most `u64::MAX` before each multiplication and the
  multiplier is at most `u64::MAX`, so the intermediate is below `2^128` (L5);
  the running values are non-decreasing for `i < k <= n / 2`, so the early stop
  is sound (L6).
- Risk: an empty operand placed after two huge operands is reported as overflow
  instead of cardinality zero. Severity: medium. Likelihood: high for a naive
  left fold. Mitigation: scan for a zero length before multiplying; witness
  `[2^40, 2^40, 0]` and a seeded fault in EP-V2.
- Risk: a consumer treats `AdmittedCount` as permission to materialize, and
  admits `combinations(100000, 99999)` (100,000 tuples of 99,999 elements each,
  about nine billion values) or `[[0]] | product(repeat=1000000)`. Severity:
  high. Likelihood: medium. Mitigation: RFC 0015 §5.8 requires a second,
  content-size check after cardinality. The `RunningTotal` primitive with
  `StreamKind::OutputBytes` supports that pre-count, and the re-use policy and
  `AdmittedCount`'s doc comment state that it is a tuple count, not a memory
  budget.
- Risk: a stream-wide total is silently split. If the running counter were
  `Copy`, passing it by value per document in `from_yaml_all` would restart the
  count each time. Severity: medium. Likelihood: medium. Mitigation:
  `RunningTotal` is neither `Copy` nor `Clone`; a test drives one total across
  several simulated documents.
- Risk: `serde_json` enforces its own recursion limit, which admits 127 nested
  containers and rejects the 128th, while this helper admits 128 and rejects
  the 129th; `serde-saphyr` recurses during parsing before any visitor runs.
  Severity: medium. Likelihood: high once roadmap 6.2.1 lands. Mitigation:
  define depth precisely (D-06), document that `NestingDepth` only guards
  recursion Netsuke owns and must be checked before recursing, and hand the
  reconciliation to roadmap 6.2.1 with the acceptance shape "128 levels pass
  and 129 fail identically for JSON and YAML". A consumer must never invent an
  observed depth it did not measure.
- Risk: a consumer forwards a refusal's `Display` text into a template error,
  leaking unlocalized English and bypassing module codes. Severity: medium.
  Likelihood: medium. Mitigation: the re-use policy forbids it; the refusal
  types expose fields so consumers can build localized messages.
- Risk: Fluent formats numeric arguments through `f64`. Only a cardinality in
  `CardinalityError::Exceeded` can exceed `2^53`, because every other value is
  bounded by a ceiling of at most 8,388,608 plus one. Severity: low.
  Likelihood: medium once roadmap 6.1.4 localizes refusals. Mitigation: record
  the hazard in the developers' guide and leave the choice of string or number
  formatting to 6.1.4, which must pin a test with a count above `2^53`.
- Risk: adding files under `src/stdlib/` changes derived whole-crate contracts
  that file-scoped gates do not notice: the Kani proof scope
  (`tools/kani/proof-scope.toml`, recomputed by
  `tests/workflow_contracts/kani_proof_scope_test.py`), the workspace source
  scan (`tests/env_access_suppressions/`), the Kani mutation-evidence contract
  (`tests/kani_mutation_evidence_tests.rs`, which runs under `make test`), and
  the `cargo-mutants` exclusion lists. Severity: medium. Likelihood: high.
  Mitigation: run `make test-workflow-contracts` unconditionally at every
  milestone; derive the proof scope from source; commit one mutation patch per
  harness; add the new `verification.rs` to all three exclusion lists.
- Risk: the Verus model is a handwritten mirror, so it can drift from the
  production kernel. Severity: medium. Likelihood: medium. Mitigation: write
  the model as `exec fn`s over `u64` and `u128` that match `kernel.rs` token
  for token where `vstd` specifies the operations used; drive the production
  kernel with Kani within explicit bounds; check it against independent oracles
  with Proptest; state the residual gap in the proof file.
- Risk: every later pull request touching `src/stdlib` runs the new harnesses
  and their mutation codegens in the `kani-smoke` job's 30-minute budget.
  Severity: medium. Likelihood: high. Mitigation: keep each harness under 60
  seconds, record measured times in the Kani harness table, and treat a breach
  as a Kani tolerance event.
- Risk: the 8 MiB output-content ceiling had no row in RFC 0006 table 3, and
  RFC 0014 reads the input row as the output ceiling. Severity: medium.
  Likelihood: high. Mitigation: D-12 includes the stream kind, and EP-M5 adds
  the table 3 output-content row under the user's approval to amend referenced
  RFCs, with the ADR recording the decision. The RFC edit is deliberate and
  reviewed, not a silent change.

## Progress

- [x] (2026-10-17 10:05Z) Renamed the branch to
  `6-1-3-shared-bounded-materialization-helper`; the remote branch does not
  exist yet and is created when the plan is pushed.
- [x] (2026-10-17 10:40Z) Wyvern reconnaissance: stdlib layout, the 6.1.2
  canonical value precedent, localization and diagnostic-code conventions,
  existing bounds, verification infrastructure, child RFC requirements, and ADR
  allocation.
- [x] (2026-10-17 11:20Z) Prior-art research: `serde_json` recursion limit,
  Fluent numeric formatting, and checked binomial computation.
- [x] (2026-10-17 12:30Z) Drafted the plan; committed as `9f6c0f5f` after
  `make check-fmt`, `make markdownlint`, an explicit `typos` check, and the
  ExecPlan status contract test passed.
- [x] (2026-10-17 14:10Z) Expert-panel design review (structure and contracts,
  formal verification, failure modes and operations, alternatives, conventions)
  and revision. Verdict: proceed with conditions; every condition is folded
  into this revision (see `Revision note`).
- [x] (2026-11-01) Plan approved by the user; D-03 and D-12 accepted. RFC
  0006 table 3 gains an output-content row under D-12, and the ADR number is
  re-derived across all branches before the ADR is committed.
- [ ] EP-M1: characterized toolchain.
- [ ] EP-M2: checked cardinality kernels and admission.
- [ ] EP-M3: input length, nesting depth, running totals, and refusal
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
  `fluent_bundle::types::FluentNumber` API documentation. Impact: see the
  Fluent risk.
- Observation: RFC 0006 table 3 has no output-content row, yet RFCs 0013,
  0014, 0015, 0016, 0018, and 0019 all rely on an 8 MiB output ceiling with an
  `output_too_large` code, and no roadmap task owns it. Evidence: RFC 0015 §5.8
  ("the group's own rather than a table 3 row") and the sibling §5.8 sections;
  a search of `docs/roadmap.md` for `output_too_large` finds nothing. Impact:
  D-12.
- Observation: `unsafe_code = "forbid"` is set in `[workspace.lints]`, so a
  counting global allocator cannot be written in a test binary. Evidence:
  `Cargo.toml` line 266. Impact: "rejected before allocation" is established
  structurally and by unmaterializable witnesses (EP-V5).
- Observation: RFC 0015 §5.8 and roadmap 6.4.2 require cardinality and
  overflow to be distinct codes, overflow taking precedence, and an exact
  count. Evidence: RFC 0015 lines 231–245 and 339–349. Impact: the two variants
  of `CardinalityError`.
- Observation: RFC 0013 §5.8 says the `from_yaml_all` diagnostic "reports the
  stream total", which is exact for input length (one call over the whole text)
  but cannot be exact for a streamed count that stops at the first excess item.
  Evidence: RFC 0013 lines 276–279. Impact: `StreamLimitExceeded` carries no
  total (D-08); the alias-node case is handed to roadmap 6.2.2.
- Observation: the binomial and falling-factorial loops can be capped
  structurally. `binom(67, 33) = 14226520737620288370` is the largest central
  value that fits `u64`, and `binom(n, k) >= binom(68, 34) > u64::MAX` whenever
  `34 <= k <= n / 2`; `20!` fits and `21!` does not. Evidence: the formal
  verification reviewer's exhaustive check, to be re-confirmed by the oracle in
  EP-M2. Impact: at most 33 binomial and 20 falling-factorial iterations (L8,
  L9).
- Observation: sweep for an existing equivalent helper, as the AGENTS.md
  abstraction policy requires. Commands:
  `rg -l 'LimitExceeded|MAX_DEPTH|max_bytes|checked_mul|budget' src/` and
  `rg -il 'bounded|materializ|ceiling|cardinality' src/`. Findings:
  `src/stdlib/path/bounded_read/mod.rs` (a byte-read cap using a one-past
  sentinel), `src/stdlib/command/error/support.rs` (`LimitExceeded` for command
  output), `src/stdlib/network/redirect/` (`FETCH_REDIRECT_LIMIT`),
  `src/stdlib/which/lookup/workspace/mod.rs` (`WORKSPACE_MAX_DEPTH`, a
  directory-walk cap), `src/stdlib/time/mod.rs` (one `checked_mul`),
  `src/manifest/budget/` (the ADR-018 manifest evaluation budget, including the
  `foreach` cardinality limit), and its configuration layering under `src/cli/`
  (`manifest_budget`, `discovery/budget`). Conclusion: none computes a
  combinatorial cardinality, none is a table 3 row, and each limits an I/O or
  evaluation resource with its own configurable value and diagnostic. None is
  reused or migrated (D-01).
- Observation: at planning time the highest ADR number allocated on any remote
  branch is 056 (`issue-592-…`), so the next free number is 057. Evidence:
  remote-branch sweep on 2026-10-17. Impact: the ADR in EP-M5 is provisionally
  ADR-057 and must be re-derived immediately before it is committed.

## Decision log

- D-01. Decision: one shared module, `src/stdlib/resource_bounds/`, owns the
  ceilings this task covers, the checked cardinality arithmetic, and the
  refusal types. Existing I/O and evaluation limits are neither reused nor
  migrated. Rationale: step 6.1 asks whether one shared contract can carry
  every later helper; six child RFCs reach these bounds, and duplicating the
  arithmetic gives six chances to get overflow precedence or zero absorption
  wrong. The swept limits bound different resources with configurable values,
  so merging them would couple unrelated contracts. Date/Author: 2026-10-17,
  planning agent.
- D-02. Decision: the module is independent of `minijinja` and works on counts
  (`usize` lengths, `u64` parameters and cardinalities). Rationale: the bounded
  quantity is always a count the consumer can read before materializing anything
  (`Value::len()`, a byte length, a recursion level, an iterator step).
  Keeping template values out makes the kernel pure, Kani-tractable, and
  mirrorable in Verus, and preserves the hexagonal dependency rule: template
  adapters depend on the kernel, never the reverse. Date/Author: 2026-10-17,
  planning agent.
- D-03. Decision: refusals are typed, `Copy`, unlocalized values with an exact
  English `Display` rendering for developers and logs. This task adds no Fluent
  keys, no diagnostic codes, and no conversion into `minijinja::Error`.
  Rationale: roadmap 6.1.4 owns the domain-error and Fluent scaffolding; the
  child RFCs assign every code to a consumer module, so a `bounds` namespace
  would contradict them; and a localized key now would mean editing 35
  catalogues for a message no user can trigger. The roadmap criterion "the
  diagnostic names both the computed cardinality and the ceiling" is met at the
  contract level: `CardinalityError::Exceeded` carries both as typed fields and
  renders both, pinned by snapshots. The user-facing half is discharged by
  roadmap 6.1.4 with the first consumer, 6.4.2, whose localized message must
  name both numbers; the ADR records that hand-off. Alternative considered:
  `stdlib.bounds.*` keys now. Rejected for the reasons above. Date/Author:
  2026-10-17, planning agent; confirmed by the expert panel. Approved by the
  user on 2026-11-01: refusals stay typed and English-only, and the Fluent keys
  and `netsuke::jinja::` codes are deferred to roadmap 6.1.4 and the consuming
  modules. The roadmap's "diagnostic names both values" is discharged at the
  Rust contract level here; the user-facing message arrives with 6.1.4 and
  6.4.2.
- D-04. Decision: cardinalities are `u64`; binomial intermediates are `u128`. A
  cardinality above `u64::MAX` is overflow. Rationale: every ceiling is far
  below `u64::MAX`, so a wider type only moves the overflow boundary, and a
  `u128` cardinality would need a 256-bit intermediate. Arbitrary precision
  needs a dependency and makes work unbounded. Date/Author: 2026-10-17,
  planning agent; confirmed by the expert panel.
- D-05. Decision: ceilings are associated with kinds, not passed by callers.
  `CardinalityRequest::admit` selects its ceiling from the request kind, and
  `RunningTotal` takes a `StreamKind` whose ceiling is fixed. There is no
  public ceiling type to misuse. Rationale: removes the defect classes
  "permutations checked against the product ceiling" and "match stream checked
  against the depth ceiling". Tests that need small ceilings use private kernel
  functions or a `#[cfg(any(test, kani))]` constructor, never a public one.
  Date/Author: 2026-10-17, revised after panel review.
- D-06. Decision: nesting depth counts enclosing containers. The root value is
  at depth 0; entering a container moves to depth `d + 1`; depths 1 to 128 are
  admitted, and the attempt to enter depth 129 is refused with observed depth
  129. Rationale: the literal reading of "nesting depth 128". The one-level
  difference from `serde_json`, and the fact that parser-internal recursion
  happens before any visitor runs, are recorded for roadmap 6.2.1, which owns
  `from_json` and may need its own depth visitor or parser configuration.
  Date/Author: 2026-10-17, planning agent.
- D-07. Decision: do not apply the depth bound to the canonical value snapshot
  in this task. Rationale: table 3 does not list canonical key derivation, and
  applying the bound would change an existing public function's refusal set.
  Record it as a follow-up candidate in the developers' guide. Date/Author:
  2026-10-17, planning agent.
- D-08. Decision: a running total that passes its ceiling reports
  `StreamLimitExceeded` with the kind and ceiling but no total. Rationale: the
  stream stops at the first excess item, so the true total is unknown; reporting
  `ceiling + 1` as the count would be an estimate. A checked-add overflow of
  the total is also `StreamLimitExceeded`, because the total is then certainly
  past the ceiling. Date/Author: 2026-10-17, planning agent.
- D-09. Decision: each entry point returns a refusal type containing only the
  outcomes it can produce: `CardinalityError` (`Exceeded`, `Overflow`),
  `InputLengthExceeded`, `DepthExceeded`, and `StreamLimitExceeded` (with an
  exhaustive `StreamKind`). Rationale: consumers map refusals to their own
  module codes; precise types make every mapping total without `unreachable!()`
  arms, and an overflow cannot be mapped to `cardinality_exceeded` by mistake.
  An exhaustive `StreamKind` forces each consumer to choose a code when a kind
  is added. Fields are public plain data; by policy only this module constructs
  refusals, recorded in the re-use policy. Alternative considered: one
  `BoundError` enum over `(variant, kind)`, rejected because it admits
  meaningless pairs such as overflow of a nesting depth. Date/Author:
  2026-10-17, revised after panel review.
- D-10. Decision: verification is layered. Verus proves the arithmetic
  contract over a model for all `u64` inputs; Kani drives the production kernel
  within explicit bounds; Proptest compares the production kernel with
  independent oracles; named `rstest` witnesses pin boundaries and controls.
  Rationale: Verus cannot compile the production crate, Kani cannot cover the
  33-step binomial loop symbolically, and Proptest only samples. Skipping Verus
  would leave large `n` with `5 <= r <= 33` unchecked, which is exactly where
  the binomial overflow boundary lies. Date/Author: 2026-10-17, confirmed by
  the expert panel.
- D-11. Decision: a substantive ADR records the shared bounds contract,
  provisionally ADR-057, with `Status: Accepted`. It notes a separate small
  workspace crate as the move to make if a third pure kernel appears.
  Rationale: the decision is hard to reverse once eight consumer tasks depend
  on it, and it answers step 6.1's central question. Date/Author: 2026-10-17,
  planning agent.
- D-12. Decision (approved 2026-11-01): include `StreamKind::OutputBytes`
  with an 8 MiB ceiling now, beyond the four families roadmap 6.1.3 names.
  Rationale: six accepted child RFCs fix the value and the `output_too_large`
  behaviour, no roadmap task owns it, and leaving it out means the first
  consumer writes it locally and the rest copy it, which is the duplication
  D-01 exists to prevent. The cost is one enum variant and one constant.
  Resolution of the table 3 question: RFC 0006 table 3 gains an output-content
  row for this ceiling in this work rather than dropping the ceiling, because
  six child RFCs already depend on it and the user approved the necessary RFC
  amendments. The ADR records the row and its rationale, and the plan edits the
  RFC directly instead of raising the question in the pull request.
  Alternative: omit it and keep only the `admit(amount)` shape so the variant
  could be added later; rejected when the user approved D-12. Date/Author:
  2026-10-17, after panel review (alternatives and failure-modes reviewers
  recommended inclusion; the contracts reviewer preferred leaving the value to
  the RFC owner); 2026-11-01, approved by the user.
- D-13. Decision: `RunningTotal::admit(amount)` adds with `checked_add`, and
  `admit_one()` is shorthand for `admit(1)`. `RunningTotal` is neither `Copy`
  nor `Clone`. Rationale: every output-content consumer adds element sizes, not
  ones; a copyable counter silently splits stream-wide totals. Date/Author:
  2026-10-17, after panel review.
- D-14. Decision: provide both `collect_matches` (infallible items) and
  `try_collect_matches` (items of `Result<T, E>` with
  `E: From<StreamLimitExceeded>`), each counting against
  `StreamKind::MatchCount` and pulling at most one item past the ceiling.
  `try_collect_matches` stops at the first error. Rationale: `regex_findall`
  can fail per match (`no_such_group`, RFC 0016) and `glob` traversal is
  fallible (RFC 0018); a collector over `Result` items would otherwise keep
  pulling past an error and return nested results. Date/Author: 2026-10-17,
  after panel review.
- D-15. Decision: cap the loops structurally. `checked_binomial` returns
  `None` when `k >= 34`, and `checked_falling_factorial` returns `None` when
  `r >= 21`, before entering their loops. Rationale: bounded work becomes a
  property of the code rather than of arithmetic growth, and the caps are
  justified by lemmas L8 and L9. A cap set too low is a detectable behavioural
  fault (`binom(67, 33)` and `P(20, 20)` must still be computed). Date/Author:
  2026-10-17, after panel review.
- D-16. Decision: `admit_input` takes `impl AsRef<[u8]>` rather than a
  length. Rationale: a caller cannot pass a character count where a byte count
  is meant. A private length-taking function serves boundary tests near 8 MiB
  without allocating 8 MiB buffers. Date/Author: 2026-10-17, after panel review.
- D-17. Decision: red tests are written at the start of the milestone that
  turns them green, and are never committed red. Behavioural scenarios are
  added to the feature file in the milestone that implements them. Rationale:
  every commit must pass the gates; writing all red tests up front would make
  the EP-M2 commit impossible. Stashing is not used to juggle red files,
  because the stash stack is shared with other worktrees. Date/Author:
  2026-10-17, after panel review.

## Outcomes & retrospective

Not yet started. Complete this section at each milestone boundary and at
completion, comparing the result against `Purpose / big picture` and
reconciling every discovery with the artefacts in `Conformance basis`.

## Context and orientation

Netsuke is a build tool. It reads a YAML manifest called a `Netsukefile`,
expands embedded Jinja templates with the MiniJinja engine, and compiles the
result to a Ninja build file. Netsuke ships a template standard library, a set
of filters, functions, and tests available to every template. Its code lives
under `src/stdlib/`, in the main crate (package `netsuke-build`, library name
`netsuke`). `src/stdlib/mod.rs` declares private modules and re-exports a
public surface, for example
`pub use canonical_value::{CanonicalKey, CanonicalValueError, canonical_eq,
canonical_key, stable_deduplicate};`.
Registration of template helpers happens in `src/stdlib/register.rs`, already
at the 400-line cap, and `src/stdlib/register/query_helpers.rs`. This task
registers nothing, so neither file changes.

RFC 0006 (`docs/rfcs/0006-ansible-inspired-template-standard-library.md`) is
the parent design for the standard-library expansion. §6.8 and table 3 define
resource bounds; §6.6 says overflow is a diagnostic; §6.9 defines diagnostics,
which roadmap 6.1.4 implements. The child RFCs specify the consumers:

- RFC 0013 (`from_json`, `from_yaml`, `from_yaml_all`): input length 8 MiB and
  nesting depth 128, codes `interchange::length_exceeded` and
  `interchange::depth_exceeded`; serializers pre-count output against 8 MiB.
- RFC 0014 (`combine(recursive=true)`, `subelements`): nesting depth, code
  `transform::depth_exceeded`; output content ceiling.
- RFC 0015 (`product`, `combinations`, `permutations`, `zip_longest`): exact
  checked cardinality, distinct `collections::cardinality_exceeded` and
  `collections::overflow` codes with overflow taking precedence, permutations
  under the lower ceiling, then an 8 MiB content check (`output_too_large`).
- RFC 0016 (`regex_findall`, `regex_replace`): match count checked as it
  grows, code `pattern::match_limit`; output content ceiling.
- RFC 0018 (`glob(files_only=false)`, `expandvars`): a 100,000-match ceiling
  with code `path::match_limit`; an 8 MiB output ceiling.
- RFC 0019 (`b64decode`, `comment`): input length; checked output size.

The term *materialization* in this plan means building a template value, such
as a list of tuples, in memory. A *ceiling* is the largest admitted value of a
bounded quantity. A *cardinality* is the number of output tuples a
combinatorial request would produce. *Admission* is the check that a quantity
is at most its ceiling. A *seeded fault* is a deliberate temporary bug used to
show that a test or proof would catch it.

The closest precedent is roadmap 6.1.2, the canonical value module in
`src/stdlib/canonical_value/` (`mod.rs`, `admission.rs`, `error.rs`, `node.rs`,
and a `tests/` subtree). It exposes a `thiserror` error enum without Fluent
keys, keeps its tests in `src/stdlib/canonical_value/tests/`, pins rendered
messages with `insta` snapshots under `src/snapshots/canonical_value/` via
`crate::snapshot_test_support::snapshot_settings`, has a behavioural feature at
`tests/features/canonical_value.feature` with steps in
`tests/bdd/steps/stdlib/canonical_value.rs` and `TestWorld` slots in
`tests/bdd/fixtures/mod.rs`, and a standalone Verus proof at
`verus/canonical_value/main.rs`.

Existing repository limits are unrelated to this task; the sweep in
`Surprises & discoveries` lists them. In particular,
`src/stdlib/config/types.rs` holds configurable I/O byte limits, and
`src/manifest/budget/` holds the manifest evaluation budget from ADR-018.
Neither is a table 3 row, and a helper subject to both this module and the
manifest budget enforces both.

Formal verification tools in this repository:

- Kani is a bounded model checker for Rust. Harnesses live beside the code in a
  `verification.rs` file gated by `#[cfg(kani)] mod verification;`. Each is
  listed in the Kani harness table in `docs/developers-guide.md`, owns a
  mutation patch under `docs/verification/mutations/` named after its path with
  `::` replaced by `__` (for example
  `stdlib__resource_bounds__verification__<harness>.patch`), and is checked by
  `tests/kani_mutation_evidence_tests.rs` under `make test` and by
  `make test-kani-mutations`. The derived module closure in
  `tools/kani/proof-scope.toml` must list every reachable source file. The
  `cargo-mutants` exclusion lists in `.github/workflows/mutation-testing.yml`,
  `tests/workflow_contracts/mutation_testing_test.py` (`KANI_EXCLUDES`), and
  `docs/developers-guide.md` must name every `cfg(kani)` module. Local runs
  always use the `systemd-run` wrapper from "Kani scope wrapper" in
  `docs/developers-guide.md`.
- Verus is a deductive verifier for Rust-like code. Proofs live under
  `verus/<kernel>/main.rs`, outside Cargo, are run with
  `make verus VERUS_FLAGS='--proof-file verus/<kernel>/main.rs'`, and are not
  part of any routine gate. The pinned version is in `tools/verus/VERSION`.
  `docs/formal-verification-methods-in-netsuke.md` ("Optional Verus proof
  kernel") lists the accepted kernels.

Documentation and skills to consult while implementing:

- `docs/netsuke-design.md` §4.7 "Template Standard Library".
- `docs/terms-of-reference.md` goals G4 "Deterministic plans" and G8
  "Actionable failures", and §2.3 "Trust and purity".
- `docs/developers-guide.md` "Canonical value identity for template helpers"
  (the model for the new re-use policy section), "Executable availability
  predicate" (the `ResolveError` boundary consumers will follow), "Kani scope
  wrapper", and the Kani harness table.
- `docs/rust-testing-with-rstest-fixtures.md` (fixtures and parameterized
  cases), `docs/rust-doctest-dry-guide.md` (the doctest examples required on
  every public function),
  `docs/reliable-testing-in-rust-via-dependency-injection.md` (confirms the
  module needs no injected seam because it reads no ambient state),
  `docs/ortho-config-users-guide.md` (consulted only to confirm the constants
  stay out of configuration), and `docs/rstest-bdd-users-guide.md` (feature
  files, steps, and world slots).
- `docs/formal-verification-methods-in-netsuke.md` and
  `docs/documentation-style-guide.md` (ExecPlan and ADR rules).
- Skills: `execplans`, `rust-router`, `rust-types-and-apis`, `rust-errors`,
  `rust-unit-testing`, `proptest`, `kani`, `verus`, `rust-verification`,
  `hexagonal-architecture`, `codegraph-mcp`, `arch-decision-records`, and
  `en-gb-oxendict-style`.

## Conformance basis

- Baseline: `main` at `da06ffb11d6ccdc30bb4afccbbdbe43adc2207ad`, the
  rebase target. The plan was drafted against the earlier
  `56c58027fcdfc4dd027834f9fc8bb2708898e8ed` and replayed unchanged onto the
  current target.
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
- RFC 0016 §5.8 and RFC 0018 §5.8 (`EP-RQ-STREAM`): match counts checked as
  they grow; fallible match streams.
- RFC 0013, 0014, 0015, 0016, 0018, and 0019 §5.8 (`EP-RQ-OUTPUT`): an 8 MiB
  output-content ceiling counted with checked arithmetic (D-12).
- RFC 0013 §5.8, RFC 0014 §5.8, RFC 0019 §5.8 (`EP-RQ-CONSUMERS`): length and
  depth consumers.
- RFC 0006 §6.9 and roadmap 6.1.4 (`EP-RQ-DIAG`): Fluent keys and codes are out
  of scope here (D-03).
- RFC 0006 §6.3 (`EP-RQ-DET`): purity and determinism.
- `docs/formal-verification-methods-in-netsuke.md` and the Verus and Kani
  policy in `docs/developers-guide.md`: this plan adds a third accepted Verus
  kernel and the first non-IR Kani harnesses.
- No existing ADR governs standard-library resource bounds. ADR-018 governs the
  separate manifest evaluation budget and is not changed.

Trace:

```plaintext
EP-RQ-613 + EP-RQ-BOUNDS + EP-RQ-OVERFLOW + EP-RQ-TWO-CODES
  -> EP-M2 checked kernels + AdmittedCount + CardinalityError
  -> EP-V1, EP-V2, EP-V3, EP-V5 (tests::cardinality, tests::oracle, BDD)
  -> EP-M4 Verus (L1-L9) + Kani (EP-V4)
EP-RQ-613 + EP-RQ-STREAM + EP-RQ-OUTPUT + EP-RQ-CONSUMERS
  -> EP-M3 admit_input, NestingDepth, RunningTotal, collect_matches
  -> EP-V6, EP-V7, EP-V8
EP-RQ-613 (diagnostic) + EP-RQ-DIAG
  -> EP-M3 refusal rendering -> EP-V9 (insta snapshots, BDD)
EP-RQ-DET -> EP-V10 (module review, seeded env import)
All -> EP-M5 ADR, design §4.7, developers' guide re-use policy
```

## Interface and representation

The new module is `src/stdlib/resource_bounds/`. Its public names are
re-exported from `src/stdlib/mod.rs`, matching the canonical value precedent.
This is the complete public surface; anything else stays private to the module.

```rust
/// RFC 0006 table 3 ceilings in scope, plus the output-content ceiling (D-12).
pub const INPUT_LENGTH_CEILING: u64 = 8 * 1024 * 1024;
pub const NESTING_DEPTH_CEILING: u32 = 128;
pub const OUTPUT_TUPLE_CEILING: u64 = 100_000;
pub const PERMUTATION_TUPLE_CEILING: u64 = 10_000;
pub const MATCH_COUNT_CEILING: u64 = 100_000;
pub const OUTPUT_BYTES_CEILING: u64 = 8 * 1024 * 1024;

/// A combinatorial request described by operand lengths, never by output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CardinalityRequest<'a> {
    Product { lengths: &'a [usize], repeat: u64 },
    Combinations { length: usize, r: u64 },
    Permutations { length: usize, r: u64 },
}

impl CardinalityRequest<'_> {
    /// Exact cardinality, or `None` when it exceeds `u64::MAX`.
    pub fn cardinality(&self) -> Option<u64>;
    /// The ceiling this request kind is checked against.
    pub const fn ceiling(&self) -> u64;
    /// Admit the request or refuse it, before any output exists.
    pub fn admit(&self) -> Result<AdmittedCount, CardinalityError>;
}

/// An approved tuple count, at most its ceiling. Not a memory budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedCount(usize);
impl AdmittedCount { pub const fn get(self) -> usize; }

/// Why a combinatorial request was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CardinalityError {
    Exceeded { cardinality: u64, ceiling: u64 },
    Overflow { ceiling: u64 },
}

/// Admit an input whose byte length is checked against `INPUT_LENGTH_CEILING`.
pub fn admit_input(input: impl AsRef<[u8]>) -> Result<(), InputLengthExceeded>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub struct InputLengthExceeded { pub length: u64, pub ceiling: u64 }

/// The number of containers enclosing the current value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NestingDepth(u32);
impl NestingDepth {
    pub const fn root() -> Self;
    pub const fn level(self) -> u32;
    pub fn descend(self) -> Result<Self, DepthExceeded>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub struct DepthExceeded { pub depth: u32, pub ceiling: u32 }

/// A quantity counted as it grows; each kind fixes its own ceiling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamKind { MatchCount, OutputBytes }
impl StreamKind { pub const fn ceiling(self) -> u64; }

/// A checked running total. Deliberately neither `Copy` nor `Clone`.
#[derive(Debug)]
pub struct RunningTotal { kind: StreamKind, admitted: u64 }
impl RunningTotal {
    pub const fn new(kind: StreamKind) -> Self;
    pub fn admit(&mut self, amount: u64) -> Result<(), StreamLimitExceeded>;
    pub fn admit_one(&mut self) -> Result<(), StreamLimitExceeded>;
    pub const fn admitted(&self) -> u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub struct StreamLimitExceeded { pub kind: StreamKind, pub ceiling: u64 }

/// Collect at most `MATCH_COUNT_CEILING` items, pulling at most one more.
pub fn collect_matches<I: IntoIterator>(items: I) -> Result<Vec<I::Item>, StreamLimitExceeded>;

/// As `collect_matches`, stopping at the first item error.
pub fn try_collect_matches<T, E, I>(items: I) -> Result<Vec<T>, E>
where
    I: IntoIterator<Item = Result<T, E>>,
    E: From<StreamLimitExceeded>;
```

The signatures are a specification sketch: doc comments, attributes, and exact
`const` qualifiers are settled during implementation within the interface
tolerance. `CardinalityRequest` borrows a slice of lengths and returns owned
values, so it is not a borrow-centric lookup API and needs no
`docs/polonius.md` entry; the developers' guide section records that
conclusion. Lengths are `usize` because they come from collection sizes; `r` and
`repeat` are `u64` because they come from template integers that the consumer
has already checked are non-negative.

The private arithmetic kernel lives in `src/stdlib/resource_bounds/kernel.rs`.
It is the unit that the Verus model mirrors and Kani drives:

```rust
pub(super) fn checked_product(lengths: &[usize], repeat: u64) -> Option<u64>;
pub(super) fn checked_binomial(n: u64, r: u64) -> Option<u64>;
pub(super) fn checked_falling_factorial(n: u64, r: u64) -> Option<u64>;
pub(super) fn classify(cardinality: Option<u64>, ceiling: u64) -> Result<u64, CardinalityError>;
pub(super) fn admit_total(limit: u64, total: u64, amount: u64) -> Option<u64>;
```

The planned algorithms, stated so that the proof obligations are visible:

1. `checked_product`: if `repeat == 0`, return `Some(1)` (one empty tuple, as
   Python's `itertools.product` gives). Otherwise, if any length is zero, return
   `Some(0)`. Otherwise convert each length with `u64::try_from` (failure
   means overflow), fold with `checked_mul`, and on overflow return `None`.
   Raise the base to `repeat`: a base of 1 gives 1; a base of at least 2 with
   `repeat >= 64` gives `None`; otherwise use `checked_pow` with `repeat`
   converted to `u32`. Work is linear in the number of operands, which the
   consumer's own input bounds.
2. `checked_binomial`: if `r > n`, return `Some(0)`. Let `k = min(r, n - r)`.
   If `k >= 34`, return `None` (L8). Carry `acc: u128 = 1`; for `i` in `0..k`,
   set `acc = acc * (n - i) / (i + 1)` and return `None` as soon as
   `acc > u64::MAX`. Return `acc` as `u64`. At most 33 iterations.
3. `checked_falling_factorial`: if `r > n`, return `Some(0)`. If `r >= 21`,
   return `None` (L9). Carry `acc: u64 = 1`; for `i` in `0..r`, set
   `acc = acc.checked_mul(n - i)?`. At most 20 iterations.
4. `classify`: `None` gives `Overflow`; `Some(v)` with `v > ceiling` gives
   `Exceeded { cardinality: v, .. }`; otherwise `Ok(v)`.
5. `admit_total`: `total.checked_add(amount)` that is at most `limit`, else
   `None`; `RunningTotal::admit` maps `None` to `StreamLimitExceeded` and leaves
   `admitted` unchanged on refusal.

Each refusal renders a fixed English message for developers and logs, for
example:

```plaintext
combinatorial cardinality 100172 exceeds the ceiling of 100000 output tuples
combinatorial cardinality exceeds 18446744073709551615; the ceiling is 100000 output tuples
input length 8388609 bytes exceeds the ceiling of 8388608 bytes
nesting depth 129 exceeds the ceiling of 128 levels
more than 100000 matches; the ceiling is 100000 matches
output exceeds the ceiling of 8388608 bytes
```

Exact wording is fixed by the snapshot tests in EP-M3 and may be polished
there; each message must name the observed value, where one exists, and the
ceiling.

Consumers map refusals to their own module codes. The mapping, recorded in the
developers' guide and written by the consumer tasks, not here, is:

```plaintext
CardinalityError::Exceeded           -> collections::cardinality_exceeded
CardinalityError::Overflow           -> collections::overflow
InputLengthExceeded                  -> interchange::length_exceeded (and the encoding module for b64decode)
DepthExceeded                        -> interchange::depth_exceeded or transform::depth_exceeded
StreamLimitExceeded { MatchCount }   -> pattern::match_limit or path::match_limit
StreamLimitExceeded { OutputBytes }  -> <module>::output_too_large
```

A consumer's own domain error carries the context this module deliberately
omits, such as the helper name and the operand lengths that RFC 0015 requires
in the `product` diagnostic.

## Verification plan

### Behavioural contracts and invariants introduced

1. Exactness: `cardinality()` equals the mathematical cardinality whenever that
   value is at most `u64::MAX`, and is `None` exactly when it exceeds
   `u64::MAX` (EP-V1).
2. Precedence and absorption: overflow precedes the ceiling check; a zero
   length or `r > n` gives zero regardless of other operands (EP-V2).
3. Admission soundness: `admit` returns `AdmittedCount(v)` exactly when the
   cardinality `v` is at most the request kind's ceiling; `get()` never exceeds
   that ceiling (EP-V3).
4. Panic freedom for every input, and work bounded by 33 binomial and 20
   falling-factorial iterations (EP-V4).
5. Rejection before allocation (EP-V5).
6. Depth, running-total, match-collection, and input-length boundaries
   (EP-V6 to EP-V8).
7. Refusal rendering names the observed value and the ceiling (EP-V9).
8. Purity and determinism (EP-V10).

### Proof sketch for the arithmetic contract

This sketch states what must be shown, not how the solver will be driven.
Solver plumbing, auxiliary induction lemmas, trigger management, and
nonlinear-arithmetic helpers are deferred until implementation exposes the real
verification conditions. The lemmas are hypotheses about the proof's structure;
implementation may merge, split, or drop them, and any change is recorded in
`Decision log`. Expect `nonlinear_arith` helper lemmas for the multiplicative
identities; they do not come free.

Abstract model, independent of the implementation. Over unbounded naturals:

- `spec_product(lens, repeat)` is the product of the sequence `lens` raised to
  `repeat`, with the empty product equal to 1.
- `spec_falling(n, r)` is 0 when `r > n`, and otherwise the product of `n - i`
  for `i` in `0..r`.
- `spec_binom(n, r)` is defined by Pascal's rule, deliberately not by the
  multiplicative formula the implementation uses: `binom(n, 0) = 1`;
  `binom(0, r + 1) = 0`; `binom(n + 1, r + 1) = binom(n, r) + binom(n, r + 1)`.
- `MAX` is `2^64 - 1`.

Representation relation: the executable kernels take `u64` (lengths arrive as
`usize` and are converted with `u64::try_from`; on every supported target
`usize` is at most 64 bits) and return `Option<u64>`. `Some(v)` represents the
natural `v`; `None` represents "greater than `MAX`".

Contract theorem, one per kernel. For all `u64` inputs, the kernel returns
`Some(v)` if and only if the specification value equals `v` and `v <= MAX`, and
returns `None` if and only if the specification value exceeds `MAX`. For
`classify` composed with each kernel, the result is `Ok(v)` exactly when the
specification value is at most the ceiling, `Exceeded` exactly when it lies in
`(ceiling, MAX]`, and `Overflow` exactly when it exceeds `MAX`.

Preconditions: none beyond the types. Postconditions: as in the contract
theorem, plus termination. Loop invariants: for the binomial loop,
`acc == spec_binom(n, i)` and `acc <= MAX` at the head of iteration `i`, with
`k <= 33`; for the falling-factorial loop, `acc == spec_falling(n, i)` with
`i <= r <= 20` and `r <= n`; for the product fold, `acc` equals the product of
the prefix, and every remaining factor is at least 1.

Lemma hypotheses:

1. L1, zero absorption: if some factor is 0 and `repeat >= 1`,
   `spec_product` is 0. This is why the zero scan precedes multiplication.
2. L2, monotone prefixes: if every factor is at least 1, every prefix product
   is at most the full product, so the first overflowing prefix proves the full
   product exceeds `MAX`.
3. L3, power bounds: for a base of at least 2 and `repeat >= 64`, the power
   exceeds `MAX`; a base of 1 gives 1; `repeat == 0` gives 1.
4. L4, binomial symmetry and range: `binom(n, r) = binom(n, n - r)` for
   `r <= n`, and `binom(n, r) = 0` for `r > n`. Each needs its own induction
   from Pascal's rule, with care at `r = n`.
5. L5a, absorption: `(k + 1) * binom(n + 1, k + 1) = (n + 1) * binom(n, k)`,
   by induction on `n` for every `k`. The inductive step expands
   `binom(n + 2, k + 1)` with Pascal's rule and applies the hypothesis at
   `k - 1` and `k`; `k = 0` is handled separately. This is the bridge from
   Pascal's rule to the multiplicative step.
6. L5, exact multiplicative step:
   `binom(n, i) * (n - i) = binom(n, i + 1) * (i + 1)`, derived from L5a, so
   the loop's integer division is exact; and when `acc <= MAX` and
   `n - i <= MAX`, `acc * (n - i) <= (2^64 - 1)^2 < 2^128`.
7. L6, central monotonicity: `binom(n, i) <= binom(n, i + 1)` for
   `i < k <= n / 2`, because `n - i >= i + 1`; stopping when `acc > MAX` is
   therefore sound.
8. L7, falling-factorial monotonicity: for `i < r <= n` every factor is at
   least 1, so the first overflowing prefix proves the result exceeds `MAX`.
9. L8, binomial cap: for `34 <= k <= n / 2`,
   `binom(n, k) >= binom(n, 34) >= binom(68, 34) > MAX`, using L6 for
   monotonicity in `k` and Pascal's rule for monotonicity in `n`.
10. L9, falling-factorial cap: for `21 <= r <= n`, `spec_falling(n, r) >= r! >=
    21! > MAX`.

Concrete witnesses such as `binom(68, 34)` cannot be computed by naive
evaluation of the Pascal recursion, which takes about `binom(68, 34)` calls.
Either mark `spec_binom` with `#[verifier::memoize]` and evaluate with
`by (compute_only)`, or derive the witness by chaining the proven L5 step; the
choice is made when the verification condition is seen.

These specifications pressure-tested the interface. They are the reason for
`u64` cardinality with `u128` binomial intermediates (L5); the symmetric
reduction (L4, L6); the structural loop caps (L8, L9); the zero scan before
multiplication (L1); the separate `Overflow` variant (the contract's third
outcome); and a total `repeat == 0` case (L3), with the consumer, not the
kernel, rejecting `repeat < 1` per RFC 0006 §8.3.

Verus is warranted only here, because this arithmetic is contractual business
logic with an all-inputs guarantee that sampling cannot give. Depth, running
totals, and input length are single comparisons or one checked addition, and
diagnostics are formatting; they get conventional tests and Kani, not proof
obligations.

### EP-V1 — exact cardinality

- Obligation: the contract theorem for `checked_product`,
  `checked_binomial`, and `checked_falling_factorial`.
- Method: Verus proof over the model, plus Proptest comparing the production
  kernel with independent oracles, plus named witnesses.
- Rationale: the guarantee is universal and the arithmetic nonlinear, so only
  proof establishes it; the oracles connect the production code to the model's
  semantics on real executions.
- Domain: the model is proved for all `u64` inputs; the production kernel is
  checked by Kani within bounds (EP-V4) and by Proptest here. Proptest classes:
  product with 0 to 6 operands drawn log-uniformly up to `2^20`, with some
  operands biased to 0 and 1, and `repeat` up to 8, against a `u128` checked
  oracle in which any `u128` overflow also means "above `MAX`"; binomial with
  `n <= 130` against an additive Pascal-triangle `u128` oracle
  (`binom(130, 65)` is about `9.51e37`, below `2^128`); binomial with `r <= 2`
  over all `u64` `n` against the closed form `n * (n - 1) / 2` in `u128`, which
  exercises the `u128` intermediate near `u64::MAX`; falling factorial with
  `n <= 40` against a `u128` checked product oracle.
- Artefacts: `verus/resource_bounds/main.rs`;
  `src/stdlib/resource_bounds/tests/oracle.rs`,
  `src/stdlib/resource_bounds/tests/properties.rs`, and
  `src/stdlib/resource_bounds/tests/cardinality.rs` for witnesses.
- Evidence: `cargo nextest run --lib resource_bounds` fails before the kernels
  exist and passes after;
  `make verus VERUS_FLAGS='--proof-file verus/resource_bounds/main.rs'` reports
  every function verified and zero errors.
- Witnesses: `[4294967295, 4294967297]` gives exactly `u64::MAX`;
  `[4294967296, 4294967296]` overflows; `binom(67, 33) = 14226520737620288370`
  fits and `binom(68, 34)` overflows; `P(20, 20) = 2432902008176640000` fits and
  `P(21, 21)` overflows. Confirm each value against the oracle when
  implementing; do not trust the plan's figures alone.
- Non-vacuity: properties run through an explicit `TestRunner` loop with
  per-kernel class counters for zero, admitted, exceeded, and overflow, and the
  test fails if any class is empty. Seeded faults that must be rejected, each
  on its own: the binomial step computed in `u64` (caught by `binom(67, 33)`,
  because `binom(67, 32) * 35` exceeds `u64::MAX`); the symmetric reduction
  removed (caught by `combinations(100000, 99999)`, which would then wrongly
  overflow or hit the cap); dividing before multiplying (gives
  `binom(5, 2) = 8`); a product that saturates instead of returning `None`; a
  binomial cap of 33 instead of 34 (caught by `binom(67, 33)`); a falling cap
  of 20 instead of 21 (caught by `P(20, 20)`). For Verus, show the lemma
  antecedents are inhabited with the witnesses above, and review the trusted
  base: only `vstd`, with no `assume`, `admit`, or `external_body`.

### EP-V2 — overflow precedence and zero absorption

- Obligation: a zero length or `r > n` gives `Some(0)` whatever the other
  operands are; overflow is reported only when the true value exceeds `MAX`.
- Method: named `rstest` witnesses plus L1 in Verus.
- Artefact: `src/stdlib/resource_bounds/tests/cardinality.rs`.
- Witnesses: product `[2^40, 2^40, 0]` gives 0; product `[1]` with repeat
  1,000,000 gives 1; product `[2]` with repeat 63 gives `2^63` (exceeded, not
  overflow); `[2]` with repeat 64 overflows; repeat 0 gives 1, including for
  `[0]`; `combinations(5, 6)` and `permutations(5, 6)` give 0.
- Non-vacuity: the seeded fault "fold left without the zero scan" must fail the
  `[2^40, 2^40, 0]` witness; the seeded fault "repeated multiplication without
  the base-of-1 shortcut" must fail or time out on repeat 1,000,000.

### EP-V3 — admission soundness and ceiling selection

- Obligation: `admit` returns `AdmittedCount(v)` iff the cardinality `v` is at
  most `request.ceiling()`; `Exceeded` carries the exact cardinality and the
  ceiling; permutations use `PERMUTATION_TUPLE_CEILING` and the other kinds use
  `OUTPUT_TUPLE_CEILING`.
- Method: `rstest` boundary table, Proptest over `classify` with arbitrary
  ceilings through the private kernel, and a Kani harness asserting
  `get() <= ceiling` for every admitted result within its bound.
- Artefacts: `src/stdlib/resource_bounds/tests/admission.rs`;
  `src/stdlib/resource_bounds/verification.rs`.
- Witnesses: product `[100000]` admitted; `[100001]` refused with cardinality
  100001 (RFC 0015's `range(100001) | product` example); `[316, 317]` refused
  with 100172; `combinations(100000, 99999)` admitted with 100000;
  `combinations(100001, 1)` refused; `permutations(10000, 1)` admitted;
  `permutations(10001, 1)` refused; `permutations(8, 8)` refused with 40320 and
  ceiling 10000.
- Non-vacuity: the seeded fault "permutations checked against
  `OUTPUT_TUPLE_CEILING`" must fail the `permutations(8, 8)` witness; the
  seeded fault `<` for `<=` must fail the `[100000]` witness.

### EP-V4 — production panic freedom and bounded work

- Obligation: no production kernel panics (arithmetic overflow, division by
  zero, failed conversion) for any input; the binomial loop runs at most 33
  iterations and the falling-factorial loop at most 20, by construction (D-15).
- Method: Kani harnesses over the production kernel with overflow checks
  enabled, plus unit witnesses at each cap. Bounded work rests on the
  structural caps, lemmas L8 and L9, and the witnesses; there is no symbolic
  iteration-count harness, because 33 symbolic `u128` steps would not finish.
- Harnesses and bounds (names are provisional; each gets a mutation patch):
  1. `product_kernel_respects_zero_and_repeat`: up to three symbolic `usize`
     lengths and a symbolic `u64` `repeat`; asserts `repeat == 0` gives
     `Some(1)` and a zero length otherwise gives `Some(0)`.
  2. `binomial_kernel_matches_closed_form_for_r_up_to_two`: symbolic `u64` `n`,
     `r <= 2`, unwind 3; asserts agreement with `n * (n - 1) / 2` computed in
     `u128` and classified against `MAX`.
  3. `binomial_kernel_never_panics_for_small_r`: `n <= 2^40`, `r <= 4`,
     unwind 5.
  4. `falling_factorial_kernel_matches_checked_product`: symbolic `u64` `n`,
     `r <= 4`, unwind 5; asserts agreement with a checked `u128` product.
  5. `admission_never_exceeds_ceiling`: symbolic `Option<u64>` and each
     ceiling through `classify`; `Ok(v)` implies `v <= ceiling`, and `Exceeded`
     implies `cardinality > ceiling`.
  6. `running_total_never_admits_past_its_limit`: three symbolic amounts
     against a small limit through the `#[cfg(any(test, kani))]` constructor.
- Covers: `kani::cover!` must reach `r > n` giving 0; an admitted case such as
  `(100000, 1)`; an exceeded case such as `(100001, 1)`; overflow with
  `n >= 2^33` and `r = 2`; and an intermediate above `u64::MAX` whose result
  fits, such as `n = 5_000_000_000` with `r = 2`. That last cover is the only
  one that distinguishes the `u128` step from the seeded `u64` fault.
- Artefacts: `src/stdlib/resource_bounds/verification.rs`, wired by
  `#[cfg(kani)] mod verification;` in `src/stdlib/resource_bounds/mod.rs`; one
  patch per harness in `docs/verification/mutations/`; rows in the Kani harness
  table in `docs/developers-guide.md` with measured times.
- Evidence: inside the Kani scope wrapper,
  `make kani-full KANI_FLAGS='--harness stdlib::resource_bounds::verification'`
  reports `VERIFICATION:- SUCCESSFUL` for each harness and every cover
  satisfied; `make test-kani-mutations` shows each patch makes its harness
  fail; one full `make kani-ir` inside the wrapper passes before the pull
  request is marked ready.
- Witnesses: `checked_binomial(u64::MAX, u64::MAX / 2)` exits at the cap check
  and `checked_falling_factorial(10^18, 10^18)` likewise;
  `checked_binomial(u64::MAX, 33)` and `checked_falling_factorial(10^18, 20)`
  overflow inside the loop; `(67, 33)` fits; `(68, 34)` is refused by the cap.
- Non-vacuity: an unreached cover fails the milestone; each mutation patch must
  produce a failing harness for the intended reason.

### EP-V5 — rejection before allocation

- Obligation: no output is allocated before admission, and constructing a
  refusal performs no allocation.
- Method: a structural argument backed by tests. `CardinalityRequest` contains
  only lengths, so the helper has no output to allocate. Every refusal type is
  `Copy` and holds only integers and enumerations; a compile-time `Copy`
  assertion pins that. `AdmittedCount` has a private field, so `admit` is the
  only way to obtain an approved count. Witnesses request cardinalities that no
  machine could materialize, such as
  `combinations(usize::MAX, usize::MAX / 2)`, and observe a prompt typed
  refusal; an implementation that allocated first would abort.
- Rationale: a counting global allocator would be stronger evidence, but it
  needs `unsafe` (forbidden) or a new dev-dependency (outside tolerance).
- Artefact: `src/stdlib/resource_bounds/tests/admission.rs`.
- Non-vacuity: the unmaterializable witnesses fail by aborting if output is
  built before the check.

### EP-V6 — nesting depth boundary

- Obligation: from `NestingDepth::root()`, 128 successive `descend` calls
  succeed and the 129th returns `DepthExceeded { depth: 129, ceiling: 128 }`;
  `level()` reports the depth reached.
- Method: `rstest` boundary cases at 0, 1, 127, 128, and 129, plus a Proptest
  property that `d` descents succeed iff `d <= 128`.
- Artefact: `src/stdlib/resource_bounds/tests/limits.rs`.
- Non-vacuity: the seeded fault `<` for `<=` must fail at 128.

### EP-V7 — running totals and match collection

- Obligation: `RunningTotal::admit` accepts amounts while the checked total
  stays at most the kind's ceiling, refuses otherwise without changing
  `admitted()`, and treats a checked-add overflow as a refusal.
  `collect_matches` returns every item when there are at most
  `MATCH_COUNT_CEILING` of them, otherwise `StreamLimitExceeded`, and pulls at
  most `MATCH_COUNT_CEILING + 1` items. `try_collect_matches` additionally
  returns the first item error unchanged and pulls nothing after it.
- Method: Proptest over amount sequences against `admit_total` with small
  limits; a counting iterator wrapper (an `rstest` fixture) around streams near
  the ceiling; named witnesses: an infinite iterator under `collect_matches`
  refuses after pulling exactly 100,001 items; an error at item 5 is returned
  before the ceiling with exactly 5 items pulled; amounts `[u64::MAX, 1]`
  refuse on overflow; one `RunningTotal` driven across three simulated
  documents trips the ceiling on the third.
- Artefact: `src/stdlib/resource_bounds/tests/limits.rs`.
- Non-vacuity: the infinite-iterator witness cannot pass for an implementation
  that collects first; the seeded fault "pull one item fewer" must fail the
  exact-ceiling case; the seeded fault "wrapping add" must fail the overflow
  witness.

### EP-V8 — input length boundary

- Obligation: an input of `n` bytes is admitted iff `n <= 8_388_608`; the
  refusal carries the exact byte length.
- Method: `rstest` cases through the private length function at 0, 8,388,607,
  8,388,608, 8,388,609, and `usize::MAX`, plus one case through `admit_input`
  with a multibyte string whose character count and byte count differ.
- Artefact: `src/stdlib/resource_bounds/tests/limits.rs`.
- Non-vacuity: the seeded fault `<` for `<=` must fail at 8,388,608; a seeded
  `chars().count()` must fail the multibyte case.

### EP-V9 — refusal rendering

- Obligation: every refusal that can arise renders a message naming the
  observed value, where one exists, and the ceiling, with exact integer
  formatting up to `u64::MAX`.
- Method: `insta` snapshots, one per refusal shape and stream kind, under
  `src/snapshots/resource_bounds/` via `snapshot_settings("resource_bounds")`,
  kept beside typed `googletest` matcher assertions on the fields so that a
  snapshot cannot pass for the wrong variant. Behavioural scenarios assert the
  cardinality and ceiling through the typed fields.
- Artefacts: `src/stdlib/resource_bounds/tests/diagnostics.rs`;
  `tests/features/resource_bounds.feature`;
  `tests/bdd/steps/stdlib/resource_bounds.rs`.
- Non-vacuity: a rendering that drops either number changes a snapshot.

### EP-V10 — purity and determinism

- Obligation: the module performs no I/O and reads no ambient state; repeated
  calls with the same arguments return equal results.
- Method: review of the module's `use` list (no `minijinja`, `std::env`,
  `std::fs`, `cap_std`, time, or randomness), a repeat-call Proptest assertion
  on the kernels, and the existing environment-access contract in
  `tests/env_access_suppressions/`, which scans the new files automatically.
- Artefact: `src/stdlib/resource_bounds/tests/properties.rs`.
- Non-vacuity: a seeded `std::env::var` call in the module must be rejected by
  the environment-access contract.

### External assumptions and residual gaps

- Axioms: Rust's documented semantics for `checked_mul`, `checked_add`,
  `checked_pow`, `u64::try_from`, and `u128` arithmetic; `usize` is at most 64
  bits on every supported target; Verus and `vstd` are sound for the constructs
  used, and `vstd`'s specifications of the checked operations are faithful;
  Kani's documented bounded semantics with overflow checks enabled.
- Residual gap: the Verus proof covers a transcription of the production
  kernel, not the compiled kernel. It is narrowed by token-for-token
  correspondence, by Kani on the production kernel within bounds, and by
  Proptest against independent oracles. The proof file's module comment must
  state the gap, as `verus/canonical_value/main.rs` does.
- No end-to-end or command-line test is added: this task registers no template
  helper and changes no command-line behaviour. The consumer tasks (6.2.1,
  6.4.2, 6.5.3, and others) add end-to-end tests through manifests.

## Plan of work

### Stage A — understand and characterize (no production code)

Confirm the toolchain: `make check-build-tools`, the pinned Verus install
(`make install-verus` if `.verus/` is absent), and the Kani front end
(`make kani-check`). Record versions in `Progress`. Re-read RFC 0015 §5.8, RFC
0016 §5.8, and RFC 0018 §5.8 to confirm nothing conflicts with the interface.
Confirm that `snapshot_settings` accepts a new subject name without
configuration, and check whether `vstd` specifies `checked_mul` and
`checked_pow`; if `checked_pow` is unspecified, keep the power step in the
model and discharge it with L3. Go/no-go: the toolchains work and no conflict
is found; otherwise record the issue and escalate.

### Stage B — red tests per milestone

At the start of EP-M2 and again at the start of EP-M3, write that milestone's
tests first (D-17). The module skeleton declares the public types with their
real signatures, and kernel bodies return a deliberately wrong value (`None`,
`Ok(0)`, or an always-admitting total), not `todo!()`, so the suite compiles
and fails for the expected reason. Record each red run's failing test names and
messages in `Progress`, and confirm each focused filter selects a nonzero
number of tests. Do not commit a red tree.

Test style follows `docs/developers-guide.md` and the 6.1.2 precedent: `rstest`
fixtures for shared setup such as the counting iterator; parameterized
`#[rstest]` cases for boundary tables; `googletest` matchers for typed refusal
fields, with `#[googletest::test]` placed before `#[rstest]`;
`pretty_assertions::assert_eq` for collected vectors; `.expect(...)` rather than
`.unwrap()`; and fallible fixtures consumed by tests that return `Result`.

The behavioural specification, `tests/features/resource_bounds.feature`. The
first three scenarios are added in EP-M2 and the last two in EP-M3:

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
```

Steps live in `tests/bdd/steps/stdlib/resource_bounds.rs` and are declared in
`tests/bdd/steps/stdlib/mod.rs`; `tests/bdd_tests.rs` collects the feature file
through its existing `scenarios!` call. The steps call the public API in
process, as the canonical value steps do. New `TestWorld` slots in
`tests/bdd/fixtures/mod.rs` hold the last request's operand lengths, the last
admission result, the depth, and the pulled-item count; each scenario starts
from empty slots.

### Stage C — implementation and verification together

Implement `kernel.rs` first, mirroring the algorithms in
`Interface and representation`, then `cardinality.rs` (`CardinalityRequest`,
`AdmittedCount`, `CardinalityError`), and in EP-M3 `limits.rs` (`admit_input`,
`NestingDepth`, `DepthExceeded`, `InputLengthExceeded`) and `streams.rs`
(`StreamKind`, `RunningTotal`, `StreamLimitExceeded`, `collect_matches`,
`try_collect_matches`). Constants live in `mod.rs`. Re-export the public names
from `src/stdlib/mod.rs`. Make the red tests pass one obligation at a time. In
EP-M4, write `verus/resource_bounds/main.rs` from the proof sketch and the Kani
harnesses in `verification.rs`. If the proof or a harness shows the algorithm
is wrong, fix the algorithm and its model together, then record the change in
`Decision log` and `Verification plan`.

### Stage D — refactor, documentation, and wider validation

After each functional milestone commit, review the changed code against the
AGENTS.md refactoring heuristics. Any refactor is its own atomic commit with
the same gates, never folded into a functional or documentation commit. Run the
seeded-fault controls listed in each obligation by temporarily editing,
observing the failure, and reverting before committing; record each outcome in
`Progress` and confirm `git diff` shows no production change afterwards. Kani
seeded faults are the committed mutation patches instead. Then write the
documentation in EP-M5 and run the full gates.

## Milestones and plateaus

Each milestone ends in a committed, gated, coherent repository state. The plan
file is updated at every milestone, so each milestone's gates include the
Markdown gates.

The standard gate set used below runs sequentially, each tee'd to `/tmp`:
`make check-fmt`, `make lint`, `make test`, `make doc-coverage`,
`make test-workflow-contracts`, and `make markdownlint`. Format touched
Markdown with
`mdtablefix --wrap --renumber --breaks --ellipsis --fences --in-place <file>`
(run twice) rather than `make fmt`, which also reformats unrelated files.

### EP-M1 — characterized toolchain

Assigned: Stage A. End state: no production change; `Progress` records
toolchain versions, the `vstd` findings, and the status change to
`IN PROGRESS`. Acceptance: the plan update passes `make check-fmt` and
`make markdownlint`. Conformance check: the interface still matches this plan.
Recovery: revert the plan commit. Remaining gaps: everything else.

### EP-M2 — checked cardinality kernels and admission

Assigned: `EP-RQ-613` (cardinality), `EP-RQ-OVERFLOW`, `EP-RQ-TWO-CODES`. End
state: `kernel.rs`, `cardinality.rs`, the constants, the re-exports, the
doctests, and the first three behavioural scenarios exist; EP-V1 (tests),
EP-V2, EP-V3, and EP-V5 pass. Update `tools/kani/proof-scope.toml` by deriving
it from source when `make test-workflow-contracts` reports the new files,
excluding the `tests/` subtree. Acceptance: the standard gate set, plus the
focused runs in `Concrete steps`. Conformance check: no `minijinja` import in
the module; every refusal is `Copy`. Recovery: revert the milestone commit.
Remaining gaps: limits, streams, rendering, proofs, documentation.

### EP-M3 — input length, nesting depth, running totals, and refusal rendering

Assigned: `EP-RQ-613` (length, depth, match counts), `EP-RQ-STREAM`,
`EP-RQ-OUTPUT`, `EP-RQ-CONSUMERS`, and the `EP-RQ-DIAG` boundary. End state:
`limits.rs`, `streams.rs`, the diagnostics snapshots, the remaining behavioural
scenarios, and their `TestWorld` slots exist; EP-V6 to EP-V10 pass. Acceptance:
the standard gate set. Conformance check: no Fluent key or diagnostic code was
added; `RunningTotal` is neither `Copy` nor `Clone`. Recovery: revert the
milestone commit. Remaining gaps: proofs and documentation.

### EP-M4 — Verus proof and Kani harnesses

Assigned: the proof sketch (L1 to L9) and EP-V4. End state:
`verus/resource_bounds/main.rs` verifies with no `assume`, `admit`, or
`external_body`; the Kani harnesses pass with every cover reached; each harness
has a committed mutation patch; `src/stdlib/resource_bounds/verification.rs` is
added to the `exclude-globs` input in `.github/workflows/mutation-testing.yml`,
to `KANI_EXCLUDES` in `tests/workflow_contracts/mutation_testing_test.py`, and
to the `cargo-mutants` exclusion list in `docs/developers-guide.md`; the
remaining seeded faults from EP-V1 and EP-V2 are observed and reverted.
Acceptance:

```sh
make verus VERUS_FLAGS='--proof-file verus/resource_bounds/main.rs' 2>&1 \
  | tee /tmp/verus-netsuke-6-1-3-shared-bounded-materialization-helper.out
```

Then, inside the Kani scope wrapper (the `systemd-run --user --scope` command
in "Kani scope wrapper" in `docs/developers-guide.md`, with its `bash -c`
payload replaced as below):

```sh
env LD_LIBRARY_PATH="$HOME/.kani/kani-0.67.0/toolchain/lib:$HOME/.kani/kani-0.67.0/lib" \
  make kani-full KANI_FLAGS='--harness stdlib::resource_bounds::verification' 2>&1 \
  | tee /tmp/kani-netsuke-6-1-3-shared-bounded-materialization-helper.out
```

Then `make test-kani-mutations`, also inside the wrapper because it performs a
Kani code generation per patch, and the standard gate set, which now includes
`make lint`'s workflow linting of the edited mutation workflow. The Verus run
reports zero errors; the Kani run reports success for every new harness with
measured times under 60 seconds each. Conformance check: the proof's module
comment states the transcription gap; the trusted base is only `vstd`.
Recovery: revert the milestone commit; if a required lemma cannot be proved
within tolerance, escalate rather than weaken it. Remaining gaps: documentation
and the one full `make kani-ir` run.

### EP-M5 — documentation, ADR, roadmap, and final gates

Assigned: the documentation obligations in AGENTS.md. End state:

1. An ADR, provisionally
   `docs/adr-057-shared-resource-bounds-for-stdlib-helpers.md`, following the
   documentation style guide's ADR format with `Status: Accepted`, the date,
   and a one-line summary. Sections: context and problem statement; decision
   drivers; options considered, with a captioned comparison table (per-group
   bounds, one `BoundError` enum, precise per-API refusals, a separate crate);
   decision outcome; consequences; and known risks (the `serde_json`
   off-by-one, the output-content row, the diagnostic hand-off to 6.1.4 and
   6.4.2). It records the RFC 0006 table 3 output-content row, which item 8
   adds. Before creating it, re-derive the next free ADR number and take a
   number above the highest allocation on any branch:

   ```sh
   git fetch --all --prune -q
   for ref in $(git for-each-ref --format='%(refname)' refs/remotes/origin); do
     git ls-tree -r --name-only "$ref" -- docs/ 2>/dev/null \
       | grep -oE 'docs/adr-[0-9]{3}-' | sed 's|docs/adr-||; s|-$||' \
       | sort -u | while read -r n; do echo "$n ${ref#refs/remotes/origin/}"; done
   done | sort -u | awk '$1 >= "050"'
   ```

   Index the ADR in `docs/contents.md`, then confirm every ADR index label
   matches its filename by set comparison, not by count.
2. `docs/netsuke-design.md` §4.7: a "Resource bounds" subsection stating the
   ceilings, the exactness and precedence rules, the ownership boundary, and
   the reference to the ADR.
3. `docs/developers-guide.md`: a "Resource bounds for template helpers"
   section after "Canonical value identity for template helpers", giving the
   public surface and a **Re-use policy**. The policy states: every bounded
   template helper in `src/stdlib/` calls this module; no helper re-implements
   the arithmetic or invents a ceiling; I/O byte limits, redirect limits, and
   the ADR-018 manifest budget are out of scope; check the input length before
   parsing and the depth before each recursive step; run the output-content
   check after cardinality admission; `AdmittedCount` is a tuple count, not a
   memory budget; `RunningTotal` is shared by reference across a whole stream;
   only this module constructs refusals; consumers map refusals with the
   mapping table above and wrap them with the helper name and operands;
   `Display` text is never forwarded into `minijinja::Error`; and the Fluent
   `f64` hazard for counts above `2^53`. The section also records the depth
   definition and the `serde_json` difference, the canonical-value follow-up
   candidate, and the Polonius classification note. In the same guide, add the
   Kani harness rows with measured times, retitle the harness table and amend
   the sentence "all Kani harnesses are IR harnesses" so both stay true, add
   the new module to the `cargo-mutants` exclusion paragraph if EP-M4 did not,
   and amend the Verus policy paragraph to list the third accepted kernel.
4. `docs/formal-verification-methods-in-netsuke.md` "Optional Verus proof
   kernel" and "Current repository state": add the resource-bounds kernel.
5. `docs/repository-layout.md`: add `src/stdlib/canonical_value/` (missing
   since 6.1.2), `src/stdlib/resource_bounds/`, and `verus/resource_bounds/`.
6. `docs/users-guide.md` and `docs/stdlib-yaml-and-jinja-guide.md`: no change,
   because no user-visible behaviour changes; record that conclusion in
   `Outcomes & retrospective`.
7. `docs/roadmap.md`: mark 6.1.3 done (`- [x]`), keeping the text shape that
   `tests/rfc_stdlib_coverage/roadmap.rs` parses.
8. RFC 0006 table 3: add the output-content row for the 8 MiB ceiling, under
   the user's approval to amend referenced RFCs, and note the row in the ADR.
   Keep the table's existing column shape so the documentation gates and the
   RFC coverage tests still pass.

Acceptance: the standard gate set plus `make nixie` and
`make test-rfc-stdlib-coverage`, then one full `make kani-ir` inside the Kani
scope wrapper. Conformance check: every `Conformance basis` item is traced to
evidence; set `Status: COMPLETE` only after reconciling discoveries with the
upstream documents. Recovery: revert the documentation commit; the code
milestones stand on their own. Remaining gaps: the consumer tasks 6.1.4, 6.2.1,
6.4.2, 6.5.3, and the RFC owner's decision on the table 3 row.

## Concrete steps

Run every command from the worktree root and capture long output with `tee`:

```sh
make check-fmt 2>&1 | tee /tmp/check-fmt-netsuke-6-1-3-shared-bounded-materialization-helper.out
make lint 2>&1 | tee /tmp/lint-netsuke-6-1-3-shared-bounded-materialization-helper.out
make test 2>&1 | tee /tmp/test-netsuke-6-1-3-shared-bounded-materialization-helper.out
make doc-coverage 2>&1 | tee /tmp/doc-coverage-netsuke-6-1-3-shared-bounded-materialization-helper.out
make test-workflow-contracts 2>&1 \
  | tee /tmp/workflow-contracts-netsuke-6-1-3-shared-bounded-materialization-helper.out
make markdownlint 2>&1 | tee /tmp/markdownlint-netsuke-6-1-3-shared-bounded-materialization-helper.out
```

Run gates sequentially, never in parallel. Delegating full gate runs to the
`scrutineer` agent keeps bulky logs out of the planning context but is
optional. A build waiting on `build-limits` admission is queued, not hung. For
the focused red-green loop, including doctests, which nextest does not run:

```sh
cargo nextest run --lib resource_bounds 2>&1 \
  | tee /tmp/focused-netsuke-6-1-3-shared-bounded-materialization-helper.out
cargo nextest run --test bdd_tests resource_bounds 2>&1 \
  | tee /tmp/bdd-netsuke-6-1-3-shared-bounded-materialization-helper.out
cargo test --doc --all-features resource_bounds 2>&1 \
  | tee /tmp/doctest-netsuke-6-1-3-shared-bounded-materialization-helper.out
```

Each focused run must report a nonzero number of selected tests. Commit after
each green milestone with an imperative subject, for example
`Add checked cardinality kernels for stdlib bounds (6.1.3)`. Each step is
re-runnable: tests and proofs have no side effects beyond `target/` and `/tmp`
logs.

## Validation and acceptance

The feature is accepted when all of the following hold:

1. `cargo nextest run --lib resource_bounds` passes, and the Proptest class
   counters show every class reached for every kernel.
2. `cargo nextest run --test bdd_tests resource_bounds` passes all five
   scenarios, each of which failed before its milestone's implementation.
3. `cargo test --doc --all-features resource_bounds` passes the doctest on
   every public function.
4. The `insta` snapshots render, among others:

   ```plaintext
   combinatorial cardinality 100172 exceeds the ceiling of 100000 output tuples
   ```

5. `make verus VERUS_FLAGS='--proof-file verus/resource_bounds/main.rs'` ends
   with a summary reporting zero errors.
6. The wrapped `make kani-full` run reports success for each new harness with
   every cover reached, `make test-kani-mutations` passes, and one full wrapped
   `make kani-ir` passes.
7. `make check-fmt`, `make lint`, `make test`, `make doc-coverage`,
   `make markdownlint`, `make nixie`, `make test-workflow-contracts`, and
   `make test-rfc-stdlib-coverage` pass on the final commit.
8. `docs/roadmap.md` shows 6.1.3 as done, and the ADR is indexed.

## Idempotence and recovery

All steps are additive. To abandon a milestone, `git revert` its commit. The
seeded-fault controls are temporary edits that must be reverted before
committing; confirm `git diff` is empty for production files after each one. If
the ADR number collides with another branch after pushing, renumber this
branch's ADR in a separate commit, updating its heading, its index entry, and
every reference, then re-check every ADR index label against its filename.

## Artefacts and notes

Planning-time evidence for the `serde_json` depth semantics:

```plaintext
serde_json-1.0.154/src/de.rs:63:            remaining_depth: 128,
serde_json-1.0.154/src/de.rs:1375:            $this.remaining_depth -= 1;
serde_json-1.0.154/src/de.rs:1376:            if $this.remaining_depth == 0 {
```

## Revision note

2026-10-17, initial draft: written from Wyvern reconnaissance of the stdlib,
the verification infrastructure, and the child RFCs.

2026-11-01, approved. The user approved the plan and accepted D-03 and D-12.
D-03 leaves refusals typed and English-only, with Fluent keys and
`netsuke::jinja::` codes deferred to 6.1.4 and the consuming modules. D-12
keeps the 8 MiB output-content ceiling and resolves the table 3 question by
adding the row to RFC 0006, which the user's approval of necessary RFC
amendments permits. ADR-057 remains provisional and is re-derived across every
branch immediately before it is committed. The series was rebased onto `main` at
`da06ffb11` without conflict.

2026-10-17, expert-panel revision. What changed:

- A single `BoundError` and a public `Ceiling` type became precise per-API
  refusal types (D-09), and ceilings became fixed per kind (D-05), so every
  consumer mapping is total and wrong pairings cannot be built.
- `RunningCount` became `RunningTotal` with a checked `admit(amount)`, neither
  `Copy` nor `Clone` (D-13). `try_collect_matches` was added for fallible
  streams (D-14), with RFC 0018 added to the consumers.
- `StreamKind::OutputBytes` was added, flagged for approval (D-12).
- The binomial and falling-factorial loops gained structural caps of 33 and 20
  iterations (D-15, L8, L9). The false "about 65 iterations" claim and the
  intractable symbolic iteration-count harness were removed. The proof sketch
  gained the absorption lemma L5a and a note on computing witnesses.
- The Kani plan adopted the mandatory scope wrapper, a harness filter,
  realistic bounds, a cover that separates the `u128` step from the `u64`
  fault, committed mutation patches, and the `cargo-mutants` exclusion lists.
- Red tests moved to the start of each milestone (D-17); doctest examples, the
  helper sweep, the re-use policy's call sites and composition rules, the
  `TestWorld` wiring, non-vacuity checks for EP-V8 and EP-V10, separate
  refactor commits, the ADR's status and sections, and the missing EP-M5
  recovery fields were added. The behavioural feature was trimmed from seven to
  five scenarios.

Why: the expert panel found that the draft's API would need a redesign for
fallible match streams and output-byte totals, that its Kani and red-test plans
would have made milestone commits impossible, and that one bounded-work claim
was false. Effect on remaining work: the interface is larger by four small
types but every consumer mapping is total; EP-M4 carries more repository
integration (patches, exclusion lists); the tolerances were widened to 40 files
and 2,000 lines to fit.

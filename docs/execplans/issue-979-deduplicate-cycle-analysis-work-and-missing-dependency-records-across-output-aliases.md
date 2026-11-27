# Deduplicate cycle-analysis work and missing-dependency records across output aliases (#979)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: COMPLETE

Branch:
`issue-979-deduplicate-cycle-analysis-work-and-missing-dependency-records-across-output-aliases`

Roadmap item: none. The work closes security finding
`3fe32c4b-0d10-48cb-8411-4f0b5b81cc59` from issue
[#979](https://github.com/leynos/netsuke/issues/979).

## Purpose / big picture

A Netsuke target may declare several output names, called *aliases*, for one
canonical build edge. Cycle detection currently keys its depth-first traversal
by alias path, so an edge with `k` aliases is expanded `k` times and its `d`
dependency entries are scanned `k × d` times. Each absent dependency is also
copied into the diagnostic report once per alias, so a compact manifest can
produce an allocation proportional to the product of its alias and dependency
counts.

After this change, cycle detection keys traversal by the canonical `EdgeId`.
Each edge is expanded exactly once per analysis pass, regardless of how many
aliases index it. Missing dependencies are recorded once per examined
dependency entry, under the alias by which the edge was first entered, so the
report stays proportional to the declared dependency entries. Cycle witnesses
still name real aliases, so existing single-output diagnostics and user-facing
error text are unchanged.

A manifest such as one target named `[out0, out1, out2]` with absent `sources`
`[ext0, ext1, ext2]` currently retains nine `(alias, dependency)` pairs; after
the change it retains three, all naming the entry alias `out0`. A user running
`netsuke` observes the same commands and the same localized cycle error text.

## Constraints

- Preserve the public API of `BuildGraph`, `CycleDetectionReport`, and
  `IrGenError::CircularDependency`; no new public items are required.
- Preserve the `Display` text and the fields of
  `IrGenError::CircularDependency`.
- Preserve exact single-output cycle witnesses already asserted in
  `src/ir/cycle/tests.rs`, `src/ir/cycle/participation_tests.rs`, and
  `src/ir/graph/property_tests/cycles.rs`.
- Preserve traversal of `inputs`, `implicit_deps`, and `order_only_deps`.
  Order-only dependencies participate by decision
  [ADR-0068](../../docs/adrs/adr-0068-detect-cycles-across-all-prerequisite-kinds.md)
  and by the live Kani harness `order_only_two_node_cycle_reports_cycle`; the
  supplied coding plan's line "Exclude `order_only_deps`" is not actioned
  because it contradicts that accepted decision and a live proof.
- Keep every source file at or below 400 lines, per `AGENTS.md`.
- No process-wide environment mutation in tests; no new dependencies.
- Do not modify `docs/users-guide.md`; the change is not user-facing.
- Kani harness claims and `tools/kani/proof-scope.toml` stay valid; mutation
  patches under `docs/verification/mutations/` must apply and compile.
- Historical ExecPlans and completed records are not edited.

## Tolerances (exception triggers)

Comparison base is `git diff --numstat` against commit `56d1e0a2`. ExecPlan
files are excluded from every count.

- Files: stop if more than 16 non-ExecPlan files change.
- Code: stop if net code changes exceed 450 lines across production and tests.
- Configuration: stop if configuration changes exceed 20 net lines.
- Documentation: stop if non-ExecPlan documentation changes exceed 120 net
  lines.
- Interface: stop if a public API change beyond the constraints is required.
- Dependencies: stop if a new dependency is required.
- Architecture: stop if ADR-0068 must be reversed.
- Iterations: stop after three failed focused attempts at one defect.
- Ambiguity: stop if alias-entry policy materially changes diagnostics and the
  choice is not settled by the supplied plan.

Budget rationale: comparable cycle work (4.4.2, commit `a6009675e`) touched the
detector, tests, and two documentation sections. The supplied coding plan names
four production modules, one integration test module, a new property module,
and two documentation sections, so 16 files and 450 net code lines allow the
planned tests without inviting scope growth. No comparable plan repeatedly
raised these budgets.

## Risks

- Risk: alias-keyed diagnostics change for multi-alias self-cycles.
  Severity: low Likelihood: medium Mitigation: exact regression tests for the
  entry-alias policy; the accepted plan expects the requested alias in the
  witness.
- Risk: Kani's bounded `IrHashMap` gains a new key type and loses capacity.
  Severity: medium Likelihood: low Mitigation: `EdgeId` keys are fewer than
  path keys; run `make kani-check` and `make kani-full`.
- Risk: mutation patches stop applying after the detector is restructured.
  Severity: medium Likelihood: high Mitigation: regenerate affected patches and
  run `make test-kani-mutations`.
- Risk: property tests over generated graphs expose a witness that is not a real
  alias-level path. Severity: medium Likelihood: medium Mitigation: assert
  every consecutive witness pair against the producer's dependency lists.

## Progress

- [x] (2026-11-25 21:30Z) Reconnaissance of the detector, Kani harnesses,
  mutation patches, and documentation completed.
- [x] (2026-11-25 21:35Z) Branch and Lody session renamed; ExecPlan drafted.
- [x] (2026-11-25 22:55Z) `EP-M1`: Red regression tests and scan accounting
  fail on the previous detector. Measured 16 records instead of 4, 6 scans
  instead of 2, and the duplicate entry-alias record.
- [x] (2026-11-25 23:20Z) `EP-M2`: Edge-keyed traversal and bounded missing
  records make all 55 focused cycle tests green. The library check passes under
  `-D warnings`.
- [x] (2026-11-25 23:45Z) Property and manifest-lowering integration tests are
  green. The property fails on the pre-fix detector with "5 missing records
  exceed 3 declared entries" and "7 dependency scans exceed 3 declared
  entries", proving the bound is non-vacuous.
- [x] (2026-11-26 05:20Z) `EP-M3`: Documentation and mutation evidence are
  complete, and all nine gates pass. `make check-fmt`, `make markdownlint`,
  `make doc-coverage` (98.38%), `make test` (6671/6671), `make lint`,
  `make test-workflow-contracts`, `make kani-check`, and `make kani-full`
  (17/17) passed on commit `16567c54b`. `make test-kani-mutations` timed out
  its cold prewarm at 900.032s on the first attempt; a warm retry passed the
  prewarm (671.9s), the negative control, and every patched-tree compile
  (1228.6s).
- [x] (2026-11-26 06:00Z) `EP-M4`: Branch pushed over SSH and draft pull
  request [#1198](https://github.com/leynos/netsuke/pull/1198) opened against
  `main`. CodeRabbit's final review reports zero findings.

## Surprises & discoveries

- Observation: the supplied coding plan says to exclude `order_only_deps`, but
  ADR-0068 and the Kani harness `order_only_two_node_cycle_reports_cycle`
  require them to participate. Evidence:
  `docs/adrs/adr-0068-detect-cycles-across-all-prerequisite-kinds.md`,
  `src/ir/cycle/verification.rs:75`, and the matching mutation patch. Impact:
  the plan is followed except for that clause; exclusion would reverse an
  accepted decision and break a live proof.
- Observation: the plan cites a `nightly-2026-10-22` pin, but the repository
  pins `nightly-2026-11-10`. Evidence: `rust-toolchain.toml`. Impact: the pull
  request records the actual pin.
- Observation: the first `make test-kani-mutations` run timed out its cold
  prewarm at 900.032s without a compiler diagnostic. Evidence:
  `/tmp/test-kani-mutations-issue-979-…out` and the 536M shared
  `target/kani-mutation-compile` tree it left behind. Impact: the gate allows
  900s for a cold prewarm; the warm retry passed the prewarm in 671.9s and the
  patch sweep in 1228.6s. Treat the first timeout as host-capacity latency, not
  a code defect.

## Decision log

- D1. Decision: key traversal state by `EdgeId` while keeping alias paths in
  stack frames and witnesses. Rationale: an edge owns one dependency list, so
  expansion belongs to the edge; aliases are needed only for diagnostics. The
  rejected alternative — memoizing per alias — leaves the multiplicative scan
  and record growth. Date/Author: 2026-11-25, implementing agent.
- D2. Decision: record a missing dependency under the alias by which its edge
  was entered, once per examined dependency entry. Rationale: the entry alias
  is the alias whose expansion actually examined the entry, and sorted root
  enumeration makes it deterministic. Duplicating the record for every alias is
  the reported amplification. Date/Author: 2026-11-25, implementing agent.
- D3. Decision: keep `order_only_deps` in traversal and missing-record
  accounting. Rationale: ADR-0068 and a live Kani proof require it; the plan
  clause excluding it is contradictory and therefore not actioned. Date/Author:
  2026-11-25, implementing agent.
- D4. Decision: stack frames carry `(EdgeId, entry_alias)` and are pushed only
  in path mode. Rationale: the entry alias is needed for witnesses and missing
  records; presence mode needs only the edge-keyed state map, so it stays
  allocation free. Tests and Kani's insertion-order walk resolve an alias
  through `is_visited`, compiled under `cfg(any(test, kani))` to avoid dead
  code. Date/Author: 2026-11-25, implementing agent.
- D5. Decision: regenerate five mutation patches whose hunk context moved.
  Rationale: `rotate_index`, `visit_dependency`, `visit_known_edge`, and
  `visit` changed shape; the seeded faults are semantically unchanged.
  Date/Author: 2026-11-25, implementing agent.
- D6. Decision: after the CodeRabbit remedy commit `ac3e008e0`, re-ran only
  `check-fmt`, `markdownlint`, `doc-coverage`, `test`, and `lint`. Rationale:
  that commit edits one test module; the production tree, Kani harnesses, and
  mutation patches are byte-identical to the head that passed `kani-check`,
  `kani-full`, and `test-kani-mutations`, so re-running those would re-prove
  unchanged inputs. Date/Author: 2026-11-26, implementing agent.

## Outcomes & retrospective

The detector now expands each canonical edge once. Measured on the focused
regressions, four aliases over four absent entries retain four records (was
sixteen), and three aliases over two entries are scanned twice (was six). The
generated-graph property fails on the previous detector with five records and
seven scans against three declared entries, so the bound is not vacuous.

Single-output witnesses are unchanged. Multi-alias self-cycles now report the
requested alias, for example `[x, x]` rather than the first alias; the accepted
plan calls for that. The public API, error display text, and users' guide are
unchanged.

The heavy gates are load-sensitive: the Kani mutation prewarm's 900-second
allowance is sized for a loaded host, and this host exceeded it on a cold
target. The shared Kani target persists, so re-running the same target warmed
passed. Record that as a capacity retry, not a code defect.

## Context and orientation

`src/ir/cycle/mod.rs` exposes `analyse(&BuildGraph) -> CycleDetectionReport`.
`src/ir/cycle/detector.rs` owns `CycleDetector`, the DFS stack, the visitation
map, and the `missing_dependencies` vector. `src/ir/cycle/support.rs` resolves
an alias to its producer and canonicalizes cycles. `src/ir/graph/mod.rs` owns
`BuildGraph`, `BuildEdge`, `EdgeId`, and the output-alias index; the Kani-only
map lives in `src/ir/graph/kani_map.rs`.

Today `states` is an `IrHashMap<&Utf8Path, VisitState>` keyed by alias and the
stack holds alias references. `visit_dependency` pushes a cloned
`(alias, dependency)` pair for every absent dependency it meets, so an edge
indexed by several aliases records the same dependency repeatedly.

Relevant tests:

- `src/ir/cycle/tests.rs` — focused detector tests and missing-record tables.
- `src/ir/cycle/participation_tests.rs` — exact multi-alias cycle witnesses.
- `src/ir/cycle/property_tests/` — generated graph properties.
- `src/ir/graph/property_tests/cycles.rs` — lowering-level cycle obligations.
- `tests/ir_from_manifest_tests/canonical_edge_storage.rs` — manifest-level
  canonical storage and alias resolution.
- `docs/verification/mutations/` — one mutation patch per Kani harness.

Kani harnesses in `src/ir/cycle/verification.rs` drive `contains_cycle`, a
`cfg(kani)` entry point. `tests/kani_mutation_evidence_tests.rs` checks that
every patch applies and compiles.

Tooling: the repository pins Rust `nightly-2026-11-10`; Linux gates use the
`mold` linker and `-Zthreads=8`. Kani is invoked through the Makefile targets
`kani-check`, `kani-full`, and `test-kani-mutations`.

## Conformance basis

- Issue [`#979`](https://github.com/leynos/netsuke/issues/979), finding
  `3fe32c4b-0d10-48cb-8411-4f0b5b81cc59` (CWE-400, medium).
- [ADR-0068](../../docs/adrs/adr-0068-detect-cycles-across-all-prerequisite-kinds.md):
  cycle detection spans inputs, implicit dependencies, and order-only
  dependencies.
- `docs/netsuke-design.md` §5.3 step 4 (graph validation).
- `docs/developers-guide.md` "IR cycle detection" and the build-graph lookup
  note.

## Verification plan

Invariants:

- V1: each canonical edge's dependency lists are scanned at most once per
  `analyse` pass, so scans ≤ total declared
  `inputs + implicit_deps + order_only_deps` entries.
- V2: retained missing records ≤ total declared dependency entries, one per
  examined absent entry, named under that edge's entry alias.
- V3: every reported cycle witness is closed and each consecutive pair `(x, y)`
  satisfies: the producer of `x` lists `y` in one of its dependency lists.
- V4: cycle presence agrees with the independent edge-level oracle.
- V5: single-output witnesses and `Display` text are unchanged.

Methods and evidence:

- Focused `rstest` cases in `src/ir/cycle/tests.rs` assert exact witnesses,
  exact missing-record vectors, and exact scan counts via a test-only
  thread-local counter. Red evidence: these fail on `56d1e0a2`.
- A `proptest` module generates multi-alias graphs with explicit and implicit
  outputs, resolvable and absent dependencies, and optional back edges; it
  asserts V1–V4 against declared data and `edge_oracle`.
- Manifest-lowering tests in
  `tests/ir_from_manifest_tests/canonical_edge_storage.rs` assert one canonical
  edge for 64 aliases, and, for a cyclic variant, an exact missing-record count
  and a witness containing the requested alias.
- Kani harnesses keep their existing claims; mutation patches are regenerated
  where the detector moved and `make test-kani-mutations` proves they apply and
  compile.
- Gates: `make check-fmt`, `make lint`, `make doc-coverage`, `make test`,
  `make kani-check`, `make kani-full`, `make test-kani-mutations`, and
  `make test-workflow-contracts`.

Non-vacuity: the focused Red tests fail for the intended reason (record count
and scan count) before the implementation; the property generator is required
to reach missing records, multi-alias edges, and back edges, and the Kani
mutation patches seed faults that the harnesses reject.

## Revision note

- 2026-11-25: initial draft created from the supplied coding plan, with the
  `order_only_deps` exclusion recorded as not actioned and the actual toolchain
  pin noted.
- 2026-11-26: recorded the Red and Green evidence, the non-vacuity probe, the
  nine-gate results, the Kani prewarm capacity retry, the CodeRabbit remedy,
  and the draft pull request; the plan is complete.

# Document which dependency kinds participate in cycle detection (roadmap 4.4.2)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

Revision 2. Drafted on 2026-09-27 against `origin/main` at `ebcedaef`, then
revised after an expert-panel design review the same day (see `Revision note`).
No implementation may begin until the plan is explicitly approved.

## Purpose / big picture

Netsuke compiles a YAML manifest (a `Netsukefile`) into a Ninja build file.
Before it writes any Ninja text it lowers the manifest into an intermediate
representation (IR), a graph of build edges, and rejects that graph if it
contains a dependency cycle. A dependency cycle is a chain of build steps in
which each step must wait for the next and the last must wait for the first, so
no step can ever start.

A manifest target names its prerequisites in three fields: `sources` (explicit
inputs), `deps` (implicit dependencies), and `order_only_deps` (order-only
dependencies). The IR also carries explicit and implicit outputs. Roadmap item
4.4.2 asks which of those kinds take part in cycle detection, and asks for that
rule to be written down in the users' guide and matched by the implementation
and tests.

At planning time the rule is partly written down and partly wrong:

- `docs/users-guide.md` (section *Targets, inputs, and dependencies*) says
  "Cycle detection follows `sources` and `deps`. Order-only dependencies
  enforce ordering but do not participate in cycle detection." The code agrees:
  `src/ir/cycle_detector.rs::CycleDetector::visit_known_edge` walks
  `BuildEdge::inputs` and `BuildEdge::implicit_deps` only.
- Ninja, the only backend Netsuke drives, does not agree. Ninja 1.11.1 rejects
  a cycle closed by an order-only dependency with
  `ninja: error: dependency cycle: a -> b -> a` (evidence in
  `Artefacts and notes`). A manifest with such a cycle therefore passes every
  Netsuke check, `netsuke generate` writes a Ninja file that Ninja refuses, and
  `netsuke build` fails late with Ninja's untranslated message instead of
  Netsuke's localized, JSON-capable `CircularDependency` diagnostic.
- Nothing states whether implicit outputs participate. The code already treats
  them as aliases of their producing edge, so a dependency that names an
  implicit output reaches that edge, but no test or document says so.

After this change:

- An order-only dependency that closes a cycle is rejected by Netsuke during IR
  validation, with the same `CircularDependency` diagnostic used for `sources`
  and `deps` cycles, before any Ninja file is written or Ninja is run.
- A new users' guide section, **Dependency and build-graph semantics**, states
  one rule: every dependency kind (`sources`, `deps`, `order_only_deps`)
  participates in cycle detection; every output of a target, including every
  entry of a multi-output `name` list and every IR implicit output, names the
  same build step; and `dependency_order: serial` adds no dependency of its own.
- A new Architectural Decision Record (ADR),
  `docs/adr-041-every-dependency-kind-participates-in-cycle-detection.md`,
  records the decision and why the earlier "order-only dependencies only affect
  freshness" rationale was wrong. The design document, the formal-verification
  document, and the developers' guide point at it.
- Unit, property, bounded-model-checking (Kani), behavioural (`rstest-bdd`),
  and end-to-end tests pin the rule, including an independent-oracle property
  test showing the per-output traversal finds a cycle exactly when the
  edge-level graph has one.

The change is observable by running `make test` and seeing the new tests pass
(they fail before the change for the reasons stated in
`Validation and acceptance`), by running `make kani-ir` and seeing the new
harnesses verified, and by running the built `netsuke generate` against the
order-only cycle example in the users' guide and seeing Netsuke's own
diagnostic and a non-zero exit status.

## Context and orientation

This section assumes no knowledge of the repository.

### The pipeline

`docs/users-guide.md` section *Understand the build model* lists Netsuke's six
stages. Stage 5 builds and validates the IR; stage 6 generates Ninja and, for
`build` or `clean`, runs Ninja. Everything in this plan lives in stage 5, its
documentation, and its tests.

### The manifest fields

`src/ast/target.rs` defines `Target`. The relevant fields are `name` (one or
more output paths), `sources`, `deps`, `dependency_order`, and
`order_only_deps`. There is no manifest field for implicit outputs: `name` is
the only output field, and every entry becomes an explicit output.

### The IR

`src/ir/graph.rs` defines the public `BuildEdge` struct. Its path-bearing
fields are `inputs` (from `sources`), `implicit_deps` (from `deps`),
`order_only_deps` (from `order_only_deps`), `explicit_outputs` (from `name`),
and `implicit_outputs` (always empty when lowered from a manifest; populated
only by Rust callers of the unstable `netsuke::ir` API and by tests).
`BuildEdge::dependency_order` is a scheduling policy for `implicit_deps`.

`BuildGraph` stores each edge once in an arena and keeps an output index,
`targets: IrHashMap<Utf8PathBuf, EdgeId>`, keyed by every explicit and implicit
output alias (`BuildGraph::index_output_aliases`). `BuildGraph::insert_edge`
rejects a duplicate alias. `BuildGraph::target_for_output` resolves any alias
to its stored key and its edge.

`src/ir/from_manifest.rs::process_targets` lowers each manifest target into a
`BuildEdge` (`implicit_outputs: Vec::new()` at the time of writing), and
`BuildGraph::detect_cycles` in the same file calls `cycle::analyse` once the
whole graph exists. A dependency that no edge produces is treated as an
external file: it is collected into `missing_dependencies`, logged at `info`,
and never an error.

### The cycle detector

`src/ir/cycle.rs` holds the crate-private entry point
`analyse(graph) -> CycleDetectionReport` and, under `cfg(kani)`,
`contains_cycle(graph) -> bool`. The traversal lives in
`src/ir/cycle_detector.rs`:

- `CycleDetector::detect_targets` visits every output alias, in sorted order,
  that has not yet been fully visited.
- `CycleDetector::visit` marks a node `Visiting`, pushes it on the stack,
  resolves its edge, and calls `visit_known_edge`.
- `visit_known_edge` calls `visit_dependencies` for `edge.inputs`, then for
  `edge.implicit_deps`. It does not touch `edge.order_only_deps`.
- `visit_dependency` resolves a dependency through the output index; an
  unresolved one is recorded as missing and not followed.
- A dependency that resolves to a `Visiting` node is a back edge; the stack
  slice becomes the cycle, normalized by `support::canonicalize_cycle`.

Nodes are output aliases, not edges. Two aliases of one edge are two nodes
whose successors are identical (the edge's dependencies). `Verification plan`
states the lemma that makes that safe.

The detector is written as plain indexed loops rather than chained iterators
because chained iterators stalled the CBMC back end in the Kani harnesses (see
the comment on `BuildGraph::index_output_aliases`). New traversal code must
follow the same shape.

### Where the rule is written today

- `docs/users-guide.md`, *Targets, inputs, and dependencies*, the two-sentence
  paragraph after the field list.
- `docs/formal-verification-methods-in-netsuke.md`, *Cycle-participation
  contract*.
- `docs/netsuke-design.md` §5.3, step 4 (*Graph Validation*), which also says
  "Keys are cloned from the `targets` map", a statement made stale by the
  borrow-centric traversal.
- `docs/developers-guide.md`, the paragraph beginning
  "`src/ir/cycle.rs::CycleDetector::visit` traverses `inputs` and
  `implicit_deps`", and the section *IR cycle detection*.
- Rustdoc on `src/ir/cycle.rs` (module and `analyse`) and on
  `src/ir/cycle_issue322_property_tests.rs`.

The roadmap names "the user guide's dependency and build-graph semantics
chapter". No such heading exists; this plan creates it (see `Decision log`).

### Existing tests that encode the old rule

- `src/ir/cycle_issue322_property_tests.rs::order_only_back_edge_has_no_cycle`
  asserts that an order-only back edge produces no cycle. It is the executable
  form of the rule this plan reverses and must be inverted, not deleted.
- `src/ir/cycle_tests.rs`, `src/ir/cycle_analyse_tests.rs`,
  `src/ir/cycle_property_tests.rs`, and `src/ir/cycle_verification.rs` never
  populate `order_only_deps` or `implicit_outputs`.
- `tests/features/ir.feature` and `tests/features/ir_generation.feature` each
  check only that `tests/data/circular.yml` (a `sources` cycle) fails.
- `tests/ir_from_manifest_tests.rs::manifest_error_cases` asserts the cycle
  path for `tests/data/circular.yml`.
- `src/diagnostic_json_tests.rs` snapshots the display and JSON forms of a
  `CircularDependency` error.
- No subprocess (`assert_cmd`-style) test runs `netsuke` on a cyclic manifest.

### Kani evidence contract

`tests/kani_mutation_evidence_tests.rs` requires every `#[kani::proof]` harness
to own a mutation patch under `docs/verification/mutations/`, named after the
harness path with `::` replaced by `__`, that still applies with
`git apply --check`. Its `module_path_for_source` derives the harness path from
the file name alone: `src/ir/cycle_verification.rs` maps to
`ir::cycle::verification`, so a second file such as
`cycle_participation_verification.rs` would map to a different module and break
the contract. Five existing patches edit `src/ir/cycle_detector.rs`, at hunks
around `back_edge_result` and `visit_dependency`, away from `visit_known_edge`.
`docs/developers-guide.md` *Kani harness inventory* lists every harness in a
table that must be extended.

### Quality gates

`AGENTS.md` names the gates: `make check-fmt`, `make typecheck`, `make lint`,
`make test`, and, for Markdown, `make markdownlint` (which runs
`make spelling`) and `make nixie`. `make kani-ir` runs the Kani IR suite. Gates
run sequentially, never in parallel, with output captured by `tee` to
`/tmp/<action>-netsuke-4-4-2-document-cycle-detection-scope.out`.

## Signposts: documentation and skills

Read before starting:

- `AGENTS.md` — conventions, gates, 400-line file cap, doc-comment rules, the
  Polonius and next-generation trait-solver rules, and the ban on in-process
  environment mutation in tests.
- `docs/roadmap.md` item 4.4.2 and
  `docs/formal-verification-methods-in-netsuke.md` *Cycle-participation
  contract*.
- `docs/netsuke-design.md` §5.3 *The Transformation Process: AST to IR*.
- `docs/developers-guide.md` *IR cycle detection*, *Kani harness inventory*,
  and the dependency-classes paragraph near "`ast::DependencyOrder` is the
  closed manifest enum".
- `docs/users-guide.md` *Understand the build model*, *Targets, inputs, and
  dependencies*, *Run direct dependencies serially*, and *Use the canonical
  build graph*.
- `docs/adr-004-bound-kani-ir-harnesses-to-small-n.md` (Kani bounds) and
  `docs/adr-011-use-ninja-dyndep-for-serial-dependency-ordering.md` (why serial
  gates stay out of the IR).
- `docs/documentation-style-guide.md` — ADR section order and status values,
  ExecPlan status vocabulary, no first or second person outside `README.md`,
  en-GB-oxendict spelling, and the rule that `docs/contents.md` changes when a
  document is added.
- `docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`,
  `docs/rust-doctest-dry-guide.md`, and
  `docs/reliable-testing-in-rust-via-dependency-injection.md`.
- `docs/ortho-config-users-guide.md` — read to confirm no configuration surface
  is needed (see `Decision log`).

Skills to load: `rust-router` first; then `rust-unit-testing` (rstest tables,
`googletest`, `pretty_assertions`, `insta`), `proptest` (the oracle property),
`kani` (the new harnesses), `rust-verification` if the rigour choice is
revisited, `nll-to-polonius` before touching any borrow in the detector,
`hexagonal-architecture` for the boundary check, `arch-decision-records` for
ADR-041, `execplans` to keep this plan current, `en-gb-oxendict-style` for
prose, and `pr-creation` and `commit-message` when publishing.

## Conformance basis

Upstream artefacts, all at `origin/main` `ebcedaef`:

- `docs/roadmap.md` 4.4.2 and its four sub-items (identifiers `RM-4.4.2-a`
  order-only decision, `RM-4.4.2-b` implicit-output decision, `RM-4.4.2-c`
  users' guide chapter, `RM-4.4.2-d` alignment of implementation, tests, and
  documentation). Requires 4.2.1, which is complete.
- `docs/formal-verification-methods-in-netsuke.md` *Cycle-participation
  contract* (`FV-CYCLE`). This plan changes that section's substance; the
  change is a deliberate, recorded deviation (see `Decision log`, D1).
- `docs/netsuke-design.md` §5.3 step 4 (`DES-5.3-4`).
- `docs/adr-004-bound-kani-ir-harnesses-to-small-n.md` (`ADR-004`), binding
  the Kani bounds.
- `docs/adr-011-use-ninja-dyndep-for-serial-dependency-ordering.md`
  (`ADR-011`), binding that serial gates are backend artefacts outside the IR.
- The Ninja manual's definitions of explicit, implicit, and order-only
  dependencies and implicit outputs, and Ninja's `DependencyScan` behaviour
  (`NINJA-AX`), treated as axioms (see `Verification plan`).

No Terms of Reference revision governs this item (PR 786 proposes one but is
unmerged); none is invented here.

Trace links:

```plaintext
RM-4.4.2-a -> FV-CYCLE (revised) -> ADR-041 -> EP-M2
  -> participation_tests::order_only_*; kani order_only_two_node_*
  -> e2e order_only_cycle_rejected_before_ninja
  -> bdd "Order-only dependency cycle is rejected"
RM-4.4.2-b -> ADR-041 -> EP-M1
  -> participation_tests::implicit_output_*; kani implicit_output_alias_*
  -> property alias_traversal_agrees_with_edge_oracle (EP-M2)
RM-4.4.2-c -> EP-M3 -> users-guide "Dependency and build-graph semantics"
  -> documented example guide-order-only-cycle-manifest
RM-4.4.2-d -> EP-M1..EP-M4 -> every gate green; roadmap 4.4.2 ticked
DES-5.3-4 -> EP-M3 -> netsuke-design.md §5.3 step 4 revised, cites ADR-041
ADR-011 -> EP-M2 -> bdd "Serial ordering adds no cycle"
  -> e2e serial_earlier_dependency_on_later_builds
```

## Constraints

- Do not change the public `netsuke::ir` API: `BuildEdge`, `BuildGraph`,
  `EdgeId`, `IrGenError`, and their fields and method signatures stay as they
  are. The change is behavioural, inside the crate-private detector. The
  content of `IrGenError::CircularDependency::missing_dependencies` changes
  (D4) and is announced; its type does not.
- Do not change the `CircularDependency` Fluent message key
  `ir.circular_dependency`, its message text, or the JSON diagnostic schema. An
  order-only cycle reuses them unchanged.
- Do not add a manifest field for implicit outputs, and do not change
  `from_manifest` lowering of any field.
- Do not move serial-dependency gates or dyndep sidecars into the IR (ADR-011).
- Keep the detector's plain indexed-loop shape; do not introduce chained
  iterators on the Kani path.
- Respect Polonius rules from `AGENTS.md`: no defensive clones, double lookups,
  or eager error context added to appease the borrow checker; compile the
  natural borrow-returning form first.
- No new dependencies. `rstest`, `rstest-bdd` 0.5.0, `insta`, `proptest`,
  `googletest`, and `pretty_assertions` are already dev-dependencies.
- No in-process environment mutation in tests; subprocess tests configure the
  child with `Command::env` only.
- No file may exceed 400 lines. `src/ir/cycle_verification.rs` is at 361 and
  `tests/documentation_examples_tests.rs` is already at 458, so new harnesses
  need room made first (Stage B) and the documented-example case lives in the
  new end-to-end file, with only its identifier added to `EXPECTED_EXAMPLE_IDS`.
- Every Kani harness lives in a file whose name
  `tests/kani_mutation_evidence_tests.rs::module_path_for_source` maps to the
  harness's module path: new cycle harnesses therefore go in
  `src/ir/cycle_verification.rs` (module `ir::cycle::verification`), not in a
  second `*_verification.rs` file.
- Users' guide prose uses no first or second person, and every new YAML fence
  in the guide carries a `tested-example` marker and a registry entry.
- Do not edit historical ExecPlans that mention order-only dependencies.

## Tolerances (exception triggers)

- Production code: if more than 3 non-test source files or more than 40 net
  production lines change (Rustdoc included), stop and escalate. The planned
  production edit is one call in `cycle_detector.rs` plus Rustdoc.
- Overall scope: if more than 35 files or more than 1,200 net added lines
  change (tests, fixtures, patches, and documentation included), stop and
  escalate. The planned inventory is about 30 files.
- Interface: if any public item in `netsuke::ir`, the diagnostic JSON schema,
  or a Fluent message must change, stop and escalate.
- Behaviour: if any existing test other than
  `order_only_back_edge_has_no_cycle` must change its expectation, or any file
  under `examples/` or `tests/data/` turns out to contain an order-only cycle,
  stop; it means a shipped manifest relies on the old rule.
- Kani: if a new harness needs an unwind bound above 8, or `make kani-ir` wall
  time grows by more than 25% over the Stage A baseline, stop and record
  measurements before choosing between a smaller harness and a property-only
  discharge.
- Mutation evidence: if an existing cycle mutation patch fails
  `git apply --check` after the detector edit and cannot be regenerated to seed
  the same fault, stop.
- Iterations: if a gate still fails after three fix attempts for the same
  cause, stop and escalate.
- Approval: D1 reverses a documented contract. Approval of this plan is the
  explicit acceptance of D1. If review rejects D1, stop; the plan then reduces
  to documenting and testing the current rule and needs re-approval.

## Risks

- Risk: a manifest that worked before now fails. Ninja checks cycles only in
  the part of the graph it is asked to build: with no `default` statement it
  builds root nodes, and a self-contained cycle has no root, so Ninja ignores
  it (probe in `Artefacts and notes`); `netsuke clean` runs a Ninja tool and no
  cycle scan at all. A manifest whose order-only cycle is never requested
  therefore built successfully before and is rejected at stage 5 after this
  change. Severity: medium. Likelihood: low. Mitigation: the three
  release-admission canary `Netsukefile`s (`leynos/repovec-appliance`
  `b1393fd7`, `leynos/mxd` `73704801`, `leynos/ortho-config` `64cd6cb9`)
  contain no `order_only_deps` at all, and every `order_only_deps` in
  `examples/` points at a directory target with no path back (checked
  2026-09-27). The CHANGELOG entry is marked **Breaking**, and the migration
  guide says the manifest is now rejected even when the cycle was never built,
  and how to find and remove the back edge.
- Risk: Netsuke's scope stays wider than Ninja's. Netsuke checks the whole
  manifest; Ninja checks only what it is asked to build. Severity: low.
  Likelihood: certain. Mitigation: already true for `sources` and `deps`
  cycles; ADR-041 records that the whole-graph scope applies uniformly, and why
  the reachable-subgraph alternative was rejected (`netsuke generate` has no
  requested target set).
- Risk: editing `cycle_detector.rs` stops existing cycle mutation patches from
  applying. Severity: low. Likelihood: low. The edit site (`visit_known_edge`)
  is outside every existing hunk and its context, and `git apply --check`
  tolerates line offsets. Mitigation: run the contract test after the edit and
  regenerate only a patch that fails, re-checking its seeded fault rather than
  its context lines.
- Risk: a third dependency loop raises CBMC cost in existing harnesses.
  Severity: medium. Likelihood: low. Mitigation: harnesses leave
  `order_only_deps` empty unless they test it, so the loop runs zero iterations;
  `make kani-ir` (which runs the whole `kani-full` suite) is timed before and
  after.
- Risk: the detection walk is recursive, and long order-only chains (stamp
  chains) deepen it. Severity: low. Likelihood: low. This risk predates the
  change. Mitigation: none in scope; recorded for the future edge-keyed
  traversal noted under `LEM-1`.
- Risk: PR 805 (rstest-bdd 0.6.0 migration) merges first and changes step or
  world syntax. Severity: low. Likelihood: medium. Mitigation: re-check at each
  rebase; the new step and world slot are small and move with the migration.
- Risk: ADR number collision. ADR-039 (`jm5/kani-change-scoped-gate`) and
  ADR-040 (`6-1-1-split-rfc-0006-...`) are taken on other branches, and 030 and
  031 were vacated by a renumbering. Severity: low. Likelihood: medium.
  Mitigation: sweep every remote branch for `docs/adr-0*` before writing the
  ADR and again at each rebase; renumber this plan's ADR, never another.
- Risk: a fixture uses a YAML 1.1 Boolean name (`y`, `n`, `on`, `yes`) and
  fails at parse time, so a "fails" assertion passes for the wrong reason.
  Severity: medium. Likelihood: medium. Mitigation: every cycle assertion
  checks the typed cycle path, never just failure, and fixtures use names such
  as `tool` and `headers.stamp`.

## Verification plan

### Axioms

- `NINJA-AX-1`: Ninja rejects a cycle reachable from the targets it is asked
  to build through any mix of explicit, implicit, and order-only inputs, and
  resolves a dependency on an implicit output to the edge that produces it.
  Evidence: the Ninja 1.11.1 probe in `Artefacts and notes`, and
  `DependencyScan::RecomputeEdgesInputsDirty` in Ninja's `src/graph.cc`, which
  recurses into every input before consulting `is_order_only`. Treated as an
  axiom; Ninja's internals are not verified here.
- `NINJA-AX-2`: Ninja does not examine cycles it is not asked to build (probe
  in `Artefacts and notes`). This is why the whole-graph check is a Netsuke
  decision, not a restatement of Ninja's.
- `NINJA-AX-3`: validations (`|@`) are not modelled; Netsuke's `BuildEdge` has
  no validation field.
- `IR-AX-1`: `BuildGraph::insert_edge` guarantees output aliases are unique,
  so each alias maps to exactly one edge. This is already verified by the 4.2.1
  duplicate-output harness and tests; it is relied on, not re-proved.

### Definitions

For a `BuildGraph` `G`, let `deps(e)` be the concatenation of `e.inputs`,
`e.implicit_deps`, and `e.order_only_deps`, and `aliases(e)` be
`e.explicit_outputs` followed by `e.implicit_outputs`. The *edge graph* has an
arc `e -> f` when some `d` in `deps(e)` is in `aliases(f)`. The *alias graph*,
which the detector walks, has an arc `p -> q` when `q` is an alias in the index
and `q` is in `deps(edge(p))`. A cycle *closes through an order-only arc* when
the edge graph is cyclic with all arcs and acyclic once order-only arcs are
removed.

### Obligations

`OB-1 order-only participation.` For every `G`, if the edge graph has a cycle
that uses an order-only arc, `analyse(G).cycle` is `Some`, and the reported
path contains that arc.

- Method: `rstest` table, inverted property test, and one Kani harness.
- Rationale: the finite partitions (self-edge, two-node, three-node, mixed
  kinds, cycle closed last by an order-only arc) are specification examples;
  the property covers generated sizes up to 50 nodes; Kani proves the two-node
  shape exhaustively within ADR-004's bounds. The one-call production change
  reuses `visit_dependencies`, which the existing harnesses already verify, so
  one order-only harness is proportionate.
- Domain: `rstest` cases `order_only_self_edge`, `order_only_two_node`,
  `order_only_three_node`, `order_only_closes_mixed_cycle`; property
  `order_only_back_edge_produces_cycle` over 2–49 nodes; Kani
  `order_only_two_node_cycle_reports_cycle`.
- Artefact: `src/ir/cycle_participation_tests.rs` (new, `#[cfg(test)]`,
  included from `src/ir/cycle.rs`), the inverted property in
  `src/ir/cycle_issue322_property_tests.rs`, and the harness in
  `src/ir/cycle_verification.rs`.
- Evidence: before the detector change the `rstest` cases and the property
  fail with `cycle` equal to `None`, and Kani reports a failed assertion;
  afterwards `make test` passes and `make kani-ir` reports the harness
  `SUCCESSFUL`.
- Non-vacuity: every case has a witness graph whose only cycle uses an
  order-only arc; the property's strategy inserts exactly one order-only back
  edge into an otherwise acyclic chain, so a pass cannot come from the other
  kinds. The mutation patch
  `docs/verification/mutations/ir__cycle__verification__order_only_two_node_cycle_reports_cycle.patch`
  removes the order-only `visit_dependencies` call and must make the harness
  fail.

`OB-2 implicit-output participation.` A dependency that names an implicit
output of edge `f` makes `f` a successor exactly as naming an explicit output
does, so a cycle closed through an implicit output is reported, and the
reported path names the implicit alias that was traversed.

- Method: `rstest` cases plus one Kani harness.
- Domain: `implicit_output_closes_cycle` (edge `a | a.extra` depends on `b`,
  `b` depends on `a.extra`; expected `[a.extra, b, a.extra]`),
  `implicit_output_self_cycle` (edge `a | x` depends on `x`; expected `[x, x]`),
  `non_first_explicit_output_closes_cycle` (the manifest-level equivalent,
  `name: [a, a.extra]`); Kani `implicit_output_alias_cycle_reports_cycle` with
  one-byte aliases.
- Artefact: `src/ir/cycle_participation_tests.rs` and
  `src/ir/cycle_verification.rs`.
- Evidence: these pass before and after the change. They are
  characterization tests pinning existing behaviour, not red tests. Their
  failing form is shown by the mutation patch
  `docs/verification/mutations/ir__cycle__verification__implicit_output_alias_cycle_reports_cycle.patch`,
  which skips the implicit-output loop in `BuildGraph::index_output_aliases`.
- Non-vacuity: each witness has the cycle closed only through the implicit
  alias; with the mutation applied, the dependency resolves to nothing and is
  reported missing, so the assertion on `cycle` fails. The `rstest` cases also
  assert the missing-dependency list is empty, which the mutation breaks
  independently.

`OB-3 serial ordering adds no arc.` `dependency_order` does not change which
arcs exist: for any `G`, `analyse(G)` is identical whether each edge's
`dependency_order` is `Parallel` or `Serial`.

- Method: property test, behavioural scenario, and end-to-end build.
- Rationale: the detector never reads `dependency_order`, so the property
  guards against a future edit that makes it do so. The end-to-end build shows
  the generated Ninja is schedulable when an earlier serial dependency depends
  on a later one, which the IR alone cannot show.
- Domain: property `dependency_order_does_not_change_analysis`, added to
  `src/ir/cycle_issue322_property_tests.rs` beside the `dag_strategy` and
  `cyclic_graph_strategy` it reuses, with every edge's order flipped; BDD
  scenario *Serial ordering adds no cycle*; end-to-end test
  `serial_earlier_dependency_on_later_builds` using
  `tests/data/serial_earlier_depends_on_later.yml`.
- Evidence: the property and the BDD scenario pass before and after. The
  end-to-end test runs real Ninja through
  `test_support::ninja::ninja_integration_workspace`, which skips when Ninja is
  absent and panics under `NETSUKE_REQUIRE_NINJA=1` (set in CI); it asserts
  exit 0, that `a` and `b` exist, and that `b` was built first (Ninja's `[1/2]`
  line names `b`). The alchemist run recorded in `Surprises & discoveries` is
  the planning-time evidence.
- Non-vacuity: the strategies produce both cyclic and acyclic graphs, and the
  property compares the full report (cycle and missing list), so a detector
  that read `dependency_order` would diverge on some case. The seeded fault
  (skipping `implicit_deps` when `Serial`) must fail the property; it is run
  once, recorded here, and not committed. The end-to-end test uses real Ninja,
  never a fake, because a fake Ninja exits 0 whatever the file says.

`OB-4 missing order-only dependencies are reported (D4).` An order-only
dependency that no edge produces appears in `missing_dependencies` like an
unresolved input or implicit dependency.

- Method: extend the existing property
  `generated_missing_dependencies_are_absent_targets`.
- Artefact: `missing_graph_strategy` and `injected_missing_deps` in
  `src/ir/cycle_issue322_property_tests.rs` gain order-only injection.
- Evidence: fails before the detector change (order-only injections are not
  reported), passes after.
- Non-vacuity: the strategy injects at least one missing order-only dependency
  in every generated case.

`LEM-1 alias traversal agrees with the edge graph.` For every `G` satisfying
`IR-AX-1`, `analyse(G).cycle.is_some()` if and only if the edge graph of `G`
has a cycle.

- Argument: (only if) every alias-graph arc `p -> q` induces the edge-graph arc
  `edge(p) -> edge(q)`, so an alias cycle maps to a closed edge walk, which
  contains a cycle. (if) given an edge cycle `e1 -> e2 -> ... -> ek -> e1`,
  choose for each `i` the alias `q(i+1)` of `e(i+1)` that `e(i)` depends on;
  then `q1 -> q2 -> ... -> qk -> q1` is an alias-graph cycle, because the
  successors of `q(i)` are `deps(e(i))`. Self-loops (`k = 1`) are the case
  where an edge depends on one of its own aliases. The detector's depth-first
  search visits every alias, so it finds some cycle when one exists.
  Dependencies absent from the index contribute no arc to either graph.
- Cost: walking aliases re-reads an edge's dependency list once per alias, so
  the traversal costs `O(sum over edges of |aliases(e)| * |deps(e)|)` hash
  lookups; 500 aliases against 500 order-only dependencies is about 250,000
  lookups. Edge-keyed visit state would remove the multiplier but touches
  Polonius- and Kani-sensitive code, so it is deferred and noted in ADR-041.
- Method: property test against an independent oracle.
- Rationale: the lemma is the correctness argument for walking aliases instead
  of edges and becomes load-bearing once every dependency kind participates.
  See D6 for why no Verus proof is planned here.
- Domain: a strategy built on a DAG base: 1–12 edges, each with 1–3 aliases
  split between explicit and implicit outputs, whose dependencies of every kind
  point only at aliases of lower-indexed edges or at absent paths. A
  `prop_oneof!` then chooses one of five variants: no back arc (acyclic), or
  one back arc from a lower-indexed edge's dependency list to a higher-indexed
  edge's alias, added as an input, an implicit dependency, an order-only
  dependency, or an arc to an implicit-output alias. Each class is therefore
  present by construction, so no filtering and no seed-sensitive class-coverage
  companion is needed.
- Artefact: `src/ir/cycle_participation_property_tests.rs` (new, included from
  `src/ir/cycle.rs` under `#[cfg(test)]`), property
  `alias_traversal_agrees_with_edge_oracle`. The oracle builds the edge graph
  from `BuildGraph::edges()` and `edge_id_for_output`, keying in-degrees by
  `EdgeId` (which is `Hash + Eq`; its inner index is private), and runs Kahn's
  topological sort; it shares no code with the detector. It counts a self-arc
  once and de-duplicates repeated arcs in both the arc set and the in-degree
  counts.
- Evidence: passes after EP-M2. Before EP-M2 it fails on the order-only
  variant, which is one fifth of generated cases, so a 256-case run fails with
  near certainty; the OB-1 `rstest` cases remain the primary red evidence.
- Non-vacuity: every variant is reachable by construction; the order-only
  variant is the seeded fault the pre-change detector must fail.

`OB-5 unchanged diagnostics and no Ninja run.` An order-only cycle yields
`IrGenError::CircularDependency` whose human and JSON forms have the same shape
as a `sources` cycle, and Ninja is never started.

- Method: end-to-end subprocess tests with an `insta` snapshot.
- Artefact: `tests/cycle_participation_e2e_tests.rs` (new).
  `order_only_cycle_rejected_before_ninja` runs `netsuke build` with
  `NETSUKE_NINJA` pointing at a fake Ninja that writes a sentinel file, and
  asserts failure, the `Circular dependency detected` message, and an absent
  sentinel. `order_only_cycle_json_diagnostic` runs
  `netsuke --json --no-input generate` and snapshots stderr after
  `normalize_fluent_isolates` and an `insta` filter for the temporary
  directory, with the locale pinned by `--locale en-US` and
  `Command::env("LANG", "C")` in the child.
  `documented_order_only_cycle_is_rejected` loads
  `guide-order-only-cycle-manifest` through `manifest_workspace` and asserts
  rejection and the cycle path, following the precedent at
  `tests/readme_security_tests.rs`.
- Evidence: before EP-M2 `generate` exits 0 and `build` reaches the fake
  Ninja, so all three fail; afterwards they pass.
- Non-vacuity: the snapshot and assertions contain the cycle path
  `["headers.stamp", "tool", "headers.stamp"]`, which only the order-only arc
  closes; the sentinel check fails if Netsuke reaches stage 6.

Canonicalization, missing-dependency ordering, and deterministic traversal
order are unchanged and remain covered by the 4.2.1 and 4.2.2 harnesses and the
existing property suites.

## Milestones and plateaus

### EP-M1 — pin implicit-output participation

- Outcome: characterization tests and a Kani harness pin the existing rule
  that implicit outputs participate as aliases of their producing edge. The
  canonicalization helpers in `cycle_verification.rs` move to a harness-free
  support file to make room.
- Requirements and gaps: `RM-4.4.2-b`, `OB-2`.
- Acceptance evidence: `cycle_participation_tests::implicit_output_*` and
  `non_first_explicit_output_closes_cycle` pass; `make kani-ir` verifies
  `implicit_output_alias_cycle_reports_cycle` and every existing harness;
  `tests/kani_mutation_evidence_tests.rs` passes; the new patch, applied by
  hand, fails its harness.
- Conformance check: no production behaviour changes; ADR-004 bounds
  respected; harness paths match `module_path_for_source`.
- Recovery: revert the milestone commits; nothing depends on them.
- Remaining gaps: order-only participation, documentation.
- Compatibility decision: none.

### EP-M2 — make order-only dependencies participate

- Outcome: the detector walks `order_only_deps`; every test and harness agrees.
- Requirements and gaps: `RM-4.4.2-a`, `RM-4.4.2-d`, `OB-1`, `OB-3`, `OB-4`,
  `OB-5`, `LEM-1`.
- Acceptance evidence: the red tests listed in `Validation and acceptance`
  fail first, then pass; `make kani-ir` verifies the new harness; every
  mutation patch applies; the four standard gates pass.
- Conformance check: `netsuke::ir` API unchanged; no new dependency; D1
  accepted with the plan.
- Recovery: revert the milestone commits; EP-M1 remains valid.
- Remaining gaps: user-facing and design documentation.
- Compatibility decision: none. Netsuke is pre-1.0 (0.1.0-beta3 released, the
  Unreleased section heading for 0.1.0-beta4); the change is announced, not
  shimmed.

### EP-M3 — document the rule

- Outcome: ADR-041 exists; the users' guide gains *Dependency and build-graph
  semantics* with a tested example; the design document, formal-verification
  document, developers' guide, Rustdoc, CHANGELOG, v0.1.0 migration guide, and
  `docs/contents.md` agree with it.
- Requirements and gaps: `RM-4.4.2-c`, `RM-4.4.2-d`, `DES-5.3-4`, `FV-CYCLE`.
- Acceptance evidence: `documented_order_only_cycle_is_rejected` passes;
  `make markdownlint` and `make nixie` pass; the stale-rule search below prints
  nothing.
- Conformance check: no current document or source file restates the old
  rule. This multiline search skips historical ExecPlans. On the planning-time
  tree it finds seven statements (users' guide, formal-verification document,
  developers' guide, design document, and three Rustdoc comments); after EP-M3
  it prints nothing:

  ```bash
  rg -U -n -i --glob '!docs/execplans/**' \
    -e 'order.only\s+dependencies[^.]{0,80}not\s+participate' \
    -e 'order_only_deps`\s+(are|is)\s+intentionally\s+excluded' \
    -e 'intentionally\s+does\s+not\s+traverse' \
    -e 'order.only\s+dependencies\s+are\s+ignored' \
    -e 'excludes[\s/!]+order.only\s+dependencies' docs src
  ```

- Recovery: documentation-only; revert the commit.
- Remaining gaps: roadmap closure.
- Compatibility decision: none.

### EP-M4 — close out

- Outcome: roadmap 4.4.2 and its four sub-items are ticked with a completion
  note; this plan is `COMPLETE`. The PR description proposes a follow-up for a
  dependency-kind hint in cycle diagnostics (D7); an issue is filed only if the
  maintainer agrees.
- Acceptance evidence: every gate listed in `Validation and acceptance` passes
  on the final commit.

## Plan of work

### Stage A — confirm (no code changes)

Re-run the ADR sweep and confirm 041 is free. Confirm the location of
`visit_known_edge`. Record `make kani-ir` wall time on the unchanged tree as
the baseline in `Artefacts and notes`. Grep `examples/` and `tests/data/` for
`order_only_deps` and confirm none closes a cycle.

### Stage B — EP-M1 characterization

Move the canonicalization-only helpers of `src/ir/cycle_verification.rs`
(`assert_kernel_canonical_properties` through `is_closed_id_cycle`, about 220
lines) into a new harness-free `src/ir/cycle_verification_support.rs`, declared
from `cycle_verification.rs` with
`#[path = "cycle_verification_support.rs"] mod support;`. The harnesses stay in
`cycle_verification.rs`, so every existing mutation-patch name stays valid. Run
`make kani-ir` to confirm the move is behaviour-neutral.

Create `src/ir/cycle_participation_tests.rs` with a `build_edge` fixture taking
an `EdgeSpec` struct with named path-list fields (keeping the argument count
within Clippy's limit) and an `rstest` table `implicit_outputs_participate`
with the three OB-2 cases. Assert with `googletest` matchers
(`assert_that!(report.cycle, some(eq(&expected)))`) and
`pretty_assertions::assert_eq` for path vectors. Include it from
`src/ir/cycle.rs` with
`#[cfg(test)] #[path = "cycle_participation_tests.rs"] mod participation_tests;`.

Extend the harness-local `edge` helper in `cycle_verification.rs` to accept
implicit outputs and order-only dependencies (or add a second small builder),
and add `implicit_output_alias_cycle_reports_cycle`. Write its mutation patch
and add its row to *Kani harness inventory* in `docs/developers-guide.md`.

### Stage C — EP-M2 red, then green

Red: add the OB-1 `rstest` table `order_only_dependencies_participate`; invert
`order_only_back_edge_has_no_cycle` into `order_only_back_edge_produces_cycle`,
asserting the reported cycle contains the injected back edge; extend
`missing_graph_strategy` and `injected_missing_deps` for OB-4; add
`dependency_order_does_not_change_analysis`; add
`src/ir/cycle_participation_property_tests.rs` with the oracle property; add
the Kani harness `order_only_two_node_cycle_reports_cycle`; add the BDD
scenarios, the world slot and step, and the data files; add
`tests/cycle_participation_e2e_tests.rs`. Run them and record the failures.

Green: in `src/ir/cycle_detector.rs::visit_known_edge`, after the
`implicit_deps` call, add a third `visit_dependencies` call over
`&edge.order_only_deps` with the same early return. Update the Rustdoc on
`visit_known_edge`, the module doc of `src/ir/cycle.rs`, and the doc on
`analyse`, replacing "`order_only_deps` are intentionally excluded" with the
new rule and a pointer to ADR-041. Update the module doc of
`cycle_issue322_property_tests.rs`. Run
`cargo nextest run --test kani_mutation_evidence_tests`; regenerate only a
patch that no longer applies, and add the new one. Run the focused tests, then
`make kani-ir`, then the four standard gates.

Refactor: if `visit_known_edge` now reads as three repetitions, express it as
one loop over a fixed array of the three slices only if CBMC time does not
regress; otherwise keep the three calls and say why in a comment.

### Stage D — EP-M3 documentation, EP-M4 close

Write ADR-041 with *Status* (`Accepted`), *Date*, *Context and Problem
Statement*, *Decision Drivers*, *Considered Options*, *Decision Outcome*, and
*Consequences*. The options are: keep the exclusion and document it; include
order-only dependencies (chosen); include them but only warn; a tiered rollout
(warn in one release, reject in the next); and Ninja-parity scope (check only
the subgraph reachable from requested targets). Record why each lost: warning
or tiering keeps `netsuke generate` emitting Ninja files that Ninja refuses for
a pre-1.0 tool with no known affected manifest; the reachable-subgraph scope
has no meaning for `generate`, which has no requested targets, and would loosen
the whole-graph check `sources` and `deps` already get. Cite the Ninja probes
and the GNU Make comparison, and note the deferred edge-keyed traversal.

In `docs/users-guide.md`, replace the two-sentence paragraph under *Targets,
inputs, and dependencies* with a one-line pointer, and add a new
`### Dependency and build-graph semantics` section immediately after *Run
direct dependencies serially* and before `## Use Jinja safely`. The section
states: what a cycle is; that `sources`, `deps`, and `order_only_deps` all
participate, because a cycle through any of them can never be scheduled; that
the check covers the whole manifest, including targets never requested; that
every path in a multi-output `name` names the same build step; that a
dependency naming a path no target produces is treated as an existing file and
cannot close a cycle; that `dependency_order: serial` adds no dependency; and,
for Rust callers, that a `BuildEdge`'s implicit outputs behave like explicit
ones. It carries one tested example, `guide-order-only-cycle-manifest`, showing
a rejected order-only cycle and the diagnostic text. Register the identifier in
`EXPECTED_EXAMPLE_IDS`. Add one-sentence pointers from *Understand the build
model*, from the IR bullet in *Interpret failures*, and from *Use the canonical
build graph*.

Update `docs/formal-verification-methods-in-netsuke.md` *Cycle-participation
contract*, `docs/netsuke-design.md` §5.3 step 4 (also correcting "Keys are
cloned"), the two developers' guide passages, `docs/contents.md` (ADR-041 under
*Decision records*, newest first), `CHANGELOG.md` under *Unreleased* →
*Changed* with a **Breaking** entry that also notes the `missing_dependencies`
content change, and `docs/v0-1-0-migration-guide.md` *At-a-glance changes* plus
a short section *Remove order-only dependency cycles* (the release is still
0.1.0, so the v0.1.1 guide's "every v0.1.0 manifest stays compatible" promise
is untouched). Run `make fmt`, then the Markdown gates.

Tick roadmap 4.4.2 and its sub-items with a completion note naming ADR-041, set
this plan to `COMPLETE`, and run every gate.

## Behavioural specification

Append to `tests/features/ir.feature`:

```gherkin
  Scenario: Order-only dependency cycle is rejected
    When the manifest file "tests/data/order_only_cycle.yml" is compiled to IR
    Then IR generation fails
    And the IR cycle is "headers.stamp -> tool -> headers.stamp"

  Scenario: Cycle through a non-first output is rejected
    When the manifest file "tests/data/multi_output_cycle.yml" is compiled to IR
    Then IR generation fails
    And the IR cycle is "a.extra -> b -> a.extra"

  Scenario: Serial ordering adds no cycle
    When the manifest file "tests/data/serial_earlier_depends_on_later.yml" is compiled to IR
    Then the graph has 3 targets
```

The world today stores only the error's display text
(`TestWorld::generation_error: Slot<String>` in `tests/bdd/fixtures/mod.rs`,
filled with `e.to_string()` in `tests/bdd/steps/ir.rs::compile_manifest_impl`),
and anyhow's non-alternate display carries only the outer context, so it cannot
see the cycle. Add a slot `generation_cycle: Slot<Vec<String>>` and, in
`compile_manifest_impl`, before converting the error to text, fill it from
`error.downcast_ref::<IrGenError>()` when the variant is `CircularDependency`.
The new step `the IR cycle is {path:string}` splits on ` -> ` and compares with
that slot, failing if the slot is empty, so a parse failure can never pass as a
cycle. The first scenario is red before EP-M2; the other two are
characterization.

The data files are:

```yaml
# tests/data/order_only_cycle.yml
netsuke_version: "1.0.0"
targets:
  - name: tool
    command: "touch tool"
    order_only_deps: headers.stamp
  - name: headers.stamp
    command: "touch headers.stamp"
    deps: tool
```

```yaml
# tests/data/multi_output_cycle.yml
netsuke_version: "1.0.0"
targets:
  - name: [a, a.extra]
    command: "touch a a.extra"
    sources: b
  - name: b
    command: "touch b"
    sources: a.extra
```

```yaml
# tests/data/serial_earlier_depends_on_later.yml
netsuke_version: "1.0.0"
targets:
  - name: b
    command: "touch b"
  - name: a
    command: "touch a"
    deps: b
actions:
  - name: all
    dependency_order: serial
    deps:
      - a
      - b
```

## Concrete steps

All commands run from the repository root of this worktree.

```bash
git branch --show-current   # expect 4-4-2-document-cycle-detection-scope
for b in $(git for-each-ref --format='%(refname:short)' refs/remotes/origin); do
  git ls-tree --name-only "$b" docs/ | grep -o 'adr-0[0-9][0-9]'
done | sort -u | tail -5    # 041 must be absent
```

Focused red and green runs:

```bash
cargo nextest run --lib \
  -E 'test(/participation|order_only_back_edge|missing_dependencies_are_absent|dependency_order_does_not/)' \
  2>&1 | tee /tmp/red-netsuke-4-4-2-document-cycle-detection-scope.out
cargo nextest run --test bdd_tests -E 'test(/order_only|non_first_output|serial_ordering_adds/)' \
  2>&1 | tee -a /tmp/red-netsuke-4-4-2-document-cycle-detection-scope.out
cargo nextest run --test cycle_participation_e2e_tests \
  2>&1 | tee -a /tmp/red-netsuke-4-4-2-document-cycle-detection-scope.out
```

Expected red excerpt before the detector change (the exact case name is
confirmed during the run):

```plaintext
FAIL ... ir::cycle::participation_tests::order_only_dependencies_participate::case_2_order_only_two_node
  Value of: report.cycle
  Expected: has a value which is equal to ["headers.stamp", "tool", "headers.stamp"]
  Actual: None
```

Gates, sequentially, one at a time:

```bash
make check-fmt 2>&1 | tee /tmp/check-fmt-netsuke-4-4-2-document-cycle-detection-scope.out
make typecheck 2>&1 | tee /tmp/typecheck-netsuke-4-4-2-document-cycle-detection-scope.out
make lint 2>&1 | tee /tmp/lint-netsuke-4-4-2-document-cycle-detection-scope.out
make test 2>&1 | tee /tmp/test-netsuke-4-4-2-document-cycle-detection-scope.out
make doc-coverage 2>&1 | tee /tmp/doc-coverage-netsuke-4-4-2-document-cycle-detection-scope.out
make markdownlint 2>&1 | tee /tmp/markdownlint-netsuke-4-4-2-document-cycle-detection-scope.out
make nixie 2>&1 | tee /tmp/nixie-netsuke-4-4-2-document-cycle-detection-scope.out
make kani-ir 2>&1 | tee /tmp/kani-ir-netsuke-4-4-2-document-cycle-detection-scope.out
```

Kani harnesses need `LD_LIBRARY_PATH` on this host as recorded in the
developers' guide; follow it rather than changing code.

## Validation and acceptance

Behaviour a human can check after EP-M2, using the built binary:

```bash
cd "$(mktemp -d)" && cat > Netsukefile <<'EOF'
netsuke_version: "1.0.0"
targets:
  - name: tool
    command: "touch tool"
    order_only_deps: headers.stamp
  - name: headers.stamp
    command: "touch headers.stamp"
    deps: tool
EOF
<path-to>/target/debug/netsuke generate; echo "exit=$?"
```

Before: exit 0 and Ninja text on stdout. After: a non-zero exit and a
diagnostic beginning `Circular dependency detected:` naming `headers.stamp` and
`tool`; no Ninja text.

Red-Green-Refactor evidence to record in `Artefacts and notes`:

- Red: the focused commands above, with the OB-1 cases, the inverted
  property, the OB-4 property, the oracle property, the BDD order-only
  scenario, the three end-to-end tests, and the order-only Kani harness failing
  because no cycle is reported. OB-2, OB-3, and the other two BDD scenarios
  pass at this point by design.
- Green: the same commands passing after the one-call detector change.
- Refactor: focused commands again, then all gates.

Quality criteria:

- Tests: `make test` passes, including every new test named in this plan.
- Verification: `make kani-ir` reports every harness `SUCCESSFUL`, including
  the two new ones; `tests/kani_mutation_evidence_tests.rs` passes; each new
  mutation patch, applied by hand, makes its harness fail (recorded once).
- Lint and types: `make check-fmt`, `make typecheck`, `make lint`, and
  `make doc-coverage` pass.
- Documentation: `make markdownlint` and `make nixie` pass.

## Idempotence and recovery

Every step is re-runnable. Tests and harnesses are additive except the inverted
property, whose old form is recoverable from Git. The detector change is one
call and reverts cleanly. If a mutation patch stops applying after a rebase,
regenerate it from the same fault and re-run the contract test. Scratch
directories live under `/tmp`; nothing writes to the repository at test time
except `insta` pending snapshots, which are reviewed and either accepted or
deleted.

## Artefacts and notes

Ninja 1.11.1 probes, 2026-09-27, scratch `/tmp/ninja-cycle-wfFe`:

```plaintext
# build a: touch || b ; build b: touch a
ninja: error: dependency cycle: a -> b -> a        exit=1
# build a | a.extra: touch b ; build b: touch a.extra
ninja: error: dependency cycle: a.extra -> b -> a.extra   exit=1
# build a: touch || a
ninja: error: dependency cycle: a -> a             exit=1
# build a: touch || b ; build b: touch a ; build c: touch   (no default)
[1/1] touch c                                      exit=0
```

The last probe shows `NINJA-AX-2`: with no `default`, Ninja builds only root
nodes, and the `a`/`b` cycle has no root, so Ninja never sees it.

GNU Make 4.4.1 on `a: | b` and `b: a` prints
`make: Circular b <- a dependency dropped.`, so Make also treats order-only
prerequisites as graph edges for cycle purposes.

Ninja manual, quoted: order-only dependencies are those where "when these are
out of date, the output is not rebuilt until they are built, but changes in
order-only dependencies alone do not cause the output to be rebuilt"; implicit
outputs have "semantics identical to explicit outputs". The first sentence is
the reason the old rationale fails: an order-only dependency still has to be
*built first*, which is what makes a cycle through it unschedulable.

## Interfaces and dependencies

No public interface changes. New crate-private test and harness support modules:

- `src/ir/cycle_participation_tests.rs` (`#[cfg(test)]`).
- `src/ir/cycle_participation_property_tests.rs` (`#[cfg(test)]`).
- `src/ir/cycle_verification_support.rs` (`#[cfg(kani)]`, harness-free,
  declared from `cycle_verification.rs`).
- `tests/cycle_participation_e2e_tests.rs`. Cargo auto-discovers it;
  `tests/integration_test_wiring_tests.rs` checks that discovery.
- `TestWorld::generation_cycle: Slot<Vec<String>>` in
  `tests/bdd/fixtures/mod.rs`.

The only production edit is inside
`crate::ir::cycle::detector::CycleDetector::visit_known_edge`. No configuration
surface is added, so `ortho_config` is not involved. From the
hexagonal-architecture view, the cycle rule is domain policy inside the IR
core; it has no port, and none is introduced. Ninja is the driven adapter whose
semantics the domain rule now matches; the adapter (`src/ninja_gen`), the graph
renderer (`src/graph_view`), and the runner do not change, and all three
already treat order-only dependencies as graph arcs.

## Progress

- [x] (2026-09-27) Renamed branch to `4-4-2-document-cycle-detection-scope`.
- [x] (2026-09-27) Reconnaissance of detector, tests, and documentation.
- [x] (2026-09-27) Probed Ninja 1.11.1 and GNU Make 4.4.1 cycle behaviour.
- [x] (2026-09-27) Serial-ordering hypothesis H1 tested: not falsified.
- [x] (2026-09-27) Baseline O2 recorded: `generate` accepts an order-only
  cycle and `build` fails inside Ninja.
- [x] (2026-09-27) Expert-panel design review: verdict "proceed with
  conditions" from both reviewers; every condition folded into revision 2.
- [x] (2026-09-27) Verified review claims: Kani module-path derivation, BDD
  world error storage, Ninja root-only scheduling, and canary `Netsukefile`s (no
  `order_only_deps` in any of the three).
- [ ] Plan approved.
- [ ] EP-M1.
- [ ] EP-M2.
- [ ] EP-M3.
- [ ] EP-M4.

## Surprises & discoveries

- Observation: the users' guide already states the order-only exclusion, so
  roadmap sub-item `RM-4.4.2-c` is not a blank page but a correction. Evidence:
  `docs/users-guide.md` *Targets, inputs, and dependencies*. Impact: this plan
  revises existing text rather than adding a first statement.
- Observation: Ninja rejects cycles through order-only dependencies and
  implicit outputs. Evidence: `Artefacts and notes`. Impact: the documented
  exclusion lets Netsuke emit Ninja files that Ninja refuses; this motivates D1.
- Observation: implicit outputs already participate through the output index,
  but manifests cannot declare them. Evidence:
  `BuildGraph::index_output_aliases` and `from_manifest.rs` setting
  `implicit_outputs: Vec::new()`. Impact: the user-visible equivalent is a
  multi-output `name`; the IR rule is documented for Rust callers.
- Observation: `netsuke graph` renders order-only arcs (`EdgeClass::OrderOnly`
  in `src/graph_view/mod.rs`) that cycle detection ignores. Impact: after D1
  the renderer and the detector agree on which arcs exist.
- Observation: H1 held. With `all` declared `dependency_order: serial` and
  `deps: [a, b]`, where target `a` has `deps: b`, `netsuke build all` exited 0
  and printed `[1/2] touch b` then `[2/2] touch a`: Ninja followed the true
  dependency order, and no `.netsuke/serial` path appeared in any cycle.
  Evidence: alchemist run, 2026-09-27, scratch `/tmp/netsuke-h1-Yo4d`, logs
  `/tmp/h1-stdout.log` and `/tmp/h1-stderr.log`. Impact: supports D3; the
  manifest becomes `tests/data/serial_earlier_depends_on_later.yml`.
- Observation: baseline O2. For `x` with `order_only_deps: "y"` and `y` with
  `deps: x`, `netsuke generate --output …` exited 0 and emitted
  `build x: … || y` and `build y: … | x`; `netsuke build x` passed stage 5,
  then failed with `ninja: error: dependency cycle: x -> y -> x` and
  `✖ Error: running ninja with build file …`, exit 1. Evidence: alchemist run,
  2026-09-27, scratch `/tmp/netsuke-o2-IhIW`, logs `/tmp/o2-build2-stderr.log`
  and `/tmp/o2-generate-stderr.log`. Impact: confirms the defect D1 fixes and
  gives the red-state expectation for the end-to-end test.
- Observation: a bare `y` (and `n`, `yes`, `on`, and similar) in a manifest is
  a YAML 1.1 Boolean, not a path, and fails `StringOrList` decoding before IR
  lowering. Evidence: the first O2 attempt failed at manifest parsing. Impact:
  every fixture and example in this plan uses names such as `tool` and
  `headers.stamp`; a parse failure must never be mistaken for a cycle
  rejection, so the behavioural and end-to-end tests assert the cycle path, not
  just failure.
- Observation: Ninja ignores a cycle it is not asked to build. With no
  `default` statement it builds root nodes only, and a self-contained cycle has
  no root. Evidence: the fourth Ninja probe in `Artefacts and notes`
  (`[1/1] touch c`, exit 0). Impact: revision 1's claim that an order-only
  cycle "could never build" was too strong; Risk 1, the migration-guide text,
  and ADR-041 now say the change rejects such manifests even when the cycle was
  never requested.
- Observation (design review): a second Kani file named
  `cycle_participation_verification.rs` would map to module
  `ir::cycle_participation::verification` under
  `tests/kani_mutation_evidence_tests.rs::module_path_for_source`, orphaning
  the planned patch names. Impact: harnesses stay in `cycle_verification.rs`,
  and helpers move out to make room (D9).
- Observation (design review): the BDD world keeps only the error's display
  text, and anyhow's non-alternate display omits the `CircularDependency`
  payload. Impact: a typed `generation_cycle` slot is added (D10).
- Observation (design review): the revision 1 oracle strategy (dependencies
  drawn freely from the alias pool) would be cyclic in about 99% of 12-edge
  cases, making its acyclic floor unattainable. Impact: the strategy is rebuilt
  on a DAG base with one optional back arc per chosen kind.
- Observation (design review): revision 1's 20-file tolerance was exceeded by
  its own inventory (about 30 files). Impact: tolerances now separate
  production code (3 files, 40 lines) from the overall change (35 files).

## Decision log

- D1. Decision: order-only dependencies participate in cycle detection.
  Rationale: a cycle is a scheduling impossibility, not a freshness question;
  an order-only dependency must still be built first, so a cycle through one
  can never run. Ninja rejects it, so excluding it only defers the failure to
  Ninja's untranslated message and lets `netsuke generate` emit a file Ninja
  refuses. This reverses the stated rationale in `FV-CYCLE`, the users' guide,
  the design document, and the developers' guide; that deviation is surfaced
  for approval with this plan and recorded in ADR-041. The panel's strongest
  alternative, a tiered rollout (warn for one release, then reject), was
  weighed and rejected: it keeps `generate` emitting Ninja that Ninja refuses,
  no known manifest (examples or the three release-admission canaries) is
  affected, and Netsuke is pre-1.0. Ninja-parity scope (check only the subgraph
  reachable from requested targets) was rejected because `generate` has no
  requested targets and it would loosen the whole-graph check that `sources` and
  `deps` already get. Date/Author: 2026-09-27, planning agent; pending
  approval.
- D2. Decision: implicit outputs participate as aliases of their producing
  edge; no change to code. Rationale: matches Ninja's "identical to explicit
  outputs" semantics and the existing output index. Date/Author: 2026-09-27,
  planning agent.
- D3. Decision: `dependency_order` adds no arcs; serial gates stay out of the
  IR. Rationale: ADR-011 keeps gates backend-only, and a later serial
  dependency that another path reaches still starts through that path (H1
  evidence). No user path can name a gate, because `.netsuke/serial` and
  `.netsuke/dyndep` are reserved and rejected by
  `ninja_gen::dyndep::reject_reserved_paths`; so a cycle through a gate would
  need a revealed dependency that transitively reaches the serial edge's own
  output, and that cycle already exists in the IR through `implicit_deps` and
  is reported there. Date/Author: 2026-09-27, planning agent.
- D4. Decision: an order-only dependency that no target produces is recorded
  in `missing_dependencies` like the other kinds. Rationale: uniform treatment;
  the list is informational and logged only. Date/Author: 2026-09-27, planning
  agent.
- D5. Decision: create a `### Dependency and build-graph semantics` section
  after *Run direct dependencies serially*, rather than a new top-level
  chapter. Rationale: the roadmap names a chapter that does not exist; keeping
  it beside the field list keeps the rule next to the fields it governs.
  Date/Author: 2026-09-27, planning agent.
- D6. Decision: no Verus proof in this item. Rationale: `LEM-1` is standard
  graph reasoning whose production form is not a pure kernel; the Verus
  question is owned by roadmap 4.4.3, and the formal-verification document
  keeps Verus optional and proof-kernel-only. The oracle property plus Kani
  witnesses discharge the lemma for this item. A symbolic Kani harness
  comparing the detector with the oracle over 2–3 edges was considered and left
  out: the one-call change reuses already-verified traversal code, and the
  added CBMC cost is not justified for a documentation-led item. Date/Author:
  2026-09-27, planning agent.
- D7. Decision: no new diagnostic wording naming the dependency kind on the
  cycle. Rationale: the diagnostic key and JSON schema are constraints; kind
  annotation is a separate usability change. Order-only cycles are the class
  most likely to confuse ("but it is only order-only"), so EP-M4 proposes an
  additive hint as a follow-up in the PR description. Date/Author: 2026-09-27,
  planning agent.
- D8. Decision: allocate ADR-041. Rationale: 038 is the highest on `main`;
  039 and 040 are taken on open branches; 030 and 031 were vacated by a
  renumbering and are avoided to prevent confusion. Date/Author: 2026-09-27,
  planning agent.
- D9. Decision: add the two new Kani harnesses to `src/ir/cycle_verification.rs`
  and move its canonicalization-only helpers into a harness-free
  `src/ir/cycle_verification_support.rs`. Rationale: keeps harness paths under
  `ir::cycle::verification`, matching the mutation-evidence contract, and keeps
  the file under 400 lines without extending the contract test. Date/Author:
  2026-09-27, planning agent.
- D10. Decision: add `TestWorld::generation_cycle: Slot<Vec<String>>`, filled
  by downcasting to `IrGenError::CircularDependency`, and assert the cycle path
  in BDD scenarios. Rationale: asserting only failure lets a YAML parse error
  pass as a cycle rejection. Date/Author: 2026-09-27, planning agent.
- D11. Decision: announce the change in `docs/v0-1-0-migration-guide.md`.
  Rationale: `CHANGELOG.md` *Unreleased* is heading for 0.1.0-beta4 (PR 804),
  so the change ships within 0.1.0; the v0.1.1 guide's promise that every
  v0.1.0 manifest stays compatible is not affected. Re-check if 0.1.0 final is
  tagged before this merges. Date/Author: 2026-09-27, planning agent.
- D12. Decision: one order-only Kani harness (two-node), not two, and no
  seed-based class-coverage companion. Rationale: the production change is one
  call into verified code; `rstest` covers the self-edge, and the DAG-based
  oracle strategy guarantees every class by construction. Date/Author:
  2026-09-27, planning agent.

## Outcomes & retrospective

Not started.

## Revision note

Revision 1 (2026-09-27): initial draft.

Revision 2 (2026-09-27): folded in the expert-panel design review (both
reviewers returned "proceed with conditions"). Moved the new Kani harnesses into
`cycle_verification.rs` with a helper split (D9); added a typed BDD cycle slot
(D10); gated the serial end-to-end build on real Ninja; pinned locale and
filters for the JSON snapshot; rebuilt the oracle strategy on a DAG base and
dropped the seed-based companion (D12); added OB-4 for missing order-only
dependencies and renumbered the diagnostics obligation to OB-5; corrected Risk
1 after probing Ninja's root-only scheduling and checking the canaries;
recorded the tiered-rollout and reachable-subgraph alternatives in D1;
recalibrated tolerances; replaced blanket mutation-patch regeneration with a
check-then-regenerate step; scoped the stale-rule search away from historical
ExecPlans; and added users' guide pointers from *Understand the build model*
and *Interpret failures*. The remaining work is unchanged in shape: EP-M1 to
EP-M4, awaiting approval.

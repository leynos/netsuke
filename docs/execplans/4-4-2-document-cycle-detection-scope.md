# Document which dependency kinds participate in cycle detection (roadmap 4.4.2)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

Revision 1. Drafted on 2026-09-27 against `origin/main` at `ebcedaef`. No
implementation may begin until the plan is explicitly approved.

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
`git apply --check`. Five existing patches edit `src/ir/cycle_detector.rs`;
editing that file can make them stop applying, and the contract test will then
fail. `docs/developers-guide.md` *Kani harness inventory* lists every harness
in a table that must be extended.

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
  -> participation_tests::order_only_*; kani order_only_*
  -> bdd "Order-only dependency cycle is rejected"
RM-4.4.2-b -> ADR-041 -> EP-M1
  -> participation_tests::implicit_output_*; kani implicit_output_alias_*
  -> property alias_traversal_agrees_with_edge_oracle (EP-M2)
RM-4.4.2-c -> EP-M3 -> users-guide "Dependency and build-graph semantics"
  -> documented example guide-order-only-cycle-manifest
RM-4.4.2-d -> EP-M1..EP-M4 -> every gate green; roadmap 4.4.2 ticked
DES-5.3-4 -> EP-M3 -> netsuke-design.md §5.3 step 4 revised, cites ADR-041
ADR-011 -> EP-M2 -> bdd "Serial ordering adds no cycle"; e2e serial example
```

## Constraints

- Do not change the public `netsuke::ir` API: `BuildEdge`, `BuildGraph`,
  `IrGenError`, and their fields and method signatures stay as they are. The
  change is behavioural, inside the crate-private detector.
- Do not change the `CircularDependency` error shape, its Fluent message key
  `ir.circular_dependency`, or its JSON diagnostic schema. An order-only cycle
  reuses them unchanged.
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
- No file may exceed 400 lines; `src/ir/cycle_verification.rs` is already at
  361, so new harnesses go in a new sibling file.
- Users' guide prose uses no first or second person, and every new YAML fence
  in the guide carries a `tested-example` marker and a registry entry.

## Tolerances (exception triggers)

- Scope: if implementation needs more than 20 changed files or more than 900
  net added lines (tests and documentation included), stop and escalate.
- Interface: if any public item in `netsuke::ir`, the diagnostic JSON schema,
  or a Fluent message must change, stop and escalate.
- Behaviour: if any existing test other than
  `order_only_back_edge_has_no_cycle` and prose that restates the old rule must
  change its expectation, stop; it means some shipped manifest or fixture
  relies on an order-only back edge.
- Kani: if a new harness needs an unwind bound above 8, or exceeds the
  resource cap enforced by `make kani-ir`, stop and record measurements before
  choosing between a smaller harness and a property-only discharge.
- Mutation evidence: if an existing cycle mutation patch cannot be regenerated
  to seed the same fault after the detector edit, stop.
- Iterations: if a gate still fails after three fix attempts for the same
  cause, stop and escalate.
- Ambiguity: if review rejects decision D1 (order-only participation), stop;
  the plan then reduces to documenting and testing the current rule, and needs
  re-approval.

## Risks

- Risk: a real manifest relies on an order-only back edge and breaks.
  Severity: medium. Likelihood: low. Mitigation: such a manifest could never
  build under Ninja, which rejects the same cycle; the change converts a late
  Ninja failure into an early Netsuke one. The CHANGELOG entry is marked
  **Breaking** and the v0.1.0 migration guide says how to find and remove the
  back edge, because `netsuke generate` and `netsuke graph` used to succeed.
- Risk: the scope of Ninja's check is narrower than Netsuke's. Ninja checks
  only the part of the graph reachable from the requested targets, while
  Netsuke checks the whole graph. Severity: low. Likelihood: certain.
  Mitigation: this is already true for `sources` and `deps` cycles; ADR-041
  states that the whole-graph scope applies uniformly.
- Risk: editing `cycle_detector.rs` stops the five existing cycle mutation
  patches from applying. Severity: medium. Likelihood: high. Mitigation: EP-M2
  regenerates each patch with the same seeded fault and re-runs
  `tests/kani_mutation_evidence_tests.rs`; the fault in each patch is reviewed,
  not only its context lines.
- Risk: a third dependency loop raises CBMC cost in existing harnesses.
  Severity: medium. Likelihood: medium. Mitigation: harnesses leave
  `order_only_deps` empty unless they test it, so the added loop has zero
  iterations; the full `make kani-ir` time is recorded before and after.
- Risk: PR 805 (rstest-bdd 0.6.0 migration) merges first and changes step
  syntax. Severity: low. Likelihood: medium. Mitigation: re-check at each
  rebase; new steps reuse existing ones where possible.
- Risk: ADR number collision. ADR-039 (`jm5/kani-change-scoped-gate`) and
  ADR-040 (`6-1-1-split-rfc-0006-...`) are taken on other branches, and 030 and
  031 were vacated by a renumbering. Severity: low. Likelihood: medium.
  Mitigation: sweep every remote branch for `docs/adr-0*` before writing the
  ADR and again at each rebase; renumber this plan's ADR, never another.
- Risk: `missing_dependencies` gains order-only entries and a log-sensitive
  test changes. Severity: low. Likelihood: low. Mitigation: D4 makes this
  deliberate; the detector property test for missing dependencies is extended
  to cover order-only injection.

## Verification plan

### Axioms

- `NINJA-AX-1`: Ninja rejects a build graph when a cycle is reachable from a
  requested target through any mix of explicit, implicit, and order-only
  inputs, and resolves a dependency on an implicit output to the edge that
  produces it. Evidence: the Ninja 1.11.1 probe in `Artefacts and notes`, and
  `DependencyScan::RecomputeEdgesInputsDirty` in Ninja's `src/graph.cc`, which
  recurses into every input before consulting `is_order_only`. Treated as an
  axiom; Ninja's internals are not verified here.
- `NINJA-AX-2`: validations (`|@`) are not modelled; Netsuke's `BuildEdge` has
  no validation field.
- `IR-AX-1`: `BuildGraph::insert_edge` guarantees output aliases are unique,
  so each alias maps to exactly one edge. This is already verified by the 4.2.1
  duplicate-output harness and tests; it is relied on, not re-proved.

### Definitions

For a `BuildGraph` `G`, let `deps(e)` be the concatenation of `e.inputs`,
`e.implicit_deps`, and `e.order_only_deps`, and `aliases(e)` be
`e.explicit_outputs` followed by `e.implicit_outputs`. The *edge graph* has an
arc `e -> f` when some `d` in `deps(e)` is in `aliases(f)`. The *alias graph*,
which the detector walks, has an arc `x -> y` when `y` is an alias in the index
and `y` is in `deps(edge(x))`.

### Obligations

`OB-1 order-only participation.` For every `G`, if the edge graph restricted to
order-only arcs, or mixing order-only arcs with the other kinds, contains a
cycle, `analyse(G).cycle` is `Some`.

- Method: `rstest` table plus inverted property test plus Kani harnesses.
- Rationale: the finite partitions (self-edge, two-node, mixed kinds, cycle
  closed last by an order-only arc) are specification examples; the property
  test covers generated sizes up to 50 nodes; Kani proves the two smallest
  shapes exhaustively for every insertion order within ADR-004's bounds.
- Domain: `rstest` cases `order_only_self_edge`, `order_only_two_node`,
  `order_only_closes_mixed_cycle`, `order_only_three_node`; property
  `order_only_back_edge_produces_cycle` over 2–49 nodes; Kani
  `order_only_self_dependency_reports_cycle` and
  `order_only_two_node_cycle_reports_cycle` (both insertion orders).
- Artefact: `src/ir/cycle_participation_tests.rs` (new, `#[cfg(test)]`,
  included from `src/ir/cycle.rs`), the inverted property in
  `src/ir/cycle_issue322_property_tests.rs`, and
  `src/ir/cycle_participation_verification.rs` (new, `#[cfg(kani)]`).
- Evidence: before the detector change the `rstest` cases and the property
  fail with `cycle` equal to `None`; afterwards `make test` passes;
  `make kani-ir` reports the new harnesses `SUCCESSFUL`.
- Non-vacuity: every case has a witness graph containing an order-only arc on
  the cycle; the property's strategy always inserts exactly one order-only back
  edge and no other arc closes a cycle, so a pass cannot come from the other
  kinds. The mutation patch
  `ir__cycle__participation_verification__order_only_two_node_cycle_reports_cycle.patch`
  removes the order-only `visit_dependencies` call and must make that harness
  fail.

`OB-2 implicit-output participation.` A dependency that names an implicit
output of edge `f` makes `f` a successor exactly as naming an explicit output
does, so a cycle closed through an implicit output is reported, and the
reported path names the implicit output alias that was traversed.

- Method: `rstest` cases plus Kani harness.
- Domain: `implicit_output_closes_cycle` (edge `a | a.extra` depends on `b`,
  `b` depends on `a.extra`), `implicit_output_self_cycle` (edge `a | x` depends
  on `x`), `non_first_explicit_output_closes_cycle` (the manifest-level
  equivalent: `name: [a, a.extra]`); Kani
  `implicit_output_alias_cycle_reports_cycle` with one-byte aliases.
- Artefact: `src/ir/cycle_participation_tests.rs`,
  `src/ir/cycle_participation_verification.rs`.
- Evidence: these pass before and after the change (the behaviour already
  exists); they are characterization tests that pin it. Their failing form is
  shown by the mutation patch
  `ir__cycle__participation_verification__implicit_output_alias_cycle_reports_cycle.patch`,
  which skips the implicit-output loop in `BuildGraph::index_output_aliases`.
- Non-vacuity: each witness has the cycle closed only through the implicit
  alias; with the mutation applied the dependency resolves to nothing and is
  reported missing instead, so the assertion on `cycle` fails. The `rstest`
  case also asserts the missing-dependency list is empty, which the mutation
  breaks independently.

`OB-3 serial ordering adds no arc.` `dependency_order` does not change which
arcs exist: for any `G`, `analyse(G)` is identical whether each edge's
`dependency_order` is `Parallel` or `Serial`.

- Method: property test plus behavioural and end-to-end witness.
- Rationale: the detector never reads `dependency_order`, so the property is a
  guard against a future edit that makes it do so. The behavioural scenario and
  the end-to-end run establish that the generated Ninja is schedulable when an
  earlier serial dependency depends on a later one, which the IR cannot show.
- Domain: property `dependency_order_does_not_change_analysis` over the
  existing `dag_strategy` and `cyclic_graph_strategy` with every edge's order
  flipped; BDD scenario *Serial ordering adds no cycle* with
  `tests/data/serial_earlier_depends_on_later.yml`.
- Evidence: the property passes before and after; the BDD scenario passes.
  The end-to-end build of the same manifest (behind `check_ninja`) exits 0. The
  alchemist experiment recorded in `Surprises & discoveries` is the
  planning-time evidence.
- Non-vacuity: the strategies produce both cyclic and acyclic graphs, and the
  property asserts equality of the full report (cycle and missing list), so a
  detector that read `dependency_order` in either direction would diverge on
  some generated case. The seeded fault (temporarily skipping `implicit_deps`
  when `Serial`) must fail the property; it is run once, recorded here, and not
  committed.

`LEM-1 alias traversal agrees with the edge graph.` For every `G` satisfying
`IR-AX-1`, `analyse(G).cycle.is_some()` if and only if the edge graph of `G`
has a cycle.

- Argument: (only if) every alias-graph arc `x -> y` induces the edge-graph arc
  `edge(x) -> edge(y)`, so an alias cycle maps to a closed edge walk, which
  contains a cycle. (if) given an edge cycle `e1 -> e2 -> ... -> ek -> e1`,
  choose for each `i` the alias `y(i+1)` of `e(i+1)` that `e(i)` depends on;
  then `y1 -> y2 -> ... -> yk -> y1` is an alias-graph cycle, because the
  successors of `y(i)` are `deps(e(i))`. The detector's depth-first search
  visits every alias, so it finds some cycle when one exists. Missing
  dependencies are not in the index and contribute no arc to either graph.
- Method: property test against an independent oracle.
- Rationale: the lemma is the correctness argument for walking aliases instead
  of edges, and becomes load-bearing once every dependency kind participates.
  An oracle comparison over generated multi-output graphs with every dependency
  kind is the proportionate check; see `Decision log` D6 for why no Verus proof
  is planned in this item.
- Domain: new strategy generating 1–12 edges, each with 1–3 aliases split
  between explicit and implicit outputs, and 0–4 dependencies per kind drawn
  from the alias set plus a pool of absent paths.
- Artefact: `src/ir/cycle_participation_property_tests.rs` (new, included from
  `src/ir/cycle.rs` under `#[cfg(test)]`), property
  `alias_traversal_agrees_with_edge_oracle`. The oracle builds the edge graph
  from `BuildGraph::edges()` and `edge_id_for_output` and runs Kahn's
  topological sort, which shares no code with the detector.
- Evidence: passes after EP-M2; fails before EP-M2 on any generated case whose
  only cycle uses an order-only arc (the oracle counts order-only arcs).
- Non-vacuity: the strategy is tuned so both classes occur; a companion
  deterministic test runs the strategy's generator with a fixed seed for 256
  cases and asserts that at least 20% are cyclic, at least 20% acyclic, and at
  least one cyclic case closes through an implicit output and one through an
  order-only arc. The seeded fault of dropping order-only traversal must be
  rejected (this is the EP-M2 red state).

`OB-4 unchanged diagnostics.` An order-only cycle yields
`IrGenError::CircularDependency` whose display and JSON forms have the same
shape as a `sources` cycle.

- Method: `insta` snapshot of the end-to-end JSON diagnostic, plus the
  existing `src/diagnostic_json_tests.rs` snapshots unchanged.
- Artefact: `tests/cycle_participation_e2e_tests.rs` (new) with snapshot
  `order_only_cycle_json_diagnostic`.
- Evidence: before EP-M2 the run exits 0 from `netsuke generate`, so the test
  fails; afterwards it exits non-zero and the snapshot matches.
- Non-vacuity: the snapshot contains the cycle path
  `["headers.stamp", "tool", "headers.stamp"]`, which only the order-only arc
  can close.

No other invariant is introduced. Canonicalization, missing-dependency
reporting order, and deterministic traversal order are unchanged and remain
covered by the 4.2.1 and 4.2.2 harnesses and the existing property suites.

## Milestones and plateaus

### EP-M1 — pin implicit-output participation

- Outcome: characterization tests and a Kani harness pin the existing rule
  that implicit outputs participate as aliases of their producing edge.
- Requirements and gaps: `RM-4.4.2-b`, `OB-2`.
- Acceptance evidence: `cycle_participation_tests::implicit_output_*` and
  `non_first_explicit_output_closes_cycle` pass; `make kani-ir` verifies
  `implicit_output_alias_cycle_reports_cycle`; its mutation patch applies and,
  when applied, fails the harness.
- Conformance check: no production code changes; ADR-004 bounds respected.
- Recovery: revert the milestone commit; nothing depends on it.
- Remaining gaps: order-only participation, documentation.
- Compatibility decision: none.

### EP-M2 — make order-only dependencies participate

- Outcome: the detector walks `order_only_deps`; every test and harness agrees.
- Requirements and gaps: `RM-4.4.2-a`, `RM-4.4.2-d`, `OB-1`, `OB-3`, `OB-4`,
  `LEM-1`.
- Acceptance evidence: the red tests listed in `Validation and acceptance`
  fail first, then pass; `make kani-ir` verifies the new harnesses; all
  mutation patches apply; the four standard gates pass.
- Conformance check: `netsuke::ir` public API unchanged; no new dependency; the
  deviation from `FV-CYCLE` is recorded as D1 and awaits approval with this
  plan.
- Recovery: revert the milestone commits; EP-M1 remains valid.
- Remaining gaps: user-facing and design documentation.
- Compatibility decision: none. `netsuke` is pre-1.0 (0.1.0-beta3 released);
  the behavioural change is announced, not shimmed.

### EP-M3 — document the rule

- Outcome: ADR-041 exists; the users' guide gains *Dependency and build-graph
  semantics* with a tested example; the design document, formal-verification
  document, developers' guide, Rustdoc, CHANGELOG, migration guide, and
  `docs/contents.md` agree with it.
- Requirements and gaps: `RM-4.4.2-c`, `RM-4.4.2-d`, `DES-5.3-4`, `FV-CYCLE`.
- Acceptance evidence: `tests/documentation_examples_tests.rs` runs
  `guide-order-only-cycle-manifest` and asserts rejection; `make markdownlint`
  and `make nixie` pass.
- Conformance check: every document that states the rule says the same thing;
  `rg -n "order.only" docs src` shows no surviving statement of exclusion.
- Recovery: documentation-only; revert the commit.
- Remaining gaps: roadmap closure.
- Compatibility decision: none.

### EP-M4 — close out

- Outcome: roadmap 4.4.2 and its four sub-items are ticked with a completion
  note; this plan is `COMPLETE`.
- Acceptance evidence: every gate listed in `Validation and acceptance` passes
  on the final commit.

## Plan of work

### Stage A — confirm (no code changes)

Re-run the ADR sweep and confirm 041 is free. Re-read `cycle_detector.rs` and
confirm the line of `visit_known_edge`. Record `make kani-ir` wall time on the
unchanged tree as the baseline in `Artefacts and notes`.

### Stage B — EP-M1 characterization

Create `src/ir/cycle_participation_tests.rs` with a `build_edge` fixture that
takes all five path lists (a small struct `EdgeSpec` with named fields keeps
the argument count within Clippy's limit) and an `rstest` table
`implicit_outputs_participate` with the three OB-2 cases. Assert with
`googletest` matchers (`assert_that!(report.cycle, some(eq(&expected)))`) and
`pretty_assertions::assert_eq` for path vectors. Include it from
`src/ir/cycle.rs` with
`#[cfg(test)] #[path = "cycle_participation_tests.rs"] mod participation_tests;`.

Create `src/ir/cycle_participation_verification.rs`, included from
`src/ir/cycle.rs` under `#[cfg(kani)]`, reusing the `edge` helper shape from
`cycle_verification.rs` but with an order-only and implicit-output parameter.
Add `implicit_output_alias_cycle_reports_cycle`. Write its mutation patch. Add
the row to *Kani harness inventory* in `docs/developers-guide.md`.

### Stage C — EP-M2 red, then green

Red: add the OB-1 `rstest` table `order_only_dependencies_participate` to
`cycle_participation_tests.rs`; invert `order_only_back_edge_has_no_cycle` into
`order_only_back_edge_produces_cycle`, asserting the reported cycle contains
the injected back edge; add `cycle_participation_property_tests.rs` with
`alias_traversal_agrees_with_edge_oracle`, its class-coverage companion, and
`dependency_order_does_not_change_analysis`; add the two order-only Kani
harnesses; add the BDD scenarios and data files; add
`tests/cycle_participation_e2e_tests.rs`. Run them and record the failures. The
Kani harnesses fail with a counterexample.

Green: in `src/ir/cycle_detector.rs::visit_known_edge`, after the
`implicit_deps` call, add a third `visit_dependencies` call over
`&edge.order_only_deps` with the same early return. Rename nothing else. Update
the Rustdoc on `visit_known_edge`, the module doc of `src/ir/cycle.rs`, and the
doc on `analyse`, replacing "`order_only_deps` are intentionally excluded" with
the new rule and a pointer to ADR-041. Update the module doc of
`cycle_issue322_property_tests.rs`. Regenerate the five existing cycle mutation
patches against the edited file and add the two new ones. Run the focused
tests, then `make kani-ir`, then the four standard gates.

Refactor: if `visit_known_edge` now reads as three repetitions, express it as
one loop over a fixed array of the three slices only if CBMC time does not
regress; otherwise leave the three calls and say why in a comment.

### Stage D — EP-M3 documentation, EP-M4 close

Write ADR-041 with sections *Status* (`Accepted` once this plan is approved),
*Date*, *Context and Problem Statement*, *Decision Drivers*, *Considered
Options* (keep exclusion; include order-only; include order-only but only
warn), *Decision Outcome*, and *Consequences*, citing the Ninja probe and the
GNU Make comparison.

In `docs/users-guide.md`, replace the two-sentence paragraph under *Targets,
inputs, and dependencies* with a one-line pointer, and add a new
`### Dependency and build-graph semantics` section immediately after *Run
direct dependencies serially* and before `## Use Jinja safely`. The section
states: what a cycle is; that `sources`, `deps`, and `order_only_deps` all
participate because a cycle through any of them can never be scheduled; that
the check covers the whole manifest, not only requested targets; that every
path in a multi-output `name` names the same build step; that a dependency
naming a path no target produces is treated as an existing file and cannot
close a cycle; that `dependency_order: serial` adds no dependency; and, for
Rust callers, that implicit outputs of a `BuildEdge` behave like explicit ones.
It carries one tested example, `guide-order-only-cycle-manifest`, showing a
rejected order-only cycle and the diagnostic text. Register that identifier in
`EXPECTED_EXAMPLE_IDS` and add a case that runs `netsuke generate` and asserts
failure and the localized message. Add a sentence to *Use the canonical build
graph* pointing at the new section for alias semantics.

Update `docs/formal-verification-methods-in-netsuke.md` *Cycle-participation
contract*, `docs/netsuke-design.md` §5.3 step 4 (also correcting "Keys are
cloned"), the two developers' guide passages, `docs/contents.md` (ADR-041 under
*Decision records*, newest first), `CHANGELOG.md` under *Unreleased* →
*Changed* with a **Breaking** entry, and `docs/v0-1-0-migration-guide.md`
*At-a-glance changes* plus a short section *Remove order-only dependency
cycles*. Run `make fmt`, then the Markdown gates.

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

The step `the IR cycle is {path:string}` is new, in `tests/bdd/steps/ir.rs`; it
splits on ` -> ` and compares with the `cycle` field of the stored
`CircularDependency` error. The exact expected paths are confirmed against
canonicalization during the red run. The data files are:

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

`tests/data/serial_earlier_depends_on_later.yml` is the manifest from the
alchemist experiment in `Surprises & discoveries`.

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
cargo nextest run --lib -E 'test(/cycle_participation|order_only_back_edge/)' \
  2>&1 | tee /tmp/red-netsuke-4-4-2-document-cycle-detection-scope.out
cargo nextest run --test bdd_tests -E 'test(/order_only|non_first_output|serial_ordering_adds/)' \
  2>&1 | tee -a /tmp/red-netsuke-4-4-2-document-cycle-detection-scope.out
cargo nextest run --test cycle_participation_e2e_tests \
  2>&1 | tee -a /tmp/red-netsuke-4-4-2-document-cycle-detection-scope.out
```

Expected red excerpt before the detector change:

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

- Red: the focused commands above, with the OB-1 cases, the inverted property,
  the oracle property, the BDD order-only scenario, the end-to-end test, and
  the two order-only Kani harnesses failing because no cycle is reported.
- Green: the same commands passing after the one-call detector change.
- Refactor: focused commands again, then all gates.

Quality criteria:

- Tests: `make test` passes, including every new test named in this plan.
- Verification: `make kani-ir` reports every harness `SUCCESSFUL`, including
  the three new ones; `tests/kani_mutation_evidence_tests.rs` passes; each new
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

Ninja 1.11.1 probe, 2026-09-27, scratch `/tmp/ninja-cycle-wfFe`:

```plaintext
# build a: touch || b ; build b: touch a
ninja: error: dependency cycle: a -> b -> a        exit=1
# build a | a.extra: touch b ; build b: touch a.extra
ninja: error: dependency cycle: a.extra -> b -> a.extra   exit=1
# build a: touch || a
ninja: error: dependency cycle: a -> a             exit=1
```

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

No public interface changes. New crate-private test and harness modules:

- `src/ir/cycle_participation_tests.rs` (`#[cfg(test)]`).
- `src/ir/cycle_participation_property_tests.rs` (`#[cfg(test)]`).
- `src/ir/cycle_participation_verification.rs` (`#[cfg(kani)]`).
- `tests/cycle_participation_e2e_tests.rs`, registered as the integration-test
  wiring contract requires (see `tests/integration_test_wiring_tests.rs`).

The only production edit is inside
`crate::ir::cycle::detector::CycleDetector::visit_known_edge`. No configuration
surface is added, so `ortho_config` is not involved. From the
hexagonal-architecture view, the cycle rule is domain policy inside the IR
core; it has no port, and none is introduced. Ninja is the driven adapter whose
semantics the domain rule now matches; the adapter (`src/ninja_gen`) does not
change.

## Progress

- [x] (2026-09-27) Renamed branch to `4-4-2-document-cycle-detection-scope`.
- [x] (2026-09-27) Reconnaissance of detector, tests, and documentation.
- [x] (2026-09-27) Probed Ninja 1.11.1 and GNU Make 4.4.1 cycle behaviour.
- [x] (2026-09-27) Serial-ordering hypothesis H1 tested: not falsified.
- [x] (2026-09-27) Baseline O2 recorded: `generate` accepts an order-only
  cycle and `build` fails inside Ninja.
- [ ] Design review by expert panel and plan revision.
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

## Decision log

- D1. Decision: order-only dependencies participate in cycle detection.
  Rationale: a cycle is a scheduling impossibility, not a freshness question;
  an order-only dependency must still be built first, so a cycle through one
  can never run. Ninja rejects it, so excluding it only defers the failure to
  Ninja's untranslated message and lets `netsuke generate` emit a file Ninja
  refuses. This reverses the stated rationale in `FV-CYCLE`, the users' guide,
  the design document, and the developers' guide; that deviation is surfaced
  for approval with this plan and recorded in ADR-041. Date/Author: 2026-09-27,
  planning agent; pending approval.
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
  witnesses discharge the lemma for this item. Date/Author: 2026-09-27,
  planning agent.
- D7. Decision: no new diagnostic wording naming the dependency kind on the
  cycle. Rationale: the diagnostic key and JSON schema are constraints; kind
  annotation is a separate usability change. Date/Author: 2026-09-27, planning
  agent.
- D8. Decision: allocate ADR-041. Rationale: 038 is the highest on `main`;
  039 and 040 are taken on open branches; 030 and 031 were vacated by a
  renumbering and are avoided to prevent confusion. Date/Author: 2026-09-27,
  planning agent.

## Outcomes & retrospective

Not started.

## Revision note

Revision 1 (2026-09-27): initial draft.

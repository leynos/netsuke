# 4.3.1. Add Proptest coverage for deterministic Ninja emission

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT — AWAITING APPROVAL (revised after design review)

## Purpose / big picture

Netsuke reads a `Netsukefile` manifest and writes a `build.ninja` file that the
Ninja build tool then executes. `docs/netsuke-design.md` §1.2 states the
promise: "Netsuke's pipeline is **deterministic**. Given the same `Netsukefile`
and environment variables, the generated `build.ninja` will be byte-for-byte
identical."

That promise is held up by three explicit sort calls inside the emitter, and by
an invariant established a layer away in the manifest-to-IR lowering. Nothing
in the test suite stops a future change from deleting a sort, reordering
validation, or adding a new collection whose iteration order leaks into the
output. Existing coverage is eight fixed-manifest snapshots plus narrow
properties over single-edge graphs.

After this work, a maintainer who deletes any one of the emitter's ordering
guarantees sees a named property fail with a small, reproducible
counter-example. A maintainer who adds a `HashMap` to the emission path and
iterates it directly fails a source-shape contract test. And the guarantee that
users actually read — run Netsuke twice, get the same bytes — is checked
end-to-end through the binary, which nothing checks today.

The work also discharges obligations that earlier items deferred here. Roadmap
`4.2.1` and `ADR-004` both record that Kani proves duplicate-output rejection
and cycle rejection only for one to three nodes, and that "the larger-N graph
property is handed off to the future Proptest roadmap item `4.3.1`". There is a
crisp reason Kani cannot close this itself: under `#[cfg(kani)]`, `IrHashMap`
becomes an ordered bounded map (`src/ir/graph_kani_map.rs`), so the
nondeterminism under test does not exist in the verification build.

Finally, the plan settles a contract question that
`docs/formal-verification-methods-in-netsuke.md` flags but no roadmap item
owns: which determinism statement is a public guarantee. Verifying an unstated
property is verification without a specification, so the contract is written
**first**, in `EP-M1`, before any property is authored.

Success is observable without reading code. Run `make proptest` and see the
suite pass. Apply a recorded mutation patch, re-run, and see one named property
fail with a minimal counter-example. Revert. Run `netsuke generate` twice on a
fixture and diff the bytes. Read `docs/users-guide.md` and find a stated
guarantee that matches what the tests check.

## Revision note on this draft

This plan was rewritten after a six-lens design review. The review found the
first draft structurally unsound in ways that mattered, and the changes are
substantial enough that reviewers of the first draft should re-read rather than
diff. What changed, and why, is recorded in full in `Design review findings`
near the end. The four that reshaped the plan:

1. The shared strategy cannot live in `test_support`. This was **proven by
   compile probe**, not argued: passing a `netsuke`-typed value from
   `test_support` into a `src/`-side `#[cfg(test)]` module fails with "there
   are multiple different versions of crate `netsuke` in the dependency graph".
   All strategies therefore live in `src/`.
2. The insertion-order property as first drafted was **not a deterministic
   function of its Proptest seed**, because `RandomState` is outside Proptest's
   control. That silently breaks shrinking and makes committed regression seeds
   decorative. The obligation is restructured around extracted pure ordering
   helpers over explicitly shuffled vectors.
3. `docs/verification/mutations/` is governed by an existing contract test,
   `tests/kani_mutation_evidence_tests.rs`, which the first draft did not know
   existed. It rejects any patch stem whose first segment is not `ir`, so the
   proposed `ninja_gen` patches were unrepresentable.
4. `src/ninja_gen/mod.rs` is exactly 400 lines, the ceiling `AGENTS.md`
   imposes, so the first draft breached a constraint on its first commit.

## Context and orientation

### What Netsuke is

Netsuke is a Rust command-line build front end. A user writes a `Netsukefile`
in YAML. Netsuke parses it, expands control constructs (`foreach`, `when`),
renders string fields with MiniJinja, lowers the result into an intermediate
representation (IR) called a *build graph*, and emits a `build.ninja` file.
Ninja, a separate program, reads that file and runs the build. Netsuke does not
run compilers itself.

The crate is published as `netsuke-build`; the library target is `netsuke`.
Nothing in this plan changes the compiled behaviour of the shipped binary.

### The emission pipeline this plan targets

`src/ir/graph.rs` defines the build graph:

```rust
pub struct BuildGraph {
    pub actions: IrHashMap<String, Action>,
    pub targets: IrHashMap<Utf8PathBuf, BuildEdge>,
    pub default_targets: Vec<Utf8PathBuf>,
}
```

`IrHashMap<K, V>` is a plain `std::collections::HashMap` in ordinary builds
(`src/ir/graph.rs:28`) and an ordered bounded map under `#[cfg(kani)]`. An
`Action` is a recipe plus Ninja rule metadata; a `BuildEdge` is one `build`
statement.

`src/ir/from_manifest.rs` builds the graph. Four facts are load-bearing:

1. Actions are interned by content hash. `register_action`
   (`src/ir/from_manifest_support.rs:48`) hashes the `Action` with
   `crate::hasher::ActionHasher::hash` and uses the hexadecimal digest as the
   map key.
2. Duplicate outputs are rejected. `find_duplicates`
   (`src/ir/from_manifest_support.rs:306`) fails the lowering if any output
   path is claimed twice, across targets or within one target. So no two
   distinct edges in a graph built this way share an output path.
3. A multi-output edge is stored **once**, not once per output.
   `insert_edge_for_outputs` (`src/ir/from_manifest_support.rs:142`) is a thin
   wrapper over `graph.insert_edge`, which validates the output aliases and
   calls `insert_canonical_edge` to push a single arena entry
   (`src/ir/graph.rs:82`). An output-less edge is pushed the same way and is
   therefore *not* dropped: `render_edges` emits a `build` line for it with an
   empty left-hand side. Whether a manifest can construct one is unresolved and
   is `EP-M4`'s question, not an established drop. See `ADR-030` amendments 1
   and 2.
4. `register_action` hard-codes
   `depfile: None, deps_format: None, pool: None, restat: false`. No manifest
   can currently populate those fields.

`src/ninja_gen/` emits the text. `generate` (`src/ninja_gen/mod.rs:105`) is the
simple path and rejects graphs needing staged serial lowering. `generate_bundle`
(`src/ninja_gen/dyndep.rs:88`) is what production uses and returns
`GeneratedNinja { build_file, dyndep_files }`. Both:

1. `reject_unsupported_path_characters` — rejects `$`, `:`, `|`, and any
   Unicode control character including NUL (`src/ninja_gen/path_syntax.rs:52`).
2. `reject_reserved_paths` — rejects paths under `.netsuke/serial` or
   `.netsuke/dyndep`.
3. `write_action_rules` — sorts `graph.actions` by map key, writes one `rule`
   block per non-dependency-only action.
4. Edge rendering — sorts `graph.targets.values()` by
   `path_key(&edge.explicit_outputs)`, deduplicates on the same key, writes one
   `build` statement per surviving edge.

Then, if `default_targets` is non-empty, both clone it, `sort()`, and write one
`default` line last.

`path_key` (`src/ninja_gen/mod.rs:242`):

```rust
pub(crate) fn path_key(paths: &[Utf8PathBuf]) -> String {
    let mut parts: Vec<String> = paths.iter().map(|p| p.as_str().to_owned()).collect();
    parts.sort_unstable();
    parts.join(&char::from(0).to_string())
}
```

### What "deterministic" means here, precisely

Four statements are easy to conflate. The plan keeps them apart, and `EP-M1`
decides which are public.

The **process-level statement**: the same manifest, environment, platform, and
Netsuke version give byte-identical output. This is what users care about and
what caching depends on. Nothing tests it today.

The **graph-level statement**: emission is invariant under `HashMap` insertion
order. Two `BuildGraph` values equal as values but whose maps were populated in
different sequences emit identical bytes. This is what roadmap `4.3.1` names.

The **declaration-level statement**: permuting the order of target declarations
in a manifest does not change the emitted bytes. Stronger, and not currently
stated anywhere.

The **platform-and-shell parameter**, which the first draft wrongly omitted.
`generate`, `generate_bundle`, *and* `from_manifest` are all thin wrappers over
`*_for_shell` functions taking `RecipeShell::host_default()`, which is
`PowerShell` on Windows and `Posix` elsewhere, overridable at runtime by
`NETSUKE_WINDOWS_SHELL` whose `Bash` route additionally depends on a filesystem
probe. PowerShell takes an `rspfile` branch that POSIX does not
(`src/ninja_gen/mod.rs:379`). Emitted bytes are therefore a function of
(manifest, environment, platform, shell selection, Netsuke version). The design
document's unqualified claim is false across platforms as written, and the
contract must name the parameter tuple.

What is explicitly **not** claimed: within a single edge, `DisplayEdge`
(`src/ninja_gen/display_edge.rs:22`) renders each path vector verbatim in
declaration order, so emission is *not* invariant under permuting an edge's own
output list — even though `path_key` is. Nor is determinism claimed for error
paths: `reject_unsupported_path_characters` and `reject_reserved_paths` both
iterate `graph.targets.values()` and return on the *first* offender, so which
error a multi-fault graph reports is insertion-order dependent.

### Prior art in this repository

`src/graph_view/tests_property.rs` already contains
`graphview_is_insertion_order_invariant`: it builds the same logical graph
twice with reversed insertion order, projects both through
`GraphView::from_build_graph`, and asserts equality of the view and two
renderers — in 169 lines, with no shared strategy crate. The first draft of
this plan claimed "there is no arbitrary-`BuildGraph` strategy today", which is
wrong.

This matters three ways. Its `arb_graph_inputs` is the strategy this plan must
absorb or explicitly diverge from, or the repository ends up with two
incompatible `BuildGraph` generators. Its `retain_disjoint_output_edges`
*drops* conflicting elements rather than filtering the case, which is the house
answer to the filtering trap and is adopted here. And `GraphView` is itself a
canonical projection of `BuildGraph` computed by a structurally different
mechanism (`BTreeMap`/`BTreeSet` iteration rather than explicit `sort_by_key`),
which makes it usable as a differential oracle rather than only prior art.

### What Proptest is and how this repository already uses it

Proptest generates values from a *strategy*, checks a property, and shrinks
failures to a minimal counter-example. It is a dev-dependency at `1.11.0`.

House conventions, confirmed against code: the `proptest! { }` macro block;
`prop_assert*` rather than `assert!`/`unwrap`; strategies as functions returning
`impl Strategy<Value = T>` (no file implements `Strategy` or `Arbitrary`
directly, and neither `proptest-derive` nor `test-strategy` is a dependency);
inline `#![proptest_config(...)]` case counts between 8 and 256; library-side
suites in sibling files wired from the production module with
`#[cfg(test)] #[path = ...]`; committed regression seeds.

One deviation is already established:
`src/ninja_gen_property_tests/ninja_oracle.rs` drives `TestRunner::run`
directly rather than using the macro, because it must skip when the `ninja`
binary is absent. This plan reuses that pattern wherever a post-run assertion
or a conditional skip is needed.

### Terms used in this plan

- **Build graph / IR** — the `BuildGraph` value above.
- **Edge** — one `BuildEdge`, rendered as one Ninja `build` statement.
- **Action** — one `Action`, rendered as one Ninja `rule` block.
- **Dyndep staging** — when an edge has `DependencyOrder::Serial` and more than
  one implicit dependency, `generate_bundle` lowers it into Ninja `dyndep`
  sidecar files under `.netsuke/dyndep/` so the dependencies run in declared
  order. `generate` refuses such graphs. See `ADR-011`.
- **Well-formed graph** — a `BuildGraph` satisfying the invariants
  `from_manifest` establishes: every edge has at least one explicit output;
  explicit output sets are pairwise disjoint and internally duplicate-free; no
  output path is empty; every `action_id` exists in `actions`; every edge under
  key `k` has `k` among its explicit outputs; and no path contains a rejected
  character.
- **Insertion-order permutation** — building two `BuildGraph` values from the
  same pairs in two different sequences.
- **Metamorphic property** — one relating outputs of two runs on *related*
  inputs, rather than checking one output against a fixed expectation.
- **Differential oracle** — comparing against an independently computed
  reference (here, `GraphView`, or the real `ninja` binary).
- **Non-vacuity** — evidence the property could actually fail.
- **Mutation patch** — a committed `.patch` deliberately breaking one
  production behaviour, used to show a named test detects it.

## Signposts: documentation and skills

Repository documentation, in the order it becomes useful:

- `docs/roadmap.md` §4.3 (the item) and §4.2 (the deferred obligations).
- `docs/formal-verification-methods-in-netsuke.md` §"Proptest for determinism
  and manifest semantics" (`FV-DET`) and §"Determinism contract"
  (`FV-CONTRACT`).
- `docs/adr-004-bound-kani-ir-harnesses-to-small-n.md` (`ADR-004`).
- `docs/netsuke-design.md` §1.2, §5.4, §5.5.
- `docs/developers-guide.md` §"Property-based testing with proptest".
- `docs/adr-011-use-ninja-dyndep-for-serial-dependency-ordering.md`.
- `docs/documentation-style-guide.md` and `AGENTS.md`.
- `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md`,
  `docs/snapshot-testing-in-netsuke-using-insta.md`,
  `docs/rstest-bdd-users-guide.md`, `docs/rust-doctest-dry-guide.md`.

Code and configuration that constrains this work, all of which the first draft
missed and none of which is optional reading:

- `tests/kani_mutation_evidence_tests.rs` — the contract governing
  `docs/verification/mutations/`. Read before writing any patch.
- `src/graph_view/tests_property.rs` — the prior-art insertion-order property.
- `src/ninja_gen_property_tests/ninja_oracle.rs` — the `TestRunner::run` and
  skip-when-absent pattern.
- `test_support/src/ninja.rs` — `ninja_is_required` and the
  `NETSUKE_REQUIRE_NINJA` escalation, set in `.github/workflows/ci.yml:78` and
  `coverage-main.yml:77` but **not** in `ci-windows.yml`.
- `.config/nextest.toml` — the explicit no-blanket-retry policy.
- `.github/workflows/mutation-testing.yml` — `cargo-mutants`, nightly at 03:05
  UTC over `src/`, informational.
- `clippy.toml` — `std::env::var`/`var_os` are in `disallowed-methods`.

Deliberate non-reference: `docs/rfcs/0012-netsukefile-property-testing.md` is a
manifest-author-facing feature for Netsuke *users* (roadmap phase 10),
unrelated to this item's internal Rust property tests.

Agent skills: `execplans`; `rust-router` then `rust-verification` then
`proptest`; `rust-unit-testing`; `hexagonal-architecture`; `codegraph-mcp`;
`arch-decision-records`; `en-gb-oxendict`.

External reference: the Ninja manual v1.13.1, specifically "A default target
statement must appear after the build statement that declares the target as an
output file", which the emitter satisfies by writing `default` last.

## Conformance basis

There is no Terms of Reference document in this repository. Upstream artefacts:

- `docs/roadmap.md` item 4.3.1 and its three sub-items, `RM-4.3.1.a`
  (insertion-order stability), `RM-4.3.1.b` (`default` ordering), `RM-4.3.1.c`
  (`path_key` invariance).
- `docs/roadmap.md` item 4.2.1, whose first and third sub-items record that
  "4.3.1 closes the larger-N Proptest coverage": `RM-4.2.1.dup` and
  `RM-4.2.1.cyc`.
- `docs/roadmap.md` item 4.2.2 (cycle canonicalization, complete). `OBL-CYCLE`
  must state its boundary against it: this plan checks *rejection*, not the
  canonical *content* of the reported cycle, which 4.2.2 owns.
- `FV-DET`, which lists a fourth bullet the roadmap omits — "action-hash
  stability for field-preserving permutations" — discharged as `OBL-ACTION`.
- `FV-CONTRACT`, the requirement to decide and document what is guaranteed.
- `ADR-004`, recording the hand-off.
- A new architecture decision record, referred to throughout by the stable
  identifier **`ADR-NNN`** and now concretely numbered **`ADR-030`**. The first
  draft deliberately deferred allocation because `adr-021-*` was then claimed
  on four open branches and `adr-020` on two. That rationale is stale:
  `origin/main` merged `adr-020` through `adr-029`, and `adr-039`–`adr-041` are
  claimed in flight. Allocating now, having swept every ref and every open pull
  request, avoids a second collision. `ADR-NNN` is retained as the trace
  identifier so the links below stay stable; it denotes `ADR-030`.

Trace links:

```plaintext
RM-4.3.1.a   -> FV-DET      -> EP-M5 -> ninja_gen::determinism::order::ordered_edges_ignore_input_order
RM-4.3.1.a   -> FV-DET      -> EP-M5 -> ninja_gen::determinism::order::emission_is_insertion_order_invariant
RM-4.3.1.b   -> FV-DET      -> EP-M5 -> ninja_gen::determinism::defaults::default_line_is_the_ascending_sort
RM-4.3.1.c   -> FV-DET      -> EP-M4 -> ninja_gen::determinism::path_key::path_key_is_permutation_invariant
RM-4.3.1.c   -> FV-DET      -> EP-M4 -> ninja_gen::determinism::path_key::path_key_is_injective_on_validated_paths
FV-DET       -> ADR-NNN     -> EP-M4 -> ninja_gen::determinism::no_loss::every_distinct_edge_is_emitted_once
FV-DET       -> ADR-NNN     -> EP-M5 -> tests::ninja_gen_hashmap_boundary::no_hashmap_iteration_in_ninja_gen
FV-DET       -> FV-DET      -> EP-M6 -> ir::action_hash_property_tests::equal_actions_hash_equally
FV-CONTRACT  -> ADR-NNN     -> EP-M6 -> tests::ninja_determinism_process::two_runs_emit_identical_bytes
RM-4.3.1.a   -> ADR-NNN     -> EP-M6 -> ninja_gen::determinism::declaration::declaration_order_does_not_change_emission
RM-4.2.1.dup -> ADR-004     -> EP-M7 -> ir::graph_property_tests::duplicate_outputs_are_rejected_at_larger_n
RM-4.2.1.cyc -> ADR-004     -> EP-M7 -> ir::graph_property_tests::cycles_are_rejected_at_larger_n
FV-CONTRACT  -> ADR-NNN     -> EP-M1 -> docs/adr-030-ninja-emission-determinism-contract.md
```

Roadmap §4.4 is titled "Contract documentation and optional proof kernels", and
4.4.1 is its structural twin — a verification item handing a contract to a
documentation item. This plan writes `ADR-NNN` in `EP-M1` because the contract
must exist before the properties that check it, and proposes adding roadmap
4.4.4 "Document the Ninja emission determinism contract" to own the
*user-facing prose* if reviewers prefer that split. Recorded as an open
question.

## Constraints

1. No change to the observable behaviour of the shipped binary. Pure,
   behaviour-preserving refactors are permitted and expected — `EP-M2` splits
   two files and extracts two ordering helpers — but no change to what any
   input produces. If a property fails against current production code, that is
   a discovery to escalate, not to patch.
2. No widening of the public API of `netsuke::ir` or `netsuke::ninja_gen`.
   `path_key` stays `pub(crate)`; library-side tests reach it through `super::`.
3. No new `[dependencies]`, `[dev-dependencies]`, or `[build-dependencies]`.
   `proptest 1.11.0`, `rstest`, `googletest`, `pretty_assertions`, `insta`, and
   `assert_cmd` are present and sufficient. Do not add `proptest-derive` or
   `test-strategy`.
4. No file may exceed 400 lines. `src/ninja_gen/mod.rs` is **at** 400 and
   `tests/kani_mutation_evidence_tests.rs` is at 383; both are split in `EP-M2`
   before anything is added to them.
5. No in-process environment mutation in tests, and no direct `std::env::var`
   or `var_os` — both are in `clippy.toml`'s `disallowed-methods` and
   `make lint` runs with `-D warnings`. Use an injected `Env`, as
   `test_support/src/ninja.rs` does. Subprocess isolation via `assert_cmd` or
   `Command::env` is the only exemption.
6. Strategies construct valid values; they do not filter for them. Where
   conflicts are unavoidable, *drop* the conflicting elements as
   `retain_disjoint_output_edges` does, rather than rejecting the case.
7. Every property is validated by a mutation that it detects, recorded as a
   patch under `docs/verification/mutations/` and conforming to the contract in
   `tests/kani_mutation_evidence_tests.rs`.
8. Regression seeds are committed. Because a seed is a persisted RNG seed and
   not a value, any strategy change silently repurposes it; every retained seed
   is therefore paired with a directed `#[test]` encoding the concrete
   counter-example as a literal.
9. Assertions inside `proptest!` bodies use `prop_assert*`. Where a property
   needs a post-run assertion or a conditional skip, it uses the
   `TestRunner::run` form established in
   `src/ninja_gen_property_tests/ninja_oracle.rs`, and ordinary assertions are
   permitted *after* the runner returns.
10. Generated graphs stay within the roadmap bound of 50 actions and 100 edges,
    **and** within a total of 200 explicit outputs. The second bound is a
    **cost budget**, not a correctness bound, and the mechanism once stated for
    it is stale: `insert_edge_for_outputs` no longer stores an edge once per
    output. Commit `2c030fd1` ("Store multi-output build edges once", #652/#714)
    made it call `graph.insert_edge` directly, which pushes a single arena
    entry, so `graph.edges()` yields the edge count and the emitter's sort sees
    that rather than `targets.len()`. The bound is retained because the measured
    per-case cost table was derived under it and the cost is real; the mechanism
    is withdrawn. See `ADR-030`, amendment 1.
11. All prose is en-GB-oxendict, wrapped at 80 columns; code blocks at 120.
    Markdown must be `mdtablefix`-canonical. After running `make fmt` over
    authored prose, read the diff: `--renumber` will silently convert a wrapped
    line beginning with a number and a full stop into an ordered-list item, and
    no gate catches it.
12. `make check-fmt`, `make typecheck`, `make lint`, `make doc-coverage`,
    `make test`, `make markdownlint`, and `make nixie` pass at every milestone
    boundary.

## Tolerances (exception triggers)

- **Scope.** Stop if the change touches more than 34 files beyond this
  ExecPlan. That number is deliberately larger than the first draft's 16, which
  its own artefact list breached on day one; a tolerance that fires immediately
  teaches the reader to ignore tolerances. The budget is roughly 12 new test
  and strategy files, 5 mutation patches, 1 ADR, 8 edited documents, and 8
  edited source or configuration files. Also stop if net added lines exceed
  2,600 — the axis on which the 4.2.3 precedent overran by 2.2x and which its
  successor dropped.
- **Production change.** If any property cannot pass without changing
  production behaviour, stop immediately, record the counter-example, and
  escalate. Note that `OBL-E2E` no longer carries a pre-authorized weakening
  clause: narrowing it is an ADR amendment with a visible diff, not a drafting
  choice.
- **API widening.** If a test needs a symbol more public than `pub(crate)`,
  stop and present options.
- **Runtime.** The measured budget is ~6 s of CPU for the whole new suite. Stop
  and reconsider if it exceeds 30 s, or if any single test approaches
  `.config/nextest.toml`'s 60-second slow warning. That file SIGKILLs a test at
  300 s and a killed process persists no regression seed, so the 60-second warn
  is the real ceiling, not a soft one.
- **Shrinking.** `max_shrink_iters` defaults to `4 x cases`; a `GraphSpec` has
  roughly 1,500 shrink dimensions, so full minimization is not achievable
  within the timeout. Set `max_shrink_time` to 30 s on the heavy properties,
  accept partial minimization, and rely on the compact `Debug` from `EP-M3`. If
  counter-examples are still unreadable, stop and redesign the strategy rather
  than raising the iteration cap into the kill window.
- **Assertion shape.** A property whose assertion is independent of the input's
  internal representation **must** carry a structural gradient or a compact
  value-level diagnostic. `EP-M0` question 4 measured this: with a predicate
  having no gradient, shrinking consumed the full 30-second wall at ~940
  candidates/s and stopped on the wall rather than on a minimum, producing a
  different "minimal" input on each run of the same seed. So: no whole-graph
  hash comparison as an assertion, and pair any representation-independent
  assertion with the `classify()`-plus-digest diagnostic prototyped in `EP-M0`.
  If a compact diagnostic cannot be constructed for an obligation, stop and
  redesign the obligation to assert something with a gradient.
- **Rejection rate.** If any strategy needs `prop_filter` or `prop_assume!` on
  a structural condition, stop and redesign to construct or drop instead.
- **Mutation discipline.** If any property still passes with its patch applied,
  stop and redesign the property.
- **Contract test.** If generalizing `tests/kani_mutation_evidence_tests.rs`
  requires more than a mechanical widening of `supplemental_property_location`
  plus a file split, stop and escalate: that file is a repository-wide rot
  detector and weakening it is worse than dropping a patch.
- **Lint friction.** If Clippy or Whitaker cannot be satisfied without a broad
  `#[allow(...)]`, stop and escalate.
- **Gates.** If a gate fails after two focused fix attempts, stop and escalate
  with the captured `/tmp` log paths.
- **Review.** If `coderabbit review --agent` raises unresolved concerns, do not
  proceed until they are addressed or waived.
- **Contract disagreement.** If `ADR-NNN` cannot state the guarantee without
  contradicting `README.md` or `docs/netsuke-design.md` §1.2, stop and escalate
  before editing either.

## Risks

- **A property fails against current production code.** *Severity: high.
  Likelihood: moderate.* Most likely on `OBL-E2E`, which depends on the
  lowering never smuggling declaration order into an edge. *Mitigation:*
  `EP-M0` spikes it before `EP-M1` writes the contract.
- **Weak strategy makes a strong property look strong.** *Severity: high.
  Likelihood: moderate.* *Mitigation:* per-case non-vacuity assertions wherever
  the restructured obligations allow, recorded classification counts elsewhere,
  and named intersection classes rather than only marginal ones.
- **Shrinking is unsound because the predicate is not seed-determined.**
  *Severity: high. Likelihood: was certain in the first draft.* *Mitigation:*
  `OBL-ORDER` is restructured so its core is a pure function of the seed, and
  the end-to-end variant holds the graph value fixed and grades itself
  inconclusive when no case's iteration orders diverged, rather than
  re-materializing and hoping. The original bounded-retry mitigation was
  **falsified by `EP-M0` question 1** — see `OBL-ORDER` — so this risk is
  partially realized and now closed by construction rather than by retry.
- **A counter-example cannot be shrunk to something readable.** *Severity:
  moderate. Likelihood: measured, certain for representation-independent
  properties.* *Mitigation:* `Tolerance 5` requires every such property to
  state a compact, value-level diagnostic — class counts plus a digest — so a
  failure is diagnosable from its printed case without a re-run log. See
  `EP-M0` question 4.
- **Mutation patches rot.** *Severity: moderate. Likelihood: high.* The 4.2.3
  retrospective records all five of its patches going stale twice in two days.
  `every_patch_applies_cleanly` turns rot into a red `make test` for whoever
  touched the code, not whoever owns the patch. *Mitigation:* five patches, not
  eleven; prefer single-file patches; lean on the existing nightly
  `cargo-mutants` job for the faults nobody wrote a patch for.
- **`git apply --check` proves a patch applies, not that it still kills.**
  *Severity: moderate. Likelihood: moderate.* *Mitigation:* `EP-M8` re-applies
  every patch against the final tree and records which properties failed.
- **Real-Ninja oracle silently vanishes from a lane.** *Severity: moderate.
  Likelihood: moderate.* `ci-windows.yml` installs Ninja but does not set
  `NETSUKE_REQUIRE_NINJA`, so that lane skips silently today. *Mitigation:* use
  the existing `ninja_is_required` mechanism and record the Windows gap as a
  finding for a separate item rather than fixing it here.
- **~~Regression seeds in `tests/` are inert.~~** ***Falsified 2026-09-27.***
  Proptest's default `SourceParallel` persistence does print
  `failed to find lib.rs or main.rs` from an integration-test crate, but that
  is a fallback notice, not a failure: it retries against the crate root,
  saves, and replays. Verified by `EP-M0` question 5. The three suspect files
  are live. No mitigation is required, and the scope note that proposed a
  separate fix for them is withdrawn.
- **Counter-examples are unreadable.** *Severity: moderate. Likelihood: high.*
  Proptest prints the input's `Debug` regardless of the assertion message; a
  50/100 `GraphSpec` is tens of kilobytes. *Mitigation:* a handwritten compact
  `Debug` is part of `EP-M3`, not a later refinement — and `EP-M0` question 4
  showed it is not sufficient on its own, because the *shrink* does not
  converge. `Tolerance 5` now bounds what such a property may assert.
- **ADR number collision.** *Severity: low. Likelihood: high.* Four branches
  claim `adr-021` and two claim `adr-020`. *Mitigation:* `ADR-NNN` placeholder,
  allocated in the final commit.

## Verification plan

### Axioms (assumed, not proved)

- **AXIOM-PROPTEST.** Proptest 1.11.0 generates and shrinks correctly. Its
  internals are out of scope. Verified for this plan's use: `prop_shuffle`
  exists and `Vec<T>: Shuffleable`; `max_shrink_iters()` returns `cases * 4`
  when unset.
- **AXIOM-HASHMAP.** `std::collections::HashMap` with `RandomState` gives no
  iteration-order guarantee, and distinct instances generally draw distinct
  keys. Crucially, those keys are **not** part of the Proptest seed, so any
  predicate depending on them is not seed-reproducible. This axiom is the reason
  `OBL-ORDER` is shaped the way it is rather than the naive way.
- **AXIOM-CAMINO.** `Utf8PathBuf`'s derived `Ord` is lexicographic over the
  UTF-8 string, so `Vec::sort` gives a total, content-determined order.
- **AXIOM-HASHER.** `ActionHasher::hash` is a deterministic pure function of the
  `Action`'s canonical JSON serialization. Its collision resistance is out of
  scope, which is why `OBL-ACTION` claims only the direction Proptest can
  support.
- **AXIOM-NINJA.** The `ninja` binary parses per its manual. Two behaviours were
  measured against ninja 1.11.1 rather than assumed: a `default` statement
  preceding its `build` statement **is** rejected (`unknown target`); and
  `ninja -t commands` does **not** load dyndep sidecars, so a missing sidecar
  exits 0 under `-t commands` and fails only under `-n` or a real build.
- **AXIOM-SORT.** `sort_by_key` is stable and calls its key function O(n log n)
  times. Stability matters: if the edge sort key stopped being unique, ties
  would fall back to `HashMap` order invisibly.

### Obligations

______________________________________________________________________

**OBL-PATHKEY** — *`path_key` is permutation-invariant, and injective over
non-empty validated paths.*

Statement: for every path list `p` and permutation `q` of `p`,
`path_key(p) == path_key(q)`. Further, for lists `p` and `r` whose elements are
all non-empty and contain no NUL, `path_key(p) == path_key(r)` if and only if
`p` and `r` are permutations of one another.

The non-emptiness precondition is not decoration. `path_key([])` and
`path_key([""])` are both `""`, and the empty string passes
`unsupported_character` because it contains no rejected character. The first
draft stated injectivity without it and was simply wrong.

- **Method:** Proptest, plus directed witness tests.
- **Rationale:** `RM-4.3.1.c`. Permutation invariance makes `path_key` a
  canonical key for an output *set*; injectivity makes it safe as a dedup key.
- **Domain:** lists of 0 to 8 paths from a shared pool, permuted with
  `prop_shuffle`; the injectivity arm draws two lists from one pool so
  collisions are reachable.
- **Oracle:** the permutation arm compares `path_key` against itself on a
  shuffled input. The injectivity arm compares against sorted-multiset equality
  computed in the test — structurally different from join-with-NUL.
- **Artefact:** `src/ninja_gen/determinism/path_key.rs`.
- **Non-vacuity:**
  - *Covers.* Per-case *classification* (never an assertion) of whether the
    shuffle actually reordered, recorded for length ≥ 2. Classes: empty list,
    singleton, already-sorted, reverse-sorted, repeated elements. Counts
    recorded.
  - *Witnesses.* Two directed tests. `path_key(["a","b"]) == path_key(["a\0b"])`
    shows the NUL precondition is load-bearing;
    `path_key([]) == path_key([""])` shows the non-emptiness precondition is.
    Both are stated as *disjunctions*: either the encoding admits the collision,
    in which case the corresponding precondition is load-bearing, or the
    encoding is injective without it. A future length-prefixed encoding would
    be a strict improvement and must not be blocked by these tests.
  - *Mutation.* `MUT-PATHKEY` deletes `parts.sort_unstable()`.
  - *Note on `prop_shuffle`.* The per-case reorder assertion must be guarded,
    not assumed: `prop_shuffle` may return the input order, and a list of
    repeated identical values or a singleton cannot reorder whatever the
    shuffle does. "The shuffle reordered" is therefore a *classification* the
    run records and counts, never a `prop_assert!` — asserting it would fail on
    perfectly valid generated cases. The manually-constructed `reverse` and
    `sorted` cases carry the guaranteed perturbation here.

______________________________________________________________________

**OBL-NOLOSS** — *every distinct edge is emitted exactly once; no edge is
silently dropped.*

Statement: for every well-formed graph, the number of `build` statements in the
emitted text equals the number of distinct edges in the graph, and every
explicit output appears exactly once on the left-hand side of a `build` line.

This replaces the first draft's `OBL-GUARD`, which asserted a *mechanism*
("validation runs before sorting") as a proxy for the failure that matters. The
failure that matters is a real edge being dropped between the graph and the
emitted text. Asserting the outcome survives refactors that legitimately move
the validator without weakening it.

The "silent drop of output-less edges", which the first draft recorded as a
surprise and this obligation was partly justified by, **does not survive a
re-read**. `insert_canonical_edge` pushes unconditionally and `render_edges`
renders every arena entry with no `seen` set, so an output-less edge is not
dropped: it emits a `build` line with an empty left-hand side. Whether such a
graph is constructible through the loader at all is unresolved — `Target::name`
accepts `StringOrList::Empty`, which maps to an empty vector, and
`get_target_display_name` tolerates the empty case, but `REJECTED_EMPTY` is an
error for command lists and the name path is untested. `EP-M4` establishes
reachability before either half of the claim is asserted. Recorded as `ADR-030`
amendment 2.

- **Method:** Proptest over well-formed graphs, plus directed tests for the
  known bypasses.
- **Rationale:** this is the real content of `RM-4.3.1.c`'s safety argument, and
  the only obligation covering the `path_key`-collision failure mode as an
  observable.
- **Domain:** well-formed graphs of 1 to 30 edges, including multi-output
  edges. Two directed non-well-formed cases: an edge with
  `explicit_outputs: []`, and two edges whose output lists are permutations of
  one another (which `from_manifest` rejects but a direct constructor permits).
- **Oracle:** the count and set of outputs computed from the spec, independently
  of the emitter.
- **Artefact:** `src/ninja_gen/determinism/no_loss.rs`.
- **Non-vacuity:**
  - *Covers.* Per-case assertion that at least one graph in the run had a
    multi-output edge. Directed cases pin the two bypasses, but only after
    `EP-M4` has established what each actually does; until then the
    output-less case is a reachability question, not a known bypass.
  - *Mutation.* `MUT-EDGEDROP` makes `render_edges` skip an arena entry when
    `explicit_outputs` is empty, so an output-less edge vanishes from the text.
    This is the mutation this obligation actually needs, and it replaces
    `MUT-GUARD`, which could not serve. `MUT-GUARD` moved
    `reject_unsupported_path_characters` after the sort; re-reading
    `src/ninja_gen/path_syntax.rs:24` shows that function *returns*
    `Result<(), NinjaGenError>` and validates without dropping or merging
    anything, so reordering it changes *when* an error is raised, never
    *whether* an edge is emitted. It cannot fail a no-loss property. If
    `EP-M4` finds the output-less case unreachable through the loader, the
    patch targets the directly-constructed graph instead, and the obligation
    says so rather than keeping a mutation that demonstrates nothing.

______________________________________________________________________

**OBL-ORDER** — *emission does not depend on `HashMap` iteration order.*

This obligation is split into two properties because the naive single property
is not seed-reproducible.

*Core (seed-deterministic).* `EP-M2` extracts the two collection points as pure
helpers,
`ordered_edges<'a>(impl Iterator<Item = &'a BuildEdge>) -> Vec<&'a BuildEdge>`
and `ordered_actions`, changing no behaviour. The property feeds each helper an
explicitly `prop_shuffle`d `Vec` and asserts the output is invariant. Every
case is genuinely permuted by construction, non-vacuity is structural rather
than measured, shrinking is exact, and seeds replay.

*End-to-end (composition).* One property builds two `BuildGraph` values from
one spec in two insertion orders and compares whole bundles — `build_file`, and
every sidecar's path, content, and position.

Because `RandomState` is outside the seed, the two graphs may still iterate
identically, and the case then says nothing. The first draft resolved this by
re-materializing up to eight times and failing the case if the orders never
differed. **`EP-M0` question 1 falsified that bound**, because iteration order
is a function of the *set of keys inserted together*, not of insertion order
alone: a map with fewer than five keys very often has an order set with no
alternative order available at all. Measured over 200 trials per size, with the
two orders drawn independently:

| keys | trials exhausting 8 attempts | attempts needed when they do differ |
| ---- | ---------------------------- | ----------------------------------- |
| 1    | 200 / 200                    | never                               |
| 2    | 23 / 200                     | up to 8                             |
| 5    | 0 / 200                      | at most 2                           |
| 12   | 0 / 200                      | 1                                   |
| 50   | 0 / 200                      | 1                                   |

A single-key map has exactly one insertion order, so its iteration order can
*never* differ and the loop was guaranteed to fail every `EP-M3` minimal case.
The end-to-end arm therefore holds the **graph value fixed** and varies only
insertion order: a single `GraphSpec` is materialized twice, so both
`BuildGraph` values have identical key sets and identical values, and the
property asserts the two emissions agree. A whole-bundle comparison over graphs
with *differing* key sets does **not** establish I-1: a differing key set is a
differing graph, so any divergence is legitimate, and any agreement says
nothing about order independence. Varying the key set would have made the arm
unfalsifiable rather than merely imprecise.

Divergence in iteration order is therefore *not* required for the end-to-end
arm, which is sound but may be vacuous on a given case. Two guards keep it
honest. The collector-level arm perturbs an explicitly shuffled `Vec`, where
divergence is by construction and is asserted. The end-to-end arm records, per
case, whether the two iteration orders actually differed, and the end-to-end
assertion is graded inconclusive — not passed — when they did not, so a run in
which no case diverged cannot be reported as evidence for I-1. Drawing two
permutations of a ≥ 5-key domain makes the orders differ with probability ≥
0.99 per draw; at 1–4 keys the end-to-end arm is unreachable in principle
rather than by bad luck, and the case is counted, not silently skipped. The
generated domain's lower bound is raised from 1 to 5 actions and edges for the
same reason.

- **Method:** Proptest, metamorphic.
- **Rationale:** `RM-4.3.1.a`.
- **Domain:** `GraphSpec` values with 1 to 50 actions, 1 to 100 edges, and at
  most 200 explicit outputs in total. Both `DependencyOrder` variants; serial
  edges get 0 to 4 implicit dependencies so dyndep staging is genuinely
  exercised. The end-to-end arm requires at least `ORDER_MIN_DIVERGENCE` (5)
  distinct keys per collection before it treats a case as informative, and
  otherwise records the case without asserting on it.
- **Oracle:** the second emission — metamorphic. Additionally, a *differential*
  arm asserts that whenever
  `GraphView::from_build_graph(g_u) == GraphView::from_build_graph(g_v)`, the
  bundles are equal. `GraphView` derives its ordering through `BTreeMap`/
  `BTreeSet` iteration rather than explicit `sort_by_key`, so it is an
  independent reference, not a reimplementation. Metamorphic testing alone
  cannot catch two emissions being wrong in the same way; this arm can.
- **Artefact:** `src/ninja_gen/determinism/order.rs`.
- **Non-vacuity:**
  - *Covers.* Per-case: the shuffle reordered; and, separately, whether the two
    iteration orders differed. Recorded classes must include the *intersection*
    multi-output ∧ phony ∧ dyndep-staged ∧ non-empty defaults, not only the
    marginals, and all four combinations of `BuildEdge::always` and
    `Action::restat` — `DisplayEdge` emits `restat` only when
    `edge.always && !action_restat` (`src/ninja_gen/display_edge.rs:37`).
  - *Mutation.* `MUT-EDGESORT` deletes `edges.sort_by_key` in both paths;
    `MUT-ACTIONSORT` deletes `actions.sort_by_key`. Each must fail the core
    property, which pinpoints the helper rather than the whole emission.
  - *Honesty of the end-to-end arm.* The arm asserts emission equality over one
    fixed graph value, so it is sound but can be vacuous for a case whose two
    iteration orders coincide. It is graded inconclusive — not passed — when no
    case in the run produced divergent iteration orders, and the run reports
    that count. This is the trap `EP-M0` question 1 found at 1 key; the grading
    is what stops the arm quietly degenerating into a no-op.

______________________________________________________________________

**OBL-NOHASH** — *no unordered collection is iterated inside the emitter.*

Statement: no source file under `src/ninja_gen/` lets an **unordered**
collection's iteration order reach the output. Concretely, every iteration of a
`std::collections::HashMap`/`HashSet` is either membership-only, or is
immediately followed by a total order being imposed before the values are
consumed.

The obligation is stated over *sorted* iteration rather than over iteration
outright, because iteration outright is already false of the code it protects.
`write_action_rules` (`src/ninja_gen/mod.rs:215`) collects `graph.actions` — an
ordinary-build `std::collections::HashMap` — into a `Vec` and then sorts it by
action ID at `:216`. That is the correct pattern: the collect-then-sort pair is
exactly what `MUT-ACTIONSORT` deletes to produce a real defect. A rule banning
all map iteration would reject this path, and a rule scanning for a literal
`HashMap` while ignoring the sort would miss the hazard it exists to catch.
Membership-only `HashSet` use is exempt, with the one existing set named as the
documented exemption.

- **Method:** a source-shape contract test, following
  `tests/whitaker_boundary_contract.rs` and
  `tests/integration_test_wiring_tests.rs`.
- **Rationale:** this is what actually delivers the plan's stated success
  criterion. No property can guarantee that a *future* map — keyed on pools, or
  variables, or anything not yet generated — will be caught, because the
  generator will not vary the field it is keyed on. A source contract catches
  it the day it is written. Neither a property nor an ordered-map port provides
  this.
- **Artefact:** `tests/ninja_gen_hashmap_boundary.rs`.
- **Non-vacuity:** the test is validated by temporarily adding an iterating
  `HashMap` to a scratch copy of `src/ninja_gen/mod.rs` and observing the
  failure; the exemption list is asserted to be exactly the one known set
  (`staged_sidecars`, `src/ninja_gen/dyndep.rs:246`), so adding a second
  requires a deliberate edit. An earlier revision also held a `seen` set in the
  deleted per-output storage path; `2c030fd1` removed it, and the obligation's
  wording did not follow. See `ADR-030` amendment 3.

______________________________________________________________________

**OBL-DEFAULT** — *the `default` line is the ascending sort of
`default_targets`, and follows every `build` statement.*

Statement: for every well-formed graph with non-empty `default_targets`, the
emitted text contains exactly one line beginning `default`, its operands equal
`default_targets` sorted ascending with duplicates preserved, and its byte
offset exceeds that of every `build` line.

- **Method:** Proptest over generated default sets.
- **Rationale:** `RM-4.3.1.b` for the ordering. The positional half encodes
  `AXIOM-NINJA`: a `default` preceding its `build` statement is rejected by
  real Ninja (measured), so ordering it last is correctness, not style, and
  nothing else pins it.
- **Domain:** graphs of 1 to 20 edges whose `default_targets` is a generated
  sub-multiset of declared outputs, shuffled, with 0 to 3 deliberate repeats.
- **Oracle:** a sorted clone computed in the test. That half is weak alone,
  since it mirrors the production call; its strength comes from the
  permutation-invariance half, which compares two input permutations and cannot
  be satisfied by copied code.
- **Artefact:** `src/ninja_gen/determinism/defaults.rs`.
- **Non-vacuity:**
  - *Covers.* Empty (asserting no `default` line at all), singleton,
    already-sorted, reverse-sorted, and duplicate-bearing. The duplicate class
    pins the documented decision that `Vec::sort` does not deduplicate.
  - *Mutation.* `MUT-DEFSORT` deletes `defs.sort()`; `MUT-DEFPOS` moves the
    `default` block before edge rendering and must fail both the positional half
    and `OBL-NINJA`.

______________________________________________________________________

**OBL-ACTION** — *equal actions intern to equal identifiers.*

Statement: for every pair of `Action` values, `a == b` implies
`ActionHasher::hash(a) == ActionHasher::hash(b)`; and over the generated
domain, no two unequal actions were observed to collide.

The first draft claimed "if and only if", whose converse is collision-freedom
of SHA-256 — explicitly out of scope under `AXIOM-HASHER` and not establishable
by sampling. A passing run would have read as proving it.

- **Method:** Proptest over generated `Action` values.
- **Rationale:** the `FV-DET` bullet the roadmap omits, and a genuine
  prerequisite for `OBL-ORDER`: the action sort key's uniqueness depends on
  interning being content-determined.
- **Domain:** three arms — `(a, a.clone())`; two independent values; and a base
  value with exactly one field mutated.
- **Oracle:** structural equality via the derived `PartialEq`, independent of
  the serialization path.
- **Artefact:** `src/ir/action_hash_property_tests.rs`.
- **Non-vacuity:**
  - *Covers.* All three arms fire; the single-field arm reaches each of the six
    `Action` fields. Record explicitly that `depfile`, `deps_format`, `pool`,
    and `restat` are **currently unreachable from any manifest**
    (`register_action` hard-codes them), so this obligation's evidence over
    those fields concerns the emitter's input domain, not the shipped pipeline.
    Stating that prevents the evidence being read as stronger than it is.
  - *Mutation.* `MUT-HASHMETA` makes the hasher skip `pool`.

______________________________________________________________________

**OBL-E2E** — *permuting target declaration order does not change the emitted
bytes, for manifests with distinct rule names.*

Statement: for every generated manifest with pairwise-distinct rule names that
lowers successfully, and every permutation of its `targets` list, lowering and
emitting the permuted manifest yields byte-identical output.

The rule-name precondition is required, not cosmetic. `process_rules`
(`src/ir/from_manifest.rs:100`) is last-writer-wins on duplicate names and no
`DuplicateRule` error exists anywhere in `src/`. Permuting two same-named rules
with different bodies changes which body every referencing target resolves, and
so changes the bytes. The first draft asserted the unrestricted claim, which is
false, and its generator allocated unique names — so it would have passed
vacuously on precisely the class where the contract fails.

Note also that moving a declaration between `manifest.actions` and
`manifest.targets` is a permutation of the declaration multiset that is *not*
order-neutral, since `process_targets` chains them. The obligation permutes
within a list, not across the two.

- **Method:** Proptest, metamorphic, end-to-end from typed manifest through
  `from_manifest` to `generate_bundle`.
- **Domain:** typed `NetsukeManifest` values (not YAML text — parsing and Jinja
  belong to 4.3.2 and 4.3.3) with 1 to 12 rules and 1 to 30 targets. Outputs
  allocated without replacement; dependencies drawn only from lower indices so
  acyclicity holds by construction. `NetsukeManifest` does not derive `Clone`,
  so permutation rebuilds from the spec rather than cloning.
- **Oracle:** the permuted manifest's emission — metamorphic.
- **Artefact:** `src/ninja_gen/determinism/declaration.rs`.
- **Non-vacuity:**
  - *Covers.* The permutation reordered; at least two targets share an
    identical recipe so interning collapses them; declared `defaults` present;
    cross-target dependencies present. Additionally, a **directed test that
    reaches the excluded class**: two same-named rules with different bodies,
    asserting that the emission *does* differ under permutation. Without it the
    precondition is an unexamined escape hatch rather than a documented
    boundary.
  - *Mutation.* `MUT-DEFSORT` must fail this property too, since
    `default_targets` is the field that carries declaration order into the
    graph.
- **Note on strength.** The first draft called this "strictly stronger" than
  `OBL-ORDER`. It is not. `from_manifest` can never produce `implicit_outputs`,
  `pool`, `depfile`, `deps_format`, or `restat`, so `OBL-ORDER`'s direct-graph
  strategy is the only obligation reaching those emitter branches. `OBL-E2E` is
  stronger over the lowering and weaker over the emitter's input domain.

______________________________________________________________________

**OBL-PROCESS** — *running Netsuke twice on one manifest emits identical bytes.*

Statement: invoking the built binary twice over a fixture manifest, in separate
processes, produces a byte-identical **bundle** — the main build file text,
every dyndep sidecar's relative path, and every sidecar's content. Comparing
only `build.ninja` would be weaker than `G-1`, which is stated over the
complete artefact, and a process-dependent sidecar path or body would pass an
`build.ninja`-only comparison while violating the published guarantee. The two
runs write to separate output directories so neither can satisfy the other's
sidecar reference.

- **Method:** a directed `assert_cmd` integration test, not a property.
- **Rationale:** this is the statement `EP-M1`'s ADR publishes and the one users
  read, and **nothing in the repository tests it**. The eight
  `tests/snapshots/ninja/*.snap` fixtures are single-run. Every other
  obligation here runs two emissions in one process. Roughly twenty lines close
  the gap between what is promised and what is checked; without it the plan
  publishes a guarantee it does not verify.
- **Domain:** two or three existing fixture manifests, run twice each, plus one
  run under a different `TMPDIR` and locale set with `Command::env` so
  `Constraint 5` holds. The fixture set **must include one manifest with at
  least five distinct actions**, and that requirement is load-bearing rather
  than incidental. The eight existing `tests/snapshots/ninja/*.snap` fixtures
  carry only one to three distinct actions each; at that size `EP-M0` question
  1 measured that a map frequently has *no* alternative iteration order to
  reach, so deleting `actions.sort_by_key` would leave the two runs agreeing
  and the validation would pass without having tested anything. Do not satisfy
  this obligation with the snapshot fixtures alone.
- **Oracle:** byte equality between runs.
- **Artefact:** `tests/ninja_determinism_process_tests.rs`.
- **Non-vacuity:** the mutation is `MUT-ACTIONSORT`, **not** `MUT-DEFSORT`.
  This distinction is the whole point of the obligation. `MUT-DEFSORT` deletes
  `defs.sort()`, but for a *fixed* manifest the unsorted `default_targets`
  order is fixed too, so both process runs emit the same unsorted line and
  agree. A single-run snapshot with multi-default fixtures catches
  `MUT-DEFSORT`; a two-run comparison cannot, because the fault is
  deterministic across processes. What `OBL-PROCESS` exists to detect is a
  fault whose value differs *between* processes — the `HashMap` `RandomState`
  that is reseeded per process. `MUT-ACTIONSORT` deletes the sort that is the
  *only* guard over the *only* unordered collection the emitter iterates:
  `graph.actions` is an `IrHashMap`, so `write_action_rules`
  (`src/ninja_gen/mod.rs:216`) reads it at risk and re-establishes determinism
  by sorting. Deleting that sort restores per-process iteration order, and a
  second process emits the action-rule blocks in a different order — a genuine
  cross-process divergence.

  This validator carries a precondition of its own, and it is the same trap one
  level down: an action map small enough to have only one iteration order makes
  the mutation unobservable. `EP-M0` question 1 measured that maps below
  roughly five keys frequently have no alternative order to reach, and the
  existing snapshot fixtures carry one to three distinct actions each. The
  obligation's domain therefore requires a fixture with **at least five
  distinct actions**; see the domain note above.

  An earlier revision of this obligation named `MUT-HASHMETA` here, on the
  theory that a hasher deriving an `Action`'s identity from a per-process
  `RandomState` would be caught. That theory is wrong. `MUT-HASHMETA` is a
  sound validator for `OBL-ACTION`, which generates `Action` values directly,
  sets `pool` to `Some`, and asserts that unequal actions do not collide — so
  skipping `pool` merges two distinct generated actions and fails it. It is
  *not* a sound validator here, because `OBL-PROCESS` never sees a generated
  `Action`: `pool` is never `Some` in any construction site in `src/`, and the
  hash `register_action` computes is recomputed identically in both processes
  for a fixed manifest. Perturbing the hash *values* changes both runs alike,
  so the two-run comparison cannot notice. Validate the test with
  `MUT-ACTIONSORT`, and state in the test's own documentation which class of
  fault it can and cannot see: it detects cross-process divergence, and it is
  not a substitute for the single-run properties. `OBL-E2E` remains the place
  where `MUT-DEFSORT` must fail.

______________________________________________________________________

**OBL-DUP** — *duplicate outputs are rejected at larger N.*

Statement: for every generated manifest containing an output claimed twice —
across targets or within one target — `from_manifest` returns
`Err(IrGenError::DuplicateOutput { .. })` naming the colliding path; and for
every manifest whose outputs are pairwise distinct, lowering succeeds.

- **Method:** Proptest with an injected collision and a control arm.
- **Rationale:** `RM-4.2.1.dup`, deferred here by `ADR-004`. Kani proves it for
  fixed minimal manifests; this extends to 30 targets and adds the negative
  arm, which Kani does not cover.
- **Domain:** 2 to 30 targets over an allocated namespace, with a generated
  collision mode (`across`, `within`, `none`).
- **Oracle:** the injected collision is known by construction; the expected
  variant and path are computed without calling `find_duplicates`.
- **Artefact:** `src/ir/graph_property_tests/duplicates.rs`.
- **Non-vacuity:**
  - *Covers.* All three modes fire. The `none` arm must assert a *successful*
    lowering, not merely a non-`DuplicateOutput` error — otherwise a lowering
    that rejected everything would pass.
  - *Mutation.* `MUT-DUPWITHIN` disables only the within-one-target half. Note
    that `ir__from_manifest__verification__duplicate_output_always_rejected.patch`
    already exists from 4.2.1 and flips `||` to `&&`; the new patch is more
    surgical so the three modes can be told apart.

______________________________________________________________________

**OBL-CYCLE** — *cycles are rejected at larger N; missing dependencies do not
create false cycles.*

Statement: for every generated manifest whose dependency relation contains a
cycle, `from_manifest` returns `Err(IrGenError::CircularDependency { .. })`.
For every acyclic manifest, lowering succeeds even when some declared
dependencies name paths no target produces.

Boundary against roadmap 4.2.2 (complete): this obligation asserts *rejection*
only. The canonical *content* of the reported cycle — preserved length, closed
cycle, interior multiset, stable start node — is 4.2.2's, proved by Kani over
`canonicalize_cycle_by`. This plan does not restate it.

- **Method:** Proptest over three generator arms.
- **Domain:** 2 to 30 nodes. The acyclic arm draws dependencies only from
  strictly lower indices, so acyclicity is structural. The cyclic arm must
  **construct** its cycle, not merely add a back edge: with dependencies drawn
  only from lower indices, a single added edge `u -> v` closes a path only when
  a path `v -> … -> u` already exists, so a generator that adds one back edge
  with arbitrary endpoints produces acyclic graphs in most cases and the
  obligation passes without testing anything. The arm therefore builds a known
  path first — pick a chain `v -> … -> u` and then add `u -> v` — or injects a
  self-edge, which is a cycle unconditionally. The third arm adds dependencies
  outside the namespace.
- **Oracle:** cyclicity known from the generator's index discipline, not from
  the production detector.
- **Artefact:** `src/ir/graph_property_tests/cycles.rs`.
- **Non-vacuity:**
  - *Covers.* Self-edges; two-node cycles; **cycles of length at least 8** —
    the range Kani cannot reach and the entire point of the obligation; acyclic
    with and without missing dependencies. If the long-cycle count is zero the
    obligation is not discharged.
  - *Mutation.* `MUT-CYCLEDEPTH` caps traversal depth at 4; the long-cycle
    class must fail while short cycles still pass.

______________________________________________________________________

**OBL-NINJA** — *emitted files are accepted by real Ninja, and permuted
emissions are semantically identical.*

Statement: when a `ninja` binary is available, the emitted bundle written to
disk parses without error and yields the same command list across insertion
permutations.

- **Method:** Proptest via `TestRunner::run`, using
  `test_support::ninja::ninja_is_required` so the skip escalates to a panic
  under `NETSUKE_REQUIRE_NINJA=1` as `ci.yml` sets.
- **Rationale:** byte equality could in principle be preserved by an emitter
  producing identical garbage. An independent oracle closes that, and exercises
  the `AXIOM-NINJA` interactions.
- **Domain:** 1 to 8 actions, 1 to 12 edges, paths legal on POSIX and Windows.
  The domain **must** generate serial edges with at least two implicit
  dependencies, or the dyndep path is never reached.
- **Oracle:** the `ninja` binary. Two invocations are required, not one:
  `-t commands` for the command list, **and** `-n`, because `-t commands` does
  not load dyndep sidecars — a graph referencing a missing sidecar exits 0 under
  `-t commands` and fails only under `-n`. As first drafted this obligation
  was vacuous over the sidecar bundle, the newest and most intricate part of
  the emitter. Measured cost is 1.63 ms and 1.91 ms respectively.
- **Artefact:** `src/ninja_gen/determinism/ninja_oracle.rs`. The existing
  `NinjaCommandOracle::ninja_commands` cannot be reused: `batch_ninja_files`
  discards the graph and scrapes only `command =` lines. A new harness is
  budgeted, which must create `.netsuke/dyndep/` and `.netsuke/serial/` before
  writing sidecars, materialize or exclude out-of-namespace inputs, and use a
  **fresh temporary directory per case** — the existing oracle reuses one
  directory, so content-addressed sidecars from earlier cases could satisfy a
  later case's reference and mask a missing-sidecar bug.
- **Non-vacuity:**
  - *Covers.* Multi-output edges, phony edges, dyndep-staged edges, and a
    non-empty `default` line. Assert a non-zero case count when Ninja is
    present.
  - *Mutation.* `MUT-DEFPOS` must cause real Ninja to reject the file
    (`unknown target`, measured), confirming the oracle detects a semantic break
    that byte comparison alone would not.

### Residual gaps

- Proptest samples; it does not prove. Where an exhaustive small-N result
  exists it is the 4.2.x Kani harnesses, and the layers are complementary by
  construction: under `#[cfg(kani)]`, `IrHashMap` is an ordered map, so Kani
  cannot reach `OBL-ORDER` at all.
- Manifest parsing, `foreach`/`when` expansion, and Jinja rendering are out of
  scope (4.3.2, 4.3.3). `OBL-E2E` starts from a typed manifest.
- Only one `RecipeShell` is exercised per machine, since every entry point takes
  `RecipeShell::host_default()`. The determinism properties pin
  `RecipeShell::Posix` explicitly, as the existing suite does
  (`src/ninja_gen_property_tests.rs:79`), so results are comparable across
  developer machines; the PowerShell `rspfile` branch is therefore covered only
  on the Windows lane and only by existing tests.
- Determinism of **error paths** is not claimed and not tested. Both rejection
  passes return on the first offender found while iterating
  `graph.targets.values()`, so which error a multi-fault graph reports is
  insertion-order dependent, as is the order of
  `CycleDetectionReport::missing_dependencies`. `ADR-NNN` states this exclusion
  explicitly rather than leaving a reader to assume coverage.
- Cross-version byte stability is explicitly **not** promised; see `ADR-NNN`.
- `src/graph_view/` has its own determinism guarantee and its own property. It
  is out of scope here, and `ADR-NNN` records it as adjacent.

## Plan of work

Stage A measures and confirms, changing nothing. Stage B states the contract.
Stage C makes the repository able to hold the work — two file splits, the
mutation-evidence contract, and the shared strategy. Stage D discharges the
obligations in dependency order. Stage E documents and validates.

Red-green-refactor cannot apply in its usual form, because production code is
expected to be correct and a new property therefore passes on first run. The
substitute is mutation-driven red, recorded in `Validation and acceptance`.

## Milestones and plateaus

### EP-M0 — feasibility and measurement spike (prototyping)

*Prototype; scratch commits, not merged.* Seven questions, all now answered and
recorded in `Artefacts and notes`.

1. *(Answered — design change.)* Re-materialization **cannot** make two
   iteration orders differ at small N, so the bounded loop is unsound as
   written. See `Artefacts and notes` and the `OBL-ORDER` rewrite.
2. *(Answered — `OBL-E2E` holds.)* Byte-identical emission across 24
   declaration permutations, including the reachable failure class.
3. *(Answered.)* Per-case cost.
4. *(Answered — **no**.)* A 50/100 counter-example does not shrink to something
   readable within 30 seconds; it does not converge at all.
5. *(Answered — **falsified**.)* Integration-test regression seeds persist and
   replay. The plan's "probably inert" risk is wrong.
6. *(Answered — superseded.)* `adr-021` was claimed on four branches, but
   `origin/main` has since absorbed `add-020`…`add-029`; the next free number
   is **`ADR-030`**.
7. *(Answered — no.)* A `src/`-side `#[cfg(test)]` module cannot receive a
   `netsuke`-typed value from `test_support`.

*Acceptance:* every question has a recorded answer. *Conformance check:* if
question 2 answers "no", stop and escalate before `EP-M1`. Question 2 answered
"yes"; no escalation was needed. *Recovery:* discard the scratch commits.

*Outcome:* the spike found two defects in this plan before implementation began
— an unsound `OBL-ORDER` loop and a false regression-seed risk — plus a
three-hour cost finding that reshaped how obligations may be assessed. The
prototype source was scratch and has been deleted; nothing from it is merged.

### EP-M1 — state the determinism contract

*Assigned:* `FV-CONTRACT`, `ADR-NNN`.

Write `docs/adr-030-ninja-emission-determinism-contract.md` in the repository's
Y-Statement style, **before any property is authored**, from the four
statements in `Context and orientation`. It must state:

- which statements are public guarantees and which are internal invariants;
- the full parameter tuple the guarantee is a function of — manifest,
  environment, platform, shell selection, and **Netsuke version**;
- that byte stability is explicitly **not** promised across Netsuke versions,
  stated as loudly as the promise, since `ADR-011`'s dyndep schema tags exist
  precisely so the staging format can change;
- a numbered precondition list each property can cite: well-formedness as a
  stated precondition of `generate`/`generate_bundle`; duplicate
  `default_targets` emitted twice; output-less edges retained and emitted with
  an empty left-hand side, their *constructibility* being an open reachability
  question rather than an established drop; duplicate rule names
  last-writer-wins;
- which entry point the guarantee covers, and that sidecars are part of the
  artefact;
- that error-path diagnostic selection and ordering are excluded;
- considered-and-rejected options, with honest reasons: the ordered-map port,
  rejected because it would make the sort calls dead and the mutation evidence
  evaporate, *not* because a constraint in this plan forbids it; and
  precomputing `path_key`, rejected as unnecessary at production scale
  (measured at 16 ms release for 10,000 edges);
- why Kani cannot discharge `OBL-ORDER`: `#[cfg(kani)]` replaces `IrHashMap`
  with an ordered map, so the nondeterminism under test does not exist there;
- the two domain-purity leaks: `Action` derives `Serialize` and its identity
  *is* its canonical JSON digest; and a verification cfg chooses the domain
  model's collection type.

*End state:* the contract exists and is reviewable before anything claims to
verify it. *Acceptance:* `make markdownlint`, `make nixie`, `make check-fmt`
pass; the ADR is indexed in `docs/contents.md`. *Recovery:* documentation-only.

*Outcome:* `docs/adr-030-ninja-emission-determinism-contract.md` is written and
indexed. It publishes one guarantee (`G-1`, process-level reproducibility over
the five-element parameter tuple, sidecars included) and records the
insertion-order, declaration-order, and within-edge statements as internal
invariants `I-1` … `I-3`; five numbered preconditions `P-1` … `P-5` that
obligations now cite instead of restating; four explicit non-promises `N-1` …
`N-4`; the Kani argument; the rejected options with their real reasons; and the
two domain-purity leaks.

Deriving the contract from the code at *this* revision, rather than from the
plan, falsified two of the plan's own mechanism claims — both recorded as
amendments in the ADR and corrected above. Neither changes what any obligation
asserts; both change why. This is the whole reason `EP-M1` precedes the work.

The review round that followed added two more amendments rather than more
milestones: one corrected the same unsound `OBL-ORDER` mechanism in the ADR
that the review had found in this plan, and one corrected the `OBL-NOHASH`
exemption count. Amendments 3 and 4 are therefore review-driven where 1 and 2
are derivation-driven. All four are mechanism corrections; no obligation's
*assertion* changed at any point in `EP-M1`.

### EP-M2 — make room, and make the patch contract usable

*Assigned:* `Constraint 4`, `Constraint 7`, and the `make proptest` target.

Three preparatory pieces, none of which changes behaviour:

- Split `src/ninja_gen/mod.rs`, which is exactly at the 400-line ceiling. The
  `NamedAction` impl block (lines ~256–391) is the obvious seam.
- Split `tests/kani_mutation_evidence_tests.rs` (383 lines) and generalize
  `supplemental_property_location`, which currently hard-codes
  `ensure!(*root == "ir", …)` and derives `src/ir/<segments joined by _>.rs`.
  It must accept a `ninja_gen` root and directory-style module paths. Update
  the developers'-guide section describing it. This is the single largest piece
  of unbudgeted work the design review found.
- Extract `ordered_edges` and `ordered_actions` as pure helpers, used by both
  emission paths. Behaviour-preserving; enables `OBL-ORDER`'s
  seed-deterministic core.
- Add a `proptest` Make target. **The measurement says do not tier it**: the
  whole new suite costs about 6 s of CPU and 2–3 s wall, against a 45-second
  budget, so `PROPTEST_HEAVY`, `make proptest-heavy`, and a separate CI job are
  branches that would never be taken. A standalone CI job would additionally be
  ~98% compile — roughly 275 s of build to run 10 s of tests. `make proptest`
  is therefore a convenience selector over the same tests `make test` runs.
  Select the suites with a nextest filter that actually matches all of them
  (the first draft's `-E 'test(determinism_property_tests)'` missed its own
  `action_hash_property_tests` and `graph_property_tests`), or add a nextest
  test-group in `.config/nextest.toml`.

*Acceptance:* `make test` passes with no file over 400 lines; `make proptest`
runs and reports the existing property tests; a scratch `ninja_gen`-rooted
patch is accepted by the generalized contract test. *Recovery:* the splits and
the extraction are pure refactors, revertible independently.

### EP-M3 — the shared bounded graph strategy

*Assigned:* the plan's central artefact, given its own milestone because
`EP-M4` onwards all depend on it.

Strategies live in `src/`, **not** `test_support`, because the latter does not
compile from `src/`-side tests (proven; see `EP-M0` question 7). They go in
`src/ninja_gen/determinism/strategy/`, wired `#[cfg(test)]`, and are reachable
by every library-side suite in this plan.

Absorb or explicitly diverge from `src/graph_view/tests_property.rs`'s
`arb_graph_inputs`. If the new strategy supersedes it, migrate that test onto
it in this milestone; if not, record in `Decision log` why two `BuildGraph`
generators are justified. Leaving both unreconciled is the decay path.

Deliver a handwritten compact `Debug` for `GraphSpec` — counts per class plus a
stable digest — in this milestone, not later. Proptest prints the input's
`Debug` on failure regardless of the assertion message, and a 50/100 spec is
tens of kilobytes on one line of the seed file.

*Acceptance:* the strategy compiles and is exercised by a smoke property;
classification counts reproduce `EP-M0` question 1; a deliberately failed
assertion prints a counter-example that fits on a screen.

### EP-M4 — `path_key` canonicality and edge preservation

*Assigned:* `RM-4.3.1.c`, `OBL-PATHKEY`, `OBL-NOLOSS`.

*Acceptance:* `make proptest` passes; `MUT-PATHKEY` fails
`path_key_is_permutation_invariant`; `MUT-EDGEDROP` fails
`every_distinct_edge_is_emitted_once`; both revert cleanly. The two directed
witness tests are present and phrased as disjunctions.

### EP-M5 — ordering invariance and the collection boundary

*Assigned:* `RM-4.3.1.a`, `RM-4.3.1.b`, `OBL-ORDER`, `OBL-DEFAULT`,
`OBL-NOHASH`.

*Acceptance:* `make proptest` passes; `MUT-EDGESORT` and `MUT-ACTIONSORT` each
fail the seed-deterministic core property; `MUT-DEFSORT` and `MUT-DEFPOS` fail
`OBL-DEFAULT`; the boundary test rejects a scratch iterating `HashMap` in
`src/ninja_gen/`. *Conformance check:* graphs stay within 50 actions, 100
edges, and 200 total outputs.

### EP-M6 — interning, declaration order, and the published guarantee

*Assigned:* `OBL-ACTION`, `OBL-E2E`, `OBL-PROCESS`.

*Acceptance:* `make proptest` passes; `MUT-HASHMETA` fails `OBL-ACTION`;
`MUT-ACTIONSORT` fails `OBL-PROCESS` — it also belongs to `EP-M5`, which owns
its first falsification, and the two milestones share the mutation rather than
duplicating it; `MUT-DEFSORT` fails `OBL-E2E` only. `MUT-DEFSORT` must *not* be
required to fail `OBL-PROCESS`: a fixed manifest's `defs.sort()` deletion is
deterministic across processes, so it cannot fail a two-run comparison. The
directed duplicate-rule test demonstrates the excluded class genuinely
diverges. *Conformance check:* if `EP-M0` question 2 answered "no", `ADR-NNN`
must already have been amended in `EP-M1`; this milestone does not start
otherwise.

### EP-M7 — inherited larger-N IR obligations

*Assigned:* `RM-4.2.1.dup`, `RM-4.2.1.cyc`, `OBL-DUP`, `OBL-CYCLE`.

These discharge `ADR-004`, not the determinism contract, and touch a different
module tree. The design review recommended splitting them into a sibling
roadmap item; the user has directed that they stay in scope, so they are kept
as a self-contained milestone that could be lifted out unchanged if that
changes.

*Acceptance:* `make proptest` passes; `MUT-DUPWITHIN` fails only the
within-target arm; `MUT-CYCLEDEPTH` fails only the long-cycle class. The
long-cycle class count is non-zero.

### EP-M8 — Ninja oracle, documentation, and final validation

*Assigned:* `OBL-NINJA`, the documentation set, and the evidence sweep.

Documentation is deliberately light here because `EP-M1` already did the hard
part. Remaining: the users'-guide guarantee in user-facing terms; a README
sentence — noting that the anchor `FV-CONTRACT` cites ("a reproducible, fully
static dependency graph") **is not present in `README.md`**, whose nearest
match at line 165 concerns the `graph` subcommand's renderer, so the anchor
must be chosen deliberately and `FV-CONTRACT`'s stale citation corrected in the
same commit; a decision, recorded in `Decision log`, on whether the six
translated READMEs are updated in step or tracked separately;
`docs/netsuke-design.md` §5.5 referencing `ADR-030` and correcting its
unqualified determinism claim; `docs/formal-verification-methods-in-netsuke.md`
footnote `[^6]` repointed from the nonexistent `../src/ninja_gen.rs` to the
module directory (footnote `[^9]` is correct and stays as-is); a
developers'-guide subsection; an annotation on `ADR-004` recording its deferred
obligations as discharged; the roadmap marked done; and confirmation that
`ADR-030` is still the next free number after re-checking remote branches,
since it was allocated in `EP-M1` and the claim is only as good as that sweep.

*Acceptance:* all gates pass; every mutation patch applied and reverted once
more against the final tree with results tabulated; classification counts
recorded for every obligation; `coderabbit review --agent` returns no
unresolved findings; every trace link resolves to a passing test.

## Interfaces and dependencies

### Architectural note: where the boundary sits

`BuildGraph` is the domain model; `src/ninja_gen/` is an outbound adapter
rendering it into one backend's text format. `generate_into` writes to a
`W: Write`, so the adapter does not own the sink.

The obligations respect that. `OBL-PATHKEY`, `OBL-NOLOSS`, `OBL-ORDER`,
`OBL-DEFAULT`, and `OBL-NOHASH` are adapter properties tested at the adapter's
own entry points. `OBL-DUP`, `OBL-CYCLE`, and `OBL-ACTION` are domain
properties tested against the lowering. `OBL-E2E` and `OBL-PROCESS`
deliberately span both, which is justified because the user-facing claim is
about the composition.

Two purity leaks are real and are recorded in `ADR-NNN` rather than papered
over. `Action` derives `Serialize`, and `ActionHasher` defines action
*identity* as the SHA-256 of its canonical JSON — a serialization concern
inside the domain model. The team has already fought this: `DependencyOrder`
carries a `compile_fail` doctest whose whole purpose is to stop someone
serializing the domain enum. And `IrHashMap` swaps implementation under
`#[cfg(kani)]`, so a verification tool's cfg chooses the domain's collection
type.

The ordered-map port is rejected, but not on the circular ground that this
plan's own `Constraint 1` forbids it. It is rejected because under a `BTreeMap`
the two `sort_by_key` calls become dead code, `MUT-EDGESORT` and
`MUT-ACTIONSORT` would no longer fail anything, and the fault would stop being
observable. Structural determinism and mutation-demonstrated determinism are
alternatives here, not complements. `OBL-NOHASH` provides what the port was
reaching for, at lower cost and without that trade.

### File layout

Strategies and library-side properties, all `#[cfg(test)]`, wired from
`src/ninja_gen/mod.rs` and `src/ir/mod.rs`:

- `src/ninja_gen/determinism/mod.rs`
- `src/ninja_gen/determinism/strategy/mod.rs`, `graph.rs`, `manifest.rs`,
  `classification.rs`
- `src/ninja_gen/determinism/path_key.rs`, `no_loss.rs`, `order.rs`,
  `defaults.rs`, `declaration.rs`, `ninja_oracle.rs`
- `src/ir/action_hash_property_tests.rs`
- `src/ir/graph_property_tests/mod.rs`, `duplicates.rs`, `cycles.rs`

Integration tests (no strategy dependency, so `tests/` placement is sound):

- `tests/ninja_determinism_process_tests.rs` (`OBL-PROCESS`)
- `tests/ninja_gen_hashmap_boundary.rs` (`OBL-NOHASH`)

New top-level `tests/*.rs` files need no registration; Cargo autodiscovery
handles them and `tests/integration_test_wiring_tests.rs` confirms it. Only a
new `tests/` subdirectory with a `mod.rs` needs an explicit `mod` declaration.

Mutation patches, named after the test they falsify per the house convention,
with `__` for the module separator:

| Handle           | Falsifies                  | Mutation                                                 |
| ---------------- | -------------------------- | -------------------------------------------------------- |
| `MUT-PATHKEY`    | `OBL-PATHKEY`              | Delete `parts.sort_unstable()` in `path_key`.            |
| `MUT-EDGEDROP`   | `OBL-NOLOSS`               | Skip an arena entry whose `explicit_outputs` is empty.   |
| `MUT-EDGESORT`   | `OBL-ORDER`                | Delete `edges.sort_by_key` in both paths.                |
| `MUT-ACTIONSORT` | `OBL-ORDER`, `OBL-PROCESS` | Delete `actions.sort_by_key`.                            |
| `MUT-DEFSORT`    | `OBL-DEFAULT`, `OBL-E2E`   | Delete `defs.sort()`.                                    |
| `MUT-DEFPOS`     | `OBL-DEFAULT`, `OBL-NINJA` | Emit `default` before edge rendering.                    |
| `MUT-HASHMETA`   | `OBL-ACTION`               | Make the hasher skip `pool`.                             |
| `MUT-DUPWITHIN`  | `OBL-DUP`                  | Disable the within-one-target half of `find_duplicates`. |
| `MUT-CYCLEDEPTH` | `OBL-CYCLE`                | Cap cycle traversal depth at 4.                          |

Nine, not the first draft's eleven. Two were cut because the existing nightly
`cargo-mutants` job (`.github/workflows/mutation-testing.yml`, 03:05 UTC over
`src/` with `--all-features`) already generates sort-deletion mutants
automatically. Handwritten patches earn their keep on faults `cargo-mutants`
cannot generate — the *reordering* one, `MUT-DEFPOS`, and the *dropping* one,
`MUT-EDGEDROP`. `EP-M8` additionally records a scoped
`cargo mutants -f src/ninja_gen/mod.rs -f src/ninja_gen/dyndep.rs` run with
survivors tabulated.

### The shared graph strategy

Generates a *specification*, so one spec can be materialized twice under
different insertion orders.

```rust
/// A bounded, well-formed build-graph specification and two insertion orders.
struct GraphSpec {
    actions: Vec<(String, Action)>,
    edges: Vec<BuildEdge>,
    default_targets: Vec<Utf8PathBuf>,
    first_order: Vec<usize>,
    second_order: Vec<usize>,
}
```

Pin these signatures before `EP-M4` starts: the strategy constructor,
`materialize(&self, order: &[usize]) -> BuildGraph`, and the classification
API. The first draft specified a struct with every field private and no
methods, which is not an interface.

Well-formedness by construction, never by filtering:

- Output paths drawn from a pre-numbered `out/NNNN` namespace, each edge given
  a contiguous non-overlapping slice, so disjointness is structural and no path
  is empty. The namespace avoids `.netsuke/`, so `reject_reserved_paths` never
  fires.
- Total explicit outputs capped at 200 (`Constraint 10`).
- Action identifiers `act-NNNN`, unique by index. These stand in for
  production's content hashes; `OBL-ACTION` separately verifies the property
  that substitution relies on.
- Every `action_id` drawn from the generated list, so `MissingAction` is
  unreachable.
- Inputs and dependencies drawn from already-allocated outputs at strictly
  lower indices, plus generated external paths, so no cycle is generated and
  the missing-dependency path is exercised.
- `implicit_outputs` allocated from the *same* disjoint namespace. This is not
  optional: `find_duplicates` never examines `implicit_outputs`, and
  `from_manifest` always sets it empty, so a freely-generated implicit output
  would hand real Ninja two edges declaring the same one and `OBL-NINJA` would
  fail for a reason unrelated to determinism.
- `default_targets` a shuffled sub-multiset of allocated outputs with generated
  repeats.
- `first_order` and `second_order` independent shuffles of `0..n`.

## Concrete steps

### Running the suite

```bash
B=$(git branch --show-current)
make proptest 2>&1 | tee /tmp/proptest-netsuke-$B.out
```

Iterate on one property, or widen the search without recompiling:

```bash
cargo nextest run --all-features -E 'test(emission_is_insertion_order_invariant)'
PROPTEST_CASES=4096 cargo nextest run --all-features -E 'test(determinism)'
```

Never lower a case count to make a failure go away.

### Applying and reverting a mutation patch

```bash
git apply docs/verification/mutations/<name>.patch
make proptest 2>&1 | tee /tmp/mutation-<name>.out   # must FAIL
git apply -R docs/verification/mutations/<name>.patch
git diff --quiet && echo "tree restored"
```

Record which properties failed and which passed. A patch that fails everything
is too coarse to be evidence.

### Ordinary gates

Run sequentially; the build cache is shared with other agents.

```bash
B=$(git branch --show-current)
make check-fmt    2>&1 | tee /tmp/check-fmt-netsuke-$B.out
make typecheck    2>&1 | tee /tmp/typecheck-netsuke-$B.out
make lint         2>&1 | tee /tmp/lint-netsuke-$B.out
make doc-coverage 2>&1 | tee /tmp/doc-coverage-netsuke-$B.out
make test         2>&1 | tee /tmp/test-netsuke-$B.out
make markdownlint 2>&1 | tee /tmp/markdownlint-netsuke-$B.out
make nixie        2>&1 | tee /tmp/nixie-netsuke-$B.out
```

Prefer delegating full gate runs to the `scrutineer` subagent. If a gate fails,
read the cited log rather than re-running.

### Commits and review

Commit at every milestone boundary and whenever the tree is green within one.
Subjects are imperative and under 50 characters; bodies wrap at 72 columns.
Never commit a tree that fails a gate. Request `coderabbit review --agent` at
`EP-M5`, `EP-M7`, and `EP-M8`.

## Validation and acceptance

### Red-green-refactor evidence

Production code is expected to be correct, so a new property passes on first
run and its red stage cannot be observed conventionally. The substitute, which
is stronger and matches the discipline `ADR-004` established:

- **Red.** Apply the obligation's mutation patch and run the property. Record
  the failure and the counter-example. If it passes, it is not yet a test.
- **Green.** Revert and re-run. Record the pass.
- **Refactor.** Extract shared code, keep files under 400 lines, re-run.

### Acceptance, phrased as behaviour

1. `make proptest` on a clean tree passes and names the properties.
2. Applying `MUT-EDGESORT` and running `make proptest` fails, naming the
   insertion-order core property, with a counter-example small enough to read.
   Reverting restores a passing run.
3. The same holds for each of the other eight patches. Each fails its named
   property, **plus** `every_patch_applies_cleanly` in the mutation-evidence
   contract test — because `git apply --check` of an already-applied patch
   fails — and no other test. The first draft's criterion omitted that second
   failure and was therefore unachievable as written.
4. `make test` passes, and its wall-time delta against `origin/main` is within
   the figure recorded in `EP-M2` (expected: a few seconds).
5. With Ninja installed, the oracle property reports a non-zero case count;
   without it, a skip. Under `NETSUKE_REQUIRE_NINJA=1` an absent Ninja panics.
6. Running the built binary twice over a fixture yields an identical bundle,
   and `MUT-ACTIONSORT` breaks that. `MUT-DEFSORT` does **not**, and must not
   be required to: for a fixed manifest the unsorted `default_targets` order is
   fixed too, so the fault is deterministic across processes and both runs
   agree. `MUT-DEFSORT` belongs to `OBL-E2E`, which compares emissions within
   one process.
7. `docs/users-guide.md` states a guarantee, and `ADR-NNN` explains which
   statements are public, under which parameter tuple, and that cross-version
   stability is not promised.
8. `docs/roadmap.md` 4.3.1 is marked done, and the two `4.2.1` sub-items that
   deferred work here are annotated as discharged.
9. All gates pass.

### Quality criteria

- Every property uses `prop_assert*` inside the body; post-run assertions only
  in the `TestRunner::run` form.
- No strategy filters on a structural condition.
- Non-vacuity is asserted per case wherever the restructuring allows, and
  recorded where it cannot be.
- Counter-examples are readable, via the compact `Debug`.
- No file exceeds 400 lines.
- Prose is en-GB-oxendict and `mdtablefix`-canonical.

## Idempotence and recovery

Every step is re-runnable. The properties are pure and hold no state between
runs beyond committed seeds. If a mutation patch is left applied, `git diff`
shows it and `git apply -R` removes it; `EP-M8` re-verifies a clean tree.

Every milestone is additive or a pure refactor, so reverting means deleting
files and their wiring, or reverting a split. No milestone introduces a
compatibility shim, an alias, or a dual implementation, and none may: there is
no external consumer of any interface touched here, and every new surface is
test-only or `pub(crate)`.

If the shared Cargo cache is locked by another agent, wait rather than creating
a separate cache. If `/tmp` or the disk fills, stop and report.

## Artefacts and notes

### Answers obtained during `EP-M0` (2026-09-27)

All four remaining questions were answered against a throwaway integration-test
probe, since deleted. Each answer either confirmed a plan decision or forced
one to change; the changes are in `OBL-ORDER` and in `Risks`.

**Question 5 — do regression seeds persist for an integration-test crate? Yes,
contrary to the plan.** A deliberately failing property in
`tests/zz_scratch_probe.rs` printed

```plaintext
proptest: FileFailurePersistence::SourceParallel set, but failed to find lib.rs or main.rs
proptest: Saving this and future failures in .../tests/zz_scratch_probe.proptest-regressions
```

and then **replayed the saved seed** on the next run. The
`FileFailurePersistence` message is a *fallback notification*, not a failure:
persistence retries against the crate root and succeeds. The `Risk` entry
claiming three of the five committed files are "probably never replayed" is
**false** and has been removed. The related `Design review findings` entry,
which proposed a separate fix for those files, is withdrawn with it.

**Question 1 — the `OBL-ORDER` re-materialization loop is unsound.** Two
independently drawn insertion orders of the same key set frequently produce the
*same* iteration order, and at small sizes no other order is reachable at all.
The `OBL-ORDER` section above records the measurements and the replacement
design: the graph value is held fixed, the guaranteed perturbation moves to the
collector-level arm over an explicitly shuffled `Vec`, and the end-to-end arm
is counted and graded inconclusive when no case's iteration orders diverged.
`EP-M1`'s review round later found that the first replacement — a
well-separated pair of *key sets* — was itself unsound, and corrected it; see
`ADR-030` amendment 4 and the fourth revision note.

**Question 2 — `OBL-E2E` holds today.** Twenty-four declaration permutations of
a four-target manifest (including reversals and rotations), lowered and
emitted, were **byte-identical**. The same held for permuting
`manifest.actions` order. The rule-name precondition is load-bearing and
*reachable*: two same-named rules with different bodies do diverge
(`command = echo v2` versus `command = echo v1`), so the directed test that
`EP-M6` adds is testing a real class, not a hypothetical one. Interning was
also observed directly — two targets with an identical recipe collapse onto one
action hash (`a2ff8376…` for both `out-c` and `out-d`).

**Question 4 — a 50/100 counter-example does not shrink readably in 30 seconds;
it does not converge at all.** The prototype's compact `Debug` did print the
100-edge counter-example on one screen, so readability is solved. Convergence
is not. With a predicate carrying no structural gradient, shrinking spent the
whole 30-second wall and stopped on the wall, not on a minimal example, and the
reported "minimal" input wandered across repeated runs of the same seed and
predicate:

```plaintext
run 1 (max_shrink_iters = 128, the 4 × cases default): 39 edges
run 2 (max_shrink_iters = 200 000):                      90 edges
run 3 (max_shrink_iters = 200 000):                      43 edges
run 4 (max_shrink_time = 5 000):                         73 edges
```

Generation is not the bottleneck: the shrink loop completed roughly 4,700
candidates in a 5-second window (**≈ 940 candidates/s**), so 30 seconds buys
about 28,000 candidates, which is not enough. The consequence for this plan is
a verification-quality constraint, recorded as `Tolerance 5`: a property's
*assertion* must carry a structural gradient, and a property independent of the
internal representation must state a **compact, value-level** diagnostic
(`classify()` counts plus a digest, as prototyped) so a case is diagnosable
from its printed counter-example rather than from a re-run log.

**Question 6 (superseded).** `adr-021` was claimed on four branches when this
plan was written. `origin/main` has since absorbed `adr-020` through `adr-029`,
and two unmerged branches hold `adr-039` to `adr-041`. Sweeping every local and
remote ref, the next free number is **`ADR-030`**, not four. `adr-030` and
`adr-031` are unclaimed on every branch and in every open pull request.

### Answers obtained in the first planning pass (2026-09-09)

**Question 7 — can `src/`-side tests use `test_support` strategies carrying
netsuke types? No.** Proven by compile probe: a `test_support` function
returning `netsuke::ir::BuildGraph`, called from a `#[cfg(test)]` module in
`src/` and passed to `crate::ninja_gen::generate`, fails to compile.

```plaintext
expected `ir::graph::BuildGraph`, found `netsuke::ir::graph::BuildGraph`
note: there are multiple different versions of crate `netsuke` in the dependency graph
error: could not compile `netsuke-build` (lib test) due to 1 previous error
```

`test_support` depends on `netsuke-build`, which has `test_support` as a
dev-dependency, so the lib is compiled twice and the two `BuildGraph` types do
not unify. The existing `paths_strategy` works only because it returns
`Vec<Utf8PathBuf>`, a third-party type; `NinjaIntegrationCase`, which does
carry IR types, is consumed only from `tests/`. The probe was reverted and the
tree confirmed clean.

**Question 6 — is `adr-021` claimed? Yes, four times**, on the branches for
issues 592, 643, 644 and 646. `adr-020` is claimed twice. Hence the `ADR-NNN`
placeholder in the second draft. **Superseded:** see `EP-M0` question 6 in the
2026-09-27 answers above, which fixes the number at `ADR-030`.

**Question 3 — per-case cost.** Measured on this six-core Rocky 10 box, dev
profile (opt-level 0 with debug assertions, matching `cargo nextest`):

| `targets` entries | `path_key` calls | heap allocs | dev      | release  |
| ----------------- | ---------------- | ----------- | -------- | -------- |
| 12                | 74               | 222         | 0.039 ms | 0.005 ms |
| 100               | 1,152            | 3,456       | 0.561 ms | 0.082 ms |
| 200               | 2,616            | 10,464      | 1.866 ms | 0.229 ms |
| 400               | 6,702            | 40,212      | 7.259 ms | 0.814 ms |

One `OBL-ORDER` case costs 13–16 ms in a dev build. The whole new suite is
about 6 s of CPU and 2–3 s wall under nextest parallelism — well inside the
45-second budget, which is why no light/heavy split is planned. Recommended
case counts: `OBL-PATHKEY` 1024; `OBL-ACTION` 512; `OBL-NOLOSS`, `OBL-DEFAULT`,
`OBL-E2E`, `OBL-DUP`, `OBL-CYCLE` 256 each; `OBL-ORDER` 256; `OBL-NINJA` 64.

Also measured: `ninja -t commands` 1.63 ms, `ninja -n` 1.91 ms; a missing
dyndep sidecar exits 0 under `-t commands` and fails only under `-n`; a
`default` preceding its `build` statement is rejected. `sort_by_key` at
production scale is 16.2 ms release for a 10,000-edge graph, so the repeated
`path_key` allocation is a test-budget item, not a production defect.

### Still to record

`EP-M2` timings; per-obligation classification counts; per-mutation
transcripts; final gate logs; CodeRabbit outcomes; whether an output-less
target is constructible through the loader (`EP-M4`). All `EP-M0` questions are
answered above.

The determinism contract itself is no longer on this list: it is
[`docs/adr-030-ninja-emission-determinism-contract.md`](../adr-030-ninja-emission-determinism-contract.md),
written in `EP-M1`. Every numbered precondition an obligation cites (`P-1` …
`P-5`), every published and unpublished statement (`G-1`, `I-1` … `I-3`, `N-1` …
`N-4`), the parameter tuple, the Kani argument, and the two domain purity
leaks now live there rather than here.

## Progress

- [x] (2026-09-09T00:00:00Z) Renamed the branch and pushed it with upstream
      tracking.
- [x] (2026-09-09T00:00:00Z) Loaded the `codegraph-mcp`, `rust-router`,
      `hexagonal-architecture`, `execplans`, `proptest`, `rust-verification`,
      and `logisphere-design-review` skills.
- [x] (2026-09-09T00:00:00Z) Ran a four-agent reconnaissance team over the
      emission internals, the repository's proptest conventions, the
      documentation and gate tooling, and the ExecPlan house style.
- [x] (2026-09-09T00:00:00Z) Researched the Ninja manual v1.13.1 and confirmed
      current versions of `proptest` and its derive ecosystem.
- [x] (2026-09-09T00:00:00Z) Confirmed three scope decisions with the user:
      include the inherited larger-N IR obligations; settle the determinism
      contract here; add a `proptest` target, measure it, and split only if
      the measurement justifies it.
- [x] (2026-09-09T00:00:00Z) Drafted the first version of this plan.
- [x] (2026-09-09T00:00:00Z) Ran a six-lens community-of-experts design review.
- [x] (2026-09-09T00:00:00Z) Verified the review's decisive claims directly:
      the `test_support` compile probe; the ADR-021 collisions; the
      mutation-evidence contract; the 400-line ceiling on
      `src/ninja_gen/mod.rs`; the `graph_view` prior art; `NETSUKE_REQUIRE_NINJA`;
      the nextest no-retry policy; and the nightly `cargo-mutants` job.
- [x] (2026-09-09T00:00:00Z) Rewrote the plan against the review findings.
- [x] (2026-09-27T00:00:00Z) Approval gate: the user directed implementation to
      proceed, which is the explicit approval the gate requires.
- [x] (2026-09-27T00:00:00Z) `EP-M0` question 7: a `src/`-side `#[cfg(test)]`
      module cannot receive a `netsuke`-typed value from `test_support`.
- [x] (2026-09-27T00:00:00Z) `EP-M0` question 6: `adr-021` is superseded by
      merge; the next free number is `ADR-030`, verified by sweeping every
      local and remote ref and every open pull request.
- [x] (2026-09-27T00:00:00Z) `EP-M0` question 3: per-case cost, plus the
      `ninja -t commands` and `default`-position findings.
- [x] (2026-09-27T00:00:00Z) `EP-M0` question 5: integration-test regression
      seeds **do** persist and replay; the plan's "inert" risk is falsified and
      removed.
- [x] (2026-09-27T00:00:00Z) `EP-M0` question 2: `OBL-E2E` holds across 24
      declaration permutations; the excluded rule-name class is reachable.
- [x] (2026-09-27T00:00:00Z) `EP-M0` question 1: the bounded
      re-materialization loop is unsound at small N. `OBL-ORDER` was first
      rewritten to draw a well-separated pair of key sets; that replacement was
      itself found unsound in the `EP-M1` review round and superseded by the
      fixed-graph-value arm (see the fourth revision note).
- [x] (2026-09-27T00:00:00Z) `EP-M0` question 4: shrinking does not converge on
      a gradient-free predicate; `Tolerance 5` and a new `Risk` added.
- [x] (2026-09-27T00:00:00Z) Recorded all `EP-M0` answers and deleted the
      scratch prototype. `EP-M0` is complete; nothing from the prototype is
      merged.
- [x] (2026-10-10T00:00:00Z) `EP-M1`: wrote
      `docs/adr-030-ninja-emission-determinism-contract.md` and indexed it in
      `docs/contents.md`. Reading the code to state the contract found two
      defects in this plan, both recorded as amendments in the ADR and
      corrected here (see `Surprises & discoveries`).
- [x] (2026-10-10T00:00:00Z) `EP-M1` review round: `coderabbit review --agent`
      returned four findings, disposed **accept, accept, already fixed,
      decline**. Two were major design defects in `OBL-ORDER` and `OBL-PROCESS`
      and are corrected; the third was raised against the committed revision and
      is already fixed in the tree; the fourth asked for a bare ADR `Status`
      value, which the style guide's operative sentence forbids, and is
      declined with the style-guide citation and the ADR corpus as evidence. A
      fifth class the review did not raise was found by sweeping both
      documents: the `OBL-ORDER` defect recurred unswept in the ADR's `I-1`
      (`ADR-030` amendment 4), and the `Amendments` intro miscounted. A sixth
      was found by re-deriving the mechanism rather than propagating it:
      finding 2's proposed remedy (`MUT-HASHMETA` for `OBL-PROCESS`) is itself
      unsound, and was superseded by `MUT-ACTIONSORT` before commit. Six
      `Decision log` entries, four `ADR-030` amendments, and the mutation table
      reflect the corrected positions.
- [x] (2026-10-10T00:00:00Z) The three Markdown gates passed on the frozen
      correction, at `HEAD`
      `593c7b745c52fb60e0c9b3acb136a365d0c7d005`. `make check-fmt` exit 0,
      "167 files left unchanged"; `make markdownlint` exit 0, spelling
      prerequisite executed and "0 issues in 0 files"; `make nixie` exit 0,
      "All diagrams validated successfully!". Logs:
      `/tmp/check-fmt-netsuke-4-3-1-proptests-for-deterministic-ninja-emission.out`,
      `/tmp/markdownlint-netsuke-4-3-1-proptests-for-deterministic-ninja-emission.out`,
      `/tmp/nixie-netsuke-4-3-1-proptests-for-deterministic-ninja-emission.out`.
      The first `check-fmt` run failed and needed one `make fmt` cycle; the
      `mdtablefix` change was proven to be a whitespace-only rewrap (identical
      word sequence, unchanged ordered-list marker count) before it was
      applied.
- [ ] `EP-M2`: file splits, mutation-evidence contract, ordering helpers,
      `make proptest`.
- [ ] `EP-M3`: shared strategy and compact `Debug`.
- [ ] `EP-M4`: `OBL-PATHKEY`, `OBL-NOLOSS`.
- [ ] `EP-M5`: `OBL-ORDER`, `OBL-DEFAULT`, `OBL-NOHASH`.
- [ ] `EP-M6`: `OBL-ACTION`, `OBL-E2E`, `OBL-PROCESS`.
- [ ] `EP-M7`: `OBL-DUP`, `OBL-CYCLE`.
- [ ] `EP-M8`: `OBL-NINJA`, documentation, evidence sweep, roadmap.

## Surprises & discoveries

- (2026-09-09) The emitter's edge sort is *stable* and its key is unique only
  because `from_manifest` rejects duplicate outputs. Weaken that and ties fall
  back to `HashMap` order, dropping a real edge non-deterministically. This
  drove the obligation structure.
- (2026-09-09) `path_key` joins with NUL, so it is injective only because the
  validator runs first and rejects control characters. Two concrete collisions
  exist: `path_key(["a","b"]) == path_key(["a\0b"])`, and
  `path_key([]) == path_key([""])` — the second because the empty string passes
  validation. The first draft asserted injectivity without excluding empty
  components and was wrong.
- (2026-09-09) A `test_support` strategy carrying netsuke types **cannot** be
  used from a `src/`-side test. Proven, not argued; see `Artefacts and notes`.
- (2026-09-09) `docs/verification/mutations/` is governed by
  `tests/kani_mutation_evidence_tests.rs`, which rejects any patch stem not
  rooted at `ir` and requires harness correspondence. The first draft's patches
  were unrepresentable and would have failed `make test` on the first commit
  that added one.
- (2026-09-09) `src/ninja_gen/mod.rs` is exactly 400 lines, the `AGENTS.md`
  ceiling. Any wiring line breaches it.
- (2026-09-09) `src/graph_view/tests_property.rs` already implements an
  insertion-order-invariance property over `BuildGraph`, in 169 lines. The
  first draft claimed no such strategy existed.
- (2026-09-09) `RandomState` is not part of the Proptest seed, so a property
  whose predicate depends on `HashMap` iteration order is not reproducible from
  its seed. Shrinking silently discards valid candidates and committed
  regression seeds are decorative. This is the deepest flaw the review found.
- (2026-09-09) `ninja -t commands` does not load dyndep sidecars: a graph
  referencing a missing sidecar exits 0. An oracle using only `-t commands`
  would be vacuous over the sidecar bundle.
- (2026-09-09) *(Falsified 2026-09-27.)* Three of the five committed
  `tests/*.proptest-regressions` files are probably never replayed, because
  Proptest's default persistence finds no `lib.rs`/`main.rs` above an
  integration-test crate. **Wrong:** persistence falls back to the crate root,
  saves, and replays. The warning is cosmetic. See `EP-M0` question 5.
- (2026-09-09) `register_action` hard-codes `depfile`, `deps_format`, `pool`,
  and `restat`, so no manifest can populate them. `OBL-ORDER`'s direct-graph
  strategy is the only obligation reaching those emitter branches, which makes
  `OBL-E2E` *not* strictly stronger, contrary to the first draft.
- (2026-09-09) `process_rules` is last-writer-wins on duplicate rule names and
  no `DuplicateRule` error exists, so `OBL-E2E`'s unrestricted form is false.
- (2026-09-09) An output-less target still registers its action, so its `rule`
  block is emitted with no `build` statement referencing it. **Partly
  superseded 2026-10-10:** the *rule block* half holds, but the framing implied
  the edge is dropped. It is not — `insert_canonical_edge` pushes it and
  `render_edges` emits a `build` line with an empty left-hand side. See
  `ADR-030` amendment 2 and the corrected entry below.
- (2026-10-10) *(Corrected.)* The `Constraint 10` mechanism,
  "`insert_edge_for_outputs` stores an edge once per output, so the emitter's
  sort sees `targets.len()`", was true on 2026-09-09 and stopped being true on
  2026-09-18. `2c030fd1` ("Store multi-output build edges once", #652/#714) made
  `insert_edge_for_outputs` call `graph.insert_edge` directly, which pushes
  one arena entry. `graph.edges()` therefore yields the edge count, and the
  200-output bound is a cost budget rather than a correctness bound.
  **Lesson:** the plan was drafted against one revision and rebased onto
  another; a mechanism claim in a plan that spans a refactor must be re-derived
  at the revision it will be implemented on, not carried forward.
- (2026-10-10) *(Corrected.)* The output-less edge is **not** silently dropped.
  `insert_canonical_edge` pushes unconditionally and `render_edges` renders
  every arena entry, with no `seen` set anywhere. Such an edge emits a `build`
  line with an empty left-hand side. Whether a manifest can produce one at all
  is unresolved: `Target::name` accepts `StringOrList::Empty`, which `map_each`
  maps to an empty vector, and `get_target_display_name` explicitly tolerates
  it, but nothing tests that path. `OBL-NOLOSS`'s justification is corrected
  pending `EP-M4`'s reachability finding, and its validator is replaced: the
  original `MUT-GUARD` reordered a *validator* that returns an error rather
  than dropping an edge, so it could not falsify a no-loss property at all.
  `MUT-EDGEDROP` targets the drop directly.
- (2026-09-09) `make fmt`'s `mdtablefix --renumber` converted a wrapped line
  beginning "72." into an ordered-list item, truncating the sentence before it.
  No gate caught it; it was found by a reviewer reading the prose.
- (2026-09-27) `HashMap` iteration order is a function of the *key set* as much
  as of insertion order. A one-key map has exactly one insertion order, so two
  "different" permutations of it are the same map and can never produce
  different iteration orders. The first draft's retry-until-they-differ design
  was therefore guaranteed to fail every minimal case. Measured: 200/200 trials
  exhausted their budget at one key, 23/200 at two, 0/200 at five.
- (2026-09-27) A deliberately failing property in an integration-test crate
  saves and replays its regression seed. `failed to find lib.rs or main.rs` is
  a fallback notice, not a persistence failure. This overturned a `Risk` entry
  the design review had added.
- (2026-09-27) `OBL-E2E` holds today, confirmed empirically rather than by
  reading: 24 declaration permutations emitted byte-identical bundles. Also
  confirmed reachable — two same-named rules with different bodies *do*
  diverge, so the restriction `OBL-E2E` states is load-bearing.
- (2026-09-27) Shrinking a gradient-free predicate does not converge. Proptest
  reached ~940 candidates/s, ~4,700 in a 5-second window, and 30 seconds still
  stopped on the wall rather than on a minimum, reporting a different "minimal"
  input each run (39, 90, 43, 73 edges). A moderate iteration cap (`4 x cases`
  = 128) actually produces a *smaller* case than a 200,000 cap, because the cap
  bounds the time spent wandering. Readability needs a compact diagnostic;
  minimization needs a gradient. Both, not either.
- (2026-09-27) `origin/main` now carries `adr-020` through `adr-029`, so the
  design review's `ADR-NNN` placeholder resolves to a concrete number instead
  of being deferred. The plan's two-round-old claim that `adr-020` and
  `adr-021` were both multiply claimed is stale.
- (2026-10-10) `docs/formal-verification-methods-in-netsuke.md`, the document
  this plan descends from, carries three defects that `EP-M8` inherits and that
  no earlier section of this plan had recorded. First, footnote `[^6]` links
  `../src/ninja_gen.rs`, which does not exist — `src/ninja_gen` is a directory.
  Second, footnote `[^9]` links `../src/manifest/mod.rs`, which **does** exist,
  so that link is correct and must not be "fixed". Third, line 311 cites a
  README promise "a reproducible, fully static dependency graph" attributed to
  "line 17"; the phrase appears nowhere in `README.md`, and line 17 is a badge
  definition block. The plan already records the stale citation; the two
  footnote findings are new and are now named in `EP-M8`'s scope. **Correction
  to an earlier note in this session:** the `[^9]` path was believed broken
  too. It is not. Probe the path before recording a documentation defect, and
  re-probe at the tree you are correcting.
- (2026-10-10) Prose written *for* a gate can still fail a gate, and the two
  gates here disagree about what is legal. Three defects introduced by the
  review-round corrections, all caught by `make check-fmt` and
  `make markdownlint` on the first re-gate:
  - `MD049` requires *underscore* emphasis, and the house prose uses it
    throughout. Writing `*key sets*` instead of `_key sets_` is a lint error,
    not a style preference.
  - `MD033` rejects inline HTML, so a prose mention of an angle-bracketed
    placeholder such as `<title>` must be in backticks or reworded.
  - `mdtablefix` reflows any paragraph whose lines are not already at the
    canonical wrap, so *adding* a sentence to a paragraph re-opens
    `make check-fmt` for that paragraph. Rewrap as part of the edit, or expect
    `make fmt` to touch the file again.

  **Lesson:** the three Markdown gates are not redundant, and a docs-only
  change is not exempt from the edit-then-gate cycle. Run them after prose
  edits, not only after structural ones.
- (2026-10-10) **The remedy proposed by a review finding can be as unsound as
  the defect it names, and it has to be re-derived rather than adopted.**
  Finding 2 correctly identified that `MUT-DEFSORT` cannot fail a two-run
  comparison. Its remedy — validate `OBL-PROCESS` with `MUT-HASHMETA`, on the
  grounds that the hasher would then derive an `Action`'s identity from a
  per-process `RandomState` — was written into three places before it was
  checked, and it is wrong. `OBL-ACTION` generates `Action` values directly and
  does set `pool`, so `MUT-HASHMETA` is a sound validator *there*; but
  `OBL-PROCESS` never sees a generated `Action`. `pool` is never `Some` at any
  construction site in `src/`, and `register_action` recomputes the same hash
  from the same fixed manifest in both processes, so perturbing hash *values*
  moves both runs together. The right validator is `MUT-ACTIONSORT`, which
  deletes the only guard over the only unordered collection the emitter
  iterates: `graph.actions` is an `IrHashMap` and `actions.sort_by_key` in
  `write_action_rules` is what re-establishes order.

  **Lesson:** a mechanism claim is verified against `src/`, not against the
  plausibility of the sentence carrying it. The check that would have caught
  this on first writing is cheap — grep the field the mutation perturbs, and
  confirm it is ever populated. Two `grep`s (`pool: Some`, `RandomState`) took
  under a second and falsify the whole argument.

  **Corollary, found immediately after:** the *replacement* validator needed
  the same scrutiny one level down. `MUT-ACTIONSORT` is sound in mechanism but
  carries a precondition — it is only observable when the action map is large
  enough to iterate in more than one order. The existing snapshot fixtures hold
  one to three distinct actions, and `EP-M0` question 1 already measured that
  maps below roughly five keys usually have no alternative order at all.
  Validating with those fixtures alone would have reproduced the exact vacuity
  the obligation was written to avoid, in a form that still *looks* validated.
  The obligation's domain now requires a fixture with at least five distinct
  actions. **Lesson:** changing a validator means re-asking every question the
  old validator was asked, including its preconditions — a correct mechanism
  with an unreachable input is the same failure as a wrong mechanism.
- (2026-10-10) The sweep for that fourth defect found it in the fourth place it
  was written, not the first: the plan's non-vacuity note, its mutation table
  row, and the `EP-M6` acceptance criterion all propagated the unsound remedy
  mechanically, and only the `Decision log` entry re-deriving the mechanism
  exposed the error. **Lesson:** when a correction is mechanical propagation of
  a claim, re-derive the claim at exactly one site, before propagating.

- (2026-10-10) A `Progress` checkbox asserted "All three Markdown gates green
  on the correction" *before* the correction had been gated, and the first
  re-gate then failed on two lints. A checkbox is a claim about evidence that
  exists, not a statement of intent, and a plan whose progress log overstates
  its verification is worse than one with no log. The entry was rewritten to
  cite the gate result rather than predict it. **Rule adopted:** no `Progress`
  entry may cite a gate outcome that is not already in a log file.

## Decision log

- Decision: keep this ExecPlan pre-implementation and approval-gated.
  Rationale: the user stated the plan must be approved before implementation.
  Date/Author: 2026-09-09 / planning agent.

- Decision: include the larger-N duplicate-output and cycle-rejection
  obligations inherited from `4.2.1` and `ADR-004`. Rationale: the user chose
  the wide scope. Both upstream artefacts state that `4.3.1` closes this
  coverage, so omitting it leaves an accepted ADR undischarged with no owner.
  The design review recommended splitting them into a sibling item; they are
  kept in a self-contained milestone so that remains possible. Date/Author:
  2026-09-09 / planning agent.

- Decision: settle the determinism contract here, in `ADR-NNN` plus the users'
  guide and README. Rationale: the user chose this; `FV-CONTRACT` asks for the
  decision and no roadmap item owns it. Date/Author: 2026-09-09 / planning
  agent.

- Decision: write the ADR in `EP-M1`, before any property. Rationale: the first
  draft wrote it at milestone six, after every property was green, which would
  have let the contract be fitted to whatever the code did. Verification of a
  contract derived from the tests is circular. Date/Author: 2026-09-09 /
  planning agent.

- Decision: do not tier the suite. Rationale: measured. The whole new suite is
  about 6 s of CPU and 2–3 s wall against a 45-second budget, so the
  `PROPTEST_HEAVY` gate, `make proptest-heavy`, and a separate CI job would be
  branches never taken; a standalone CI job would be ~98% compile. This answers
  the user's instruction to measure and judge. Date/Author: 2026-09-09 /
  planning agent.

- Decision: place all strategies in `src/`, not `test_support`. Rationale: a
  compile probe proved the `test_support` route does not work for netsuke-typed
  values consumed from `src/`-side tests. Date/Author: 2026-09-09 / planning
  agent.

- Decision: restructure `OBL-ORDER` into a seed-deterministic core over
  extracted pure ordering helpers, plus an end-to-end property that
  re-materializes until iteration orders differ. Rationale: `RandomState` is
  outside the Proptest seed, so the naive property breaks shrinking and makes
  regression seeds inert. This also deletes the run-level vacuity floor, which
  had no implementation path compatible with the `proptest!` macro and was
  itself a flake source against an explicit no-retry policy. Date/Author:
  2026-09-09 / planning agent.

- Decision: replace `OBL-GUARD` with `OBL-NOLOSS`, asserting the outcome rather
  than the mechanism. Rationale: "the validator runs before the sort" is a
  proxy; "no edge is silently dropped" is the failure that matters, survives
  legitimate refactors, and additionally catches the output-less-edge drop.
  Date/Author: 2026-09-09 / planning agent.

- Decision: add `OBL-NOHASH`, a source-shape contract test. Rationale: no
  property can catch a *future* map keyed on a field the generator does not
  vary, so the plan's stated success criterion was not actually delivered by
  any obligation. A source contract delivers it, following three existing
  contract tests in this repository. Date/Author: 2026-09-09 / planning agent.

- Decision: add `OBL-PROCESS`, a two-run `assert_cmd` byte comparison.
  Rationale: the plan publishes the process-level guarantee and nothing in the
  repository tests it. Roughly twenty lines close the gap between what is
  promised and what is checked. Date/Author: 2026-09-09 / planning agent.

- Decision: reject the ordered-map port, on the ground that it would make the
  sort calls dead and the mutation evidence evaporate — not on the ground that
  `Constraint 1` forbids it. Rationale: citing this plan's own constraint as
  the reason to reject the alternative to that constraint is circular, as the
  review noted. Date/Author: 2026-09-09 / planning agent.

- Decision: nine mutation patches, not eleven, and generalize
  `tests/kani_mutation_evidence_tests.rs` rather than bypass it. Rationale: the
  nightly `cargo-mutants` job already generates sort-deletion mutants; hand
  patches earn their keep on the reordering faults it cannot generate. The
  contract test is a repository-wide rot detector and weakening it would be
  worse than dropping a patch. Date/Author: 2026-09-09 / planning agent.

- ~~Decision: use `ADR-NNN` as a placeholder and allocate the number in the
  final commit.~~ **Superseded 2026-09-27.** Decision: the number is
  **ADR-030**, allocated now rather than deferred. Rationale: the earlier
  rationale (four branches claiming `adr-021`) was true when written but has
  been overtaken — `origin/main` merged `adr-020` through `adr-029`. Sweeping
  every local and remote ref and every open pull request, `adr-030` and
  `adr-031` are the lowest unclaimed numbers; `adr-039`–`adr-041` are claimed
  in flight. Deferring allocation again risks a second collision, and `EP-M1`
  needs the number to write the filename. Date/Author: 2026-09-27 /
  implementation agent.
- Decision: `OBL-ORDER`'s end-to-end arm stops re-materializing until iteration
  orders differ, and the generated lower bound rises to five actions and five
  edges. Rationale: `EP-M0` question 1 measured that the original loop cannot
  succeed below five keys (200/200 failures at one key, 23/200 at two), so it
  would have failed every minimal counter-example. Failing a case for a
  property the *domain* makes unreachable is a flakiness source, and this
  repository has an explicit no-retry policy. The replacement first chosen — a
  well-separated pair of key sets — was itself unsound and was superseded on
  2026-10-10 by the fixed-graph-value arm; the five-key lower bound survives
  that correction unchanged, since it bounds generation rather than the
  comparison. Date/Author: 2026-09-27 / implementation agent.
- Decision: add `Tolerance 5`, bounding what a representation-independent
  property may assert, and a matching `Risk`. Rationale: `EP-M0` question 4
  showed shrinking does not converge for such predicates, so the plan could
  otherwise author obligations that are correct but not diagnosable. The
  tolerance makes "assert something with a gradient, or pair it with a compact
  value-level diagnostic" an enforceable rule rather than a preference.
  Date/Author: 2026-09-27 / implementation agent.
- Decision: withdraw the planned separate fix for the three supposedly inert
  `tests/*.proptest-regressions` files. Rationale: `EP-M0` question 5 falsified
  the premise. There is nothing to fix, and carrying a scope item for a
  non-problem would be exactly the kind of unbounded scope the `Tolerances`
  section exists to prevent. Date/Author: 2026-09-27 / implementation agent.

- Decision: state `OBL-E2E` only for manifests with distinct rule names, and
  add a directed test reaching the excluded class. Rationale: `process_rules`
  is last-writer-wins and the unrestricted claim is false; a generator that
  allocates unique names would have passed vacuously on exactly the class where
  the contract fails. Date/Author: 2026-09-09 / planning agent.

- Decision: follow the obligation-driven ExecPlan style of `4-2-3`, with
  sentence-case back-matter headings. Rationale: it is the most recent
  precedent and the closest structural analogue. Date/Author: 2026-09-09 /
  planning agent.
- Decision: publish exactly one guarantee — process-level reproducibility on a
  fixed `(manifest, environment, platform, shell selection, Netsuke version)`
  tuple, covering the main file and every sidecar — and record the
  insertion-order, declaration-order, and within-edge statements as internal
  invariants with numbered preconditions rather than as promises. Rationale:
  the four statements have different scope and strength and are routinely
  conflated; users cache against the process-level one; and only a stated
  specification gives `ADR-004`'s hand-off something to hand off to.
  Date/Author: 2026-10-10 / implementation agent.
- Decision: state **not** promised, as loudly as the promise, that byte
  stability is not promised across Netsuke versions. Rationale: `PARENT_SCHEMA`
  and `DYNDEP_SCHEMA` embed a format tag into every content-addressed dyndep
  identity precisely so the staging format can change without colliding with an
  older namespace; a cross-version promise would make those tags pointless.
  Date/Author: 2026-10-10 / implementation agent.
- Decision: keep the ordered-map port rejected, but on the *evidence* ground
  rather than on `Constraint 1`. Rationale: determinism is already structural,
  so a `BTreeMap` would leave both `sort_by_key` calls dead and
  `MUT-EDGESORT`/`MUT-ACTIONSORT` unable to fail anything. Recording the real
  reason matters because the earlier phrasing made the rejection look like an
  appeal to this plan's own constraint, which is circular. Date/Author:
  2026-10-10 / implementation agent.
- Decision: `Constraint 10`'s 200-output bound is retained as a **cost budget**
  and its stated mechanism withdrawn; `OBL-NOLOSS`'s "silently dropped" premise
  and `MUT-GUARD`'s mechanism are corrected pending a reachability finding in
  `EP-M4`. Rationale: reading the code at the implementation revision falsified
  both. `2c030fd1` removed the per-output storage, and no `seen` set exists.
  Both corrections are recorded as `ADR-030` amendments, because the
  obligations cite that document as their authority. Date/Author: 2026-10-10 /
  implementation agent.
- Decision: hold the graph **value** fixed in `OBL-ORDER`'s end-to-end arm and
  grade the arm inconclusive when no case's iteration orders diverged, rather
  than requiring a differing key set to force divergence. Rationale: a pair of
  graphs with different key sets is a pair of different graphs, so any bundle
  difference is legitimate and any agreement is uninformative — the arm could
  not have established `I-1` in either outcome. The weaker arm is the sound
  one; the collector-level arm over an explicitly shuffled `Vec` supplies the
  guaranteed perturbation the end-to-end arm cannot. Date/Author: 2026-10-10 /
  implementation agent.
- Decision: validate `OBL-PROCESS` with `MUT-ACTIONSORT`, not `MUT-DEFSORT`,
  and remove `OBL-PROCESS` from `MUT-DEFSORT`'s coverage. Rationale:
  `MUT-DEFSORT` deletes a sort whose input order is fixed for a fixed manifest,
  so it is deterministic across processes and cannot fail a two-run comparison;
  using it as the validator would have produced a test that passes whether or
  not it can see anything. `OBL-PROCESS` exists to catch *cross-process*
  divergence, which is the `HashMap` `RandomState` class, and `MUT-ACTIONSORT`
  is the mutation that reaches it: `graph.actions` is an `IrHashMap`, and
  `actions.sort_by_key` in `write_action_rules` is the only guard
  re-establishing a deterministic order over it. Date/Author: 2026-10-10 /
  implementation agent.
- Decision: **supersede** the first correction of the entry above, which had
  named `MUT-HASHMETA` as `OBL-PROCESS`'s validator. Rationale: that choice was
  unsound, and the sweep that caught it is the reason a claim is worth
  re-deriving before it is preferred to another agent's. `MUT-HASHMETA` does
  validly falsify `OBL-ACTION` — that property generates `Action` values
  directly with `pool` set — but `OBL-PROCESS` never sees a generated `Action`.
  `pool` is never `Some` in any construction site in `src/`, and the hash
  `register_action` computes is recomputed identically in both processes for a
  fixed manifest, so perturbing hash *values* moves both runs together. The
  earlier entry's own *premise* about `MUT-DEFSORT` was and remains correct;
  only its replacement was wrong. Date/Author: 2026-10-10 / implementation
  agent.
- Decision: **decline** the review finding that `ADR-030`'s `Status` must be a
  bare closed-set value, and keep `Accepted` with its date and a one-sentence
  summary on the following line. Rationale: the review cited the style guide's
  template placeholder, but the guide's operative sentence
  (`docs/documentation-style-guide.md:378-379`) *requires* "the date and a
  brief summary of what was decided" for `Accepted`. Complying would have
  violated the rule invoked to support the finding. Corpus evidence: `adr-002`
  uses the same shape, `adr-019` and `adr-027` append a summary sentence, and
  no gate in `tests/`, `scripts/`, or `tests/workflow_contracts/` reads the
  field. The value is still the closed-set token; only the qualifier is
  retained, and it sits outside the value's own paragraph. Date/Author:
  2026-10-10 / implementation agent.
- Decision: correct `ADR-030`'s `I-1` as well as the plan's `OBL-ORDER`, and
  record the change as `Amendment 4` rather than as an edit to the ADR's body.
  Rationale: both documents stated the same unsound mechanism — the review
  found it in the plan and reported the ADR as contradicting the plan, but the
  ADR carried the defect too, at `:169`–`:173`. Because the ExecPlan's
  obligations cite the ADR as their authority, the correction is recorded where
  a reader arrives from that citation. This is the fourth instance of a single
  pattern: a defect corrected in one document recurring unswept in its sibling.
  Date/Author: 2026-10-10 / implementation agent.
- Decision: keep `(2026-09-09)` and `(2026-09-27)` entries in
  `Surprises & discoveries` and `Progress` in their original wording, adding a
  dated correction beside them rather than rewriting them. Rationale: a dated
  entry records what was believed on that date, and a later reader needs to see
  that the belief was held and then corrected. Rewriting it destroys the
  evidence that the correction happened. The review asked for one such rewrite;
  it is declined for this reason. Date/Author: 2026-10-10 / implementation
  agent.
- Decision: sweep and re-verify **every** `src/` line citation in both documents
  after a citation check exposed drift, instead of fixing the cited instances.
  Rationale: the drift has a single cause — `2c030fd1` rewrote the files these
  plans cite — so instances are symptoms, not the defect. Fixing only reported
  instances would leave the next reader trusting five more stale numbers.
  Date/Author: 2026-10-10 / implementation agent.

## Design review findings

A six-lens review (structure, alternatives, scaling, contracts, failure modes,
viability) was run against the first draft. Findings that changed the plan are
recorded above in `Surprises & discoveries` and `Decision log`. Findings
accepted but deferred, so they are not lost:

- The scope tolerance in the first draft was breached by its own artefact list
  on day one. The number is now 34 files with a net-lines cap, chosen to be
  believable rather than aspirational.
- `ci-windows.yml` installs Ninja but does not set `NETSUKE_REQUIRE_NINJA`, so
  that lane's Ninja-dependent tests skip silently. Out of scope here; worth a
  separate item.
- ~~Three committed `tests/*.proptest-regressions` files are probably inert.
  This plan's properties are library-side, where persistence works; the
  existing files deserve a separate fix.~~ **Withdrawn 2026-09-27:** the
  premise was falsified by `EP-M0` question 5. The files are live and need no
  fix.
- ~~`src/ninja_gen/mod.rs:178` clones a key that is dead immediately after
  (`seen.insert(key.clone())`), where `dyndep.rs:161` already moves it. A
  trivial follow-up, not taken here under `Constraint 1`.~~ **Withdrawn
  2026-10-10:** the cited line is now a `graph.actions.get(...)` lookup, there
  is no `insert(key.clone())` anywhere in `src/ninja_gen/`, and the
  `staged_sidecars` insert at `dyndep.rs:286` already clones without a needless
  rebind. `2c030fd1` removed the code this note described, so the follow-up is
  moot.
- The review recommended splitting `EP-M7` into a sibling roadmap item and
  dropping `OBL-NINJA` entirely. Both were declined: the first because the user
  directed the wide scope, the second because the measured `-n` finding shows
  the oracle covers the sidecar path that nothing else reaches.

## Outcomes & retrospective

To be completed after `EP-M8`. Record: whether any property failed against
current production code; whether `OBL-E2E` held; the measured suite cost
against the 6-second estimate; which mutations proved hardest to make specific;
whether the `graph_view` strategy was successfully absorbed; and whether the
generalized mutation-evidence contract held up.

## Revision note

- 2026-10-10 (fifth revision, `EP-M1` review round 2): a second CodeRabbit pass,
  over `74cff9ce`, returned nine findings — six major, three minor. Every
  finding was checked against the tree by hand before anything was changed.
  Dispositions were **accept** (eight) and **accept in substance, misread in
  part** (one).

  Accepted — finding 1 (major, `OBL-PROCESS`): the two-run comparison covered
  `build.ninja` alone, while `G-1` is stated over the complete artefact, so a
  process-dependent sidecar path or body would pass it. The statement now
  compares whole bundles, and the two runs write to separate output directories
  so neither can satisfy the other's sidecar reference.

  Accepted — findings 2 and 8 (major, acceptance item 6 and `OBL-CYCLE`). The
  `MUT-DEFSORT` acceptance criterion was impossible — for a fixed manifest the
  unsorted default order is fixed too — and is replaced by `MUT-ACTIONSORT`.
  The cyclic generator arm draws dependencies from strictly lower indices, so a
  single added back edge closes a path only when one already exists; the arm
  now builds a known path before adding its closing edge, or injects a
  self-edge.

  Accepted — findings 3, 4, and 6 (major, `OBL-NOLOSS`, `OBL-NOHASH`, and the
  cross-version decision statement). `MUT-GUARD` is replaced by `MUT-EDGEDROP`
  across five sites, because reordering a validator that returns `Err` changes
  *when* it fires and never *whether* an edge is emitted. `OBL-NOHASH` is
  restated over *sorted* iteration, with its `ADR-030` twin corrected in the
  same change. The cross-version sentence now reads that byte stability is not
  promised across Netsuke versions.

  Accepted — findings 7 and 9 (minor). `G-1` now names the `netsuke generate`
  entry point and distinguishes its scope from the library entry points `P-2`
  qualifies. "You can observe success" becomes "Success is observable", per the
  second-person ban at `docs/documentation-style-guide.md:39`.

  Accepted in substance, misread in part — finding 5 (major, the `OBL-PATHKEY`
  reorder check). The warning that `prop_shuffle` may return the input order,
  and that repeated or singleton values cannot reorder at all, is correct of
  the arm that draws a list and shuffles it; the check there is now a recorded
  classification rather than a `prop_assert!`. It is not correct of the
  collector arm the finding cited, which shuffles an explicitly constructed
  `Vec` whose permutation the test controls, so that arm's perturbation is
  guaranteed. The note now says which arm carries it.

  *Process note:* a delegated head-drift analysis reported that findings 2 and
  8 were already addressed at `b68b8289`. It had matched line *numbers* rather
  than content, and both were live — the acceptance criterion still read
  `MUT-DEFSORT` and the cyclic arm still lacked its cycle construction. Do not
  accept a drift verdict that cites positions; require the quoted text.
- 2026-10-10 (fourth revision, `EP-M1` review round): a CodeRabbit pass over
  the `EP-M1` commit returned four findings. Dispositions were **accept,
  accept, already fixed, decline**. A fifth defect the review did not raise was
  found by sweeping both documents, and a sixth — an unsound remedy adopted
  from finding 2 — by re-deriving its mechanism against `src/`.

  Accepted — finding 1 (major, `OBL-ORDER`): the end-to-end arm varied the *key
  set* rather than only insertion order, which cannot establish `I-1` because a
  differing key set is a differing graph. The arm now holds the graph value
  fixed and grades itself inconclusive when no case's iteration orders
  diverged. The review's own verdict on the rest of this finding was a partial
  misread and is not acted on: it suggested the ADR contradicted the plan, but
  the ADR carried the *same* defect at `:169`–`:173`, so both documents needed
  the same correction rather than a reconciliation. That second site was found
  by the sweep below and recorded as `ADR-030` amendment 4.

  Accepted — finding 2 (major, `OBL-PROCESS`): `MUT-DEFSORT` is deterministic
  across processes for a fixed manifest, so it can never fail a two-run
  comparison. The finding propagated to two further assertions that repeated the
  `MUT-DEFSORT` claim; both are corrected. The remedy first adopted here —
  validating `OBL-PROCESS` with `MUT-HASHMETA` — was itself unsound and was
  superseded before commit by `MUT-ACTIONSORT`; see the two `Decision log`
  entries and the second `(2026-10-10)` surprise. This is the second defect in
  the round that was a wrong *mechanism* rather than a wrong assertion, the
  first being finding 1's key-set arm.

  Already fixed — finding 3 (minor, the `EP-M1` instruction list). The live
  tree has said "output-less edges **retained** and emitted with an empty
  left-hand side" since the third revision. The finding was raised against the
  committed revision `593c7b74`, which still carried the old text; the working
  tree had already moved past it. Its second half — that the dated
  `(2026-09-09)` historical entry should be rewritten — is declined: a dated
  `Surprises & discoveries` entry is a record of what was believed then, and the
  `(2026-10-10)` entry immediately below it carries the correction.

  **Declined** — finding 4 (minor, `ADR-030`'s `Status`). The finding asked for
  a bare `Accepted.` on the authority of the style guide's template
  placeholder. The operative rule in
  `docs/documentation-style-guide.md:378-379` says the opposite: "For
  `Accepted` status, include the date and a brief summary of what was decided."
  Corpus evidence agrees — `adr-002` carries the same summary shape, `adr-019`
  and `adr-027` append one, and nothing in `tests/`, `scripts/`, or
  `tests/workflow_contracts/` enforces the bare form. Complying would have
  violated the rule cited to support it. *Observation, not acted on:* the
  heading is `# Architecture decision record (ADR): <title>` where the template
  and `adr-002` use `# Architectural decision record (ADR) NNN:` plus a title.
  The whole corpus varies here and no gate reads it.

  Found by the sweep, not raised by the review — five items in three classes.
  *Unfixed twin:* the `OBL-ORDER` defect recurred verbatim in `ADR-030`'s
  `I-1`, which is `Amendment 4`. *Stale-revision drift* of the same `2c030fd1`
  origin that produced amendments 1 and 2, in five more places
  (`Constraint 10`'s blueprint block, `OBL-NOHASH`'s "two `seen` sets", a
  withdrawn `mod.rs:178` note, and six line citations), recorded as
  `Amendment 3` and the citation sweep below. *Count:* the `Amendments` intro
  said "Two facts" with three amendments below it; corrected to four once
  `Amendment 4` landed. Every `src/` line citation in both documents was
  re-verified against the tree at this revision. *Lesson:* when a review finds
  a design defect in one document and the sibling document states the same
  mechanism, fix both — the majority of these findings were the unfixed twin of
  something already corrected.

  *Process note:* the first pass at these corrections left the plan's
  `Progress` entry asserting "All three Markdown gates green on the correction"
  before the correction had been gated. A checkbox is a claim about evidence,
  so it was rewritten to point at the gate result rather than to predict it. Do
  not let a Progress entry outrun the gate log it cites.
- 2026-10-10 (third revision, `EP-M1`): `ADR-030` is written and indexed. Two
  defects found while deriving the contract from the code at the implementation
  revision, both corrected here and recorded as `ADR-030` amendments: the
  200-output bound's stated mechanism is stale (`2c030fd1` removed per-output
  edge storage), and `OBL-NOLOSS`'s "silently dropped output-less edge" premise
  is false (nothing drops edges; reachability of the construct is unresolved
  and is assigned to `EP-M4`). Neither changes a milestone or an obligation's
  statement; both change a justification, which is why the obligations now cite
  `ADR-030` rather than restating a mechanism.
- 2026-09-27 (post-`EP-M0`): all seven `EP-M0` questions are
  answered and the spike is complete. Two plan defects were found and fixed
  before any implementation began: `OBL-ORDER`'s bounded re-materialization
  loop was unsound below five keys, and the "regression seeds are inert" risk
  was false. One new tolerance (`Tolerance 5`) and one new `Risk` were added,
  both from the measured behaviour of shrinking. `ADR-NNN` resolved to
  `ADR-030`. The prototype was deleted; nothing from it is merged. Milestone
  completeness is unchanged — `EP-M1` through `EP-M8` proceed as written apart
  from the `OBL-ORDER` rewrite.
- 2026-09-09 (first draft): established the obligation set, the determinism
  statements, the shared strategy, mutation-driven red, and eight milestones.
- 2026-09-09 (second revision): rewritten after a six-lens design review. The
  strategy moved from `test_support` to `src/` after a compile probe proved the
  first layout impossible; `OBL-ORDER` was restructured because its predicate
  was not seed-reproducible; `OBL-GUARD` became `OBL-NOLOSS` to assert an
  outcome; `OBL-NOHASH` and `OBL-PROCESS` were added to cover the success
  criterion and the published guarantee, neither of which any first-draft
  obligation reached; `OBL-PATHKEY`, `OBL-ACTION`, and `OBL-E2E` had incorrect
  statements corrected; the ADR moved to the first milestone; the contract
  gained the platform, shell, and version parameters; two file splits and a
  contract-test generalization were budgeted; the tiering machinery was deleted
  on measured evidence; and the ADR number became a placeholder. Remaining work
  is `EP-M0` through `EP-M8`, gated on approval.

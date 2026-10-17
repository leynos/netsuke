# 4.3.1. Add Proptest coverage for deterministic Ninja emission

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: IN PROGRESS

The `Scope` tolerance blocked this plan from 2026-10-16 until 2026-10-17, and
the block is discharged: the user directed "Widen the cap", and the cap is now
56 files and 6,000 net added lines against a measured 28 files and 2,597 lines,
with a counted inventory for the rest. The derivation, and the reason the new
number is a measurement rather than a round figure, are in `Decision log`.

`EP-M0` through `EP-M5` are complete. `ADR-030` is written and indexed, and the
approval gate was passed on 2026-09-27. `EP-M4` closed on 2026-10-17 by
execution rather than by reading: its four files compiled for the first time,
`make proptest` passed 104 of 104 at `1b4b3fb1`, and both assigned mutations
were applied, observed to fail the named properties, and reverted. `EP-M5`
closed on 2026-10-18: its eight gates passed against `f0a2831f`, its milestone
`coderabbit review --agent` pass was finally obtained against `d4225faf`, and
every finding from that pass has a recorded disposition. Hosted CI then caught
one defect the local batch could not have caught, because `make typecheck` was
missing from it; the repair is in the working tree and the re-run batch is
`EP-M5`'s ninth local gate.

`EP-M2`'s thirteenth gate — `test-nextest` — is red on an environmental timeout
of `cli_configuration_fixture_compiles`, which is **not** this branch's test.
The branch has since been rebased onto `origin/main` and the failure **survived
the rebase**: `TIMEOUT [300.024s]` on `case_1_public_api`. The expectation that
`origin/main`'s repair would remove it was wrong, and the user's earlier
direction anticipated that case — "If that doesn't fix the timeout, address the
way the timeout is calculated to account for the queue" — so the allowance is
now widened to 900 s on the family override that carries all nine ambient
fixtures. The measurement and the reasoning are in `Surprises & discoveries` and
`Decision log`. The timeout was never this suite's cost: the child
`cargo check` is cold in a half-cleaned tree, and any of the nine fixtures can
be the one that pays it. No test's own behaviour is relaxed, and the per-test
tier is raised inside the unchanged 960 s whole-run budget.

`EP-M6` onward is unwritten. The compile pool is not a blocking factor: on
2026-10-17 it reported
`capacity 4, held 0, healthy, sccache unreachable, compiling directly`, with
nothing compiling anywhere on the host, and a compile can be started. Its
occupancy varies with other agents' work, so a slow gate is a queue measurement
rather than a failure signal.

The `EP-M5` milestone `coderabbit review --agent` pass is **obtained**, after
the seven service-side refusals recorded below. The successful run reviewed
`origin/main (077cf366)…HEAD` at `d4225faf` — the 43 files of that range — and
completed with exit 0 and six findings. Each has a disposition in `Progress`:
finding 1 is rejected with clock evidence, and findings 2 through 6 are
repaired. `docs/roadmap.md:595` shows the house pattern treats such a review as
completion evidence, and roadmap item 4.3.1 stays unchecked until `EP-M8`.

This revision follows a design review, a rebase onto `origin/main`, that
Windows repair, gate pass #5, and the `EP-M5` review round.

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

`src/graph_view/tests/property.rs` already contains
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
so it serves as a reference for the projection's own order-invariance. It is
*not* an oracle for emitted Ninja text: it never reads the emitter's output.

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
`src/ninja_gen/property_tests/ninja_oracle.rs` drives `TestRunner::run`
directly rather than using the macro, because it must skip when the `ninja`
binary is absent. This plan reuses that pattern wherever a post-run assertion
or a conditional skip is needed.

### Terms used in this plan

- **Build graph / IR** — the `BuildGraph` value above.
- **Edge** — one `BuildEdge`. On the simple `generate` path it renders as one
  Ninja `build` statement; on the bundle path a *serial* edge additionally
  renders its staged gate and sidecar lines, so the emitted `build` count
  exceeds the edge count. See `OBL-NOLOSS` for why that makes a count equality
  the wrong assertion.
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
  character. The first clause is a **property-domain restriction, not a loader
  guarantee**: an empty `Target::name` lowers to `explicit_outputs: []` and is
  accepted by `insert_edge`, so a well-formed graph means one whose generator
  excludes empty target names. See `ADR-030` P-1.
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
- `src/graph_view/tests/property.rs` — the prior-art insertion-order property.
- `src/ninja_gen/property_tests/ninja_oracle.rs` — the `TestRunner::run` and
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
RM-4.3.1.a   -> FV-DET      -> EP-M5 -> ninja_gen::determinism::property_tests::order::ordered_edges_ignore_input_order
RM-4.3.1.a   -> FV-DET      -> EP-M5 -> ninja_gen::determinism::property_tests::order::one_graph_emits_one_bundle_under_two_insertion_orders
RM-4.3.1.b   -> FV-DET      -> EP-M5 -> ninja_gen::determinism::property_tests::defaults::the_default_line_sorts_its_operands
RM-4.3.1.c   -> FV-DET      -> EP-M4 -> ninja_gen::determinism::property_tests::path_key::path_key_is_permutation_invariant
RM-4.3.1.c   -> FV-DET      -> EP-M4 -> ninja_gen::determinism::property_tests::path_key::path_key_is_injective_on_keyable_paths
FV-DET       -> ADR-NNN     -> EP-M4 -> ninja_gen::determinism::property_tests::no_loss::every_explicit_output_appears_exactly_once
FV-DET       -> ADR-NNN     -> EP-M5 -> tests::ninja_gen_hashmap_boundary::only_membership_use_of_unordered_collections_reaches_the_emitter
FV-DET       -> ADR-NNN     -> EP-M5 -> tests::ninja_gen_hashmap_boundary::the_matched_type_list_names_only_unordered_collections
RM-4.3.1.b   -> ADR-NNN     -> EP-M5 -> ninja_gen::determinism::property_tests::defaults::the_default_line_follows_every_build_line
RM-4.3.1.b   -> ADR-NNN     -> EP-M5 -> ninja_gen::determinism::property_tests::defaults::permuting_the_declared_defaults_does_not_change_the_text
RM-4.3.1.a   -> ADR-NNN     -> EP-M5 -> ninja_gen::determinism::property_tests::order::ordered_actions_ignore_input_order
FV-DET       -> FV-DET      -> EP-M6 -> ir::action_hash_property_tests::equal_actions_hash_equally
FV-CONTRACT  -> ADR-NNN     -> EP-M6 -> tests::ninja_determinism_process::two_runs_emit_identical_bytes
RM-4.3.1.a   -> ADR-NNN     -> EP-M6 -> ninja_gen::determinism::property_tests::declaration::declaration_order_does_not_change_emission
RM-4.2.1.dup -> ADR-004     -> EP-M7 -> ir::graph::property_tests::duplicates::duplicate_outputs_are_rejected_at_larger_n
RM-4.2.1.cyc -> ADR-004     -> EP-M7 -> ir::graph::property_tests::cycles::cycles_are_rejected_at_larger_n
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
4. No file may exceed 400 lines. `src/ninja_gen/mod.rs` measures **394** raw
   lines at this plan's revision, not the 400 that earlier drafts stated, and
   `tests/kani_mutation_evidence_tests.rs` is at 383; both are split in `EP-M2`
   before anything is added to them. The split buys headroom rather than
   repairing an existing breach. Extracted modules go **inside** the parent's
   existing directory (`src/ninja_gen/named_action.rs`), never as a
   `<parent>_<role>.rs` sibling — the latter violates the module-layout contract
   `main` already carries, and was measured to do so. See
   `Surprises & discoveries` for the probe and its four candidate layouts.
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
   `src/ninja_gen/property_tests/ninja_oracle.rs`, and ordinary assertions are
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
    boundary. `make typecheck` is not optional even when the milestone adds no
    Rust: it is the only gate that runs `ty check`, so a Python change reviewed
    without it is reviewed with no type checking at all. `EP-M5`'s first local
    batch omitted it and hosted CI caught a `ty` error the batch could not
    have.

## Tolerances (exception triggers)

- **Scope.** Stop if the change touches more than 56 files beyond this
  ExecPlan, or if net added lines exceed 6,000. Both figures were **widened on
  2026-10-17 by user direction**, replacing limits of 34 files and 2,600 lines;
  the derivation is recorded in `Decision log`. The earlier numbers were set
  before the remaining artefacts were costed, and the line axis reached three
  lines of headroom with four milestones unwritten. The replacement is not a
  round number chosen for comfort: it is the measured 28 files and 2,597 lines
  already spent, plus an inventory of what `EP-M5` through `EP-M8` still name,
  plus a stated margin. The 4.2.3 precedent that motivated 2,600 overran by
  2.2x, so the new cap is still bound to a counted inventory rather than to
  aspiration.
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

- **The compile-admission pool can be unavailable for minutes at a time,
  blocking every gate that compiles.** *Severity: moderate (revised down from
  high). Likelihood: observed three times on this plan.* The pool has been
  unwedged by contention, by the supervisor reconciling a live sccache service
  against its drain check, and — at 20:28 today — by a self-completed
  stop/start that needed no operator. *Impact:* `typecheck`, `lint`, `test`,
  `doc-coverage`, `proptest`, and both mutation falsifications all queue behind
  it, so `EP-M4`'s acceptance, which is defined in terms of `make proptest` and
  the two patch runs, cannot be discharged while it is down. *Mitigation:*
  wait, and verify liveness rather than reading state words — compare the
  supervisor's PID and `ActiveEnterTimestamp` across probes instead of trusting
  `UNHEALTHY`, which reads identically while a drain is in progress and after
  one has failed. Do **not** escalate an operator action from
  `build-limits-status` output alone; today's near-miss would have had an
  operator stop a shared sccache during other agents' builds, for a fault that
  had already cleared. `RUSTC_WRAPPER`/`RUSTDOC` may not be unset and `rustc`
  may not be invoked directly, so there is no route around the pool and none
  should be sought. Keep the compiler-free gates (`check-fmt`, `markdownlint`,
  `nixie`) green so that when admission returns, only compiling gates remain;
  and record which files have never been compiled, so a later reader does not
  mistake "no gate failed on this file" for "this file is sound".
- **A file that no gate has ever compiled carries defects that no gate has
  seen, and the debt compounds while admission is down.** *Severity: high.
  Likelihood: certain for any file written during an outage.* Four files are in
  this state — `path_key.rs`, `no_loss.rs`, `support.rs`, and the `mod.rs` that
  declares them — and one of them (`path_key.rs`) is *known* to contain a
  compile error that was found by reading, not by a compiler. The error class
  it belonged to is invisible to every gate except a compile, and invisible to
  inspection at the speed a reviewer normally reads. *Mitigation:* treat the
  first compile as a gate in its own right, not a formality; do not record any
  claim about such a file, including "it parses", until `rustc` has seen it,
  and say plainly in the Progress entry that the run establishes nothing about
  the file. See `Surprises & discoveries` for the class of defect and why
  reading dependency source is what caught it.
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
- **Artefact:** `src/ninja_gen/determinism/property_tests/path_key.rs`.
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

**OBL-NOLOSS** — *every explicit output is accounted for, and no edge is
silently dropped.*

Statement: for every well-formed graph, every explicit output appears exactly
once on the left-hand side of a `build` line in the emitted text, and no edge's
outputs are absent from it.

A first version of this statement said instead that "the number of `build`
statements in the emitted text equals the number of distinct edges in the
graph". **That count conjunct is false, and it was corrected on 2026-10-15
before `EP-M4` implemented it.** The bundle path lowers a serial edge into
staged dyndep gates, and `render_serial_block`
(`src/ninja_gen/dyndep/mod.rs:247-290`) emits, for each of the edge's `N`
implicit dependencies, both a sidecar-producing phony line and a gate line:

```plaintext
build <sidecar>: phony [<previous gate>]     # dyndep/mod.rs:269 or :272
build <gate>: phony || <sidecar>             # dyndep/mod.rs:277
```

then once, after the loop, the real edge itself through `render_display_edge`
(`dyndep/mod.rs:212`). A staged edge therefore contributes **`1 + 2N`** `build`
statements, and the equality fails by `2N` on exactly the graphs `EP-M3`'s
non-vacuity assertion guarantees are generated: `reaches_class_intersection`
requires `dyndep_staged_edges > 0`, and that predicate is
`dependency_order == Serial && implicit_deps.len() > 1`
(`strategy/classification.rs:95-100`), which is byte-for-byte the same
conjunction as `edge_requires_gates` (`src/ninja_gen/mod.rs:237`) — so the
generator's *interesting* cases are precisely the ones the count breaks on.

Why the surviving conjunct is the stronger one. It carries the whole safety
argument — a real edge dropped between graph and text loses its output from
every left-hand side, which is the `MUT-EDGEDROP` failure mode — and it is
robust to the lowering precisely because `reject_reserved_paths`
(`dyndep/mod.rs:333`) guarantees no user path occupies `.netsuke/serial` or
`.netsuke/dyndep`, so no gate or sidecar can supply a user output's left-hand
side. It is also still order-independent, which is what the obligation needs.

A second, independent trap sits beside this one. Any `EP-M4` property that
emits through `ninja_gen::generate` will return `Err(DyndepFilesRequired)` on
every case the generator is built to produce, because `generate_into_with_shell`
(`src/ninja_gen/mod.rs:161-165`) rejects a graph satisfying
`graph_requires_dyndep`, which is `edges().any(edge_requires_gates)` — the same
predicate again. **The oracle must use `generate_bundle(..).build_file()`**,
which is also what production uses. Choosing `generate` would make the property
vacuous in a way that reports as a clean `Err`, not as a failure.

This replaces the first draft's `OBL-GUARD`, which asserted a *mechanism*
("validation runs before sorting") as a proxy for the failure that matters. The
failure that matters is a real edge being dropped between the graph and the
emitted text. Asserting the outcome survives refactors that legitimately move
the validator without weakening it.

The "silent drop of output-less edges", which the first draft recorded as a
surprise and this obligation was partly justified by, **does not survive a
re-read**. `insert_canonical_edge` pushes unconditionally and `render_edges`
renders every arena entry with no `seen` set, so an output-less edge is not
dropped: it emits a `build` line with an empty left-hand side. The reachability
question the first draft left open is **now closed by probe**, run against the
built binary over five `name:` shapes:

```plaintext
name key absent   -> Stage 4 (deserialize) fails; no manifest
name: null        -> Stages 4-5 pass; Stage 6 emits "build : <hash>..."
name: ~           -> as above
name: ""          -> as above
name: []          -> as above
```

Two corrections follow. First, the loader *does* accept an empty target name:
`to_paths` maps it to `[]`, `duplicate_output_error` finds nothing to reject in
an empty slice, and `insert_canonical_edge` pushes unconditionally. The absent
key is different — `Target.name` has no `#[serde(default)]`, so a missing field
is a deserialization error, not `Empty`. Second, the emitted artefact is worse
than "an empty left-hand side": real `ninja` rejects it outright
(`ninja: error: ...:4: expected path`). So the directed case is a
loader-reachable input, not a direct-constructor artefact, and the generated
domain must exclude empty target names so that "well-formed" keeps meaning what
`ADR-030` P-1 says. Recorded as `ADR-030` amendment 2.

- **Method:** Proptest over well-formed graphs, plus directed tests for the
  known bypasses.
- **Rationale:** this is the real content of `RM-4.3.1.c`'s safety argument, and
  the only obligation covering the `path_key`-collision failure mode as an
  observable.
- **Domain:** well-formed graphs of 5 to 100 edges and 5 to 50 actions — the
  bounds `MIN_EDGES`, `MAX_EDGES`, `MIN_ACTIONS`, and `MAX_ACTIONS` declare in
  `strategy/graph/mod.rs` — including multi-output edges. Two directed
  non-well-formed cases, which are the two the *code* distinguishes rather than
  the two the earlier wording guessed at. First, an edge with
  `explicit_outputs: []`: reachable through the loader (an empty or value-absent
  `name:` lowers to `StringOrList::Empty`, `to_paths` maps it to `[]`, and a
  duplicate scan over an empty slice rejects nothing) and accepted by
  `insert_edge`, so the emitter renders it. Second, two edges whose output
  lists are permutations of one another: the earlier wording said "which
  `from_manifest` rejects but a direct constructor permits", and that is
  **wrong** — two edges with permuted output lists have the same output *set*,
  so `insert_edge` refuses the second exactly as the loader does, and no direct
  constructor reaches the emitter with one.
- **Oracle:** the set of outputs computed from the spec, independently of the
  emitter, compared against the set of user paths appearing on left-hand sides
  in the emitted text. The entry point must be
  `generate_bundle(..).build_file()`, **not** `ninja_gen::generate`: the
  generated domain's intersection class necessarily satisfies
  `graph_requires_dyndep`, and the simple path rejects exactly those graphs with
  `Err(DyndepFilesRequired)` — so `generate` would turn every interesting case
  into a clean-looking error. Counting `build` statements as a whole is
  deliberately *not* the assertion; see the statement above for why the count
  exceeds the edge count on this path.
- **Artefact:** `src/ninja_gen/determinism/property_tests/no_loss.rs`.
- **Non-vacuity:**
  - *Covers.* Per-case assertion that at least one graph in the run had a
    multi-output edge. `EP-M4` has now established what each directed case
    actually does: the output-less edge is **accepted** and emitted with an
    empty left-hand side, and the permuted repeat is **rejected** at insertion.
    So one of the two is a reachable bypass and the other is a boundary that
    holds, and the suite asserts each answer rather than assuming either.
  - *Mutation.* `MUT-EDGEDROP` makes `render_edges` skip an arena entry whose
    `explicit_outputs` is **non-empty**, so a real edge vanishes from the text
    and its outputs disappear from every left-hand side. This is the mutation
    this obligation actually needs, and it replaces `MUT-GUARD`, which could
    not serve: `MUT-GUARD` moved `reject_unsupported_path_characters` after
    the sort, and re-reading `src/ninja_gen/path_syntax.rs:24` shows that
    function *returns* `Result<(), NinjaGenError>` and validates without
    dropping or merging anything, so reordering it changes *when* an error is
    raised, never *whether* an edge is emitted. The trigger must drop an edge
    that carries outputs; the 2026-10-15 entry below records why the
    output-less form it replaces could never fail this property.

______________________________________________________________________

**OBL-ORDER** — *emission does not depend on `HashMap` iteration order.*

This obligation is split into two properties because the naive single property
is not seed-reproducible.

*Core (seed-deterministic).* `EP-M2` extracts the two collection points as pure
helpers,
`ordered_edges<'a>(impl Iterator<Item = &'a BuildEdge>) -> Vec<&'a BuildEdge>`
and `ordered_actions`, changing no behaviour. The property feeds each helper a
`Vec` whose order is *constructed*, not sampled: the same elements in two
orders that are derived from one another and then **checked to be distinct**,
re-drawing on the rare coincidence rather than asserting it. `prop_shuffle`
alone is not sufficient here, because sampling a permutation of a short input
returns the input order often enough to matter, and a mandatory reorder
assertion would then reject a valid case; the constructed pair is what makes
perturbation genuine. Non-vacuity is structural rather than measured, shrinking
is exact, and seeds replay.

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
honest. The collector-level arm perturbs a `Vec` whose two orders are
constructed and verified distinct, so divergence is structural and is asserted.
The end-to-end arm records, per case, whether the two iteration orders actually
differed, and the end-to-end assertion is graded inconclusive — not passed —
when they did not, so a run in which no case diverged cannot be reported as
evidence for I-1. Drawing two permutations of a ≥ 5-key domain makes the orders
differ with probability ≥ 0.99 per draw. At 1–4 keys divergence is *reachable
but only a minority of the time* — measured at 23 of 200 two-key trials
exhausting eight attempts, 200 of 200 at one key — so the minimum of five is a
**sampling threshold**, chosen to keep the arm's informative share high and the
test budget bounded, not a statement that smaller domains make the arm
unreachable in principle. The generated domain's lower bound is raised from 1
to 5 actions and edges for that budget reason.

- **Method:** Proptest, metamorphic.
- **Rationale:** `RM-4.3.1.a`.
- **Domain:** `GraphSpec` values with 5 to 50 actions, 5 to 100 edges, and at
  most 200 explicit outputs in total. Both `DependencyOrder` variants; serial
  edges get 0 to 4 implicit dependencies so dyndep staging is genuinely
  exercised. The end-to-end arm requires at least `ORDER_MIN_DIVERGENCE` (5)
  distinct keys per collection before it treats a case as informative, and
  otherwise records the case without asserting on it.
- **Oracle:** the second emission — metamorphic, with no independent check on
  emitted *content*. The first draft described a *differential arm* conditioned
  on `GraphView::from_build_graph(g_u) == GraphView::from_build_graph(g_v)`.
  That arm does not exist as a separate thing, for two reasons. `GraphView` is
  a projection of `BuildGraph` and never reads the emitted text, so it cannot
  validate emission content; and because the end-to-end arm holds the **graph
  value fixed**, `GraphView` equality holds by construction on every case, so
  the antecedent is a tautology rather than a selection predicate. Metamorphic
  testing alone cannot catch two emissions being wrong in the same way, and
  this arm does not close that gap. Closing it needs an oracle that computes
  expected Ninja text independently — `OBL-NINJA`'s real-`ninja` arm is the one
  that does.
- **Artefact:** `src/ninja_gen/determinism/property_tests/order.rs`.
- **Non-vacuity:**
  - *Covers.* Per-case: the constructed reordering was distinct; and,
    separately, whether the two iteration orders differed. Recorded classes must
    include the *intersection*
    multi-output ∧ phony ∧ dyndep-staged ∧ non-empty defaults, not only the
    marginals, and all four combinations of `BuildEdge::always` and
    `Action::restat` — `DisplayEdge` emits `restat` only when
    `edge.always && !action_restat` (`src/ninja_gen/display_edge.rs:37`).
  - *Mutation.* `MUT-EDGESORT` deletes `edges.sort_by_key` in
    `ordered_edges`; `MUT-ACTIONSORT` deletes `actions.sort_by_key` in
    `ordered_actions` (`src/ninja_gen/order.rs:28` and `:44`). `EP-M2` made both
    helpers the single shared implementation, so each sort now exists once
    rather than once per emission path. Each must fail the core property, which
    pinpoints the helper rather than the whole emission.
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
`write_action_rules` (`src/ninja_gen/mod.rs:202`) streams `graph.actions` — an
ordinary-build `std::collections::HashMap` — into `ordered_actions`, which
collects it into a `Vec` and sorts by action ID (`src/ninja_gen/order.rs:44`).
That is the correct pattern: the collect-then-sort pair is exactly what
`MUT-ACTIONSORT` deletes to produce a real defect. A rule banning all map
iteration would reject this path, and a rule scanning for a literal `HashMap`
while ignoring the sort would miss the hazard it exists to catch.
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
- **Artefact:** `tests/ninja_gen_hashmap_boundary.rs`, with the syntax-tree
  reading in `tests/hashmap_boundary/scan.rs`.
- **Non-vacuity:** the test is validated by temporarily adding an iterating
  `HashMap` to a scratch file under `src/ninja_gen/` and observing the failure
  — measured at `tests/ninja_gen_hashmap_boundary.rs`: a planted
  `probe_scratch.rs` yields `Hit { kind: Import }` and `Hit { kind: Other }`,
  while the unmodified tree passes. The exemption list is asserted to be
  exactly the one known field (`staged_sidecars`,
  `src/ninja_gen/dyndep/mod.rs:241`, holding a `HashSet<Utf8PathBuf>`), so
  adding a second requires a deliberate edit. The exemption is asserted in both
  directions: a listed site that no longer holds a hash collection fails as
  loudly as an unlisted one. An earlier revision also held a `seen` set in the
  deleted per-output storage path; `2c030fd1` removed it, and the obligation's
  wording did not follow. See `ADR-030` amendment 3.
- **Test-only strip, in two places.** Rust expresses "not compiled into a
  release build" two ways and the contract must honour both, or it rejects the
  emitter's own suites. An *item* carrying `#[cfg(test)]` is stripped by the
  visitor; a *file* named by a `#[cfg(test)] mod` declaration is skipped whole
  by the walk. The second is forced by `src/ninja_gen/property_tests/mod.rs`,
  whose `use std::collections::{HashMap, HashSet}` carries no attribute of its
  own: its test-only-ness exists only in `#[cfg(test)] mod property_tests;` at
  `src/ninja_gen/mod.rs:251`. Measured, not inferred: before the walk honoured
  the declaration, the unmodified tree failed with those two imports reported.
  The whole-file strip therefore propagates down a subtree, and a directory
  whose children are declared by a sibling file rather than a `mod.rs` is read
  from that sibling. `property_tests/defaults/` was the live case for that
  path: its `generator.rs` was named by `property_tests/defaults.rs`. Clippy's
  `self_named_module_files` then required the directory and the file to share
  one form, so `defaults.rs` became `defaults/mod.rs` and the tree no longer
  exercises the sibling path. The helper is retained and documented because a
  future split may reintroduce it, and because the alternative — reading a
  directory's children from nothing — would silently exempt every file under it.

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
- **Artefact:** `src/ninja_gen/determinism/property_tests/defaults/mod.rs`,
  with the generator and its graph fixtures in the sibling
  `defaults/generator.rs` (the split keeps both files inside the 400-line
  `module_max_lines` cap, which counts raw source lines).
- **Non-vacuity:**
  - *Covers.* Empty (asserting no `default` line at all), singleton,
    already-sorted, reverse-sorted, and duplicate-bearing. The duplicate class
    pins the documented decision that `Vec::sort` does not deduplicate.
  - *Mutation.* `MUT-DEFSORT` deletes the `.sorted()` call inside
    `ordered_default_targets` (`src/ninja_gen/order.rs:55`); `MUT-DEFPOS` moves
    the `default` block before edge rendering and must fail both the positional
    half and `OBL-NINJA`.

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
- **Artefact:** `src/ninja_gen/determinism/property_tests/declaration.rs`.
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
  reach, so deleting `actions.sort_by_key` in `ordered_actions` would leave the
  two runs agreeing and the validation would pass without having tested
  anything. Do not satisfy this obligation with the snapshot fixtures alone.
- **Oracle:** byte equality between runs.
- **Artefact:** `tests/ninja_determinism_process_tests.rs`.
- **Non-vacuity:** the mutation is `MUT-ACTIONSORT`, **not** `MUT-DEFSORT`.
  This distinction is the whole point of the obligation. `MUT-DEFSORT` deletes
  the `.sorted()` call in `ordered_default_targets`, but for a *fixed* manifest
  the unsorted `default_targets` order is fixed too, so both process runs emit
  the same unsorted line and agree. A single-run snapshot with multi-default
  fixtures catches `MUT-DEFSORT`; a two-run comparison cannot, because the
  fault is deterministic across processes. What `OBL-PROCESS` exists to detect
  is a fault whose value differs *between* processes — the `HashMap`
  `RandomState` that is reseeded per process. `MUT-ACTIONSORT` deletes the sort
  that is the *only* guard over the *only* unordered collection the emitter
  iterates: `graph.actions` is an `IrHashMap`, so `write_action_rules`
  (`src/ninja_gen/mod.rs:202`) reads it at risk through `ordered_actions`, and
  determinism is re-established by the sort at `src/ninja_gen/order.rs:44`.
  Deleting that sort restores per-process iteration order, and a second process
  emits the action-rule blocks in a different order — a genuine cross-process
  divergence.

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
- **Artefact:** `src/ir/graph/property_tests/duplicates.rs`.
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
- **Artefact:** `src/ir/graph/property_tests/cycles.rs`.
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
- **Artefact:** `src/ninja_gen/determinism/property_tests/ninja_oracle.rs`. The
  existing `NinjaCommandOracle::ninja_commands` cannot be reused:
  `batch_ninja_files` discards the graph and scrapes only `command =` lines. A
  new harness is budgeted, which must create `.netsuke/dyndep/` and
  `.netsuke/serial/` before writing sidecars, materialize or exclude
  out-of-namespace inputs, and use a **fresh temporary directory per case** —
  the existing oracle reuses one directory, so content-addressed sidecars from
  earlier cases could satisfy a later case's reference and mask a
  missing-sidecar bug.
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
  (`src/ninja_gen/property_tests/mod.rs:79`), so results are comparable across
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
   `origin/main` has since absorbed `adr-020`…`adr-029`; the next free number
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

- Split `src/ninja_gen/mod.rs` after rebasing onto `origin/main`, which measures
  386 raw lines there — inside the 400-line cap, with no headroom for the
  additions later milestones make. The `NamedAction` struct and its impl block
  remain the obvious seam. The extracted module goes in
  `src/ninja_gen/named_action.rs` and is declared from `mod.rs` with a plain
  `mod named_action;`, **not** as a `src/ninja_gen_named_action.rs` sibling;
  the sibling form was measured to violate the module-layout contract `main`
  carries, and is the one shape to avoid.

  This base reconciliation was performed on 2026-10-10 rather than deferred to
  implementation time, because the sixth revision's premise no longer held.
  `origin/main` has migrated the whole `src/ninja_gen*` family into directory
  modules — `ninja_gen_command_list.rs` to `ninja_gen/command_list/mod.rs`,
  `ninja_gen_error.rs` to `ninja_gen/error.rs`, `ninja_gen_escape.rs` to
  `ninja_gen/escape.rs`, `ninja_gen_property_tests.rs` to
  `ninja_gen/property_tests/mod.rs`, `ninja_gen_recipe_shell.rs` to
  `ninja_gen/recipe_shell.rs`, `ninja_gen_test_support.rs` to
  `ninja_gen/test_support.rs`, `ninja_gen_validation.rs` to
  `ninja_gen/validation.rs`, `ninja_gen_tests.rs` to `ninja_gen/tests/mod.rs`,
  and `ninja_gen/dyndep.rs` to `ninja_gen/dyndep/mod.rs` — and it dropped the
  parent prefix from every child filename. Against this branch's pre-rebase
  tree the layout contract reports 66 unrecorded prefix groups and 27
  prefixed-beside-directory violations; against `origin/main` it reports 0 and
  0. Implementing the split before rebasing would therefore have built on a
  seam upstream has already removed. `docs/repository-layout.md` under
  *Placement conventions* now states the rule: a shared name prefix represents
  a module hierarchy, children go under a directory module with `mod.rs`, and
  `#[path]` is not to be used to reach a sibling or parent file.
- Split `tests/kani_mutation_evidence_tests.rs` (383 lines) and generalize
  `supplemental_property_location`, which currently hard-codes
  `ensure!(*root == "ir", …)` and derives `src/ir/<segments joined by _>.rs`.
  It must accept a `ninja_gen` root and directory-style module paths. Update
  the developers'-guide section describing it. This is the single largest piece
  of unbudgeted work the design review found.

  The generalization is **not** a third hard-coded branch. Be aware that the
  sixth revision's stated reason for preferring existential resolution was
  **falsified on 2026-10-10** and is withdrawn. It argued that
  `module_path_for_source` is lossy because `src/ir/cycle_verification.rs` and
  `src/ir/cycle/verification.rs` both map to `ir::cycle::verification`, and
  that the repository used both forms. `origin/main` has since migrated every
  flat module to the directory form — `cycle_verification.rs` is now
  `cycle/verification.rs`, and a sweep for flat-and-directory pairs that
  resolve to the same module path finds **zero** across `src/`, and zero on
  `origin/main`. The ambiguity that justified existential resolution therefore
  no longer exists, and a collision-witness argument cannot be carried forward
  as though it did.

  The generalization is still required, and for a simpler reason: `origin/main`
  derives `src/<root>/<segments>/mod.rs` but still hard-codes
  `ensure!(*root == "ir", …)`, so a `ninja_gen`-rooted patch is rejected
  outright. Remove that assertion and derive `<root>` from what the patches
  actually name rather than from a fixed list, or the same class of defect
  recurs at `EP-M4`. Resolving existentially — finding the source under
  `src/<root>/` that declares the named property — remains the preferred
  implementation even though the collision that motivated it is gone, because
  it fails with a message naming the searched candidates rather than an opaque
  read error when a module moves again, and this plan's own subject matter is
  about to add modules under `src/ninja_gen/`. Note also that
  `patch_stem_for_harness` and `module_path_for_source` already round-trip, so
  no Kani-side change is needed: only the supplemental path is.
- Extract `ordered_edges` and `ordered_actions` as pure helpers, used by both
  emission paths. Behaviour-preserving; enables `OBL-ORDER`'s
  seed-deterministic core.
- Add a `proptest` Make target. **The measurement says do not tier it**: the
  whole new suite costs about 6 s of CPU and 2–3 s wall, against a 45-second
  budget, so `PROPTEST_HEAVY`, `make proptest-heavy`, and a separate CI job are
  branches that would never be taken. A standalone CI job would additionally be
  ~98% compile — roughly 275 s of build to run 10 s of tests. `make proptest`
  is therefore a convenience selector over the same tests `make test` runs.

  Select the suites with a nextest filter that actually matches all of them, or
  add a nextest test-group in `.config/nextest.toml`. The enumeration was
  performed statically against `origin/main` on 2026-10-10, and it exposes a
  trap that a single substring cannot solve: **the repository uses two
  different suffixes**, `proptests` and `property_tests`, and `propert` is a
  prefix of only the second — `property` and `proptest` diverge at index four
  (`prop` then `e` against `prop` then `t`). Existing suites named `proptests`
  include `cli/discovery/helper_proptests.rs`, `cli/merge/logging_proptests.rs`,
  `cli/discovery/layers/property_tests.rs` (this one is `property_tests`), and
  `hex/property_tests.rs`; the planned `src/ir/action_hash_property_tests.rs`
  and `src/ir/graph/property_tests/` use `property_tests`. The selector must
  therefore carry **both** suffixes, each in its own anchored alternation. Note
  that carrying the right suffix in the *selector* is not sufficient: the new
  suites must also carry it in their own **module paths**, or the selector
  matches nothing while still exiting 0. That is the defect recorded in
  `Surprises & discoveries` for the `determinism/` tree.

  Use an anchored form, not a bare one. The first draft's
  `test(determinism_property_tests)` missed its own
  `action_hash_property_tests` and `graph_property_tests`; but the obvious
  repair, widening to a bare `test(property_tests)`, is the form
  `tests/workflow_contracts/nextest_child_cargo_group_invariants.py` explicitly
  rejects as unanchored and over-matching, and `test(=NAME)` is rejected as the
  exact form that silently selects no instance of a parameterized `#[rstest]`
  because such a test is listed as `NAME::case_1_…`.

  **Open question, to settle at implementation time before writing the
  selector: what a `test(...)` selector is anchored against, and therefore
  whether a module-path selector can work at all.** The accepted spellings in
  `nextest_child_cargo_group_invariants.py` are `test(/^NAME($|::)/)`, the
  transposed `test(/^NAME(::|$)/)`, and nextest's own `test(=^NAME(::|$)/)` —
  every one of them anchored with `^`. The suites this target must select are
  library-side, under `src/`, and nextest lists such a test under a name rooted
  at its module path, as in
  `ninja_gen::property_tests::command_lists_preserve_order_boundaries_and_fail_fast_joins`
  or `ir::cycle::property_tests::…`. A `^`-anchored selector naming a *module
  segment* rather than the leading segment — `^property_tests`, say — would
  match nothing at all, and nextest exits 0 while selecting nothing, so the
  target would look green while running no property test. That is exactly the
  silent-zero-match defect the anchored form exists to prevent, re-entering
  through the anchor. **This was partially resolved on 2026-10-10, and the
  resolution is narrower than it first appears** — the two halves of the
  runtime verifier must not be conflated (`_nextest_oracle/listing.py`):

  - `_check_every_filter_selects_something` **does** ask real nextest, for
    *every* `filter = '…'` line in `.config/nextest.toml`, via
    `selected(env, alternative)`, which reads each testcase's
    `filter-match.status` and reports a selector that matches nothing as a
    hard failure. It has **no `tests/`-root restriction** — it replays whatever
    the configuration declares, wherever the selected test lives. So a dead
    selector in the configuration is **caught**, and the earlier claim that
    "the runtime verifier does not currently catch it either" is **false as
    stated** and is hereby corrected. What is true is the narrower statement
    below.
  - `parameterized_tests()` scans only `TEST_SOURCE_ROOT =
    Path(__file__).resolve().parents[3] / "tests"`, and `main()` **fails**
    with *"no anchored filter names a parameterized test"* unless at least one
    configured anchored selector names a test found **under `tests/`**. The
    restriction is thus not "the verifier is blind to `src/`" but "the verifier
    refuses a configuration whose anchored selectors name no `tests/`-rooted
    parameterized test".

  **The consequence drawn here previously was wrong, and the error is worth
  recording because it is a quantification slip rather than a misreading.**
  `main()` builds `wanted` as
  `[(name, selector) for selector, name in
  anchored_selectors() if name in parameterized]` —
  a comprehension over the **union** of anchored selectors, not a per-selector
  admission test. The precondition is therefore a property of the *whole set*:
  it is satisfied when **any one** anchored selector names a `tests/`-rooted
  parameterized test. An earlier revision of this plan reasoned from "a
  selector naming only `src/`-side tests aborts the lane" to "adding
  `src/`-side selectors is forbidden", which does not follow: extending the set
  cannot remove the member that satisfies the precondition. That claim was
  committed in `3180ccd2` and is **retracted here**.

  The correct constraint is narrower and directional: `make proptest` **may**
  add `src/`-side anchored selectors to `.config/nextest.toml`, but the
  configuration must not be left with **no** `tests/`-rooted parameterized
  selector, and removing the last one aborts the coverage lane. The tree
  currently satisfies the precondition with exactly one member. Running the
  verifier's own extractors on this revision gives:

  ```plaintext
  parameterized tests under tests/ : 258
  anchored selector entries        :  18   (distinct names: 18)
  legacy test(=NAME) names         :   0
  wanted (anchored AND parameterized):  1
      text_domains_cannot_be_swapped
  legacy names that are parameterized (MUST be empty): []
  ```

  `wanted` is non-empty, so `main()` does not abort. The single qualifying
  member is `tests/ninja_semantics_ui_tests.rs:45`, declared under `tests/` and
  reached through one arm of the six-selector union at
  `.config/nextest.toml:66`. All 18 anchored selectors in the configuration
  currently name `tests/`-rooted tests, which is why the `src/`-side question
  is not exercised by the tree today: it is a design question for the new
  target, not an existing failure.

  Note also that the two nextest spellings `property_tests` and `proptests`
  both occur in this repository, and several of the target suites are
  `rstest`-parameterized, so the case-suffix hazard is live for them; it does
  not arise for plain `proptest!` blocks, but one spelling should cover both.

  **The anchor question is now settled by measurement, not by argument.** The
  open part was whether a `^`-anchored selector can name a `src/`-side test at
  all, given that such names might carry a crate segment before the module
  path. CI run `38055325773` at `20e5f480` carries the answer in two places.
  The runtime verifier's own output reads:

  ```plaintext
  replayed 18 filter alternative(s); each selects at least one test
  ok: test(/^text_domains_cannot_be_swapped($|::)/) matches
      ['text_domains_cannot_be_swapped::case_1_needle_as_document',
       'text_domains_cannot_be_swapped::case_2_document_as_needle'];
      test(=text_domains_cannot_be_swapped) matches nothing
  verified 1 anchored selector(s) over 1 test(s)
  ```

  and the same run's test log lists library-side instances by their **module
  path with no crate segment**: `ninja_gen::property_tests::dependency_only`,
  `cmd_interpolate::property_tests::scanner_agrees_with_independent_specification`,
  `cycle::property_tests::all_rotations_canonicalize_identically`, and 54 such
  names in all. A `^`-anchored selector therefore matches from the first module
  segment, and `test(/^ninja_gen::property_tests::…($|::)/)` is a workable
  spelling for the `src/`-side suites. The earlier worry that `^property_tests`
  "would match nothing" is also confirmed — it is a module *segment*, not the
  leading one — but that is a warning about writing the selector, not an
  obstacle to the target.

  The only genuinely open part is therefore *which mechanism* the `Makefile`
  `proptest` target uses. No contract reads a `Makefile` `-E` argument —
  `.github/scripts/` and `tests/workflow_contracts/` police `filter` lines in
  `.config/nextest.toml` only — so a selector written into the `Makefile` is
  unverified by construction, while one written into the configuration inherits
  the runtime check. The discharge procedure is unchanged: run
  `cargo nextest list --all-features --all-targets --message-format json` under
  the candidate selector and read each testcase's `filter-match.status`, which
  is how the verifier itself reads selection — nextest reports selection
  through that field, not by omitting unselected tests, and its exit code is 0
  even when a selector matches nothing. Record the outcome in `Decision log`.

  The `documentation-examples` recipe on `origin/main` uses the bare
  `test(NAME)` form for a different target. That is pre-existing, outside this
  plan's scope, and not a precedent to copy here.

*Acceptance:* `make test` passes with no file over 400 lines; the layout
contract reports zero violations on the rebased tree; `make proptest` runs and
its selector is **proved** to select the intended suites — evidenced by reading
`filter-match.status` from `cargo nextest list --message-format json`, not by
the target exiting 0, since a selector matching nothing still exits 0; a scratch
`ninja_gen`-rooted patch is accepted by the generalized contract test.
*Recovery:* the splits and the extraction are pure refactors, revertible
independently.

### EP-M3 — the shared bounded graph strategy

*Assigned:* the plan's central artefact, given its own milestone because
`EP-M4` onwards all depend on it.

Strategies live in `src/`, **not** `test_support`, because the latter does not
compile from `src/`-side tests (proven; see `EP-M0` question 7). They go in
`src/ninja_gen/determinism/strategy/`, wired `#[cfg(test)]`, and are reachable
by every library-side suite in this plan.

Absorb or explicitly diverge from `src/graph_view/tests/property.rs`'s
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
`every_explicit_output_appears_exactly_once`; both revert cleanly. The two
directed witness tests are present and phrased as disjunctions.

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
required to fail `OBL-PROCESS`: a fixed manifest's `ordered_default_targets`
sort deletion is deterministic across processes, so it cannot fail a two-run
comparison. The directed duplicate-rule test demonstrates the excluded class
genuinely diverges. *Conformance check:* if `EP-M0` question 2 answered "no",
`ADR-NNN` must already have been amended in `EP-M1`; this milestone does not
start otherwise.

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
- `src/ninja_gen/determinism/property_tests/mod.rs`, `path_key.rs`,
  `no_loss.rs`, `order.rs`, `defaults/mod.rs`, `defaults/generator.rs`,
  `declaration.rs`, `ninja_oracle.rs`, `strategy/mod.rs`, `strategy/graph.rs`
- `src/ir/action_hash_property_tests.rs`
- `src/ir/graph/property_tests/mod.rs`, `duplicates.rs`, `cycles.rs`

The intervening `property_tests` segment on the third line is **load-bearing,
not stylistic**: `PROPTEST_FILTER` (`Makefile` `PROPTEST_FILTER`) keys on a
module segment ending in `proptests` or `property_tests`, so without it
`make proptest` selects **none** of the planned `determinism` suites while
still exiting 0. The same segment is also what the module-layout contract
wants, so the two constraints agree on one shape. See the corresponding entry in
`Surprises & discoveries`.

Integration tests (no strategy dependency, so `tests/` placement is sound):

- `tests/ninja_determinism_process_tests.rs` (`OBL-PROCESS`)
- `tests/ninja_gen_hashmap_boundary.rs` (`OBL-NOHASH`)

New top-level `tests/*.rs` files need no registration; Cargo autodiscovery
handles them and `tests/integration_test_wiring_tests.rs` confirms it. Only a
new `tests/` subdirectory with a `mod.rs` needs an explicit `mod` declaration.

Mutation patches, named after the test they falsify per the house convention,
with `__` for the module separator:

| Handle           | Falsifies                  | Mutation                                                   |
| ---------------- | -------------------------- | ---------------------------------------------------------- |
| `MUT-PATHKEY`    | `OBL-PATHKEY`              | Delete `parts.sort_unstable()` in `path_key`.              |
| `MUT-EDGEDROP`   | `OBL-NOLOSS`               | Skip an arena entry whose `explicit_outputs` is non-empty. |
| `MUT-EDGESORT`   | `OBL-ORDER`                | Delete `edges.sort_by_key` in `ordered_edges`.             |
| `MUT-ACTIONSORT` | `OBL-ORDER`, `OBL-PROCESS` | Delete `actions.sort_by_key` in `ordered_actions`.         |
| `MUT-DEFSORT`    | `OBL-DEFAULT`, `OBL-E2E`   | Delete `.sorted()` in `ordered_default_targets`.           |
| `MUT-DEFPOS`     | `OBL-DEFAULT`, `OBL-NINJA` | Emit `default` before edge rendering.                      |
| `MUT-HASHMETA`   | `OBL-ACTION`               | Make the hasher skip `pool`.                               |
| `MUT-DUPWITHIN`  | `OBL-DUP`                  | Disable the within-one-target half of `find_duplicates`.   |
| `MUT-CYCLEDEPTH` | `OBL-CYCLE`                | Cap cycle traversal depth at 4.                            |

Nine, not the first draft's eleven. Two were cut from that draft: the nightly
`cargo-mutants` job (`.github/workflows/mutation-testing.yml`, 03:05 UTC over
`src/` with `--all-features`) already generates sort-deletion mutants
automatically, and the two cuts are *surviving* omissions rather than rows the
table keeps. The two sort rows above (`MUT-EDGESORT`, `MUT-ACTIONSORT`) are
**not** the cuts: they were restored when the compiler showed that a literal
sort deletion does not build, so the patch had to express a fault class that
`cargo-mutants` does not generate — the wrong sort key, and a collect left
unsorted. That is why the table, `EP-M5`'s acceptance, and the Scope budget all
count them. Handwritten patches earn their keep where `cargo-mutants` cannot
produce a *building* mutant of the same class: the reordering one,
`MUT-DEFPOS`, and the dropping one, `MUT-EDGEDROP`. `EP-M8` additionally
records a scoped
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
3. The same holds for each of the other eight patches. Each fails **every**
   property the mutation table assigns it — `MUT-ACTIONSORT`, `MUT-DEFSORT`, and
   `MUT-DEFPOS` are each assigned two — **plus** `every_patch_applies_cleanly`
   in the mutation-evidence contract test, because `git apply --check` of an
   already-applied patch fails. No *unrelated* test may fail. The first draft's
   criterion said "and no other test", which contradicted the table's own
   two-obligation rows and was therefore unachievable as written.
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

Where this document still writes `ADR-NNN` — in the trace links, and in the
obligation and milestone text that cites them — the placeholder is **retained
deliberately** as the stable trace identifier for `ADR-030`, not left
unresolved. The number it denotes is settled and recorded under *Signposts*
above; the placeholder survives only so that trace links written before
allocation keep resolving. Read every remaining `ADR-NNN` as `ADR-030`.

## Progress

- [x] (2026-10-18) **Hosted CI found a `ty` failure in the `EP-M5` review round
      that no local gate could have caught, because `make typecheck` is not in
      the local batch.** Run `39073698348` at `d4225faf` — the exact revision
      the local batch was about to bind to — failed `build-test` in 5 m 54 s on
      `typecheck-python`, with one diagnostic:
      `error[not-subscriptable]: Cannot subscript object of type 'object' with
      no '__getitem__' method` at
      `tests/workflow_contracts/nextest_child_cargo_timeout_test.py:102`,
      `profile = config["profile"]["default"]`.
      This is a genuine branch-introduced defect, not an environmental one: the
      test was added by this branch, `nextest_config()` returns
      `dict[str, object]`, and indexing an `object` with `["profile"]` is
      exactly what `ty` refuses. The other 19 check runs at that revision — the
      whole Windows lane, `kani-smoke`, `netsukefile`, and all five release
      build jobs — concluded **success**, so the failure is isolated to the one
      Python diagnostic. The repair keeps the test's semantics and replaces
      raw subscripting with the module's own narrowing helpers
      (`require_mapping`, `require_list`), which this file already imports and
      uses in its sibling test. It also narrows with `str(override.get(...))`
      for the filter key, so a missing filter fails the assertion rather than
      `ty`. Verified after the repair: `make typecheck-python` exits 0 with
      `All checks passed!`, the file passes `ruff format --check` and
      `ruff check`, and
      `pytest tests/workflow_contracts/nextest_child_cargo_timeout_test.py`
      reports 4 passed. **Process lesson:** the local gate set used for the
      `EP-M5` round omitted `make typecheck`, which is where this class of
      defect is caught deterministically. The re-dispatched batch includes it,
      and so must every later review round on this branch.

- [x] (2026-10-18) **The `EP-M5` re-review returned six findings: five were
      accepted and repaired, and one is rejected with clock evidence.** The
      pass ran `coderabbit review --agent --committed --base main --fresh`
      over `origin/main (077cf366)…HEAD` at `d4225faf`, matched the 43 files of
      that range, and completed with exit 0 in 855 s. It was the first
      successful milestone review after the seven service-side refusals noted
      below. Dispositions, each verified against live source:
      **(1) rejected, with clock evidence.** The entry dated `2026-10-18`
      immediately below this one was called future-dated, on the reading that
      the eighteenth had not happened. It had: the host runs CEST (+02:00), so
      `date` reported `Sun Oct 18 00:19 CEST 2026` while `date -u` still
      reported `Sat Oct 17 22:19 UTC`; the `crgate2` gate logs are timestamped
      `Oct 18 00:03` local. The reviewer reasoned in UTC. The date is correct
      and is left alone; changing it to the seventeenth would make the entry
      disagree with the logs it cites.
      **(2) accepted.** Six first-person phrasings survived the earlier
      pronoun sweep; all are now impersonal, and a second sweep found two
      more. This is the second round on the same rule, so the lesson is
      recorded in `Surprises & discoveries`: a sweep that searches only for
      the subject forms misses the possessive predicates that read naturally
      in first person.
      **(3) accepted, and found to be broader than reported.** The reviewer
      named `emit` duplicated between two suites and a third `materialize`
      copy in the graph-strategy suite. `materialize` had four copies, not
      three, and five suites built their own random-seeded `TestRunner`. All
      five now share one `support::coverage_runner`, and all four
      `materialize` copies are gone.
      **(4) accepted.** `add-020`…`add-029` is `adr-020`…`adr-029`.
      **(5) accepted.** The ADR's inline citations named files that had been
      moved into directories and the stale line numbers had rotted; every one
      now names a live path, and the ones most likely to drift cite a symbol
      instead — `PARENT_SCHEMA`, `process_rules`, `process_targets`,
      `process_defaults`, `reject_reserved_paths`, `get_target_display_name`,
      `StringOrList::map_each`, `NamedAction::write_into`. The paragraph
      describing `write_action_rules` was rewritten: it no longer collects and
      sorts, it delegates to `ordered_actions` in `src/ninja_gen/order.rs`,
      and that is the sort `MUT-ACTIONSORT` mutates. The two
      `README.md#L172`/`#L178` anchors were broken relative links as well, and
      now point at the file.
      **(6) accepted, and the repair is the one this plan's own precedent
      prescribes.** See the entry below on the pinned coverage seed.
      Three CodeScene findings arrived by a separate route and were repaired
      in the same round: two duplication findings and one complex-method
      finding. Each was fixed by extracting genuinely shared code or by
      splitting a predicate along its natural seam, not by suppressing the
      metric.

- [x] (2026-10-18) **Every coverage tally now runs under one pinned seed, and
      the seed is proved non-vacuous rather than assumed to be.** The reviewer
      put the chance of a spurious tally failure at about 1.81 % per run on
      one generator, and named only the default-shape tally. Grepping every
      `TestRunner::new` call found five tallies sharing the defect, not one:
      the default-shape, list-shape, graph-class, emission-order and
      no-loss suites. All five now call `support::coverage_runner`, which pins
      `RngSeed::Fixed(0x4e45_5453_4b45)`.
      The reasoning is not invented here; it is the rule already written at
      `src/stdlib/canonical_value/tests/properties.rs`, which keeps Proptest's
      random seed for its law suites and pins a fixed seed for its coverage
      tallies. Restated: a law *is* worth checking against inputs no earlier
      run has tried, but a coverage assertion is a claim about the generator,
      so a random seed makes it a lottery over whether the generator was
      observed to emit a class it does emit.
      A pinned seed is only an improvement if it still covers what the tally
      asserts, so the claim was measured rather than trusted. A temporary
      probe printed the default-shape tally at that seed:
      `DefaultShapes { empty: 6, singleton: 7, already_sorted: 2,
      reverse_sorted: 6, with_repeats: 104 }`.
      All five shapes are reached, and the narrowest margin — `already_sorted`
      at 2 of 128 — is the class the reviewer's 0.73 % figure applied to.
      Under a pinned seed that 2 is guaranteed rather than drawn.
      The probe was reverted immediately; only the pinned runner remains.
      Verified after the repair: `cargo check --workspace --all-targets
      --all-features` under `-D warnings` finished clean in 15 s, and
      `cargo nextest run --lib --all-features
      -E 'test(/determinism::property_tests/)'` reported 25 of 25 passed.

- [x] (2026-10-18) **All eight gates passed against `f0a2831f`, which is the
      review-response content plus one lint repair, and the earlier batch that
      went red on that repair is superseded.** The first batch against
      `fbeafee7` passed seven gates and failed `make lint` at the Whitaker leg:
      `module scan_coverage must start with an inner doc comment`. Whitaker
      wants a `//!` as the module's first item, and the outer `///` above the
      `mod` does not satisfy it. That rule had never fired before because every
      earlier lint run aborted earlier in the recipe — the `EP-M5` Clippy
      errors, then the `src/ninja_gen/order.rs` shadow-reuse errors — so the
      Whitaker leg had not reached this file since it was written. The module
      comment is now inner, `make lint-whitaker` was confirmed clean on its
      own, and `f0a2831f` carries the fix. The re-run, sequentially against
      that exact commit: `check-fmt` (2 s), `lint` (35 s, Whitaker over both
      `netsuke-build` and `test_support`, Pylint 10.00/10 twice, `interrogate`
      100%), `doc-coverage` (7 s, 98.89%, 5,079 of 5,136), `test` (97 s,
      nextest 4,110 of 4,110 with 6 skipped, doctests 162 passed and 32
      ignored across 29 targets), `markdownlint` (16 s, 186 files, 0 issues),
      `nixie` (all diagrams), `test-workflow-contracts` (94 s, 1,317 passed and
      3 skipped, plus 27) and `test-documentation-contracts` (14 passed, 42
      skipped). HEAD and `git status --porcelain` were identical before the
      first gate and after the last, so the batch binds to that commit. No
      `launch-failed`, `refused`, `UNHEALTHY` or non-benign `build-limits` line
      appeared in any log. Logs are
      `/tmp/<gate>-netsuke-4-3-1-epm5-crgate2-<gate>.out`.

- [x] (2026-10-17) **The `EP-M5` CodeRabbit pass returned ten findings and no
      critical or major one; seven distinct defects were accepted and fixed,
      and two re-phrasings were the same defect stated twice.** The pass ran
      `coderabbit review --agent --committed --fresh` over `origin/main
      (077cf366)…HEAD` at `ed0de1ac`, matching the 43 files of that range,
      and completed in 966 s with exit 0. Dispositions, each checked against
      live source rather than taken on trust:
      **(a) accepted** — `tests/hashmap_boundary/scan.rs` never visited
      expression paths, so `let x = HashMap::new()` was invisible; the finding
      is real and the suite could not have seen that shape. Fixing it exposed
      that the obvious repair was also wrong: an expression path ends in
      `new`, so the type path's last-segment rule records nothing. A
      separate `any_segment_unordered` matcher now reads value paths, and a
      scanner-level case fails before the fix and passes after. A first attempt
      at that case asserted a hit over an unqualified `HashMap::new()` and
      **failed**, which is what caught the second defect.
      **(b) accepted** — `tests/workflow_contracts/nextest_child_cargo_timeout_test.py`
      imported `nextest_config()` and then re-parsed the file with `tomllib`,
      so a malformed config raised a bare `KeyError`; the loader is now used
      and both now-unused imports are gone.
      **(c) accepted** — six of ADR-030's ten `../src/` links named paths that
      no longer exist (`src/ir/graph.rs`, `src/ir/from_manifest.rs`,
      `src/ninja_gen/dyndep.rs`, `src/graph_view/tests_property.rs`, and two
      others); all ten now resolve to live files, verified by script.
      **(d) accepted** — the plan carried first- and second-person pronouns at
      five places, against the style guide's rule at line 39 (the ADR cited in
      the plan already records the same finding); all are now impersonal.
      **(e) accepted** — three `PROPTEST_FILTER` citations gave `Makefile:264`
      where the variable is at `:273`, and two further cites (`:228`, `:364`)
      were stale as well. All are now cited by recipe or variable name, which
      cannot rot as the Makefile grows; the line numbers had rotted because the
      file grew after they were written.
      **(f) accepted** — one `-ise` spelling of "generalize" in the boundary
      crate's doc comment became the en-GB-oxendict `-ize` form the rest of the
      repository uses.
      **(g) rejected, with evidence** — the finding that the import exemption
      in `is_permitted` is file-wide rather than type-scoped claims a later
      `HashMap` import in `dyndep/mod.rs` would pass unnoticed. An import
      alone cannot be iterated: reading it is the whole of what it does. Every
      actual use of such an import lands as a `Field` or `Other` hit, and both
      of those must name the permitted field or fail. The file carries exactly
      one unordered import and one unordered field, both the permitted pair.
      Binding the exemption to the type name would add a guard against a
      second import whose use could not escape the existing checks anyway.

- [x] (2026-10-17) **All eight gates passed against freeze `cf263af7`, and the
      milestone was committed as `9e426d12` at that exact content.** This is
      the run the previous entry said was still owed: `markdownlint` had been
      unverified because its `spelling` prerequisite had short-circuited it.
      Run sequentially in one batch: `check-fmt` (2 s), `lint` (463 s, through
      Clippy, Whitaker, Ruff, Pylint, the df12 lints, `ambrleaks`, `yamllint`
      and `actionlint`), `doc-coverage` (59 s, 98.89%, 5,079 of 5,136),
      `test` (189 s, nextest 4,108 of 4,108 passed with 6 skipped, doctests 39
      passed and 6 ignored), `markdownlint` (17 s, 186 files, 0 issues,
      `typos.toml` unchanged), `nixie` (4 s, 187 diagrams),
      `test-workflow-contracts` (98 s, 1,317 passed and 3 skipped, plus 27
      script doctests) and `test-documentation-contracts` (4 s). Every
      per-gate exit-status file holds `0`. The candidate was stable at both
      ends — freeze `cf263af7…`, HEAD `98dd3a35`, 23 entries — and that same
      freeze was recomputed before staging, so the committed content is the
      gated content rather than a later edit of it. Logs are
      `/tmp/<gate>-netsuke-4-3-1-epm5-final8-<gate>.out`, one pair per gate.
      The `EP-M5` milestone's deterministic gate phase is therefore complete;
      what remains for it is the CodeRabbit CLI review.

- [x] (2026-10-17) **Three of the `EP-M5` trace links named tests that do not
      exist, and were corrected to the tests that do.** `emission_is_insertion_order_invariant`,
      `default_line_is_the_ascending_sort` and `no_hashmap_iteration_in_ninja_gen`
      were written into the trace-link block when the milestone was planned and
      were never reconciled with the identifiers the suite landed under. The
      `EP-M8` acceptance line requires every trace link to resolve to a passing
      test, so a link naming nothing is a contract that has quietly stopped
      being one — the same failure the `EP-M4` pass caught in
      `path_key_is_injective_on_validated_paths`. They now name
      `one_graph_emits_one_bundle_under_two_insertion_orders`,
      `the_default_line_sorts_its_operands`, and two boundary tests
      (`only_membership_use_of_unordered_collections_reaches_the_emitter` and
      `the_matched_type_list_names_only_unordered_collections`), and three
      further `EP-M5` properties were linked so the milestone's whole set is
      traceable rather than only its headline case. Verified by running the
      suites under the gate's own filter: 93 property tests passed, 3,900
      skipped, and the boundary crate passed 2 of 2. The links for `EP-M6`,
      `EP-M7` and `EP-M8` still name unwritten tests; that is correct for
      milestones that have not started, and they are checked at their own
      acceptance rather than here.

- [x] (2026-10-17) **Seven of eight gates passed against freeze `168a932f`; the
      eighth was red on one misspelt word, and the word was in this branch.**
      The run took 71 minutes of wall clock with the compile pool saturated and
      the managed sccache server unreachable throughout, so every build
      compiled uncached — a performance note, not a failure. `check-fmt` (3 s),
      `lint` (764 s, through Clippy, Whitaker, Ruff, Pylint, the df12 lints,
      `ambrleaks`, `yamllint` and `actionlint`), `doc-coverage` (195 s, 98.89%
      against the 80% bar), `test` (2,985 s, nextest 4,108 of 4,108 passed with
      6 skipped, then every doctest target), `nixie` (1 s),
      `test-workflow-contracts` (128 s, 1,317 passed and 3 skipped, plus 27
      script doctests) and `test-documentation-contracts` (182 s) all passed.
      `markdownlint` failed at 18 s **in its `spelling` prerequisite**: a
      branch-introduced bare-prose `recognise` at
      `docs/execplans/4-3-1-proptests-for-deterministic-ninja-emission.md`.
      That is the en-GB-oxendict `-ize` rule and not a false positive — the
      word was neither quoted nor backticked, so no exception applies — and it
      was corrected to `recognize`. Because `markdownlint` is
      `markdownlint: spelling`, `markdownlint-cli2` never ran, so the Markdown
      lint proper is **unverified rather than passed** and is re-run below.
      The candidate identity was stable at both ends of the run: HEAD
      `98dd3a35`, freeze `168a932f…`, 23 entries. Logs are
      `/tmp/<gate>-netsuke-4-3-1-epm5-*.out`, one pair per gate.

- [x] (2026-10-17) **A gate run went red on `self_named_module_files`, which
      is the rule that made `defaults.rs` become `defaults/mod.rs`.** The
      second dispatch reached `lint-clippy` and failed there with one error:
      Clippy requires a module file to be named `mod.rs` when a directory of
      the same name exists, and `defaults/` had been created for
      `generator.rs` beside a `defaults.rs`. The file was moved, not renamed
      around: `src/ninja_gen/determinism/property_tests/defaults.rs` became
      `.../defaults/mod.rs` and its `mod generator;` declaration kept working
      unchanged, because it already named a sibling of the file. Neither the
      module path in `mod.rs` nor any mutation patch needed editing — the four
      patches for this milestone target `src/ninja_gen/order.rs`,
      `src/ninja_gen/mod.rs` and `src/ninja_gen/dyndep/mod.rs`, not the test
      file, and all 24 patches still pass `git apply --check`. The prose that
      described the old layout was retargeted in the plan and in the boundary
      test's doc comment, and the boundary test still reports `2 tests run: 2
      passed`. A focused Clippy over `netsuke-build` with `--tests` finished
      clean afterwards. Log: `/tmp/lint-netsuke-4-3-1-epm5-final4-lint.out`.

- [x] (2026-10-17) **A third gate run went red on `lint-clippy`, and the
      sixteen errors were all in the two `OBL-NOHASH` test files — the first
      time the compiler had ever seen them.** `check-fmt` passed. `make lint`
      then failed at `make lint-clippy` with exit 101 and sixteen Clippy
      errors, every one in `tests/ninja_gen_hashmap_boundary.rs` or its
      `#[path]`-included `tests/hashmap_boundary/scan.rs`: one
      `missing_const_for_fn`, ten `shadow_reuse` on `item` in the
      `is_test_only` match arms, one `shadow_unrelated` on a `permitted_source`
      closure binding, one `too_many_arguments` (5/4) on
      `collect_entry_sources`, one `needless_borrows_for_generic_args`, one
      `option_if_let_else`, and one `shadow_reuse` on `text`. The reason this
      is the *first* run to find them is worth recording: every earlier lint
      run aborted at the `src/ninja_gen/order.rs` `shadow-reuse` errors before
      the crate's test targets were reached, so these files were authored,
      formatted, and mutation-validated without ever being linted.
      The repairs kept the design and changed only its expression:
      `Kind::field` became `const fn`; the ten match arms bind `named` instead
      of shadowing the function's `item` parameter; the closure binds `owner`
      rather than re-using `permitted_source`; the two per-directory answers
      `collect_entry_sources` needs (the test-only flag and the declaring file)
      were grouped into `DirectoryContext`, which brings the signature from
      five parameters to four and reads better than the positional pair did;
      the sibling read drops its `&`; and `Declarations::read` became
      `map_or_else` plus `Option::transpose`, removing both the `if let`/`else`
      and the `text` shadow. `RUSTFLAGS="… -D warnings -Zthreads=8
      -Clink-arg=-fuse-ld=mold" cargo clippy -p netsuke-build --test
      ninja_gen_hashmap_boundary --all-features -- -D warnings` then finished
      clean. Logs: `/tmp/lint-netsuke-4-3-1-epm5-gate3-lint.out` (FAIL) and
      `/tmp/clippy-boundary-repair.out` (PASS).

- [x] (2026-10-17) **The gate run in flight when those repairs landed was
      voided rather than resumed, because a batch whose gates straddle an edit
      binds no revision.** Its `check-fmt` and `lint` ran against the pre-repair
      tree while its `doc-coverage`, already started, read the repaired one; no
      verdict from that batch describes a single candidate. It was stopped and
      discarded, and the lesson is the one
      `netsuke-authoring-under-a-live-gate-batch-voids-it` records: a batch is
      a claim about a frozen tree, so an authoring edit inside one invalidates
      the whole batch, not only the gates that had not yet started. The
      candidate was re-frozen after `make fmt` at diff hash
      `46408dba20deb6b49e25e4032c6f0537876dfabed9fdb66f8475192d029cfdf7`
      (HEAD `98dd3a35`, 23 changed or untracked entries), and a fresh full set
      is what the next entry describes.

- [x] (2026-10-17) **`EP-M5`'s four mutation patches were validated by
      execution, and each fails the property it is assigned.** The tree was
      clean and `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 make proptest` reported
      `115 tests run: 115 passed, 3999 skipped` before and after every run, so
      each failure below is attributable to its patch rather than to the tree.
      Each patched tree first passed `cargo check --lib --all-features` under
      `RUSTFLAGS="-D warnings -Zthreads=8 -Clink-arg=-fuse-ld=mold"`, which is
      what makes the evidence live rather than dead — a patch that cannot
      compile fails nothing. Logs are `/tmp/proptest-netsuke-4-3-1-epm5-*.out`.
      - `MUT-EDGESORT` (`path_key(&edge.explicit_outputs)` becomes
        `path_key(&edge.implicit_outputs)`) failed three properties:
        `ordered_edges_ignore_input_order` and
        `one_graph_emits_one_bundle_under_two_insertion_orders`, plus the
        directed `the_edge_rule_keys_on_the_output_set` with
        `left: ["out/a\0out/z", "out/a\0out/m"]` against
        `right: ["out/a\0out/m", "out/a\0out/z"]`.
      - `MUT-ACTIONSORT` (the collect-then-sort pair reduced to a bare
        `collect`) failed `ordered_actions_ignore_input_order` with a 16-action
        permutation `["act-0003", "act-0012", …]` against
        `["act-0009", "act-0002", …]`, and
        `one_graph_emits_one_bundle_under_two_insertion_orders` with the two
        rule blocks in different orders.
      - `MUT-DEFSORT` (`.sorted()` and its now-unused `itertools` import
        deleted) failed all four default properties, including the assigned
        `the_default_line_sorts_its_operands`, with the minimal counter-example
        `left: ["out/0001", "out/0000"]` against
        `right: ["out/0000", "out/0001"]`.
      - `MUT-DEFPOS` (in both emission paths) failed
        `the_default_line_follows_every_build_line` because the `default` line
        landed at byte 30 while the last `build` line began at byte 56.
      Two findings were necessary to get here and are recorded in
      `Surprises & discoveries`: the literal mutations the table describes do
      not compile under `-D warnings` and were re-expressed so they stay live;
      and a mutation run persists proptest regression seeds unless
      `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1` is set.
      The `OBL-NOHASH` half was validated the same way: a planted
      `src/ninja_gen/probe_scratch.rs` holding a `HashMap` import made
      `only_membership_use_of_unordered_collections_reaches_the_emitter` fail
      and name the file, and deleting the probe restored `2 tests run: 2
      passed`.

- [x] (2026-10-17) **The first gate run found two defects EP-M5 introduced,
      both of which had to be fixed before the milestone could be called
      done.** Neither was in the properties; both were in the machinery around
      them.
      - Ten `rustfmt` diffs across the four new `EP-M5` files. `make fmt`
        applied them and `cargo fmt --all -- --check` then exited 0. The
        candidate had been authored and run without the formatter ever seeing
        it — the mutation runs invoke `make proptest`, which does not
        format-check.
      - `make lint` failed `lint-clippy` with two `clippy::shadow-reuse`
        errors at `src/ninja_gen/order.rs:27` and `:43`. Widening both helpers
        to take `impl Iterator` made the collected `Vec` shadow its own
        parameter, which the crate denies. The bindings were renamed to
        `collected` in both functions, which keeps the borrow-centric shape
        and the documented rationale intact.
      The gate run stopped at the first failure, so `doc-coverage`, `test`,
      `markdownlint`, `nixie`, `test-workflow-contracts` and
      `test-documentation-contracts` have **not** run against the repaired
      candidate. Logs: `/tmp/check-fmt-netsuke-4-3-1-epm5-gate2-check-fmt.out`
      (PASS) and `/tmp/lint-netsuke-4-3-1-epm5-gate2-lint.out` (FAIL on
      clippy).

- [x] (2026-10-17) **The rename invalidated two mutation patches, and
  re-running the four runs showed the evidence binds to a revision rather than
  to the milestone.** `MUT-EDGESORT` and `MUT-ACTIONSORT` both anchor on lines
  containing `edges`/`actions`, so renaming the collected bindings to
  `collected` broke their context and `git apply --check` rejected them.
  `MUT-DEFSORT` and `MUT-DEFPOS` were unaffected and still applied. Both
  patches were regenerated from the renamed source by the same script that
  produced them, and all four applied. **The first four runs are therefore
  evidence about the pre-rename tree, not about the candidate**, which is
  exactly the staleness the mutation-evidence contract exists to catch — had
  the patches not been re-checked, `every_patch_applies_cleanly` would have
  gone red at `make test`, but only after the defect had already been reported
  as passed. Both regenerated patches were re-run against the renamed tree and
  reproduced their falsifications exactly: `MUT-EDGESORT` again failed
  `the_edge_rule_keys_on_the_output_set`, `ordered_edges_ignore_input_order` and
  `one_graph_emits_one_bundle_under_two_insertion_orders` (3 failed, 72
  passed), and `MUT-ACTIONSORT` again failed
  `ordered_actions_ignore_input_order` and
  `one_graph_emits_one_bundle_under_two_insertion_orders` (2 failed, 88
  passed). Logs: `/tmp/proptest-netsuke-4-3-1-epm5-edgesort2.out` and
  `-actionsort2.out`. The lesson is mechanical and worth stating: a mutation
  patch is bound to the text it patches, so any edit to the production lines it
  quotes — including a rename made to satisfy an unrelated lint — re-opens the
  evidence.

- [x] (2026-10-17) **The `cli_configuration_fixture_compiles` timeout is
      addressed by widening the ambient fixture family's allowance to 900 s,
      after the rebase failed to remove it.** The rebase onto `origin/main`
      (`077cf366`) was clean — zero conflicts, one commit dropped as already
      upstream — and the test still reported `TIMEOUT [300.024s]`. The child
      `cargo check --workspace --all-targets --all-features` measured **455 s
      cold and 1 s warm**, with 36 compile-admission queue-wait lines in the
      cold run. The allowance moved on the family override in
      `.config/nextest.toml`, because all nine ambient fixtures share that
      child build and the `nested-cargo-builds` group serializes them, so the
      first one scheduled pays the cost. The shape of the tree that forces the
      cold run — `target/debug/.fingerprint` and `deps/` empty while 2,520
      `.rmeta` files survive, and neither `cargo nextest run` nor
      `make typecheck` warming that selection — is recorded in
      `Surprises & discoveries`. The change is tracked by the updated contracts
      (94 passed), and the widened assertion was proved live by removing it and
      watching the contract fail. The full gate set ran through `scrutineer`
      against the frozen candidate (HEAD `98dd3a35`, diff hash
      `530a9d9b…`): `check-fmt`, `markdownlint` and `nixie` passed, and
      **`lint` failed on a defect this change introduced**, not on the code
      under test. The added block pushed
      `test_execution_coverage_test.py` from 397 to 420 lines against pylint's
      `C0302` 400-line cap. The test was moved, not trimmed, to
      `nextest_child_cargo_timeout_test.py` — the module whose stated remit is
      "what may a cold-build child Cargo test take?" — which put the source
      module at 353 lines and the destination at 149. The assertion was proved
      live again from its new home (`assert 5 == 6` with the widening removed,
      green with it restored), and the prose naming the old location was
      retargeted. The next action is a fresh full set through `scrutineer`
      against the amended candidate, then the milestone CodeRabbit pass.
- [ ] (2026-10-16T00:00:00Z) `make typecheck` re-run on the recovered pool, and
      **failed a second time for a different environmental reason**, having
      again never reached the `netsuke` crate. Recorded because the two
      failures have distinct causes and only one of them is the outage already
      described, and
      because a reader who sees "typecheck failed twice" will otherwise assume
      the same cause twice.
      **This time the refusal is `launch-failed`, not `unhealthy`.** The log
      carries exactly one `build-limits: the supervisor refused: launch-failed`
      and kills exactly one crate, `termtree`. The pool was **healthy** for the
      whole run — `capacity 4, held 3, healthy` before, during, and after — and
      the supervisor never restarted (its PID stayed 1878261, and the journal
      records one `supervisor_started` for the day, the 20:28 recovery).
      **The diagnosis is cgroup pids exhaustion at spawn.** The evidence is
      the shape the memory `build-limits-launch-failed-spawn-exhaustion`
      records: **zero rustc diagnostics anywhere in the log** —
      `error[E…]` 0, `cannot find` 0, `failed to compile` 0 — while the
      supervisor reports `events: max 18516` against `pids.max: 16384` on the
      user slice (that ceiling has since been raised to 338286, so this
      refusal mode is currently unreachable). The supervisor could not
      *spawn* the child, so the child
      exited 1 having diagnosed nothing, and cargo's summary line
      (`could not compile termtree`) reads like a source defect. It is not. The
      compile queue backs this up: 125 requests waiting at the point of death,
      with load average 33.70 (15 min) on six cores, and another agent's
      `cargo-mutants` run inside Podman.
      **The `netsuke` crate was never reached, again.** No `--crate-name netsuke`
      anywhere in 733 lines. Two full gate runs have now compiled ~90 minutes
      of dependencies between them without once reaching `src/`. The four new
      files still have never been seen by a compiler, so the `path_key.rs`
      error found by reading remains unconfirmed in both directions — the
      compiler has neither confirmed nor refuted anything about it.
      **A third, unrelated anomaly worth recording.** `target/debug` contains
      `build/` (336 entries, all stamped 19:55) and `incremental/`, but **no
      `deps/` and no `.fingerprint/`**. `rust-cleanup.service` swept at
      20:26:42 and deleted exactly 1 of 45 target directories
      (`eligible=1 deleted=1`, in `normal` mode at 78.44% usage, so the 2-hour
      idle rule and not disk pressure). Whether that deletion was this
      worktree's `target/` is not established — the log does not name the
      path, and `target/CACHEDIR.TAG` survives — but the missing `deps/` and
      `.fingerprint/` alongside 336 surviving `build/` entries is the
      signature of a partial removal rather than a clean one, and it would
      explain why both runs recompiled dependencies the previous run had
      already finished. Recorded as an observation with its evidence and its
      uncertainty, not as a conclusion.
      **What is and is not established.** Neither run says anything about the
      correctness of this change. What they do establish is that this machine
      cannot currently complete a cold Rust build of this workspace: two
      attempts, two kills, zero lines of this branch's source compiled. The next
      attempt should wait for the queue to drain rather than adding a third
      build to a
      pool already 125 deep, which is what the `launch-failed` memory warns
      against explicitly: *"Do not 'help' by building."*
- [ ] (2026-10-16T00:00:00Z) `make typecheck` ran on the candidate and **failed
      for an infrastructure reason, not for this change**. Recorded because the
      first run of the gate that touches the four new files is evidence
      whatever its verdict, and because the tempting reading of its output is
      the wrong one.
      **The failure is not in this branch.** Every one of the 18 `error: could not
      compile` lines names a *third-party* crate — `dirs-sys`, `unic-langid`
      (twice), `pear`, `self_cell`, `gherkin`'s build script, `anstyle-parse`,
      `fluent-syntax`, `tempfile`, `rustls-pki-types`, `time-core`,
      `icu_locale_core`, `cap-primitives`, `walkdir`, `object`, `parking_lot`,
      `icu_segmenter`, and `toml` — 17 distinct crates, each with the
      `Caused by: process didn't exit successfully: …/build-limits-rustc
      …rustc … -D warnings -Zthreads=8 -Clink-arg=-fuse-ld=mold` that pins the
      cause to the wrapper. The wrapper's own message appears 18 times:
      `build-limits: the supervisor refused: unhealthy`.
      **The `netsuke` crate was never reached.** No line carries `--crate-name netsuke`,
      and every occurrence of the string `netsuke` in the log is inside this
      worktree's path. 161 crates compiled or checked before the kill. So this
      run establishes *nothing* about the four new files — it does not even
      show they parse, and the `path_key.rs` compile error found by reading
      (see `Surprises & discoveries`) is still unconfirmed by any compiler.
      **The supervisor was caught mid-recovery, and the first reading of it
      was wrong.** Probes at 20:26, 20:27, and 20:28 showed a
      `deactivating`/`stop-sigterm` state, `build-limits-status` reading
      `capacity 4, held 3, UNHEALTHY`, and status text naming operator action:
      *"stop sccache and leave it stopped until admission confirms the empty
      drain"*. That was read as a permanent wedge, on the strength of the unit
      setting `Restart=always` *and* `RestartPreventExitStatus=2`, which makes
      exit status 2 suppress the restart. **All three probes were taken after
      the recovery had already started and before it had finished.** The
      supervisor's journal records the completion at 20:28:14 —
      `sccache service verified stopped and empty; drain tokens may return` —
      followed by a clean `Stopped`/`Starting`/`Started` cycle and a **new
      PID** (1878261, replacing 1127739). By 20:31 the pool read `capacity 4,
      held 2, healthy` with a real build waiting on a token, and a restart of
      `make typecheck` compiled without a single refusal.
      **The trap is that a transient state and a terminal state are
      indistinguishable from `build-limits-status` alone, and the status text
      does not say which it is.** The disambiguating evidence is in the journal
      (the `drain check failed` line is printed while the drain is still
      running, not only after it gives up) and in the PID changing. Two things
      would have been enough: waiting for at least one full probe cycle before
      concluding, and checking whether `NRestarts=0` alongside a *changing* PID
      means "restarted internally" rather than "refused to restart". The
      earlier reading also leaned on `RestartPreventExitStatus=2` from the
      *previous* outage's exit-status-2 state — a detail that does not carry
      over to a `stop-sigterm` deactivation, and it was applied anyway.
      **`sccache unreachable, compiling directly` is confirmed benign.** The
      status line moved to it across those same three probes, and it persisted
      into the healthy state. It is the degraded mode this plan has seen
      before, and it costs cache hits, not correctness. It is *not* evidence of
      the wedge: the same clause appears on both sides of the recovery.
      **What this means for the milestone.** Nothing is blocked any longer.
      `make check-fmt` was re-run green after the plan edits, and `make
      typecheck` was relaunched against the recovered pool. The four new files
      now reach a compiler for the first time. The `EP-M4` acceptance line is
      still open, because the run has not finished and `make proptest` and the
      two mutation falsifications have not been attempted.
      **The escalation written here previously is withdrawn.** It was addressed
      to a condition that had already resolved itself, and acting on it — an
      operator stopping sccache — would have been an unnecessary intervention
      on a shared service during other agents' active builds.
- [x] (2026-10-17T12:59:22+02:00) **`EP-M4`'s acceptance is discharged in full,
      by execution.** `make proptest` at `1b4b3fb1` reports 104 tests run, 104
      passed, 3834 skipped, exit 0 (log
      `/tmp/proptest-netsuke-4-3-1-1b4b3fb1.out`). All fourteen determinism
      tests ran and passed, including both property tests and both directed
      witnesses.
      **This is the first time these four files compiled.** Until this run the
      only evidence about them was a reference-by-reference reading, which
      leaves type inference, trait resolution, and borrow checking unverified.
      They now compile clean under `-D warnings` for a target that sets
      `cfg(test)`, which is the only kind of target that can reach them.
      **`MUT-PATHKEY` fails its named property.** Applying the patch and running
      the `path_key` suite yields 3 passed, 3 failed, exit 100. The named
      property fails with *"a permutation of a path list must key identically"*,
      and the directed witness fails on exactly the predicted input —
      `["out/a", "out/a", "out/b"]` against `["out/b", "out/a", "out/a"]`.
      **Detection is stronger than the plan predicted, and the prediction was
      deliberately pessimistic.** The entry recorded at `8a213218` judged the
      sampled property probabilistic, because `prop_shuffle` may return input
      order and `ListShapes` asserts 5 of its 7 fields. Sampling found it
      anyway, and so did `path_key_is_injective_on_keyable_paths` — an
      unplanned third detector, since removing the sort makes two distinct
      lists collide. The pessimistic reading was still the right thing to
      write: it would have been correct had only the directed test fired, and
      recording it beforehand is what makes the stronger result attributable to
      the run rather than to hindsight.
      **`MUT-EDGEDROP` fails its named property.** 3 passed, 1 failed, exit 100,
      and the failing one is `every_explicit_output_appears_exactly_once` with
      *"the graph declares 37 outputs and the text carries 0 left-hand-side
      entries"* — the mechanism traced statically at `e3056919`, observed. The
      other three `no_loss` tests pass under the mutant, so the failure is
      specific rather than a suite-wide collapse.
      **Both revert cleanly.** `git apply -R` restores a clean tree
      (`git status --porcelain` empty) and the determinism suite re-runs 14/14
      green at `1b4b3fb1`.
      **A mutation-derived regression artefact was deleted, not committed.**
      The failing runs wrote `proptest-regressions/ninja_gen/determinism/
      property_tests/{no_loss,path_key}.txt`. The three tracked seeds are
      mutation-derived too and are committed by convention, but both of these
      are redundant: each mutation is detected with certainty by means that
      survive the revert, so replaying the seed would only re-record a
      fabricated failure. The plan's `EP-M3` precedent deleted an artefact of
      exactly this kind for the same reason. Recording the distinction, since
      the repo commits the older ones: a seed earns its place when it is the
      *only* thing that catches the defect.
      **Roadmap `RM-4.3.1.c` is satisfied; the parent item is not.** The
      `path_key` invariance child of roadmap `4.3.1` can be ticked. The parent
      cannot: its other two children — insertion-order stability and `default`
      ordering — are `EP-M5`'s, assigned `RM-4.3.1.a` and `RM-4.3.1.b`.
- [x] (2026-10-16T00:00:00Z) `EP-M4` source landed: `path_key.rs`, `no_loss.rs`,
      and `support.rs` written, `mod.rs` declaring all four children, and both
      mutation patches authored and registered.
      **Domain repairs.** Two `Domain:` blocks quoted bounds the generator
      does not use: `OBL-NOLOSS` said "1 to 30 edges" — a number from the first
      draft's sketch, contradicted by `MIN_EDGES`/`MAX_EDGES` and by the class
      intersection `EP-M3` guarantees — and both blocks said "1 to 50 actions"
      where the floor is `MIN_ACTIONS = 5`. Both now quote the constants. The
      `MIN_*` floors are *sampling* thresholds, not correctness bounds: an
      order-set with fewer than five keys has at most one alternative order, so
      a perturbation property stated over it would be vacuous. Recorded in the
      domain text rather than left to be rediscovered.
      **A third Domain defect, in the directed cases.** The plan named "two
      edges whose output lists are permutations of one another (which
      `from_manifest` rejects but a direct constructor permits)" as the second
      non-well-formed case. Re-read against `BuildGraph::insert_edge`
      (`src/ir/graph/mod.rs:63`), the parenthetical is **false**: two edges with
      permuted output lists have the same output *set*, `duplicate_output`
      scans the indexed aliases, and the second insertion is refused for
      exactly the loader's reason. So that case is a boundary that holds at
      every construction path, and the suite asserts *that* rather than a
      collision that cannot occur. The reachable bypass is the output-less
      edge, which is also loader-reachable (empty `name:` → `StringOrList::Empty`
      → `to_paths` → `[]` → an empty-slice duplicate scan rejects nothing), and
      the suite asserts the emitter renders it rather than dropping it.
      **A stale trace link.** `RM-4.3.1.c`'s second link named
      `path_key_is_injective_on_validated_paths`; the tree names it
      `..._on_keyable_paths`, and the difference is load-bearing — "validated"
      is the weaker condition an empty path satisfies, so the old name
      advertised a guarantee the suite does not hold. Link corrected and the
      naming decision logged.
      **Mutation patches.** `MUT-PATHKEY` deletes `parts.sort_unstable()`
      (`src/ninja_gen/mod.rs:228`); `MUT-EDGEDROP` filters `render_edges`
      (`src/ninja_gen/dyndep/mod.rs:155`) to skip entries with a non-empty
      `explicit_outputs`. Both pass `git apply --check` and are registered in
      `SUPPLEMENTAL_PROPERTY_PATCHES`.
- [x] (2026-10-16T00:00:00Z) `EP-M4`'s four files statically verified reference
      by
      reference, while the compiler remains unavailable.
      **Overtaken by the compiler at 2026-10-17.** The files compiled, and the
      reading was vindicated: every import, signature, and borrow resolved
      exactly as traced below, with no inference objection. It is kept because
      it records *why* each path was believed correct before anything could
      check it.
      This is a **substitute for the compiler, not the compiler**. It cannot
      discharge `EP-M4`'s acceptance, which is defined by `make proptest`
      passing and two mutations failing; it closes the class of defects that a
      reader can settle by following names, and leaves type inference, trait
      resolution, and borrow checking entirely unverified — precisely where a
      first compile is most likely to object.
      **Every import resolves.** `crate::ninja_gen::path_key`
      (`src/ninja_gen/mod.rs:226`, `pub(crate)` — reachable, since
      `property_tests` is a descendant of that module);
      `crate::ninja_gen::generate_bundle` (re-exported at
      `src/ninja_gen/mod.rs:13` from `dyndep::generate_bundle`,
      `src/ninja_gen/dyndep/mod.rs:87`, returning `Result<GeneratedNinja,
      NinjaGenError>`, whose `build_file()` is `&str` at
      `dyndep/bundle.rs:54`); `crate::ir::{Action, BuildEdge, BuildGraph,
      DependencyOrder, IrGenError}` (one `pub use` at `src/ir/mod.rs:33`);
      `crate::ast::{Recipe, StringOrList}`; and
      `crate::ninja_gen::determinism::strategy::graph::GraphSpec` — the path
      `graph/mod.rs:157` declares `pub(crate) struct GraphSpec` and
      `strategy/mod.rs:15-16` re-exports `classification` and `graph` as
      `pub(crate)`, with `determinism/mod.rs:21` declaring `strategy`
      `pub(crate)`.
      **Every called signature matches.** `GraphSpec::orders(&self) ->
      (&[usize], &[usize])` (`graph/mod.rs:203`) — the `let (first, _) = …`
      destructuring is right; `materialize(&self, order: &[usize]) ->
      Result<BuildGraph, TestCaseError>` (`:184`) — its error type is exactly
      what `support.rs`'s `map_err` consumes; `classification(&self) ->
      Classification` (`:213`) with `pub(crate)` fields read at
      `classification.rs:37,45`; `arb_graph_spec() -> impl
      Strategy<Value = GraphSpec>` (`:304`).
      **Every struct literal is field-exact.** `BuildEdge` has exactly nine
      public fields (`src/ir/graph/mod.rs:275-293`: `action_id`, `inputs`,
      `implicit_deps`, `dependency_order`, `explicit_outputs`,
      `implicit_outputs`, `order_only_deps`, `phony`, `always`) and
      `no_output_edge` sets nine, with `no_output_edge`/`permuted_edge`
      supplying all of them so the `..no_output_edge()` update form in the
      directed case is complete. `Action` has exactly six
      (`:254-268`: `recipe`, `description`, `depfile`, `deps_format`, `pool`,
      `restat`) and `graph_of_one_edge` sets six. `GraphSpec` is not named in
      the files at all — it only appears in a `use` — so its private fields
      cannot be a construction error there. `StringOrList::String(String)` and
      `::Empty` exist (`src/ast/string_or_list.rs:24,26`), as does
      `Recipe::Command { command }` (`src/ast/mod.rs:158`), and
      `DependencyOrder::Parallel` (`src/ir/graph/mod.rs:244`).
      **`BuildGraph::actions` is genuinely public** (`src/ir/graph/mod.rs:41`),
      so `graph.actions.insert(...)` is admissible from a descendant module;
      `edges()` is `pub fn edges(&self) (impl Iterator<Item = &BuildEdge>)`
      (`:204`), matching the `graph.edges()` call in `declared_outputs`.
      **The Proptest surface is real in 1.11.0.** `prop::collection::vec` is
      reachable because the prelude re-exports the whole crate as a `prop`
      submodule that includes `pub use crate::collection`
      (`src/prelude.rs:35-44`), so `prop::collection::vec` needs no extra
      import. `prop_shuffle` is a `Strategy` trait method
      (`src/strategy/traits.rs:500`), and `Shuffleable` is implemented for
      `Vec<T>` (`src/strategy/shuffle.rs:39,52`), so
      `Just(paths).prop_shuffle()` type-checks. `TestRunner` is **not** in the
      prelude, and both suites import it explicitly from
      `proptest::test_runner` (`path_key.rs:23`, `no_loss.rs:36`) — correct,
      and the kind of thing that fails loudly rather than silently.
      **One residual risk the reading cannot close:** `proptest!` bodies desugar
      to closures whose free variables must be `Send`/`Sync` where the runner
      requires it, and `path_key.rs` reads `tallies.get()` inside such a body.
      The `!Send` warning that Proptest emits for a non-`Sync` feature is a
      *warning*, not an error, so it is not a gate failure — but it is a real
      inference question and is left for the compiler rather than guessed at.
- [ ] (2026-10-16T00:00:00Z) `MUT-PATHKEY` repaired: it was a mutation that
      **could not compile**, so it was dead evidence rather than a
      falsification.
      The patch deleted `parts.sort_unstable()` and left the binding declared
      `let mut parts`. With the sort gone, nothing mutates `parts`, and
      `unused_mut` is a warn-by-default rustc lint. The gate policy is
      `-D warnings` — `GATE_RUSTFLAGS` (`Makefile:78`) composes it, and
      `make proptest` (`:267`) applies it — so the patched crate would have
      **failed to build**, and
      `path_key_is_permutation_invariant` would never have run. That is
      vacuous: a build failure for an unrelated reason is not a property
      detecting the seeded fault.
      **The repository already states this standard, and the patch failed it.**
      `every_patched_tree_compiles_under_denied_warnings`
      (`tests/kani_mutation_evidence_tests/compile_guard.rs:334`) exists to
      catch exactly this, with `DENY_WARNINGS = "-D warnings"` (`:66`) applied
      to every patched tree (`:199`). Its doc comment names the consequence:
      *"A patch that applies but does not compile is dead evidence: `cargo
      kani` never reaches its harness."* The other three supplemental patches
      were checked against the same criterion and are clean — one swaps an
      argument, one changes `placeholder` to `_placeholder` and narrows a call,
      one flips an equality — none leaves an unused binding.
      **Reachability note.** That guard compiles with `--lib`, which does not
      set `cfg(test)` (see the vacuity finding above). That is enough here, and
      only because `path_key` is **production** code: the mutation lands on a
      non-test item, so a `--lib` compile does reach it. A mutation to a
      `#[cfg(test)]`-only item would slip past this guard entirely, since a
      test-only binding could never be unused *outside* a test build. Worth
      knowing before trusting a green `make test-kani-mutations` for a patch
      that touches test-only code.
      **The repair, demonstrated rather than asserted.** Breaking the line
      *and* dropping the `mut` is also the more faithful mutation: "someone
      forgot to sort" removes the sort statement, and rustc then reports the
      now-pointless `mut`. The failure mode was reproduced in isolation
      (`error: variable does not need to be mutable`, `= note: -D unused-mut
      implied by -D warnings`, `error: aborting due to 1 previous error`),
      confirming it is an abort and not a warning.
      The repaired patch drops `mut` as well:
      `-    let mut parts: Vec<String> = …` joined by
      `-    parts.sort_unstable();`, replaced with
      `+    let parts: Vec<String> = …`. Post-apply the tree has zero
      occurrences of both `let mut parts` and `parts.sort_unstable()`, and
      `git apply -R` restores both to one each with a clean `git status`.
      `MUT-EDGEDROP` was checked for the same defect and is sound: its inserted
      `continue` leaves `graph`, `out`, and `stages` all still used, so no
      parameter becomes dead.
      **What remains unverified here.** That each patch, once it compiles,
      actually *fails the named property* — the real falsification — still
      requires `make proptest` on a working pool. Only applicability, clean
      reversal, and the absence of the specific denied-warning defect above are
      established, and that last one is established by reading rather than by
      running rustc.
- [x] (2026-10-16T00:00:00Z) `EP-M4` compiler-free gates green on the candidate;
      the compiling gates are **blocked by a machine-wide admission outage**,
      not by this change.
      **Green, sequentially, on the candidate.** `make check-fmt` (exit 0)
      caught two real `rustfmt` diffs in the new sources — the `tallies.set(…)`
      call in `no_loss.rs` and the `map_err` chain in `support.rs` — which were
      applied and re-verified; it also caught a `mdtablefix` diff in this plan,
      applied with a path-scoped `--in-place` run (a bare `mdtablefix FILE`
      prints and writes nothing, and `--git` cannot be combined with a path).
      `make markdownlint` (exit 0) and `make nixie` (exit 0) also pass.
      **Blocked — since resolved; the observation above is preserved as
      historical, but its current-state claims are superseded.** At the time of
      writing, `build-limits-status` reported the supervisor unreachable and
      `buildlimits-supervisor.service` was `failed` with exit status 2, its
      journal carrying: *"start-up requires operator action: startup cannot
      account for managed cache work: live sccache service may retain detached
      requests; visible compiler roots=0; operator accounting required"*. The
      inference drawn from `RestartPreventExitStatus=2` — that the unit would
      **not** self-recover — **was wrong**, in the way recorded in `Risks`:
      the setting governs a specific exit path and does not constrain a
      `stop-sigterm` deactivation, which is what actually occurred.
      **Resolved without operator action at 21:42:31.** The supervisor
      restarted itself: `MainPID` moved 1878261 → 2867217,
      `ActiveEnterTimestamp=21:42:31 CEST`, `SubState=running`, and
      `buildlimits-sccache.service` came up under it (`MainPID=2868075`).
      `build-limits-status` then read `capacity 4, held 4, healthy, caching
      through sccache` — the managed cache is back, not merely the admission
      pool. No operator action was taken or needed; the prescribed quiet
      sccache stop would have been an unnecessary intervention on a shared
      service during other agents' builds.
      **Unrun, therefore unverified.** The four source files have never been
      compiled. The first compile attempt was killed by the outage at the
      `--print=sysroot` probe, so it establishes nothing about the code — not
      even that it parses.
      **A vacuity trap in the `--lib` probe, established after the fact.** The
      narrowed `cargo check --lib --all-features` probe that was adopted as the
      cheap way to judge these four files **cannot judge them at all**. The
      modules are gated by `#[cfg(test)]` (`src/ninja_gen/determinism/mod.rs:17`
      and `:20`), and `cargo check --lib` does not set `cfg(test)`: the flag
      means "check this package's library" in its normal, non-test build.
      `--all-targets` is what reaches them, because it includes the targets for
      which `test = true` holds — `cargo check --help` states `--tests` as
      "Check all targets that have `test = true` set", and `make typecheck`
      (`Makefile:423`) passes `--all-targets` and `--all-features`.
      **The evidence is intrinsic, not circumstantial.** Two `netsuke` library
      rmetas were built and completed by these probes
      (`target/debug/incremental/netsuke-0m5t1t8w250tg/…/metadata.rmeta` and
      `…/netsuke-3e9x4qeorcsj4/…/metadata.rmeta`). Searching each for symbols
      gives `BuildGraph` — a non-test item expected in the normal library — **13
      occurrences**, and `arb_graph_spec`, which lives under the same
      `#[cfg(test)] mod` that the four files do, **0 occurrences**. The paired
      control rules out the alternative explanation that `strings` simply
      cannot recover this rmeta's symbol table: it recovers 13 other symbols
      from the same file. A probe that leaves a cfg(test)-only symbol at zero
      has not compiled cfg(test) code.
      **Consequence.** This is the sixth run to fail to reach the code, and the
      third distinct reason. The two `--lib` probes that were believed to have
      "reached 75 crates with zero diagnostics" reached 75 crates of the
      *non-test* library and were silent about every `EP-M4` line by
      construction. No `--lib` probe result may be cited for or against
      `EP-M4`, and this must not be recorded as a code failure — there were
      still zero rustc diagnostics. The judgement has to come from a target
      that sets `test = true`: `make typecheck` (`--all-targets`), `make
      proptest`, or `make test`.
      **Checked and cleared: no orphaned compiler.** The probe agent was
      stopped, and the one `cargo check --lib` chain that survived it (pid
      558023, a `--unit-graph` render adopted from a shell whose parent had
      died) exited on its own with `rc=0` and a **0-byte** `/tmp/ug-checklib.json`.
      Its rmeta shows the same `arb_graph_spec = 0`, so it too was a non-test
      render. No probe process from this session was left holding a token or a scope.
- [x] (2026-10-15T00:00:00Z) Corrected two plan defects before implementing
      `EP-M4`, and ran the last `EP-M2` gate batch.
      **Repair 1 — stale artefact paths.** Thirteen live citations gave the six
      property homes as `src/ninja_gen/determinism/<module>.rs`, without the
      `property_tests` segment that `PROPTEST_FILTER` selects on.
      A suite built at those paths would never have run. `EP-M4` would have
      been implemented from them.
      **Repair 2 — an unfalsifiable mutation.** `MUT-EDGEDROP` skipped a
      dispatch edge whose `explicit_outputs` is empty. `blueprint.rs:119`
      gives every generated edge at least one explicit output, so no
      generated case reaches that trigger, and the one directed case that
      does renders an empty left-hand side, leaving the output set identical
      whether the edge is rendered or skipped. The trigger now drops an edge
      carrying outputs. The acceptance test name was an edge-*count* claim —
      the very assertion the 2026-10-15 statement correction removed — and is
      renamed `every_explicit_output_appears_exactly_once`, which holds on the
      generated domain because `allocate_pool` names globally unique slots.
      **Gate batch** (`2b01a3e3`, six targets): `typecheck`, `lint` (all four
      sub-targets, both Whitaker passes), `doc-coverage` (98.87% against
      80.00%), `test-workflow-contracts` and `check-fmt` all exit 0.
      `make test` is **environment-red**: one `TIMEOUT [300.012s]` in
      `cli_configuration_fixture_compiles`, zero assertion failures. The
      branch does not touch `.config/nextest.toml`, the test's 300 s allowance
      is identical to `origin/main`'s, and the run logged 573 sccache
      `unreachable` lines against a saturated shared compiler pool. No
      allowance is widened. Because `test-nextest` aborts it, the `doctest`
      pass did **not** run in that batch and is re-run below.
- [x] (2026-10-14T00:00:00Z) The third `EP-M2` gate pass (revision
      `4250152d`, twelve targets dispatched) produced one real defect and one
      environmental failure.
      **Green:** `check-fmt`, `lint` (exit 0 — and, unlike pass #2, every
      sub-leg ran: `cargo doc`, Clippy, Whitaker, `ruff`, `pylint`, the df12
      house lints, `ambrleaks`, `interrogate` at 100.0%, `yamllint`,
      `actionlint`), `typecheck` (304 s), `doc-coverage` (**98.87%** against
      the 80.00% threshold), `nixie`.
      **Real defect — `make test`.** `behavioural_nextest_targets_forward_both_worker_bounds`
      failed: `NEXTEST_TARGETS` declared five targets but the Makefile now
      invokes the runner from six, because `335ca971` added `make proptest`
      without adding it to the inventory. The contract test was doing its
      documented job — "a new recipe joins the contract or breaks the build".
      Fixed in `73d2f814` by declaring `proptest`; the recipe already forwards
      both worker bounds, so only the declaration was missing.
      **Environmental — `production_build_module_slice_has_expected_boundary`
      timed out at 300.073 s.** This is the recorded live form of the
      load-sensitive failure, and the diagnostic is a count, not the test
      name: the run carries **573** `the managed sccache server is unreachable`
      lines, the same figure previously measured alongside four to nine
      `TIMEOUT`s with **zero** source failures. The test is untouched by this
      branch and is a named member of the serialized `nested-cargo-builds`
      group, whose override sets no `slow-timeout`. A from-scratch nested
      compile under an unreachable cache cannot fit 300 s; widening the
      allowance would not have saved the attempt, so the budget question is
      deferred rather than tuned here.
      **Two gates never ran.** `markdownlint` inherits `spelling` as a
      prerequisite (`Makefile:445`), so its exit 2 was the spelling defect
      below, not a Markdown verdict of its own — markdownlint-cli2 itself has
      still never executed on this branch. Gates 9-12
      (`test-workflow-contracts`, `test-documentation-contracts`, `proptest`,
      `test-coverage-artifact`) were not reached.
- [x] (2026-10-14T00:00:00Z) `make spelling` failed on `hand-written` in the
      Progress prose added by `4250152d`, corrected to `handwritten` in
      `9105e03c`. The adjacent hyphenated forms `hand-off`, `hand-rolled` and
      `hand-maintained` are all accepted by the dictionary, so only this one
      was a violation. A tracked edit during a gate pass moved HEAD and
      invalidated the pass in flight; the rule the run applied — re-check
      `HEAD` after every gate and stop on a change — is the correct one and did
      its job.
- [x] (2026-10-14T00:00:00Z) The second `EP-M2` gate pass cleared
      `make check-fmt` (exit 0, 4s) but failed `make lint` (exit 2, 2789s):
      `clippy::shadow_unrelated` at
      `tests/kani_mutation_evidence_tests/contracts.rs:152`, where a second
      `let resolved` shadowed the earlier, unrelated binding at :143. The
      denial has been present since the test landed in `0efaea38`; the
      rustfmt remediation in `b7cfa1e4` moved it from line 147 to 152 and so
      brought it into the transcript. Fixed by renaming both bindings to name
      the module shape each pins — `directory_resolved` and
      `single_file_resolved` — which removes the shadow without a
      suppression, as `Lints must not be silenced except as a **last
      resort**` requires.
      Clippy aborts `make lint` in the `lint-clippy` recipe, so the Whitaker,
      `lint-python`, `yamllint` and `actionlint` legs have **no evidence** for
      any revision on this branch yet; they run in the next pass.
      Also corrected a stale denominator three lines into the `make proptest`
      Decision entry below: it read *0 of the 86 tests*, quoting the
      pre-`--workspace` corpus while the same entry's discharge measures the
      recipe's own flags (corpus 3924, 90 matches). The corrected sentence
      drops the denominator, since 0 holds under either corpus and naming one
      of them contradicted the paragraph beneath it.
- [x] (2026-10-13T00:00:00Z) Ran the first `EP-M2` gate pass and it failed
      `make check-fmt` on **nine** rustfmt diffs and two `mdtablefix` reflows,
      so the milestone had been committed unformatted. This is the concrete
      cost of a milestone being written without running the gates before
      committing — the four implementation commits were made with only
      targeted test runs as evidence. Three remediation commits follow, and
      the affected commit messages are amended to carry the required
      attribution trailer.
      The nine rustfmt diffs were mechanical, with one exception worth
      recording: `use super::RecipeShell;` sorts *above*
      `use super::command_list::…`, because byte order puts the capital `R`
      before the lowercase `c` and rustfmt does not case-fold a grouped
      import. A handwritten ordering that reads more naturally is still a
      gate failure.
      Of the two `markdownlint` failures, one was a real authoring defect
      rather than a reflow. A `Progress` bullet inserted by a later commit had
      orphaned the tail of the 2026-10-10 base-re-derivation entry: its head
      was deleted, leaving a sentence beginning mid-thought at a six-space
      indent, which CommonMark reads as an indented code block (MD046, the
      documented list-continuation trap). The head is restored to what
      `1a4862b3` wrote. The second, MD013 at 128 columns, was a `FAIL …`
      transcript quoted inline; it is now a fenced `plaintext` block, matching
      how the rest of this plan quotes command output.
      Also corrected the `make proptest` comment's stale figures: it recorded
      the *pre*-`--workspace` numbers, 86 selected of 3807, where the recipe's
      own flags select 90 of 3848. The comment now states both, so the next
      reader does not have to re-derive why they differ.
- [x] (2026-10-14T00:00:00Z) Found and corrected a **false factual claim** in
      `EP-M2`'s own prose, caught while re-reading the diff against the corpus
      it describes. Both the crate-root comment in
      `tests/kani_mutation_evidence_tests.rs` and the matching paragraph in
      `docs/developers-guide.md` asserted a contrast between two layouts: that
      a crate root spelling out every child with `#[path]` is unusual, and that
      other directory-module test trees "are rooted at a `tests/<name>/mod.rs`,
      which resolves its children normally and needs the attribute only for the
      one child that would otherwise collide". The second half is false. A
      census over `tests/*.rs` finds **17** crate roots that reach their own
      `<stem>/` directory through `#[path]` — `ast_tests`, `std_filter_tests`,
      `env_access_suppressions`, and 13 more — of which **16 have no `mod.rs`**
      and so spell out every child, exactly as this file does. Only
      `cli_tests` ships a `mod.rs`, and even it reaches that file by `#[path]`
      rather than by a bare declaration. The `tests/<name>/mod.rs` shape
      described is a *different* case: it is how a crate root reaches a
      **shared** tree (`support`, `fixtures`, `common`) that genuinely lives
      directly under `tests/`. So the `#[path]` here is the house shape rather
      than a local quirk, and it is the bare `mod child;` that is the marked
      form. Corrected in both files, and the count is stated as a census rather
      than as an impression.
      Recorded because it is the third instance in this milestone of the same
      failure mode: a plausible-sounding structural claim written from
      recollection and never measured. The first two were the `make proptest`
      selector count and the `ninja_gen` line-count drift. Cheap to catch by
      probe, expensive to leave for review — and a wrong *reason* is worse than
      no reason, because the next author will trust it.
- [x] (2026-10-13T00:00:00Z) Implemented all four `EP-M2` items on the working
      tree; gates and the milestone review are still to run, so this is
      implementation activity and not yet an accepted plateau.
      Item 1: `src/ninja_gen/mod.rs` split from 386 to 247 lines, with
      `NamedAction` moved verbatim to a new `src/ninja_gen/named_action.rs`
      (152 lines). Item 3: the ordering permutations both emission paths share
      were extracted to a new `src/ninja_gen/order.rs` (54 lines) as three pure
      helpers — `ordered_edges`, `ordered_actions`, `ordered_default_targets` —
      and both `mod.rs` and `dyndep/mod.rs` now call them, replacing a
      duplicated `edges.sort_by_key` and `defs.sort()`. Item 2:
      `tests/kani_mutation_evidence_tests.rs` split from 390 lines into a
      33-line crate root over four child modules (`compile_guard.rs`,
      `contracts.rs`, `discovery.rs`, `sandbox.rs`), and
      `supplemental_property_location` generalized to resolve a patch stem
      existentially against the tree, accepting both the directory-module and
      single-file-module shapes and naming every candidate it searched when
      neither declares the property. Item 4: `make proptest` added, with its
      selector and the reasoning recorded in `Decision log`.
      Two corrections to plan text are worth carrying forward. The
      **selector spelling the plan suggested selects nothing** — see the
      `Decision log` entry — and the **configuration route for it is closed**
      by a static contract, also recorded there. Verified so far: layout
      contract 5 passed; `ninja_gen` and mutation-evidence suites green (128
      and 4 tests); and `make proptest` 90/90 passing at exit 0.
- [x] (2026-10-13T00:00:00Z) Recorded the measured line-count drift for
      `tests/kani_mutation_evidence_tests.rs`: the plan's `EP-M2` text says 383
      lines and the file measured **390** before the split. The difference
      arrived with `fce1a746` (*Group prefix-named modules under directory
      modules (#811) (#813)*), which is upstream of this branch's merge base,
      and `origin/main` measures 390 too, so the plan's figure was stale rather
      than this tree being anomalous. The split target is unaffected: the file
      is now 33 lines and no file touched by this milestone exceeds 400 — the
      largest is `compile_guard.rs` at 390, which the split left untouched.
- [x] (2026-10-10T00:00:00Z) Re-derived the `EP-M2` base against `origin/main`
      before implementing, and found the branch 44 commits behind. Recorded the
      `ninja_gen` directory-module migration, the layout-contract violation
      counts (66 and 27 here, 0 and 0 upstream), the two-suffix `make proptest`
      selector problem, the open question on selector anchoring, and the
      falsified collision premise behind the `supplemental_property_location`
      generalization. No milestone or obligation changed. A `git merge-tree`
      rehearsal of `HEAD` against `origin/main` reports zero conflicts, so the
      rebase was the next step.
- [x] (2026-10-10T00:00:00Z) Rehearsed the rebase in a throwaway clone rather
      than in this worktree. All 16 commits replay onto `e1df2ead` (the true
      remote `main`, resolved from the API — the bare repository's cached
      `main` is a stale `1e60fb18`) with **zero conflicts**. On the rebased
      tree: `docs/adr-030-ninja-emission-determinism-contract.md` and this
      execplan both survive, the adr-030 link still resolves at
      `docs/contents.md:222`, `src/ninja_gen*` collapses to just
      `src/ninja_gen`, and the layout contract reports **0 prefix groups and
      0 prefixed-beside-directory** violations, against 66 and 27 pre-rebase.
      The clone was deleted; this worktree was never touched.
- [x] (2026-10-10T00:00:00Z) Received the confirming CodeRabbit pass. It
      reported **two** findings, both `trivial` and both `[type:docstyle]`, and
      both were verified against the tree and accepted: `ADR-030`'s Status
      value lacked the trailing full stop the style guide's ADR template
      requires (precedents ADR-008 and ADR-029 both write `Accepted.`), and the
      plan's surviving `ADR-NNN` uses were not marked as deliberately retained.
      Neither is a correctness finding, and neither changes a milestone,
      obligation, or acceptance clause.
      *Provenance caveat:* this pass is **not** a clean confirmation of
      `1401d0da`. The tree moved during it — `1401d0da` → `2e97da1d` →
      `e1a3a16c`, all three commits from this session — so the pass's
      `head_at_start` and `head_at_end` differ, and its findings are anchored
      by quoted content rather than by line number for that reason. The two
      findings above were re-located by quotation. The writer was this session
      between agent invocations, **not** a peer: every commit carries the same
      author and committer identity. Recorded because a review verdict's value
      is its provenance, and this one's is qualified.

- [x] (2026-10-10T00:00:00Z) Ran the compiler-free gates on the rebased tree.
      This was necessary rather than optional: the **compile-admission pool was
      wedged machine-wide** for the whole session — `build-limits-status`
      reported `capacity 4, held 4` with `drain check failed; tokens retained …
      visible compiler roots=0; operator accounting required`, while `pgrep -c
      rustc` was **0** and 23 cargo processes from nine other agent sessions sat
      queued for up to 3160 s. Nothing on this host could compile, so `make
      lint`, `make typecheck`, `make test`, `make doc-coverage`, and `make
      proptest` were all unrunnable locally. The compiler-free gates all pass:
      `make fmt`, `make check-fmt`, `make markdownlint`, `make nixie`, and
      **`make test-workflow-contracts` (1149 passed, 3 skipped)**. That last one
      is the milestone's own acceptance evidence: it is the target that runs
      `tests/workflow_contracts/rust_module_layout_test.py`, whose self-tests
      `test_added_prefix_pair_fails_contract`,
      `test_added_prefix_directory_pair_fails_contract`, and
      `test_prefixed_file_beside_directory_fails_contract` are present and
      passing, so the contract is live and green on the rebased tree — the
      66 + 27 violations it reported pre-rebase are gone. It needs no compiler,
      which is why the wedged pool does not block it.
- [x] (2026-10-10T00:00:00Z) Corrected the `make proptest` open question's
      first half after reading `.github/scripts/verify_nextest_anchored_filters.py`
      and `.github/scripts/_nextest_oracle/listing.py` in full. The plan had
      asserted the runtime verifier "does not currently catch" a dead selector
      because it "scans only `REPO_ROOT / "tests"`". That is **half wrong** and
      is corrected in place: the verifier has **two** independent halves.
      `_check_every_filter_selects_something` replays *every* `filter = '…'`
      line in `.config/nextest.toml` against real nextest via `selected()`,
      which reads each testcase's `filter-match.status`, and **fails hard** on
      any selector matching nothing — with **no `tests/`-root restriction**. So
      a dead selector *is* caught. The `tests/` restriction belongs to the other
      half: `parameterized_tests()` scans only `TEST_SOURCE_ROOT = …
      / "tests"`, and `main()` **aborts the lane** unless at least one anchored
      selector names a test drawn from that corpus.
      **The consequence first drawn from this was wrong and is retracted.** The
      original entry concluded that `make proptest` **must not** be a
      `.config/nextest.toml` `filter` covering only `src/`-side property suites,
      "because that spelling fails the CI step on the current tree". That is a
      quantification slip. `main()` builds `wanted` over the **union** of
      anchored selectors, so the precondition is satisfied when **any one**
      member names a `tests/`-rooted parameterized test; adding `src/`-side
      selectors cannot remove that member. Running the verifier's own
      extractors on this tree returns `wanted` with exactly one entry
      (`text_domains_cannot_be_swapped`, from `tests/ninja_semantics_ui_tests.rs`),
      so `main()` does not abort and the original claim is falsified by direct
      measurement. The accurate constraint, recorded in `EP-M2`, is that the
      configuration must not be left with **no** `tests/`-rooted parameterized
      anchored selector. The narrower open part — `Makefile` `-E` versus a
      configuration test-group — remains open, and the `filter-match.status`
      discharge procedure is unchanged.
- [x] (2026-10-10T00:00:00Z) Rebased the real branch onto `origin/main`. The
      rollback ref `refs/backup/4-3-1-prerebase` was pinned to `b7c498ef`
      before the rebase started, so the pre-rebase tip survives locally even
      after the branch is force-pushed. All 18 commits replayed with **zero
      conflicts** onto `e1df2ead`; `HEAD` is now `597ac3f5` and the branch is
      **0 behind**. A count check confirms the replay dropped nothing: the
      pre-rebase tip sat 18 ahead of the old merge base and the rebase replayed
      18. A content check confirms the two files this branch actually authors —
      `docs/adr-030-ninja-emission-determinism-contract.md` and this execplan —
      are **byte-identical** before and after (same blob SHAs), against the
      earlier draft's claim that the rebase "brings 44 commits of upstream
      Markdown into the tree": it brings 44 upstream commits, whose Markdown
      changes belong to those commits, and the only files where the rebase had
      to merge rather than replay are the two upstream also touched.
      `docs/contents.md` and `typos.toml` both merged cleanly and both keep
      both sides: the ADR-030 entry still resolves at `docs/contents.md:222`
      beside upstream's ADR-032 and ADR-033 entries. The `typos.toml`
      difference is **not** upstream's — it comes from this branch's own
      `8da0bb93` (*Regenerate typos.toml from the estate dictionary*) and
      predates the rebase. `make spelling` and `mdtablefix` must be re-run
      against the rebased tree, because the rebase rewrote every commit's
      parentage and `make spelling` regenerates `typos.toml` from the live
      shared dictionary.
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
- [x] (2026-10-10T00:00:00Z) `EP-M1` review round 2: `coderabbit review --agent`
      returned nine findings against `74cff9ce` — six major, three minor. All
      nine were verified against the tree before any change; eight accepted, one
      accepted in substance while its cited mechanism was a partial misread.
      Three corrections change what an obligation actually tests: `OBL-PROCESS`
      now compares whole bundles, `OBL-CYCLE`'s cyclic arm constructs its cycle,
      and acceptance item 6 names `MUT-ACTIONSORT` rather than the impossible
      `MUT-DEFSORT`. Two replace mutations that could not falsify their
      property (`OBL-NOLOSS`, `OBL-NOHASH`). One reclassifies a
      `prop_shuffle` assertion. Three are documentation corrections. All are
      recorded in the fifth `Revision note` entry and the `Decision log`.
- [x] (2026-10-10T00:00:00Z) The three Markdown gates passed on the frozen
      correction, at `HEAD`
      `c91e96c22481569c2dd34891a666762a75ee21a8`. `make check-fmt` exit 0,
      "167 files left unchanged"; `make markdownlint` exit 0, spelling
      prerequisite executed and "0 issues in 0 files"; `make nixie` exit 0,
      "All diagrams validated successfully!". Logs:
      `/tmp/check-fmt-netsuke-4-3-1-proptests-for-deterministic-ninja-emission.out`,
      `/tmp/markdownlint-netsuke-4-3-1-proptests-for-deterministic-ninja-emission.out`,
      `/tmp/nixie-netsuke-4-3-1-proptests-for-deterministic-ninja-emission.out`.
      One `make fmt` cycle was needed before the first `check-fmt`; it wrapped
      the new revision-note paragraphs and re-applied the intended second-person
      and `prop_shuffle` edits.
- [x] (2026-10-10T00:00:00Z) `EP-M1` review round 3: a confirming pass returned
      six findings — one major, five minor — all verified against the tree and
      all accepted. The major one **reverses** the previous round's `prop_shuffle`
      disposition: the collector arm's perturbation was claimed by construction
      but was in fact sampled, and both arms are now disjoint. The differential
      arm was withdrawn outright once reading `src/graph_view/mod.rs:97` showed
      its antecedent is a tautology under the fixed-graph-value construction; a
      probe against the built binary over five `name:` shapes turned `OBL-NOLOSS`'s
      open reachability question into a measured result, and corrected an
      overclaim written into the same edit. Recorded in the sixth `Revision note`
      entry.
- [x] (2026-10-10T00:00:00Z) Found and fixed a **live** contract failure by
      reading CI rather than re-running gates. The local compile-admission pool
      is still wedged, so no Rust gate could run here and the Python-only
      `make test-workflow-contracts` does not cover this class. The detail, and
      four findings that fall out of it, are recorded immediately below this
      item.
- [x] (2026-10-10T00:00:00Z) Resolved the CodeRabbit ADR-Status finding on
      evidence, and the resolution is narrower than the finding. The finding
      was that `docs/adr-030-ninja-emission-determinism-contract.md` should
      write `Accepted.` rather than `Accepted`. Reading
      `docs/documentation-style-guide.md` verbatim shows the rule is split: the
      prose says only "**Status:** One of `Proposed`, `Accepted`, `Superseded`,
      or `Deprecated`. For `Accepted` status, include the date and a brief
      summary" — no full stop on the bare value — while the ADR **template** in
      the same file shows
      `<Proposed | Accepted | Superseded | Deprecated>.` **with** one. The
      house practice matches the template: of 42 ADRs carrying a status word,
      **40 have the full stop immediately after the word**, and the only two
      exceptions continue with a qualifier phrase (`Accepted – plan recorded on
      18 November 2025 …`, `Superseded by [ADR-025]…`) rather than a bare value.
      So the finding is correct in effect and its authority is the template
      plus 40 files of practice, not the prose sentence. The change stands; the
      rationale is recorded because the prose alone does not compel it.
- [x] (2026-10-10T00:00:00Z) Retracted a claim this plan had committed to
      `3180ccd2`. See the `EP-M2` correction above and the amended Progress
      entry for the `make proptest` open question: the assertion that
      `make proptest` "**must not** be a `.config/nextest.toml` `filter`
      covering only `src/`-side property suites" is falsified by running the
      verifier's own extractors, which return a one-entry `wanted` set and so
      do not abort the lane. The accurate constraint is narrower and recorded
      where it is used.
**The live contract failure, in full.** The revision that settles it is CI run
`38051221830` at `1401d0da` — the revision a CodeRabbit checkpoint pinned —
which **failed**. Its `build-test` job failed at step 29,
`Test and Measure Coverage`, and the failing test is
`netsuke-build::execplan_status_contract_tests
every_execplan_header_status_is_within_the_closed_set`.
The message:

```plaintext
Error: every ExecPlan status must be one of ["DRAFT", "APPROVED",
"IN PROGRESS", "BLOCKED", "COMPLETE"]; off-vocabulary or qualified
values found: 4-3-1-proptests-for-deterministic-ninja-emission.md:
"DRAFT — AWAITING APPROVAL (revised after design review)"
```

The defect is in this plan's own header, not in infrastructure: line 9 read
`Status: DRAFT — AWAITING APPROVAL (revised after design review)`, and
`tests/execplan_status_contract_tests.rs` requires the value after `Status:` to
be **exactly** one member of the closed set. The qualifier is now a sentence
beneath the field, per AGENTS.md. Four further findings from the same reading,
each verified against the artefact rather than inferred:

1. The `build-test` job runs **only on pull-request runs**. In the successful
   `main` run `38046182468` (`e1df2ead`, 10:48 today) jobs `build-test` and
   `Windows` are both `skipped`; only `kani-smoke` runs. So a green push on
   `main` is *not* evidence for the seven-gate union — the PR run is.
2. Every run on this branch between `37929646063` and `38051221830` is
   `cancelled`; only the `1401d0da` run reached a verdict, and the verdict was
   failure. A cancelled run is not a pass.
3. The two CodeRabbit findings already applied are unaffected: this failure is
   a third, independent defect that neither the review nor the local Markdown
   gates could reach, because it is a Rust contract test.
4. The contract also cross-checks the style guide
   (`the_style_guide_defines_every_accepted_status_value`), so the closed set
   and the guide cannot drift silently.

Replicating the contract's own `header_lines`/`header_status` logic over all
**49** plans under `docs/execplans/` now reports every one in vocabulary, so
this failure is discharged at the tree level and not only for this file.
*Corrected on 2026-10-13:* this sentence previously read "**50** plans". The
count is 49 at every revision of this branch, including `20e5f480` where the
claim was first written, and the contract's `execplan_names` enumerates
`read_dir("docs/execplans")` filtered to the `md` extension — the same 49. No
plan has ever been deleted from the directory on any ref, so the figure was
wrong when written rather than made stale by a later removal. The miscount is
the more instructive for being in a sentence whose whole purpose was to prove a
claim by exhaustive enumeration: the replication was sound and its *headline
number* was not derived from it.

**Discharged in CI.** The fix was pushed as a fast-forward from `faef4e13` to
`20e5f480` (verified through the Git refs API afterwards, because the local
tracking ref goes stale after an SSH push). CI run `38055325773` then ran on
`20e5f480`, and the same job that had failed reports success:

```plaintext
build-test job 114222509702
  STEP 29: Test and Measure Coverage -> success
  failing steps: NONE
```

Step 29 is the step that failed at `1401d0da`; the change is one revision wide,
so the step-level A/B is direct. Job-level verdicts came from the Actions API
rather than the log, which is not downloadable until the whole run completes.

The run then finished **fully green** (watch exit 0, every job `success`), and
the log proves the specific test rather than leaving the step verdict to stand
for it:

```plaintext
PASS [0.007s] (2434/3917) netsuke-build::execplan_status_contract_tests
     every_execplan_header_status_is_within_the_closed_set
PASS [0.004s] (2442/3917) netsuke-build::execplan_status_contract_tests
     the_style_guide_defines_every_accepted_status_value
```

**This run also settled the anchor question** the plan had carried open since
its first draft. Two independent readings in the same log agree: the verifier
replays 18 filter alternatives with none dead and verifies 1 anchored selector
over 1 test, and the test log names 54 library-side instances by module path
with no crate segment (`ninja_gen::property_tests::dependency_only`,
`cmd_interpolate::property_tests::…`, `cycle::property_tests::…`). A
`^`-anchored selector therefore matches from the first module segment, which is
what `EP-M2`'s `make proptest` spelling depends on. The question is closed.

- [x] (2026-10-13T00:00:00Z) Confirmed CI run `38056865899` (workflow `CI`,
  `pull_request`) **fully green at the branch tip `7ec291a9`**, which is the
  revision previously unobserved. All five jobs report `success` — `kani-smoke`,
  `build-test`, `Windows / windows-msi-upgrade`,
  `Windows / build-test-windows`, `Windows / lint-windows` — and `build-test`
  step 29 `Test and Measure Coverage` is `success`. The run log names the
  specific contract test rather than leaving the step verdict to stand for it:
  `PASS [0.009s] (2434/3917) netsuke-build::execplan_status_contract_tests
  every_execplan_header_status_is_within_the_closed_set`.
  Two sibling runs at the same revision are also green (`38056866031`
  `Release Dry Run`, `38056865601` `Netsukefile Build Test`); the
  `pull_request_target` run `38056864662` is `skipped`, which is its normal
  disposition. **The `Status:` header defect is therefore discharged in CI at
  the tip**, superseding the `20e5f480` evidence recorded above, and the green
  result is now attributable to a commit whose tree contains every correction
  made so far.
- [x] (2026-10-13T00:00:00Z) Corrected the header `Status:` from `DRAFT` to
  `IN PROGRESS`, and removed the stale "Awaiting approval" sentence. This is a
  defect the style guide names explicitly: `DRAFT` means "written, not yet
  approved for implementation", and `APPROVED` means "approved, implementation
  not yet started". Neither holds — the approval gate was passed on 2026-09-27
  and `EP-M1` has since **delivered** a tracked artefact,
  `docs/adr-030-ninja-emission-determinism-contract.md`, indexed in
  `docs/contents.md`. The header had read `DRAFT` in every revision since the
  first, so the field had drifted eight revisions behind the `Progress` section
  that is its authority.
- [x] (2026-10-13T00:00:00Z) Recorded that `origin/main` has **moved again**,
  from `e1df2ead` to `b14bde20`, leaving this branch 12 commits behind. The
  movement does **not** disturb `EP-M2`'s base: re-deriving the seam shows
  `src/ninja_gen/mod.rs` still at 386 lines,
  `tests/kani_mutation_evidence_tests.rs` still at 390, the
  `ensure!(*root == "ir", …)` assertion still at line 212, and no change to
  `src/ninja_gen/` or `.config/nextest.toml`. The one relevant change is to
  `Makefile`, which gains the shared CV-005 CodeScene contract checker and a
  `CV005_CONTRACTS_REF` pin that `make test-workflow-contracts` now invokes
  against `.github/cv005.toml` — a file this branch does not carry. **This is a
  rebase hazard for a later point in this branch's life, not for `EP-M2`**: a
  rebase must merge that Makefile hunk, and the `proptest` target `EP-M2` adds
  must not displace it. No milestone or obligation changed.
- [x] (2026-10-13T00:00:00Z) Confirmed the compile-admission pool has
  **cleared**. `build-limits-status` reports
  `capacity 4, held 4, healthy, sccache unreachable, compiling directly`, with
  the longest wait at **16.0 s** against the **5593 s** seen while the pool was
  wedged. The stuck tokens the earlier entries describe are gone and the
  supervisor no longer reports `drain check failed`. The Rust gates are
  therefore runnable locally again and `EP-M2`'s block on admission is lifted.
  Note the separate `sccache unreachable` clause: builds now run uncached,
  which is not a failure but does make the first `EP-M2` gate run a cold one.

- [x] (2026-10-14T00:00:00Z) **Gate pass #4: eleven of twelve gates pass, and
  the four gates that had never run on this branch are all green.** Revision
  `55da63ed`, working tree clean at start and finish, HEAD unchanged across the
  whole pass, 40m13s wall clock. `check-fmt` (174 files unchanged), `lint`
  (exit 0, every sub-leg reached — Clippy did **not** abort, so Whitaker,
  `lint-python`, `interrogate`, yamllint and actionlint all actually ran),
  `typecheck`, `doc-coverage` (98.87%, 4977/5034), `markdownlint`
  (v0.23.2/v0.41.1, "0 issues in 0 files" over 174 files), `spelling` (which
  left `typos.toml` unmodified), `nixie`, `test-workflow-contracts` (27 passed),
  `test-documentation-contracts` (14 passed, 42 skipped), `proptest` (**90
  passed, 3834 skipped**), `test-coverage-artifact` (52 passed). The `proptest`
  figure independently reproduces the selector counts this plan records, so
  `make proptest` is confirmed to select real work rather than passing
  vacuously.
- [x] (2026-10-14T00:00:00Z) Closed the doctest evidence gap. `make test` is
  `test-nextest doctest`, and pass #4 aborted inside `test-nextest`, so
  `grep -c "cargo test --workspace --doc"` over that log returns **0** — the
  gate's second half had never executed on this branch. Ran `make doctest`
  alone: **88 + 2 + 39 passed, 0 failed, 6 ignored, 26 ignored**, exit 0. The
  `make test` verdict for `55da63ed` is therefore one environmental timeout
  plus a fully green doctest leg, not a gate-wide failure.
- [x] (2026-10-14T00:00:00Z) Diagnosed and **declined to repair** the
  `make test` timeout, on measured grounds rather than reflex.
  `production_build_module_slice_has_expected_boundary` timed out at 300.008s —
  its second consecutive occurrence, the previous pass having hit 300.073s.
  Three facts together rule out a source defect: this branch touches **zero**
  files in that test's dependency set (an empty `git diff --name-only` across
  `tests/build_module_slice_ui_tests.rs`, `tests/ui/`, `build.rs`,
  `src/cli/merge.rs` and `.config/nextest.toml`); the log carries **573**
  `sccache server is unreachable` lines, the exact count previously measured
  alongside timeouts with zero source failures; and `sccache --show-stats`
  reports `Compile requests 0`, meaning the server was *reset* rather than
  idle, so every nested compile ran from scratch. `build-limits-status` at
  diagnosis time read
  `capacity 4, held 4, healthy, sccache unreachable, compiling directly` with
  25 crates queued behind other agents' work. **A widened allowance was
  considered and rejected.** `origin/main` has since split
  `packaged_manifest_retains_build_script_sources` into its own override with
  `terminate-after = 10`, commented "Allow ten periods for shared-cache and
  compiler-queue delays without widening unrelated tests or adding retries" —
  superficially the same repair. It is not the same case here: that widening
  was a measured decision for a *different* test, and a from-scratch production
  graph exhausted 300s on its own before any queue delay, so ten periods would
  have bought delay tolerance without addressing the cause. Recorded guidance
  for this fixture is explicit — "do not tune an allowance to turn a gate green
  inside a gate-verification milestone" and "cache first, budget second". The
  budget decision is deferred to a milestone where the group's membership is
  final; **CI is the oracle** for this SHA.
- [x] (2026-10-14T00:00:00Z) Independently confirmed the `NEXTEST_TARGETS`
  repair. `behavioural_nextest_targets_forward_both_worker_bounds` PASSES in
  0.013s, where the previous pass failed it in 0.037s over the undeclared
  `proptest` target. `origin/main` still carries the array at length **5**
  while this branch carries **6**, so the fix is branch-local and the rebase
  must preserve it.
- [x] (2026-10-14T00:00:00Z) **The CI oracle answered the deferred timeout
  question, and the deferral was correct.** CI run `38504829270` on `8fc7a2fa`:
  `build-test` (Linux) **PASSED**, and the log shows
  `production_build_module_slice_has_expected_boundary` **PASS [26.669s]
  (3422/3929)** — the very test that exhausted the 300 s cap locally. 26.7 s
  against a 300 s budget is a factor of eleven of headroom, so the local
  exhaustion was entirely the sccache-unreachable, queue-contended environment
  and **not** a budget that was too small. Widening the allowance would have
  masked the environmental cause and bought nothing. Four other jobs passed:
  `kani-smoke`, `Windows / lint-windows`, `Windows / windows-msi-upgrade`, and
  the whole `Release Dry Run` run (`38504829410`). One job failed:
  `Windows / build-test-windows`, over a defect no Linux gate could see —
  recorded below.
- [x] (2026-10-14T00:00:00Z) **CI caught a genuine Windows-only defect in this
  branch's own test, and it is fixed.** `Windows / build-test-windows` failed on
  `8fc7a2fa` with
  `Summary [121.009s] 2307/3501 tests run: 2306 passed, 1 failed` — the single
  failure being this branch's
  `contracts::supplemental_property_location_resolves_directory_and_file_modules`
  (`FAIL [0.078s]`). The test asserted `/`-joined substrings against a message
  the helper rendered with `{candidates:?}`, and a `Debug` rendering of a path
  **escapes each backslash** — so Windows produced
  `["src\\ninja_gen\\property_tests\\mod.rs", …]` and the assertion could never
  match. Fixed in `1e1508ca` by rendering the candidates through `Display`
  instead, so the diagnostic carries the raw platform separator, **and**
  normalizing the separator in the assertion. The two halves are interdependent:
  `replace('\\', "/")` over a `Debug` rendering turns each escaped pair into
  two slashes and *still* fails, which a standalone probe confirmed before and
  after the change. The lesson is that a Linux-only local gate run cannot
  discharge a cross-platform assertion, and the local focused run passing (4/4)
  was never evidence that the Windows case would pass — only CI was.
- [x] (2026-10-14T00:00:00Z) **Gate pass #5 against `c517f6fd` — the Windows
  repair revision — is 12 of 13 green, and the thirteenth is a now
  four-times-reproduced environmental timeout.** Every leg reached a verdict
  and none failed to start. `check-fmt` (174 files unchanged, 198 Python
  formatted), `lint` (all six sub-legs reached: `cargo doc`, Clippy, Whitaker
  both packages, `ruff`, `pylint`, the df12 house lints at **100.0% passed**,
  `ambrleaks`, `yamllint`, `actionlint`), `typecheck` (30.12 s), `doc-coverage`
  (**98.87%** = 4977/5034), `markdownlint` (0 issues in 174 files — and this
  time the `spelling` prerequisite *succeeded* first, so `markdownlint-cli2`
  genuinely executed rather than inheriting an exit 2), `spelling`
  (`typos.toml` left unmodified), `nixie`, `doctest` (39 passed in the second
  target, 0 failed), `test-workflow-contracts` (1149 passed, 3 skipped),
  `test-documentation-contracts` (14 passed, 42 skipped), `proptest` (**90 run,
  3834 skipped** — reproducing the recorded selector census exactly, so the
  milestone's own new tests are selected and executing),
  `test-coverage-artifact` (52 passed). **`test-nextest` timed out.** This
  run's victim was `cli_configuration_fixture_compiles`
  (`tests/command_env_ui_tests.rs:70`, `TIMEOUT [300.028s]`), **not** the
  boundary test that failed pass #4. The gate's verdict for this revision is
  **red and is recorded as red** — an environmental attribution is a diagnosis,
  not a waiver.
- [x] (2026-10-14T00:00:00Z) **The timeout is now reproduced across four
  independent runs on two different tests, and the moving target is itself the
  evidence.** The signature is identical every time — exactly **573**
  `the managed sccache server is unreachable` lines, `sccache --show-stats`
  reporting `Compile requests 0`, a compile pool pinned at capacity 4/4, and
  **zero** source failures: `grep -c` for `test result: FAILED`, `panicked at`,
  and `assertion ... failed` across the whole 803 KB nextest log returns **0**.
  The decisive new datum is the sibling:
  `production_build_module_slice_has_expected_boundary` exhausted the 300 s cap
  in pass #4, **passed in pass #5 at 106.7 s**, and **passes in hosted CI at
  26.669 s**. The same test on the same fixture therefore spans 26.7 s → 106.7
  s → timeout depending only on the host's cache and queue state, a spread of
  more than an order of magnitude. Neither endpoint is a property of the test.
  Both victims belong to the `nested-cargo-builds` group (`max-threads = 1`),
  which *serializes* them behind every other child- Cargo test, so under
  contention the queue wait — not the test's own work — is what consumes the
  budget. **No widening of the allowance was performed, and none should be.**
  The Tolerances section pre-authorizes exactly this reading ("the 60-second
  warn is the real ceiling, not a soft one") and gives the reason that decides
  the matter: this milestone is a *determinism* suite, `.config/nextest.toml`
  SIGKILLs the process at 300 s, and **a killed process persists no regression
  seed**. Widening the budget to hide a host-contention artefact would destroy
  the counter-example artefact the milestone exists to produce. Fifteen of 3918
  tests were also cancelled after the timeout (`--no-fail-fast` unset), so
  their local verdicts are unknown for this revision; CI is the oracle that
  closes that gap, exactly as it did for pass #4.
- [x] (2026-10-14T00:00:00Z) **The oracle answered at the identical revision,
  and both victims pass there.** CI run `38507025014` (`CI`) at head
  `c517f6fd` — the *same* revision whose local `test-nextest` timed out —
  concluded **success**, all five jobs. Its `build-test` job (`115666757113`,
  03:35:42 → 03:56:57; log at `/tmp/ci-build-test-c517f6fd.log`) shows
  `cli_configuration_fixture_compiles` **PASS [66.986s]** and
  `production_build_module_slice_has_expected_boundary` **PASS [37.893s]**,
  against the 300 s cap one of them exhausted locally at that very revision.
  `Release Dry Run` (`38507024993`) also concluded success. So the local
  exhaustion is environmental by direct measurement, not by inference, and the
  refusal to widen the allowance is vindicated a second time. One detail cuts
  against complacency while supporting the disposition:
  `cli_configuration_fixture_compiles` is **itself `SLOW [> 60.000s]` in CI**
  at 66.986 s. The test is genuinely expensive everywhere; what differs is only
  the headroom the host has. CI leaves it 4.5x, a contended host with an
  unreachable cache leaves it none — so local timeouts on this fixture should
  be *expected* to recur, and CI stays the standing oracle rather than a
  one-time tiebreak. A direct probe recorded separately confirms the mechanism:
  given a 1500 s allowance the same test passes in **977 s wall**, split as 4 m
  28 s to compile the binary plus **709 s inside the test body**, where it
  spawns a child `cargo check` against a `target/.tmp…/` tree that is **cold on
  every run** and then queues behind 20+ peer crates for the same four-slot
  admission pool.
- [x] (2026-10-14T00:00:00Z) **A second CI run, at the tip, is green too — and
  the spread across three revisions is itself the finding.** Runs `38509361585`
  (`CI`), `38509361371` (`Release Dry Run`) and `38509361348`
  (`Netsukefile Build Test`) all concluded **success** at head `bc101f4c`. From
  `build-test` (job `115674067408`, log `/tmp/ci-build-test-bc101f4c.log`):

  | Test                                                  | `8fc7a2fa`   | `c517f6fd`   | `bc101f4c`   | best local           |
  | ----------------------------------------------------- | ------------ | ------------ | ------------ | -------------------- |
  | `cli_configuration_fixture_compiles`                  | —            | PASS 66.986s | PASS 36.103s | **TIMEOUT 300.028s** |
  | `production_build_module_slice_has_expected_boundary` | PASS 26.669s | PASS 37.893s | PASS 19.224s | TIMEOUT, then 106.7s |

  Four CI observations, four passes; best CI figure 19.224 s against a best
  local figure of a timeout. CI's own range on one test spans 19.2 s to 67.0 s,
  a factor of 3.5 between runs of the same code, which is why a single number
  for this fixture is meaningless and why the (compile, body, queue) split is
  the honest report. CI at the tip is green across every job including
  `Windows / build-test-windows`, so the oracle covers the current revision and
  not merely its predecessor.
- [x] (2026-10-14T00:00:00Z) **The Windows repair is CI-verified, at
      `c517f6fd`.**
  Both `CI` (`38507025014`) and `Release Dry Run` (`38507024993`) concluded
  success at that revision, including `Windows / build-test-windows` (job
  `115666757060`), which had **failed** on the pre-fix tip `8fc7a2fa` over this
  branch's own contract test. That is the single check no Linux gate could
  perform, and it now covers the fix.
- [ ] `EP-M2`: file splits, mutation-evidence contract, ordering helpers,
      `make proptest`. **Implementation complete and gate-verified as of
      `c517f6fd` (12/13 green, the thirteenth environmental); awaiting
      milestone CodeRabbit review and publication.**
- [x] (2026-10-14T00:00:00Z) **`EP-M3` implemented: the shared bounded graph
      strategy and the compact `Debug`.** Eight files under
      `src/ninja_gen/determinism/`, wired by one `#[cfg(test)] mod determinism;`
      line in `src/ninja_gen/mod.rs`:

  | File                                           | Purpose                            | Lines |
  | ---------------------------------------------- | ---------------------------------- | ----- |
  | `determinism/mod.rs`                           | module docs and wiring             | 21    |
  | `determinism/strategy/mod.rs`                  | re-exports and the divergence note | 18    |
  | `determinism/strategy/graph/mod.rs`            | `GraphSpec` and the strategy       | 354   |
  | `determinism/strategy/graph/blueprint.rs`      | drawn values resolved into a spec  | 210   |
  | `determinism/strategy/classification.rs`       | per-case shape counts              | 196   |
  | `determinism/property_tests/mod.rs`            | selector-segment rationale         | 12    |
  | `determinism/property_tests/strategy/mod.rs`   | scope line for the checks below    | 13    |
  | `determinism/property_tests/strategy/graph.rs` | the four properties                | 207   |

  Each is under the 400-line cap. `GraphSpec` holds a graph's parts unresolved
  so one specification materializes under several insertion orders, which is
  what makes an order-independence claim falsifiable rather than plausible.

- [x] (2026-10-14T00:00:00Z) **`EP-M3` acceptance, all three arms, with
      evidence.** The first arm is compilation; the second and third are
      measured below.

  *Arm 1 — the strategy compiles and is exercised by a smoke property.*
  `cargo check --lib --all-features --tests` is clean (0 errors, 0 warnings
  after two self-caught fixes: a `From`-in-`const fn` call and an `as_str`
  method-path).
  `cargo nextest run --lib --all-features -E 'test(/determinism/)'` → **4 of 4
  pass**, 0.160 s, log
  `/tmp/test-netsuke-4-3-1-proptests-for-deterministic-ninja-emission.out`.

  *Arm 2 — classification counts reproduce `EP-M0` question 1.* Reproduced, and
  the sharpest evidence is a **single** case's printed diagnostic, taken from
  the deliberate-failure probe below:

  ```plaintext
  classes: Classification { actions: 38, edges: 73, explicit_outputs: 148,
  implicit_outputs: 20, multi_output_edges: 38, phony_edges: 50,
  dyndep_staged_edges: 8, always_edges: 27, phony_flag_edges: 31,
  restat_actions: 19, default_targets: 6, distinct_default_targets: 6,
  always_restat_seen: 15 }
  ```

  `always_restat_seen: 15` is `0b1111`: all four combinations of `edge.always`
  and `action.restat` occurred in **one** case, which is `OBL-ORDER`'s
  four-combination non-vacuity requirement met by construction rather than by
  hope. `dyndep_staged_edges: 8` is non-zero, so the `edge_requires_gates` path
  is exercised. The run-wide tally additionally asserts
  `intersection_cases > 0`: **some case carried multi-output edges, phony
  edges, dyndep-staged edges, and non-empty defaults together**, which is the
  intersection `OBL-ORDER` names and which no marginal tally could show.

  *Arm 3 — a deliberately failed assertion prints a counter-example that fits
  on a screen.* Discharged by a temporary probe property (since removed), whose
  rendered counter-example measured **363 characters** against
  `MAX_DEBUG_CHARS = 4_000`:

  ```plaintext
  minimal failing input: spec = GraphSpec { digest: aea438d0c0c2385b, classes:
  Classification { actions: 38, edges: 73, explicit_outputs: 148, ... } }
  ```

  Proptest renders it through `{:#?}` (`TestError`'s `Display` uses the
  alternate marker), so the `Debug` impl deliberately ignores the alternate
  flag and writes one line regardless — measured, not assumed. The probe's
  `proptest-regressions/ninja_gen/...graph.txt` artefact was **deleted** rather
  than committed: it records a deliberate failure for a test that does not
  exist in the committed tree, and committing it would replay a fabricated
  failure forever.

- [x] (2026-10-14T00:00:00Z) **`EP-M3`'s selector question is proved by
      evaluation, not asserted.** `make proptest` selects suites by a module
      segment, and the plan warns that a selector matching nothing still exits
      0. Measured by reading `filter-match.status` from the real target's filter
      expression (`Makefile` `PROPTEST_FILTER`, `$$` intact):
      **selected 75, unselected 1400**, and all four new tests report `matches`:

  ```plaintext
  matches  ninja_gen::determinism::property_tests::strategy::graph::generated_specs_stay_within_the_roadmap_bounds
  matches  ninja_gen::determinism::property_tests::strategy::graph::the_compact_debug_stays_within_a_screen
  matches  ninja_gen::determinism::property_tests::strategy::graph::the_generator_reaches_every_required_class_across_a_run
  matches  ninja_gen::determinism::property_tests::strategy::graph::the_two_orders_preserve_the_specification_while_differing
  ```

  The `property_tests` path segment is therefore load-bearing, exactly as
  `determinism/property_tests/mod.rs` records: a suite declared outside one of
  those two segments is never run by `make proptest` at all. `make proptest`
  additionally passes `--no-tests fail`, so an empty selection is a hard
  failure rather than a silent pass.

- [x] (2026-10-15) **The first `EP-M3` gate run was red on two deterministic
      gates, and both repairs are now green.** `make check-fmt` reported 8
      `cargo fmt` diffs across the four new files; `make lint` aborted at
      `lint-clippy` with three `-D warnings` errors, all in the new tree:
  `excessive-nesting` at `strategy/graph/mod.rs:195` (the `map_err` closure sat
  at nesting depth 5 inside `for` + `if let`), and `shadow_reuse` for `left` and
  `right` at `classification.rs:165` (closure parameters shadowing the
  enclosing `edge_lists_differ`'s own parameters of those names).

  The nesting repair extracts the closure to a free function `insert_pool_edge`
  in the same module, dropping the depth to 3. That extraction **introduced a
  type error the stop hook caught**: `insert_edge` returns
  `Result<EdgeId, IrGenError>`, not `Result<(), _>`, and the inline `?` had
  been discarding the identifier implicitly, so a helper declaring
  `Result<(), _>` did not compile. The resolution keeps the proven
  `map_err(..)?` form and adds an explicit `Ok(())`, documenting that the arena
  owns the inserted edge so its identifier is unneeded. A first attempt using
  `.map(|_edge_id| ())` was rejected by inspection before running:
  `clippy::result_map_unit_fn` denies that shape.

  Both repairs are validated, first narrowly and then at full-gate scope.
  Focused
  `cargo clippy -p netsuke-build --lib --tests --all-features -- -D warnings`
  exits 0 with no diagnostics, and
  `cargo nextest run --lib --all-features -E 'test(/determinism/)'` is 4/4 PASS
  in 0.168 s.

  The full re-run is green at `429720b2`, reading the gate logs directly:

  ```plaintext
  === gate-end check-fmt rc=0 at 2026-10-15T02:05:01+02:00 ===
  === gate-end lint rc=0 at 2026-10-15T02:54:27+02:00 ===
  ```

  All four `lint` sub-targets actually executed this time, which is the point
  of interest given the finding below: `cargo doc` and `cargo clippy` (lines
  3/478), both Whitaker invocations — `--package netsuke-build` (line 1645) and
  the crate-scoped `--package test_support` pass run from `test_support/` (line
  2914) — the Python cascade (lines 3628-3640), and `yamllint`/`actionlint`
  (lines 3643-3644). The `lint` run took 49m 8s, essentially all of it queued
  on the shared compile pool.

- [x] (2026-10-15) **The repaired tree is green on both deterministic gates.**
      See the entry above for the evidence and revision. The remaining
      `EP-M3` item is the milestone CodeRabbit review, which is unblocked: no
      valid deterministic concern remains unresolved. `make test`'s
      `cli_configuration_fixture_compiles` timeout is an environmental
      question recorded under Surprises & discoveries, not a milestone defect.

- [x] (2026-10-15) **The `EP-M3` milestone CodeRabbit review completed,
      after eight attempts, and returned exactly one finding.** The first
      seven attempts failed service-side with zero findings; this one was
      admitted and completed in 71.9 s over the three milestone commits
      (`1ff0c1e1`, `429720b2`, `2b01a3e3`) against the milestone's real base
      `7f3b13c3`, with `head` and `comparisonBaseCommitId` matching exactly and
      all ten milestone paths in `reviewedFiles`. Admissibility is evidenced by
      the usage counter moving 7/10 → 6/10, not by the tool's self-description.
      The finding was a module-doc accuracy defect, verified and fixed (see
      below); no rate limit was encountered.

  ```plaintext
  ~/.coderabbit/…/review.jsonl:
    {"type":"complete","status":"review_completed","findings":1,
     "outcome":"completed","message":"Review completed"}
  ```

- [x] (2026-10-15) **The one valid concern is resolved.** The finding read
      "the doc says 'Five claims are checked here'. The file defines four
      tests", with the alternative "state that the round-trip claim and the
      divergence claim share one property". All three of its premises were
      re-verified against the tree rather than taken on trust, and the
      *alternative* was adopted because it is the accurate repair: the
      count of claims was never wrong, the *implication* that claims map
      one-to-one onto properties was. `graph.rs`'s module doc now reads "Four
      properties are defined here, between them checking five claims" and
      names the reason the two share a property. The plan's own file table at
      the `EP-M3` entry already described the file as "the four properties",
      so the plan and the doc now agree.

- [ ] (2026-10-16) **`make typecheck` on the `EP-M4` source: red, and the
      refusal count is exactly the integration-test set.** The run compiled
      the library (`Compiling netsuke-build …` present) and then died with
      `EXIT=2`, `make: *** [Makefile:423: typecheck] Error 101`. Every one of
      the 18 errors is the same cascade shape — `could not compile
      'netsuke-build' (test "…")` / `Caused by: process didn't exit
      successfully: '…/build-limits-rustc …'` — and **each traces to
      `build-limits: the supervisor refused: unhealthy`**, 18 refusals for 18
      dead targets. There are **zero** rustc diagnostics in the log:
      `error[E…]` 0, `cannot find` 0. This is a refusal, not a defect in the
      code, and the first refusal lands at 21:41 — minutes before the
      supervisor's 21:42:31 restart, so the build was cut down by the tail of
      the outage rather than by anything in the tree.
      **The 18 refused targets are all `tests/*.rs` integration tests**
      (`bdd_tests`, `ninja_gen_unit_tests`, `runner_tests`, …). The library's
      **own** unit-test harness — `--crate-name netsuke`, the name from
      `[lib] name = "netsuke"`, which is where the four `#[cfg(test)]` files
      actually compile — is **not among them**, and `netsuke_build` appears
      nowhere in the log. So a refusal-only failure of `typecheck` cannot be
      read as evidence for or against the `EP-M4` source: the gate never got
      as far as judging it. This is the fifth consecutive run to die before
      reaching the code, and the reason the ceiling is now being lowered — a
      `--lib` probe judges the same four files without dragging 105
      integration targets behind it.
      **A second, separate anomaly: `target/debug/deps/` and `.fingerprint/`
      are still absent**, at zero entries, immediately after a 241-crate
      compile that produced `incremental/` entries stamped 21:41. `target/debug/`'s
      own mtime is frozen at 19:55, which is inconsistent with a child
      directory having been added or removed after that time. Investigated and
      **deliberately left with cause unestablished**: `rust-cleanup` is
      excluded by construction — it deletes whole target directories and has
      no selective-deletion code — so the previously recorded attribution to
      it is withdrawn in `Surprises & discoveries`. Recorded rather than
      explained, because a wrong mechanism is worse than none.

- [x] (2026-10-16) **`EP-M4`'s four files and both mutation patches are now
      tracked, after an untracked-file defect that would have turned the
      compile gate red for a reason unrelated to any patch.** The three new
      modules (`no_loss.rs`, `path_key.rs`, `support.rs`) and the
      `MUT-EDGEDROP` patch were untracked; the plan had recorded them as
      written but not that Git did not hold them. The compile guard compiles a
      **sandbox**, not the checkout it runs in: `every_patched_tree_compiles_under_denied_warnings`
      (`compile_guard.rs:334`) builds it through `export_revision` (`:161`)
      from `capture_revision` (`:185`), which runs `git stash create` — a
      snapshot of tracked content only. This was a guaranteed red gate, not a
      hypothetical: captured revision `a6e93539` held exactly three files under
      `src/ninja_gen/determinism/property_tests/` (`mod.rs`,
      `strategy/graph.rs`, `strategy/mod.rs`) while its own captured `mod.rs`
      declared `mod no_loss;`, `mod path_key;`, and `mod support;` at lines 16,
      18, and 22. Three declared modules with no source file means the sandbox
      cannot build — and because the guard defers to rustc, that surfaces as
      every patch failing to compile, misattributing the fault to all fourteen
      patches instead of to the three missing files. The `MUT-EDGEDROP` patch
      was caught more directly by `ensure_no_untracked_patches` (`:291`),
      which lists `git ls-files --others --exclude-standard` under the
      mutations directory and fails on a non-empty result. Both states are
      cleared and the fix verified by **re-capturing the revision** rather
      than assuming: `9b7f0ba3` lists all six `property_tests` files, and the
      untracked list over the mutations directory prints nothing. Landed as
      `ff39e05f` (files and patch) and `f9f3c910` (module declarations and the
      patch registry), split so neither commit tracks a patch whose target is
      absent nor a module whose patch is untracked.
      **This is a tracking fix, not a compilation result.** The four files
      still have never been compiled for a target that sets `cfg(test)`.

- [x] (2026-10-16) **`MUT-EDGEDROP`'s reachability settled statically: the
      mutation kills every edge, so it is certain to break its named
      property.** The trace runs `emit` → `generate_bundle`
      (`src/ninja_gen/mod.rs`) → `generate_bundle_inner` (`dyndep/mod.rs:117`)
      → `render_edges` (`:136`). The patch inserts
      `if !edge.explicit_outputs.is_empty() { continue; }` at the head of the
      loop body, which is reachable whenever the graph has an edge — and
      `arb_graph_spec` draws `MIN_EDGES = 5` (`strategy/graph/mod.rs:51`) as a
      floor, so every generated graph has at least five. Nothing constrains a
      graph's edges to carry empty `explicit_outputs`: the property's own
      `declared_outputs` (`no_loss.rs:204`) flat-maps
      `edge.explicit_outputs.iter().chain(&edge.implicit_outputs)`, expecting
      those slices populated. So the mutation emits no `build` line at all,
      `lhs_counts` returns an empty map, `emitted.get(output).copied().unwrap_or(0)`
      reads `0`, and `prop_assert_eq!(0, 1)` fails on every output of every
      case. A mutation that kills every input is not a mutation to an
      unreachable path — the concern that would make it dead evidence, which
      is the real risk for a seeded fault, does not apply here. This is
      distinct from the compile defect repaired in `5e973841`: that made the
      patch unbuildable, this settles that the (now buildable) patch is also
      effective. **What it still is not:** a run. Whether
      `make proptest` fails for this reason is unobserved, and no static
      argument substitutes for observing the named property fail.

- [x] (2026-10-16) **`path_key` confirmed live in production, which
      `MUT-PATHKEY`
      depends on.** `src/ninja_gen/order.rs:23` sorts edges by
      `path_key(&edge.explicit_outputs)` and imports it at `:9`, and
      `path_key` is `pub(crate) fn` at `src/ninja_gen/mod.rs:226`. So the
      function under mutation is on the emission path rather than dead code
      behind a test-only facade, and the reachability that `MUT-PATHKEY` needs
      is established independently of the property's own call. Recorded
      because a mutation to a function whose only caller is a test measures the
      test rather than the code.

- [x] (2026-10-16) **`MUT-PATHKEY`'s detection narrowed: the run that catches it
      is `the_constructed_permutation_is_genuinely_perturbed`, not the sampled
      property — which matters because the acceptance criterion names the
      sampled property.** The plant eliminates the sort, so
      `path_key(["out/a","out/b"])` becomes
      `"out/a\0out/b"` where the unmutated `"out/a\0out/b"` is also what the
      sorted form yields — the mutation is invisible unless a case's two lists
      *differ in order* yet are permutations. `prop_shuffle` may return the
      input order, and `ListShapes.reordered` (`path_key.rs:173`) records
      whether a case was genuinely perturbed as a **classification**, never an
      assertion; the plan already says so at the `OBL-PATHKEY` entry
      (*Note on `prop_shuffle`*), and `covered!` asserts five fields while
      `reordered` and `unchanged` stay unasserted by design. So the sampled
      `path_key_is_permutation_invariant` catches the plant only if some case
      reorders — unobserved, and not guaranteed. The directed test does catch
      it with certainty: its witness is the repeated-path pair
      `["out/a","out/a","out/b"]` versus `["out/b","out/a","out/a"]`
      (`:255-261`), which sorts to one key but, with the sort deleted, joins in
      two different orders to two different keys, failing the
      `assert_eq!` at `:271` whether or not any sampled case reordered.
      The acceptance criterion nonetheless asks that
      `path_key_is_permutation_invariant` fail, and a run where no case
      reorders would fail it only through `prop_shuffle`'s luck. This is worth
      knowing before the run rather than after: if the property passes under
      the plant while the directed test fails, the honest reading is that the
      *suite* detected the fault and that the sampled property's detection is
      probabilistic — not that `MUT-PATHKEY` is unsound. Raising `cases` would
      not fix it, since the issue is the per-case reorder classification
      rather than the sample size.

- [x] (2026-10-17) **The `Scope` tolerance was breached and then widened, by
      user direction, to 56 files and 6,000 net added lines.** The entry below
      this one recorded the original breach on 2026-10-16 — 2,597 net added
      lines against a cap of 2,600, with `EP-M5`–`EP-M8` unwritten — and
      proposed three options. The user answered on 2026-10-17 with "Widen the
      cap". The derivation of the new figure, including the counted inventory
      of what the remaining milestones still name, is in `Decision log`. The
      figure remains measured against merge base `e1df2ead` and excludes
      `docs/execplans/`, which resolves the inside-or-outside ambiguity the
      original entry could not.

- [x] (2026-10-16) **`BLOCKED`: the `Scope` tolerance had no room left for
      `EP-M5`–`EP-M8`.** Superseded by the entry above; retained because the
      breach, and not only its resolution, is part of the plan's history.
      Measured at `8a213218` against merge base `e1df2ead` (also the hook's
      diff base): 2,597 net added lines excluding this ExecPlan, against a cap
      of 2,600 — three lines of headroom for four milestones. 28 of 34 files,
      so the file axis was not the constraint. The `EP-M3` and `EP-M4` suites
      are ~69% of the budget as it then stood.

- [x] `EP-M4`: `OBL-PATHKEY`, `OBL-NOLOSS`.
- [x] `EP-M5`: `OBL-ORDER`, `OBL-DEFAULT`, `OBL-NOHASH`.
- [ ] `EP-M6`: `OBL-ACTION`, `OBL-E2E`, `OBL-PROCESS`.
- [ ] `EP-M7`: `OBL-DUP`, `OBL-CYCLE`.
- [ ] `EP-M8`: `OBL-NINJA`, documentation, evidence sweep, roadmap.

## Surprises & discoveries

- (2026-10-17) **The sort-deletion mutations the table describes do not
  compile under the gates, so each was re-expressed as a fault of the same
  class that does.** This is the `MUT-PATHKEY` defect class again, found for
  three more patches before running any of them. Deleting `edges.sort_by_key`
  strands `use super::path_key` and leaves `path_key` unused, and the tree fails
  `cargo check` with two `unused` errors under `-D warnings`; deleting the
  `actions` sort strands `let mut`, which is `unused_mut`; deleting `.sorted()`
  strands `use itertools::Itertools`, its only use in `order.rs`. Each literal
  patch would have been dead evidence, exactly as `MUT-PATHKEY` was before
  `5e973841`. The repairs stay faults a reviewer would recognize as realistic
  while keeping every item live: `MUT-EDGESORT` now keys on `implicit_outputs`
  instead of `explicit_outputs`, so the sort still runs but no longer imposes a
  total order over the emitted keys; `MUT-ACTIONSORT` leaves
  `let actions: Vec<_> = actions.collect();`; `MUT-DEFSORT` removes the import
  in the same hunk. All four patched trees then passed
  `cargo check --lib --all-features` under the gate flags, which is the
  observation that separates a live mutation from a dead one. The table's
  wording still describes the literal deletions; it is the *fault class* that
  is authoritative, and the patch files are the artefact.

- (2026-10-17) **A mutation run persists proptest regression seeds unless
  persistence is switched off.** The first `MUT-EDGESORT` run left
  `proptest-regressions/ninja_gen/determinism/property_tests/order.txt` holding
  two shrunk seeds, because the suite runs under
  `PROPTEST_DISABLE_FAILURE_PERSISTENCE` unset. That file records the
  *injected* fault rather than a real regression, so committing it would pin a
  seed that fails only while a patch is applied, and leaving it in the tree
  would make the next clean run replay it. Every subsequent run set
  `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1`; the directory was removed after the
  first run and did not reappear. Any future mutation run over a proptest suite
  needs the same setting, and this is worth stating because the flag is read
  from the environment rather than the config the suite sets.

- (2026-10-17) **The timeout was the child `cargo check` being cold, not the
  fixture being expensive, and the rebase did not fix it.** The plan had
  expected `origin/main`'s repair there to clear the local
  `cli_configuration_fixture_compiles` failure. It did not: after a clean
  rebase onto `077cf366` (zero conflicts, one already-upstream commit dropped),
  the test still reported `TIMEOUT [300.024s]` on `case_1_public_api`.
  Measuring the child command directly explained both facts.
  `NetsukeRlib::build()` runs
  `cargo check --manifest-path <ws>/Cargo.toml --workspace --all-targets
  --all-features`
  in the ambient `target/`, and in this tree that took **455 s wall clock,
  exit 0**, with 36 `[build-limits]` queue-wait lines reaching "12 ahead". The
  same command immediately afterwards took **1 s** ("Finished in 0.51s"). So
  the cost is real, one-off, and paid by whichever fixture runs first. Three
  facts explain why the tree is always cold when a gate starts: no warm path
  prepares that graph; `cargo nextest run` prepares via `cargo test --no-run`,
  whose codegen and `--test` metadata a check-mode `.rmeta` graph does not
  satisfy; and `make typecheck` runs `cargo check` **without** `--workspace`,
  so it warms a different selection. The tree examined had
  `target/debug/.fingerprint` and `target/debug/deps` both empty while 2,520
  `.rmeta` files survived, which is the half-cleaned shape a partial
  `rust-cleanup` leaves. **The instructive part is that the earlier
  dispensation reasoned from the fixture's *selection* rather than its *cache
  state*.** "It reuses the gate graph" was true about flags and false about
  warm artefacts, and the two are different claims. `rust-cleanup.timer` is not
  the cause of the empty caches — its journal for the period shows
  `"deleted": 0` throughout — so it was not touched.

- (2026-10-17) **A `make proptest` run left no evidence it had ever started, and
  that absence was the only evidence it had died.** The log at
  `/tmp/proptest-netsuke-4-3-1-proptests-for-deterministic-ninja-emission.out`
  is 514 bytes with an mtime of 2026-10-16 21:03:50, and its whole content is
  the init banner naming head `dc1012eb`, two `build-tools:` lines, and the
  cargo command line. There is no compile output of any kind — not a
  `[build-limits]` queue line, not a `Compiling` line, not an error. No `make`,
  `cargo`, `cargo nextest`, `build-limits-rustc`, or `rustc` process exists.
  The instructive part is that this is indistinguishable, from the log alone,
  from a run that is merely early; what distinguishes them is staleness of the
  mtime plus an empty process table, and the check that settles it is the
  process table, not the log. A second surprise follows from the first: the run
  was launched at `dc1012eb`, five commits behind the then-current `f8b2d017`,
  so even a successful run would have measured the wrong tree. And the
  conclusion the plan had drawn from the surrounding failures — that the
  compile pool was refusing every admission, recorded in the `Status` line —
  was false by the time it was written. `build-limits-status` on 2026-10-17
  returns
  `capacity 4, held 0, healthy, sccache unreachable, compiling directly`, no
  requests waiting, no compiler groups, with an empty process table. **The pool
  was never the thing standing in the way of a run; the scope tolerance is.** A
  host-health claim in a living document is a measurement with a timestamp, not
  a standing fact, and this one had been copied forward past the point where it
  was true.

- (2026-10-16) **A gate that compiles an isolated copy of the tree cannot see
  work that Git does not hold, and says so in the wrong voice.** `EP-M4`'s
  three new modules and the `MUT-EDGEDROP` patch sat untracked while the plan
  recorded them as written, which is true of the files on disk and false of
  anything the compile gate compiles. The guard's sandbox is built from
  `capture_revision` (`sandbox.rs:185`), a `git stash create` snapshot of
  tracked content only, so an untracked file is not merely uncommitted — it is
  absent from the universe the gate examines. The failure mode is the
  interesting part: a sandbox missing three of its declared modules cannot
  build at all, and the guard reports a build failure as **every** patch
  failing to compile, pointing at all fourteen patches instead of at the three
  files that are not there. The repository anticipates the narrower case and
  not this one: `ensure_no_untracked_patches` (`compile_guard.rs:291`) refuses
  an untracked **patch** with a message that names the mechanism exactly — the
  sandbox "is exported from the tracked revision and cannot carry an untracked
  file" — but nothing checks the **sources** the patches target. That asymmetry
  is worth remembering: the guard was written by someone who had reasoned
  carefully about the patch list and had not asked the same question of the
  module tree. `git status` shows an untracked file; it does not show that a
  gate's input set excludes it. The check that settles it is to ask the gate's
  own capture what it holds — `git stash create`, then
  `git ls-tree -r --name-only <rev> <path>` — which is one command and read the
  captured tree directly rather than reasoning about what `git stash create`
  ought to include.
- (2026-10-16) **A narrowed probe can be silent about the exact code it was
  built to judge, while producing output that reads like a pass.** Five compile
  attempts on `EP-M4` were made with `cargo check --lib --all-features`, on the
  reasoning that a shorter dependency list reaches these four files sooner than
  the 105-target `make typecheck`. It reaches them **not at all**.
  `property_tests` and `strategy` are declared `#[cfg(test)]`
  (`src/ninja_gen/determinism/mod.rs:17`, `:20`), and `cargo check --lib` does
  not set `cfg(test)`. The flag most naturally read as "check this package's
  library, which is where the code lives" excludes every line of it. The check
  that settles it is intrinsic, not circumstantial: both completed probe builds
  left a library rmeta, and each contains `BuildGraph` — a non-test item — **13
  times** and `arb_graph_spec`, a `cfg(test)`-only item in the same module tree
  as the four files, **0 times**
  (`target/debug/incremental/netsuke-0m5t1t8w250tg/…/metadata.rmeta`,
  `…/netsuke-3e9x4qeorcsj4/…/metadata.rmeta`). The paired control rules out
  "the symbol table is simply unreadable": 13 other symbols come back from the
  same file. A probe that leaves a `cfg(test)`-only symbol at zero has not
  compiled `cfg(test)` code, whatever else it reports. This is the trap worth
  carrying forward. The probes were honest — they really did compile 75 crates
  with no refusal and no diagnostic — and that is exactly what made them
  convincing. **A probe whose scope misses the target reports the same success
  as one that covers it**, so the scope has to be evidenced from the artefact
  rather than assumed from the flag. Here that evidence was one `strings` call
  away the whole time, and the probe agent asked the right question before
  being stopped: it refused to accept its own exit 0 until it could name what
  was in the unit.
- (2026-10-16) **`build-limits-status` timing out is specific to the query,
  not to the socket.** The same command timed out repeatedly while
  `/run/user/1000/buildlimits/supervisor.sock` accepted other clients on the
  same path within seconds. Reading "timed out" as "the pool is down" would
  have written off a machine on which requests were still being admitted. The
  user manager's bus was also caught mid-recovery: at 22:56 `systemctl --user`
  and `busctl --user list` both failed with
  `Transport endpoint is not connected`, and by 23:03 `busctl --user list`
  answered normally. Both readings are true of different moments, and neither
  is a property of the supervisor.
- (2026-10-16) **An attribution of the missing-`deps/` anomaly to
  `rust-cleanup` was recorded earlier in this session and is now withdrawn.**
  The earlier reading was that `rust-cleanup.service` deletes
  `target/debug/deps` and `target/debug/.fingerprint` while leaving `build/` in
  place. Read against the installed package
  (`/home/leynos/.local/share/uv/tools/rust-cleanup/lib/python3.14/site-packages/rust_cleanup/`),
  that is impossible: `reclamation.py:128` `_delete_candidate` calls
  `self.delete_directory(candidate.path)`, and `filesystem.py:217`
  `delete_directory` is documented as "Delete a **directory tree**" — it is
  all-or-nothing on the candidate. `grep -rnE 'deps|fingerprint|incremental'`
  over the package returns **nothing**, so no selective-deletion logic exists
  to produce a half-cleaned target. The journal reports the unit explicitly as
  a directory count: `scanned=46 protected=30 … eligible=1 deleted=1`, then
  `Deleted 1 of 46 target directories`. **The observation stands; the mechanism
  does not.** The anomaly is left recorded with its cause unestablished rather
  than reassigned to a fresh guess — the withdrawal is the finding. Recorded
  here because a wrong mechanism is worse than none: it would send the next
  reader to reconfigure or escalate against host infrastructure protecting a
  shared 1.8T volume across ~46 worktrees, for a fault it demonstrably did not
  cause. A second, independent reason the sweep is a poor suspect:
  `has_recent_files` (`discovery.py:268`) protects any target whose newest
  non-marker file is newer than the cutoff, and treats an *uncertain*
  inspection as recent — so a target being actively written by a live build is
  normally protected.
- (2026-10-16) **A gate that fails for infrastructure reasons still produces
  output shaped exactly like an output that failed for code reasons, and the
  difference is only visible by reading the `Caused by:` lines.**
  `make typecheck` exited 101 and printed 18 `error: could not compile` lines.
  Read at the level a busy human reads them, that is "18 broken crates". Read
  one level down, every one is a third-party crate whose compiler process was
  killed, and the killer names itself:
  `build-limits: the supervisor refused: unhealthy`, 18 times, extracted from
  the `Caused by: process didn't exit successfully: …/build-limits-rustc …`
  line. The wrapper's refusal is the error; the crate names are casualties.
  **The transferable lesson is to classify a gate failure by the failure's
  *cause line* before acting on it, and specifically to check whether the
  failing crate is one this repository owns.** Had this been read as "18 crates
  are broken", the next hour would have gone into investigating `toml` and
  `tempfile`. A second trap sits on top of the first: the background-task
  notification for this run reported `completed (exit code 0)`. That status is
  the wrapper's, not `make`'s. The log's own last line —
  `make: *** [Makefile:423: typecheck] Error 101` — is the gate's verdict, and
  it is the one to trust. **A notification's exit status and the gate's exit
  status are different quantities whenever a pipe, `tee`, or shell wrapper sits
  between them.** Both of these are cheap to check and expensive not to.
- (2026-10-16) **A supervisor recovering from a transient fault was read as a
  supervisor that would never recover, and that conclusion was written into
  this plan, a Risk entry, and an escalation before it was checked.** Three
  probes over three minutes showed `capacity 4, held 3, UNHEALTHY`,
  `deactivating` / `stop-sigterm`, and status text that named operator action —
  *"stop sccache and leave it stopped until admission confirms the empty
  drain"*. The reading was that the pool was wedged and needed a human. **The
  pool recovered by itself at 20:28:14, during the probe window, while the
  probes were still reporting the pre-recovery state.** The journal has the
  completion the status command never showed:
  `sccache service verified stopped and empty; drain tokens may return`, then a
  clean stop/start and a new PID. Six minutes later the same command read
  `healthy` with a build queued behind a token. **The status text was not
  lying; it was describing work in progress in the grammar of work that had
  failed.** `drain check failed` reads as a terminal verdict and is actually
  the name of a check that is still running. The disambiguating evidence was
  one command away and was not run: the supervisor's PID. `NRestarts=0` plus a
  *changed* PID means the service restarted through a path systemd's counter
  does not record; `NRestarts=0` plus an *unchanged* PID is the terminal case.
  **The compounding error was carrying a detail forward from the previous
  outage.** The earlier wedge ended in `ExecMainStatus=2`, and the reading was
  that `RestartPreventExitStatus=2` meant this one would not restart either.
  But this outage never reached exit status 2 — it was a `stop-sigterm`
  deactivation, a different mechanism that the setting does not govern. **A
  diagnostic detail that explained one occurrence of a recurring fault is not
  automatically evidence about the next one, and reusing it without re-checking
  made a transient condition look permanent.** **The transferable lesson is to
  establish that a fault is *terminal* before escalating it, and to prefer a
  liveness signal (has the PID changed, has the start timestamp moved) over
  state words that read the same in both cases.** Three probes sampled one
  moment three times; they were not three independent observations, they were
  one observation with a delay. Had the escalation been acted on, an operator
  would have stopped sccache — a shared service — during other agents' active
  builds, to fix a fault that was already fixing itself. A related detail worth
  keeping: `buildlimits-token-watchdog.service` is `active` and only ever logs
  `probe_failed` and `no_stall`. It reports the condition; it does not repair
  it. So its being green is not evidence that anything will self-heal — but
  neither is its logging `probe_failed` evidence that nothing will.
- (2026-10-16) **A user-space `ENOSPC` appeared during this window and is
  inotify watch exhaustion, not disk.** Lody logged
  `root watch failed code=ENOSPC` and systemd logged
  `Failed to add control inotify watch descriptor … No space left on device` for
  `rust-cleanup.service`. The filesystem is fine: `/` has 288 GB available at
  83% used, well clear of the stop-work threshold. The exhausted resource is
  `fs.inotify.max_user_instances` (currently 512, with 153 instances in use).
  Recorded because `ENOSPC` reads as "the disk is full" and this plan's
  standing instruction is to stop and report if that happens; it has not
  happened, and the message is a different resource with the same errno.
  `rust-cleanup.service` is also running on a repeating schedule against lody
  worktrees — it reported `deleted=0, eligible=0` on its 20:31 pass, so it is
  not currently deleting anything in this worktree.
- (2026-10-15) **The mutation `OBL-NOLOSS` was designed against could not
  fail it, and the test its acceptance names asserted the claim the statement
  correction had just removed.** Correcting the count conjunct above left two
  things behind. The first was `MUT-EDGEDROP`'s trigger: it skipped an arena
  entry whose `explicit_outputs` is *empty*, but `blueprint.rs:119` gives every
  generated edge `explicit_count >= 1` (`share >= 1` at `:60-75`, then
  `(1 + plan.extra_outputs).min(share)`), so the generator cannot reach that
  trigger at all. The one directed case that does have no explicit outputs
  renders `build : <hash>` — an empty left-hand side — so skipping it leaves
  the emitted *set* of outputs identical and the property cannot fail. The
  trigger now drops an edge that *carries* outputs, which is the failure mode
  the justification paragraph had described all along. The second was the
  accepted test name `every_distinct_edge_is_emitted_once`, an edge-*count*
  claim — precisely the assertion the correction removed — carried in the
  `EP-M4` acceptance line and in a trace link, so a checked box would have
  cited a test whose name promised semantics the plan had just disproved. It is
  renamed `every_explicit_output_appears_exactly_once`, which is literally true
  on the generated domain: `allocate_pool` (`blueprint.rs:169-174`) names slots
  `out/{slot:04}` and `build_edges` advances one monotone cursor, so every
  emitted output name is globally unique. Four restatements of the old count
  also survived the correction, two of them inconsistent with it. **The
  transferable lesson is that a corrected statement does not correct the
  mutation or the test name built on it.** A mutation's trigger is a claim
  about reachability, and it needs the same against-source check as the
  statement.
- (2026-10-16) **A file that has never been compiled carries defects no
  gate has seen, and the admission outage turned that into a concrete compile
  error.** The admission blocker above meant `path_key.rs` had never reached
  `rustc`. Reading it against the vendored `camino` source found
  `.map(Utf8PathBuf::as_str)`: `as_str` is inherent on `Utf8Path`
  (`camino-1.2.6/src/lib.rs:796`, inside `impl Utf8Path` at `:660`) and
  `Utf8PathBuf` reaches it only through `Deref` (`:591-597`). Method-call
  syntax auto-derefs, but a *function path* does not, and no trait supplies
  `as_str`, so the expression names an associated function that does not exist.
  It is now a closure over the deref, matching the form this repository already
  compiles at `src/ninja_gen/property_tests/mod.rs:213`, applied to the same
  receiver type. **The transferable lesson is that when an outage blocks the
  compiler, reading production and dependency source is not a substitute for it
  but is not idle either: it can still falsify an API assumption, and the ones
  worth checking first are the calls whose receiver does not obviously own the
  method.** Everything else the four files depend on was verified the same way
  and held: `proptest` 1.11.0's prelude re-exports, `TestRunner::new`/`run`
  signatures, `From<String> for Reason` (so `TestCaseError::fail(format!(…))`
  resolves), `prop_assert_eq!`'s trailing-comma arm, `RangeInclusive<usize>` to
  `SizeRange`, `Vec<T>: Shuffleable`, and the `proptest!` body's
  `let (): () = $body;` binding that admits `?` early-returns. The same sweep
  cleared every other type-qualified call in the new files.

- (2026-10-15) **A red `lint-clippy` silently removes all evidence for the
  three lint targets after it.** `make lint` is declared
  `lint: lint-clippy lint-whitaker lint-python github-actions-lint`, and a
  failure in the first prerequisite aborts the whole target. So the previous
  `EP-M3` gate run left `lint-whitaker`, `lint-python` and
  `github-actions-lint` with **no evidence at any revision**, not merely a
  stale one. This was confirmed from the log rather than inferred: the only
  occurrence of the string `whitaker` in `/tmp/lint-…-4-3-1-…out` is
  `[build-limits] whitaker_boundary_contract: …`, which is a *crate being
  compiled*, not the Dylint suite executing. The consequence matters here
  because `module_max_lines` — the Whitaker rule that forced the `graph.rs`
  split recorded below — had therefore **never actually run against this tree**
  at the time of that finding; the `graph/mod.rs` line count was re-measured at
  364 (of 400) to keep the assumption unstrained. The `429720b2` re-run closed
  this: `lint` is rc=0 and both Whitaker passes executed, so the cap is now a
  measured pass rather than an assumption. The general lesson stands, though —
  a claim that "the 400-line cap is satisfied" must cite a `lint-whitaker`
  execution, never the file measurement alone.

  The `429720b2` run also settled the obvious follow-up question, because
  "reported nothing" is only a pass if the rule was *loaded*. Positively
  confirmed by listing the suite's lints rather than by inferring from silence:

  ```plaintext
  $ whitaker list --lib whitaker_suite
  whitaker_suite
      module_max_lines  warn  modules should stay within the configured maximum line count
  ```

  `module_max_lines` is present at `warn` severity, and the gate denies
  warnings, so a violation would have failed the run. The cap is verified.

- (2026-10-15) **Never dispatch a gate run against a tree that has not
  compiled.** A `make lint` was launched against a working tree carrying the
  `E0308` described above — before the stop hook reported it — so that run was
  doomed from its first non-formatting step. It cost a full queue wait on a
  contended pool and returned nothing. A focused compile of the changed crate
  before requesting a full gate is cheap and would have caught it. Worse, the
  doomed run overlapped the stop hook's own gate batch, compounding the
  pressure on the shared pool; the round was discarded and had to be repeated.

- (2026-10-15) **The `make test` timeout is compile-queue latency, not the
  test's own cost.** `cli_configuration_fixture_compiles`
  (`tests/command_env_ui_tests.rs:70`) is a member of the `nested-cargo-builds`
  test group, which serializes tests that **spawn a child Cargo build**; its
  allowance comes from
  `slow-timeout = { period = "60s", terminate-after = 5 }`, i.e. 300 s. The
  child Cargo inherits the shared build-limits pool, and `build-limits-status`
  taken during this work showed `capacity 4, held 4, … healthy` with roughly
  thirty requests waiting on unrelated crates from another agent's `ddlint`
  build. A 300 s cap on a test whose real work is a compile, queued behind
  thirty competitors, is a capacity observation about the *host*, not a budget
  the test is exceeding. Corroborated by an identical 300 s kill on worktree
  `0965d3fb`, whose tree contains no `determinism/` directory at all. The
  repair is to wait for the pool rather than to raise the bound.

- (2026-10-15) **`OBL-NOLOSS`'s count conjunct was false, and the same
  predicate also makes `ninja_gen::generate` the wrong oracle for `EP-M4`.**
  Found by reading the emitter while `EP-M3`'s review ran, before `EP-M4`
  began, and corrected in the obligation's own statement above. Two distinct
  defects share one root: `edge_requires_gates` (`src/ninja_gen/mod.rs:237`) is
  `dependency_order == Serial && implicit_deps.len() > 1`, and three separate
  sites restate it — the classification counter
  (`strategy/classification.rs:95-100`), `graph_requires_dyndep`
  (`src/ninja_gen/mod.rs:232`), and the serial lowering itself.

  1. A staged edge emits `1 + 2N` `build` statements, not one, so "the number
     of `build` statements equals the number of distinct edges" fails by `2N`,
     and that quoted claim is itself withdrawn; the statement above records
     why. The glossary entry at line 285 — "Edge — one `BuildEdge`, rendered as
     one Ninja `build` statement" — has since been qualified to name the
     staging, because the serial `dyndep` lowering is exactly what breaks it.
  2. Because `generate_into_with_shell` rejects any graph where
     `graph_requires_dyndep` holds, an `EP-M4` property emitting through
     `generate` gets `Err(DyndepFilesRequired)` on every case worth generating
     — the generator's intersection class *requires* `dyndep_staged_edges > 0`.
     So `OBL-NOLOSS` and its siblings must take the bundle path,
     `generate_bundle(..).build_file()`.

  The transferable lesson is that this failed **silently in both directions**:
  the count would have failed loudly on first run, but choosing `generate` as
  the oracle would have looked like a clean result until someone read what the
  errors were. A generator whose non-vacuity assertion targets class `X` cannot
  be paired with an entry point that rejects class `X`, and that pairing is
  decided by a predicate restated in several places rather than by one shared
  name.

- (2026-10-15) **A stored CodeRabbit checkpoint from before a rebase is
  *residue*, and `--fresh` is what disqualifies it.** The review store held a
  prior session for this branch (`~/.coderabbit/reviews/8ce93b54/887be735`,
  four rounds on 2026-10-10) whose checkpoint head was `1401d0da`. That commit
  is **not an ancestor of the current HEAD** —
  `git merge-base --is-ancestor 1401d0da HEAD` returns non-zero — so its stored
  findings describe a revision this branch no longer contains. `--fresh` forced
  the review to re-derive from the diff rather than from that stale cursor, and
  the run's own `comparisonBaseCommitId` confirms it reviewed
  `7f3b13c3..2b01a3e3` as intended. **Rule adopted:** a CLI review on a rebased
  branch is not admissible without `--fresh`, and the admissibility of the
  *run* is evidenced from `reviewedFiles` and the usage counter, never from the
  tool's own summary line.

- (2026-10-15) **The review's own base question has two different answers, and
  conflating them would misstate the comparison.** The branch's *milestone*
  base is `7f3b13c3` (the parent of `1ff0c1e1`, and what `--base-commit` was
  given); the *pull request* base is `origin/main`. As of this run those are
  not the same commit: `git merge-base HEAD origin/main` is `e1df2ead`, the
  point this branch forked, while the live `origin/main` tip has advanced to
  `486b876c`. So "base" names three distinct commits — fork point, branch PR
  base, and live upstream tip — and a review or a gate that names a base must
  say which. For a milestone review the correct base is the milestone's own,
  which is what was used.

- (2026-10-14) **The 400-line file cap is a *live* gate on Rust sources, and
  `GraphSpec` alone exceeded it at 538 lines.** `AGENTS.md` states "No single
  code file may be longer than 400 lines", and the enforcement is Whitaker's
  `module_max_lines` dylint rule, run by `make lint` — not the Clippy
  `too-many-lines-threshold = 70` in `clippy.toml`, which bounds a single
  *function* and is a different check. The generator was written as one
  `graph.rs` and would have failed the gate on first run. The split follows the
  house shape `foo/mod.rs` + `mod child;`: `strategy/graph/mod.rs` keeps the
  specification, its `Debug`, the strategy and the rotation helper, and
  `strategy/graph/blueprint.rs` keeps `Blueprint`, the pool allocation, and the
  resolution helpers. Max is now 354. **Lesson:**
  `find src -name '*.rs' -exec wc -l {} +` before writing, not after — every
  other file in `src/` is under 400, so the tree itself is the oracle for
  whether a cap is live.

- (2026-10-14) **`TestRunner::run` takes an `Fn`, not an `FnMut`, so a run-wide
  tally cannot be a captured `mut` binding — and the house answer is a
  `Cell`.** The natural way to write "assert some case in this run reached
  class X" is to fold cases into a local accumulator, which does not compile:
  the closure may not mutate its captures. `Cell<T>` is the shape already used
  for exactly this in the repository —
  `tests/shell_filter_property_tests/round_trip_through_an_oracle.rs:108` holds
  a `Cell<Corpus>` with the comment "The tallies live in a `Cell` because
  `TestRunner::run` takes an `Fn`", and two further suites do the same — so the
  pattern is house style rather than a local workaround. Worth recording
  because the failure is a borrow-checker diagnostic that reads like a
  closure-capture mistake and is really a signature constraint.
- (2026-10-14) **`usize::from(bool)` cannot be called in a `const fn` on this
  toolchain, and the arithmetic cast that would work is denied
  workspace-wide.** Building a four-way truth-table mask from two booleans wants
  `1u8 << (usize::from(always) * 2 + usize::from(restat))`; `From` is not yet
  stable as a const trait, so this fails with `E0658` and suggests the nightly
  feature `const_convert`. `as usize` is the obvious alternative and is denied
  by `cast_*` in the workspace lints. The resolution is two branches —
  `if always { 2 } else { 0 } + if restat { 1 } else { 0 }` — which is `const`,
  cast-free, and arguably clearer than the arithmetic it replaces.
- (2026-10-14) **A generated counter-example that *looks* short can be measured,
  and the measurement is what makes `EP-M3`'s third acceptance arm evidence
  rather than assertion.** "Fits on a screen" is a claim about rendered output,
  and the output only exists when a property fails. A temporary probe property
  produced a real one: **363 characters**, against a `MAX_DEBUG_CHARS = 4_000`
  bound asserted by a permanent test. Two details fall out of doing it that
  reasoning alone would not have produced. Proptest formats the failing input
  through `{:#?}` — `TestError`'s `Display` writes
  `minimal failing input: {:#?}` — so a `Debug` impl that honoured the
  alternate flag would have been pretty-printed across many lines and the
  boundary would be a *line* budget, not a character budget; `GraphSpec`'s impl
  therefore ignores the formatter's alternate flag and writes exactly one line.
  And the probe left a `proptest-regressions` artefact recording a *deliberate*
  failure; committing it would replay a fabricated failure on every future run,
  so it was deleted. A probe that is removed must take its side effects with it.
- (2026-10-14) **The plan's "re-draw on the rare coincidence" mechanism is
  unnecessary once the second order is a rotation, and the check that replaces
  it is stronger.** The plan anticipated that independently drawn orders would
  sometimes coincide and specified a redraw loop. A rotation of a permutation
  of two or more elements can never equal that permutation, so the coincidence
  the loop defends against cannot arise; what remains is a *check*, asserted in
  the property, that the rotation did what it claims. This is recorded in the
  `Decision log` as a deviation because it replaces a mechanism the plan names
  rather than merely implementing it, and the difference is the kind that
  should not be silently absorbed: the redraw spends case budget on rejection
  and can exhaust an attempt budget, which rotation cannot.

- (2026-10-14) **`mdtablefix --wrap` can need two passes to converge, so one
  canonicalization is not enough to clear `make check-fmt`.** After a hand-edit
  to a long paragraph in this plan, piping a single canonicalization straight
  back into the file still failed `--check` with a `+3 -17` line delta. Running
  mdtablefix over *its own output* changed it further; only that second output
  was a fixed point. The wrap and join steps interact — re-flowing a line
  alters which lines are eligible to be joined on the next run — so convergence
  is iterative. The safe repair is to diff the first pass against a second and
  copy back only once `diff` is empty. This is worth knowing because the
  failure mode looks like "the gate is flaky" when the tool is in fact
  deterministic and the repair was simply incomplete.

- (2026-10-14) **Convergence is necessary but not sufficient: `mdtablefix` can
  produce malformed Markdown that is its own fixed point, and `check-fmt`
  accepts it.** The two-pass loop above only proves the canonical form is
  stable. It does not prove the canonical form is *correct*. While repairing
  this plan, one paragraph was left carrying a stray backtick after
  `[37.893s]`; mdtablefix re-flowed it into a shape with two long lines and
  three MD038 violations, and re-running mdtablefix over that output returned
  it unchanged. `mdtablefix --check` exited 0 on the damaged file, so
  `make check-fmt` was **green on a file `make markdownlint` rejected**. The
  stray backtick was the root cause: every backtick in the paragraph is a
  single-backtick run, so one unpaired one flips the parity of *every* span
  after it, and the tool paired the wrong ones while re-flowing. The general
  lesson is that a formatter's fixed points are not a correctness oracle for
  Markdown; the two gates are not redundant, and only `markdownlint` catches
  this class. Run both.

- (2026-10-14) **A serialized test group turns a compile-cache outage into a
  timeout, and it does so at whatever test happens to be holding the slot.** The
  `nested-cargo-builds` group in `.config/nextest.toml` pins
  `max-threads = 1`, so every child-Cargo test in the workspace runs strictly
  one at a time. When the managed sccache is unreachable the whole group
  recompiles from scratch, and the cost is not paid by the tests that need it
  most but by whichever test the scheduler reaches when the budget runs out.
  That is why pass #4 killed
  `production_build_module_slice_has_expected_boundary` and pass #5 killed
  `cli_configuration_fixture_compiles`: neither is at fault; the slot was. The
  tell is that the same test passes in 106.7 s on its own run and 26.669 s in
  CI, so the failure moves with the host, not with the code. Worth remembering
  before diagnosing any `nested-cargo-builds` timeout as a property of the
  named test.

- (2026-10-14) **A timeout that migrates between tests is stronger evidence than
  a timeout that repeats.** Intuition says the opposite — a reproducible
  failure feels like a real defect and a wandering one feels like an excuse.
  Here it is inverted: a *fixed* victim could plausibly be a slow test that
  needs a bigger budget, whereas a victim that changes between runs while every
  other observable (573 sccache-unreachable lines, `Compile requests 0`, pool
  at 4/4, zero assertion failures) stays byte-identical can only be reporting
  shared resource exhaustion. The first reading argues for widening the
  timeout; the second argues for not touching it. Getting this backwards would
  have led to tuning a budget in response to a cache outage.

- (2026-10-14) **A path rendered with `Debug` does not contain the path.** A
  first repair of the Windows failure assumed `{candidates:?}` over
  `[Utf8PathBuf; 2]` produced `src\ninja_gen\...` with single separators, so
  `replace('\\', "/")` would normalize it. That is wrong: `Debug` for a path
  forwards to `str`'s `Debug`, which **escapes** the backslash, so the message
  carries `src\\ninja_gen\\...` and normalization yields `src//ninja_gen//...`,
  which still fails to match. Two independent probes settled it — a standalone
  Rust rendering of both forms, and the raw bytes of the saved CI log, which
  hold `["src\\ninja_gen\\property_tests\\mod.rs", …]` with five doubled
  separators. The durable fix is to stop rendering a path through `Debug` when
  the message is read by a human or matched by an assertion: use `Display`, or
  `as_str()`, and normalize only the platform separator that genuinely remains.

- (2026-10-14) **A `Utf8Path` equality check is separator-insensitive; a
  substring check on its rendering is not.** In the same test, the two
  assertions comparing `resolved == Utf8Path::new("src/…")` were correct on
  Windows and needed no change, because `Utf8Path` compares by component. Only
  the third assertion — which string-matched a rendered candidate list — broke.
  The asymmetry is easy to miss when writing a test: the same literal
  `"src/a/b.rs"` is right in one comparison and wrong in the next.

- (2026-10-14) **Seven path citations in this plan had gone stale, and every one
  of them pointed at code the reader is instructed to *go and read*.** `main`'s
  `#811`/`#813` migration grouped prefix-named modules under directory modules
  and dropped the parent prefix from each child filename, so
  `src/ninja_gen_property_tests/ninja_oracle.rs` is now
  `src/ninja_gen/property_tests/ninja_oracle.rs`,
  `src/ninja_gen_property_tests.rs` (cited with a line number) is now
  `src/ninja_gen/property_tests/mod.rs`, and `src/graph_view/tests_property.rs`
  is now `src/graph_view/tests/property.rs`.

  The failure mode is worth naming because it is silent: a stale citation does
  not break the build, fail a gate, or appear in any diff, so nothing catches
  it until an implementer tries to follow it. Six of the seven were
  forward-looking and have been corrected — the prior-art description, the
  required-reading list, `EP-M3`'s absorb-or-diverge directive, and two
  obligation statements. The seventh sits in a dated 2026-09-09 entry in this
  section and is left verbatim, because a historical record that names the path
  *as it was* is correct as written; only the present-tense citations are
  defects.

  Two of the corrected citations carried line numbers, and both line numbers
  still resolved correctly at the new path. That is the useful signal: the
  migration moved files without reflowing them, so a citation whose *path* is
  stale may still have an accurate *line*.

- (2026-10-14) **The planned `determinism/` property suites would not have been
  run by `make proptest` at all, and this was found only by evaluating the
  filter against the planned *test names* rather than reading it.** This is a
  third defect, independent of the two below, and the most dangerous of the
  three because it fails *silently*. `PROPTEST_FILTER` (`Makefile`
  `PROPTEST_FILTER`) selects a module *segment* ending in `proptests` or
  `property_tests`. The plan's property homes as first drafted were
  `src/ninja_gen/determinism/path_key.rs`, `no_loss.rs`, `order.rs`,
  `defaults.rs`, `declaration.rs`, and `ninja_oracle.rs` — **not one of which
  ends in either suffix**. Evaluated against the real filter,
  `ninja_gen::determinism::path_key::path_key_is_permutation_invariant` and its
  six siblings each matched **nothing**:

  ```plaintext
  plan as written : 0 of 7 planned suites selected
  with one intervening `property_tests` segment : 7 of 7 selected
  ```

  The repair is to interpolate a single `property_tests` segment —
  `src/ninja_gen/determinism/property_tests/{…}.rs` — and it *also* passes the
  module-layout contract, so both constraints agree on one shape. What makes
  this worth recording beyond the fix is the failure mode: `make proptest`
  would have exited **0** while running none of the new properties, and `EP-M4`
  through `EP-M6` all carry the acceptance line "`make proptest` passes". The
  plan already warns about exactly this class at line 1373 — "nextest exits 0
  while selecting nothing, so the target would look green while running no
  property test" — and it re-entered through *file naming* rather than selector
  spelling, which is the route the earlier analysis did not consider. The
  transferable lesson is that a selector's coverage must be checked against the
  names the new tests will actually carry, not against the selector's own
  syntax.

- (2026-10-14) **A delegated implementation packet was falsified by executing
  the repository's contracts, and the falsification is more useful than the
  packet.** The packet prepared for `EP-M3` recommended locating the shared
  strategy at `src/ir/graph/property_tests/`. That recommendation was the
  correct repair for the *wrong artefact*: it took `EP-M7`'s IR path,
  discovered by probe that it fails the layout contract, and prescribed the
  nested form — which the plan had **already** recorded here at the entry
  below. It then reused that path as the home for `EP-M3`'s shared generator,
  which the plan places at `src/ninja_gen/determinism/strategy/` (lines 1499,
  1628-1631) and which **passes** the contract unaided. Following it would have
  moved a generator consumed by the `ninja_gen` suites into an unrelated domain
  module. The packet also contradicted two plan requirements outright: it
  directed that `EP-M3` add no property, where the acceptance requires the
  strategy be "exercised by a smoke property" and that classification counts
  reproduce `EP-M0` question 1; and it specified the compact `Debug` as counts
  plus the order permutation, where the plan requires "counts per class **plus
  a stable digest**". Both contradictions were resolved in the plan's favour,
  the plan being the approved artefact and the packet a scratch one. The
  generalizable point is that a packet's *verified* premises can still support
  a *wrong* conclusion — every individual claim in it checked out, and the
  error was in which artefact the claim was applied to. Verifying a delegated
  packet's facts is not the same as verifying its recommendation.

- (2026-10-14) **`EP-M3`'s artefact list names a module layout this repository
  does not use.** The plan specifies
  `src/ir/graph_property_tests/{mod, duplicates,cycles}.rs` and
  `src/ir/action_hash_property_tests.rs`, but the established house shape is
  the property-test *directory* nested inside the module it tests —
  `src/ir/cmd_interpolate/property_tests/` and `src/ir/cycle/property_tests/`
  both exist, and `src/ir/graph/` has no tests directory at all. The
  sibling-prefix form the plan names is the layout `#811` retired. Creating it
  would satisfy the plan literally while contradicting the convention the repo
  enforces, so `EP-M3` must use `src/ir/graph/property_tests/`. This is a
  documentation defect in the plan, not a code defect, and it is corrected at
  implementation time rather than pre-emptively rewritten here, because the
  milestone owns its own file list.

- (2026-10-13) **An ExecPlan's header `Status:` is derived, not remembered, and
  this plan's had been wrong for eight revisions.** The field read `DRAFT` from
  the first revision onward while the document beneath it recorded a passed
  approval gate, a delivered ADR, and three completed review rounds. The
  failure mode is specific: `Status:` is a *summary* of `Progress`, and a
  summary maintained by hand drifts the moment the section it summarizes moves.
  The style guide resolves the disagreement in one direction only — "when the
  two disagree, the sections and the roadmap are correct, and the header is the
  defect" — so a stale header is not a cosmetic wart but a documented defect
  with a stated repair. The practical rule: **recompute the header from
  `Progress` at the start of any revision, and never carry it forward.**

- (2026-10-13) **The local `mdtablefix` is a version behind the gate's, and the
  skew corrupts prose rather than merely reformatting it.** CI pins
  `MDTABLEFIX_VERSION: '0.6.1'`; this host carries **0.6.0**. Running
  `make fmt` under 0.6.0 rewrote a factual sentence in this plan's `Progress`
  section — "the rebase replayed **18.** A content check confirms…" became
  "replayed **1.** …" — because `--renumber` read the sentence-initial cardinal
  as an ordered-list marker and renumbered it. The commit the edit was applied
  to passes `check-fmt` under the CI pin (`1 file left unchanged`, exit 0), so
  **0.6.1 accepts the "18." form and 0.6.0 is the one that is wrong**; the
  corruption is invisible to a local `make fmt && make check-fmt` cycle, which
  is self-consistent under whichever version happens to be installed. It was
  caught only by diffing the formatter's output against the committed bytes and
  asking which version adjudicates. Three further 0.6.0 rewraps in the same run
  were reverted for the same reason. **The rule: derive the gate's verdict from
  the gate's pinned tool, not the host's** — `grep -nE '_VERSION:'`
  `.github/workflows/ci.yml` against `<tool> --version`, then run the pinned
  binary from a scratch path via the Makefile's `?=` seam (`MDTABLEFIX=…`) so
  no shared binary is replaced. A formatter is not a word processor: its
  `--renumber` can change a *number* the document asserts, and a number is a
  factual claim.

- (2026-10-13) **A green run at a branch tip and a green run at its parent are
  different claims, and only the second was in hand.** The earlier entry
  recorded CI `38055325773` discharging the `Status:` defect, but that run's
  head is `20e5f480` — the **parent** of the then-current tip `7ec291a9`. The
  defect was discharged *one revision behind the tree*, and the tip's own run
  had not been observed. It now has been (`38056865899`, fully green, with the
  contract test named in the log), so the claim is finally attributed to the
  tree it describes. The general hazard is that "CI is green" silently means
  "CI is green for some revision in the neighbourhood" unless the head SHA is
  read from the same response as the verdict. This is the same class as the
  repository's existing rule that a gate log's filename suffix is not a
  revision.

- (2026-10-10) The plan's longest-standing open question — what a nextest
  `test(...)` selector is anchored against for a library-side suite — was not
  answered by reasoning but fell out of a CI log as a side effect of fixing
  something else. The question had been carried, argued, and deferred across
  several revisions. The resolving evidence is two lines the pipeline already
  prints: the verifier's `replayed 18 filter alternative(s)` /
  `ok: … matches […]` output, and 54 `src/`-side instance names listed by
  module path with no crate segment. **The lesson is procedural rather than
  technical**: an open question about tool behaviour is usually cheaper to
  close by reading a log of the tool running than by reasoning about its
  documentation, and a plan that carries such a question for several revisions
  is a plan that has not looked for the log yet.
- (2026-10-10) A **contract test in this repository reads this plan**. The
  `Status:` header field is not decorative:
  `tests/execplan_status_contract_tests.rs` parses the header of every file
  under `docs/execplans/` and fails the whole test run when the value after
  `Status:` is not exactly one of
  `DRAFT | APPROVED | IN PROGRESS | BLOCKED | COMPLETE`. It caught this plan's
  qualified header (`DRAFT — AWAITING APPROVAL (revised after design review)`)
  in CI run `38051221830` at `1401d0da`, on the coverage lane, as a hard
  failure that cancels the run. Two consequences worth carrying: the plan's
  prose is itself gated code, and a compiler-free local gate set does **not**
  cover it, because the check is a Rust test — `make test-workflow-contracts`
  runs the Python contracts only. An agent working around a wedged compile pool
  is therefore exposed to this class of defect until CI runs.
- (2026-10-10) A green `main` CI run is **not** evidence for the `build-test`
  union. In run `38046182468` at `e1df2ead`, jobs `build-test` and `Windows` are
  `skipped` and only `kani-smoke` executes; the union runs on pull-request
  events. Reading the green `main` run as "all gates pass" would be a false
  inference, and it is the exact inference available to a reader who cannot run
  the Rust gates locally. The PR run is the only run that carries the
  seven-target union.
- (2026-10-10) Nine consecutive runs on this branch are `cancelled`. Between
  `37929646063` and `38051221830` every run on branch
  `4-3-1-proptests-for-deterministic-ninja-emission` shows `cancelled`; only the
  `1401d0da` run reached a verdict, and it failed. A cancelled run carries no
  information about the tree, and a run list that looks busy may contain no
  evidence at all.
- (2026-10-10) `origin/main` has eliminated the flat/directory module
  ambiguity this plan's `supplemental_property_location` design was built on.
  The sixth revision preferred existential resolution because
  `src/ir/cycle_verification.rs` and `src/ir/cycle/verification.rs` "both map to
  `ir::cycle::verification`, and the repository uses **both** forms today". A
  sweep for flat-and-directory pairs resolving to the same module path now
  finds **zero** across `src/`, and zero on `origin/main`;
  `cycle_verification.rs` has become `cycle/verification.rs`. The premise is
  falsified, so the collision-witness argument cannot be carried forward. The
  generalization is still required, for a weakened reason: `origin/main` derives
  `src/<root>/<segments>/mod.rs` but still hard-codes
  `ensure!(*root == "ir")`, so a `ninja_gen`-rooted patch is rejected. This is
  the second time a premise in this plan has been invalidated by upstream
  motion, and it is the second time the check that found it was a direct read
  of the live tree rather than a re-reading of the plan.

- (2026-10-10) The layout contract's violation counts are a property of the
  **tree**, not of the diff. Applying
  `tests/workflow_contracts/rust_module_layout_test.py`'s own logic to this
  branch's `src/` reports 66 unrecorded prefix groups and 27
  prefixed-beside-directory violations; `origin/main` reports 0 and 0 on both
  checks. Nothing this branch changed produced them — the branch touches four
  Markdown files and no Rust — so a branch can be entirely innocent of a
  contract it violates, purely by falling behind the migration that satisfied
  it. "The diff is clean" and "the tree passes" are different claims.

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
  re-probe at the tree being corrected.
- (2026-10-10) `EP-M2`'s two splits are governed by a module-layout contract
  this branch does not yet carry, and the shape this plan implied is rejected
  by it. `fce1a746` ("Group prefix-named modules under directory modules",
  #811/#813) added `tests/workflow_contracts/rust_module_layout_test.py`, wired
  into CI at `.github/workflows/ci.yml:271`. It rejects two shapes: a group of
  siblings sharing a first `_`-prefix unless recorded in an exact-match
  exception, and any file named `<dir>_*.rs` sitting beside a directory module
  of that name. The commit is **not** an ancestor of this branch's base
  `96b89ca9` — `main` is 44 commits ahead — so the contract is absent here. The
  natural conclusion, that the shape is therefore free until the rebase, is
  wrong, and a direct probe settled it. The upstream contract was run against
  the live tree and against four candidate layouts. Its findings:

  - `src/ninja_gen_named_action.rs`, the `<parent>_<role>.rs` sibling that this
    plan's own wording implied, is a **violation on both grounds**: it is a
    prefixed file beside the `ninja_gen/` directory module, and it joins the
    already-oversized `('.', 'ninja')` prefix group.
  - `src/ninja_gen/named_action.rs`, an ordinary child *inside* the existing
    directory, is **clean** and needs no `#[path]` attribute at all. This is
    the shape `EP-M2` must use.
  - **27 files** under `src/` already violate the contract, eleven of them in
    `src/ninja_gen/` (`ninja_gen_escape.rs`, `ninja_gen_validation.rs`,
    `ninja_gen_error.rs`, `ninja_gen_command_list.rs` plus its scanner and test
    siblings). This is pre-existing debt a rebase onto `main` must clear; it is
    not something the split introduces, and it is not this plan's to fix.

  The upstream commit also renames files this plan cites
  (`ninja_gen_command_list.rs` → `command_list/mod.rs`, `ninja_gen_error.rs` →
  `error.rs`, `ninja_gen_escape.rs` → `escape.rs`), and `mod.rs` is 386 lines
  there with `NamedAction` still in it. So every `EP-M2` path and seam must be
  re-derived at implementation time rather than carried from here. **Lesson:**
  a file split is a *structural* change, governed by whatever structure
  contracts are in flight rather than by this plan alone. Reading a rule states
  what it forbids; only running it against the tree reveals which of the
  remaining shapes is clean. Do the probe before choosing the shape, not after
  lint rejects it.
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
- (2026-10-17) **A mutation table row names a fault class, and the patch is
  allowed to express it differently when the literal edit does not compile.**
  The table says `MUT-EDGESORT` "deletes `edges.sort_by_key`". Deleting it fails
  `cargo check` under `-D warnings`, because `path_key` then has no caller and
  its import no longer resolves — the same way the table's `MUT-PATHKEY` row
  produced dead evidence until `5e973841`. Three of these four patches
  therefore diverge from the row's wording while keeping its meaning: a key
  computed from the wrong field, a collect without a sort, a sort without its
  import. The decision is that **the patch file is the evidence artefact and
  the table describes the class**; a reviewer checking that the table and the
  patch agree word-for-word is checking the wrong thing, and the run — does the
  assigned property fail, on a tree that compiles — is what settles it. The
  alternative, keeping the literal rows and exempting the patches from
  `-D warnings`, was rejected: the compile guard exists precisely so a patch
  that cannot build is caught as dead evidence rather than counted as a
  falsification.

- (2026-10-17) **The table's "two were cut" sentence contradicts its own rows
  and the acceptance criteria, and the rows win.** The paragraph under the
  table says nine patches remain because two sort-deletion mutations were cut
  as already covered by `cargo-mutants`. The table two lines above it lists
  both, `EP-M5`'s acceptance names both, item 3 of the acceptance counts both
  among the nine, and the Scope derivation budgets seven patches for `EP-M5` and
  `EP-M6` together. The sentence is a fossil of the first draft's eleven and
  was never updated when the rows were restored. The patches exist and were
  validated by execution, so the rows, the acceptance criteria, and the budget
  are the authoritative description; the paragraph's claim that these faults
  are delegated to `cargo-mutants` is not relied on anywhere. Correcting the
  paragraph is a documentation edit recorded here rather than a change of scope.

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
  iterates: `graph.actions` is an `IrHashMap`, and the `actions.sort_by_key` in
  `ordered_actions` is what re-establishes order.

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
- (2026-10-17) **The timeout widening's own contract amendment breached the
  module line cap, and the correct repair was to move the responsibility, not
  to shrink the text.** `test_execution_coverage_test.py` stood at 397 of
  pylint's 400 lines before this change; the added family assertion took it to
  420 and `make lint` failed `C0302`. Compressing the comment and inlining the
  nine names would have fitted it, leaving the module at 399 lines with one
  line of headroom and a cold-build budget map living in a module whose stated
  remit is the single-execution rule and which reads none of the budget tiers.
  `nextest_child_cargo_timeout_test.py` already existed, already held the
  targeted cold-build allowances, and already declared that it pins the
  family's shared override. The whole test moved there. **Lesson:** when a cap
  is breached by an addition that belongs in another module anyway, the cap
  shows where the code belongs; trimming it to fit hides the misfiling behind a
  passing gate.

  **Second lesson from the same failure, on the plan's own provenance rule.**
  The cold child build was recorded as **456 s** at five sites in the plan, the
  guide and the configuration comment. The only evidence on disk is cargo's own
  line, `Finished dev profile ... in 7m 35s`, which is 455 s. The figure was
  retyped from a rounded reading and then propagated. It is now 455 s wherever
  it appears, because a measurement claim must be copied from the line that
  printed it, not recomputed in transit.

## Decision log

- Decision: the ambient public-API fixture family's `slow-timeout` is widened to
  fifteen periods (900 s), **on the family override rather than on
  `cli_configuration_fixture_compiles` alone**, and the plan's earlier
  expectation that `origin/main`'s repair would remove the local failure is
  recorded as falsified. Date/Author: 2026-10-17 / implementation agent, on the
  user's instruction "If that doesn't fix the timeout, address the way the
  timeout is calculated to account for the queue."

  *Why the family and not the test.* Nine fixtures in
  `tests/command_env_ui_tests.rs` call `NetsukeRlib::build()`, which runs
  `cargo check --workspace --all-targets --all-features` in the ambient
  `target/`. `.config/nextest.toml` serializes them all through
  `nested-cargo-builds` at `max-threads = 1`, so they share one cold build and
  the *first to be scheduled* pays it. Widening only the name that timed out
  would move the failure to whichever fixture next drew first place, and the
  scheduler picks by test name order rather than by cost. The measured victim
  this time was `cli_configuration_fixture_compiles`; nothing about that name
  makes it the expensive one.

  *Why the rebase could not have fixed it, established from git history rather
  than inferred.* `git show 077cf366 -- .config/nextest.toml` shows that commit
  doing two things in one hunk. It **deleted** a standalone override naming
  `cli_configuration_fixture_compiles` alone, whose comment recorded "709.2 s
  on a cold per-run tree" and which carried
  `slow-timeout = { period = "60s", terminate-after = 15 }`. It then **added
  that same name to the eight-name family filter**, an override with
  `test-group`, `success-output`, and no `slow-timeout` at all. The name moved
  from an override that granted 900 s to one that granted nothing, so it fell
  back to the 300 s profile default. The rebase therefore could not have
  removed the failure: it installed it. That is why the failure reappeared at
  `TIMEOUT [300.024s]` after a clean rebase, and it is the strongest evidence
  that this is a configuration regression rather than a slow test.

  *Why the figure is not invented.* The child command was measured at **455 s
  cold** in the gate tree against **1 s** when immediately repeated, and the
  victim was stopped at **300.024 s**. Fifteen periods is the tier the
  repository already uses for every other fixture that pays a workspace-root
  `cargo check` (`stub_env_default_does_not_compile`) or an isolated cold build
  (split-build, packaging), so this joins an existing measured tier rather than
  introducing a new one. It also satisfies the ordering the contracts enforce:
  `global-timeout` must stay strictly above the largest per-test allowance, and
  900 s leaves the 960 s CI budget 60 s clear.

  *What this does not do.* It does not change any test's behaviour, retry a
  failure, or relax the `[profile.default]` 300 s that governs every other
  test. The five other timed overrides are unchanged. It does not fix the
  underlying cost: a cold ambient `cargo check` remains a 455 s item, and the
  contracts' own comparison still caps the per-test tier below the whole-run
  budget.

  *Contracts amended with it*, because each asserted the old dispensation and
  would otherwise either fail or, worse, keep asserting something now false:
  `len(timed_overrides) == 5` becomes `== 6` and the budget map gains the
  family's compound filter — both in `nextest_child_cargo_timeout_test.py`,
  where that contract now lives because the added block pushed
  `test_execution_coverage_test.py` past pylint's 400-line cap;
  `nextest_child_cargo_timeout_test.py`'s docstring no longer says the CLI
  fixture is excluded from the cold tier; and the tier enumerations in
  `nextest_budgets.py`, `timeout_ordering_test.py`, `.config/nextest.toml`'s
  own header and `[profile.ci]` comment, and `docs/developers-guide.md`'s "Test
  timeouts" section now name the family alongside the other 900 s tests. The
  new assertion was proved live twice — once in its original home and once
  after the move — by removing the widening and watching
  `test_cold_build_budgets_are_scoped_to_the_tests_that_pay_them` fail on
  `assert 5 == 6`, then restoring it.

- Decision: `EP-M3`'s shared graph strategy **explicitly diverges** from
  `graph_view`'s `arb_graph_inputs` rather than absorbing it, and the
  divergence is recorded here as `EP-M3`'s acceptance requires. The plan left
  `EP-M3` the choice — "absorb or explicitly diverge" — with the instruction
  that leaving both generators unreconciled is the decay path. Divergence is
  the measured answer, on four grounds, all re-verified against the tree on
  2026-10-14 rather than carried over from the planning pass:

  1. **Different return type, so nothing can be absorbed.** The existing
     strategy returns a tuple of loose specifications —
     `(Vec<(String, Option<String>)>, Vec<EdgeSpec>)` — and never constructs a
     `BuildGraph`. `EP-M3` needs `materialize(&self, order: &[usize]) ->
     BuildGraph`, which is the whole point of the milestone: the properties
     perturb the *insertion order* of a real graph.
  2. **The type is unreachable from a sibling module.** `struct EdgeSpec` is
     declared without `pub` at `src/graph_view/tests/property.rs:13`, and its
     owning module is a private `#[cfg(test)] mod tests;` at
     `src/graph_view/mod.rs:299`. No sibling can name `EdgeSpec`, so no
     `src/`-side strategy could accept or return one even if the shape matched.
  3. **It filters where `EP-M3` requires construction.**
     `retain_disjoint_output_edges` at `src/graph_view/tests/property.rs:62` is
     applied at line 148 to discard edges that collide on an output path. The
     plan requires well-formedness *by construction* — a pre-numbered output
     namespace with contiguous, non-overlapping slices — because a filter that
     silently drops cases is precisely the generator defect the plan's
     non-vacuity argument forbids.
  4. **Its perturbation is too weak for the property `EP-M3` must state.** The
     prior art reverses the action and edge vectors
     (`src/graph_view/tests/property.rs:151,153`). `EP-M0` measured that
     independently drawn orders frequently coincide, which is why the plan
     derives `first_order` and `second_order` from one another and *checks them
     distinct*, redrawing on coincidence rather than relying on a shuffle that a
     short input can return unchanged.

  Migrating `graph_view` onto the new strategy was considered and rejected: it
  would couple a view-layer test to `ninja_gen`, which is a layering inversion
  for no gain, since the two properties test different guarantees. The
  three-item reconciliation the plan asks for is therefore: **diverge, and say
  why** — which is what this entry does. The plan's own text at the absorb
  directive has been left intact, because it correctly states the obligation;
  this entry is its discharge.

- Decision: the second insertion order is a **rotation** of the first, not an
  independently drawn shuffle re-drawn until it differs. The plan (at
  `OBL-ORDER`'s core arm) requires the two orders to be "derived from one
  another and then checked to be distinct, re-drawing on the rare coincidence
  rather than asserting it". Rotation is a strictly stronger mechanism than the
  redraw it replaces, and it is recorded here as a deliberate deviation because
  it changes the *shape* of the guarantee rather than only its implementation.
  `rotate_left(offset.min(last_index).max(1))` cannot return its input for any
  permutation of two or more elements and any `offset` in `1..len`, so
  distinctness holds by construction and the redraw path is unreachable — the
  generator spends no case budget on rejection and cannot exhaust an attempt
  budget the way `prop_shuffle` alone was measured to (`MIN_ACTIONS`' doc
  records the 200/200-at-one-key and 23/200-at-two-keys figures that motivated
  raising the domain's floor to five). The property still *asserts* divergence
  rather than assuming it:
  `the_two_orders_preserve_the_specification_while_differing` fails if a
  rotation of a two-or-more-element permutation equals its input, so the
  guarantee is checked at the place it is used and not merely argued. For a
  one-element permutation no second arrangement exists, so the rotation is the
  identity and the property's assertion is conditioned on `edge_count() > 1`;
  `MIN_EDGES = 5` means that branch is unreachable in practice, and leaving it
  explicit rather than relying on the floor keeps the helper correct on its own
  terms.

- Decision: `GraphSpec`'s digest uses
  `std::collections::hash_map::DefaultHasher`, **not** the crate's
  `ActionHasher`. The plan specifies "a stable digest" and does not name an
  implementation. `ActionHasher::hash` is a SHA-256 over a JSON serialization
  (`src/hasher.rs`), which would cost up to 750 serializations per generated
  case at the `MAX_ACTIONS`/`MAX_EDGES` bound; the digest's only job is to let
  a reader tell whether two `Debug` renderings describe the same value, and the
  property that consumes it (`the_compact_debug_stays_within_a_screen`) asserts
  the *length and presence* of the rendered diagnostic, not the digest's
  cryptographic strength. There is no `Hash` impl to lean on — `Action` and
  `BuildEdge` derive `PartialEq` but not `Hash` — and hashing their `Debug`
  output would fold formatting into the value, so `GraphSpec::digest` walks
  each observable field explicitly. Two consequences are stated rather than
  discovered later: the digest is stable only within a build (Rust does not
  guarantee `DefaultHasher`'s algorithm across versions), which is sufficient
  for a failure diagnostic and is deliberately *not* used as an assertion; and
  `hash_paths` mixes in each list's length so that `[]` and `[""]` cannot
  collide.

- Decision: `strategy/manifest.rs` is **deferred to `EP-M6`**, and only the two
  modules the `EP-M3` acceptance needs are delivered now. The plan's file
  layout lists a `manifest.rs` beside `graph.rs` and `classification.rs`, but
  its only consumer is `OBL-E2E`, which is assigned to `EP-M6`, so building it
  in `EP-M3` would ship a generator with no property exercising it — against
  `Constraint 7`'s requirement that every generator be mutation-audited. The
  deferral keeps this milestone's plateau honest: every artefact delivered here
  is exercised by a property that runs in `make proptest`. `EP-M6` inherits the
  obligation to add the file *and* the property that falsifies it.

- Decision: `make proptest`'s selector lives in the **Makefile recipe**, not in
  `.config/nextest.toml`, and this is **forced by measurement** rather than
  chosen for convenience — which settles the open question `EP-M2` carried as
  "which mechanism the `Makefile` `proptest` target uses". The configuration
  route is **closed**, and the reason is not the one the plan anticipated.
  `EP-M2` reasoned that a configuration `filter` "inherits the runtime check"
  and was therefore the better home; both halves of that are true, but the
  configuration is *also* held to a **static** contract that the Makefile is
  not, and that contract is what refuses the selector.
  `tests/workflow_contracts/nextest_child_cargo_group_invariants.py` matches an
  anchored selector with
  `GROUP_FILTER = re.compile(rf"test\(/\^{MODULE_PATH}([a-z0-9_]+)\(\$\|::\)/\)")`
  where `MODULE_PATH = (?:[a-z0-9_]+::)*` is **greedy**. The name group is
  therefore the *last* segment, not the first: a `src/`-rooted selector
  `test(/^ninja_gen::property_tests($|::)/)` captures `property_tests`, and
  `test_filters_match_every_declared_test_they_name` then fails it, because
  `declared_test_names()` scans only `tests/`
  (`TEST_SOURCE_ROOT = REPO_ROOT / "tests"` in
  `nextest_rust_test_discovery.py`) and no `src/`-side function is in that
  corpus. Measured, not inferred: appending that override to a copy of the
  configuration and running the three contract checks gives

  ```plaintext
  FAIL test_filters_match_every_declared_test_they_name —
  filters name tests that no Rust source declares: ['property_tests']
  ```

  while the control run on the unmodified configuration passes all three. So
  **no** `src/`-rooted selector can be written into the configuration, whatever
  its shape, because the failure follows from the greedy capture and the
  `tests/`-only corpus rather than from the spelling. Options considered: (a)
  write the selector into the configuration and amend the contract; (b) write
  it into the Makefile recipe. (b) was chosen because (a) would weaken a
  shipped contract that is correct about the class of defect it polices (a
  filter naming a nonexistent test selects nothing and looks enforced), and the
  plan's own `documentation-examples` recipe precedent already puts a selector
  in a recipe. The cost accepted is the one `EP-M2` named: a Makefile `-E`
  argument is unverified by construction, so the selector is discharged by the
  plan's stated procedure instead — replaying it through real nextest and
  reading every testcase's `filter-match.status`. Note the plan's **suggested
  spelling was wrong** and is corrected here:
  `test(/^property_tests($|::)/) | test(/^proptests($|::)/)` selects **0**
  tests — under either corpus, with or without `--workspace` — because the `^`
  anchor names a leading segment and the suites are reached through a module
  path (`ir::cmd_interpolate::property_tests::…`). The corrected shape anchors
  the *segment*:
  `test(/(^|::)[a-z0-9_]*proptests($|::)/) | test(/(^|::)[a-z0-9_]*property_tests($|::)/)`.
  Affected identifiers: `EP-M2`, `Constraint 4`, `Constraint 7`. Date/Author:
  2026-10-13 / implementing agent.

- Decision: record the `make proptest` selector's discharge **by
  `filter-match.status`, with `--no-tests fail` on the target**. Replayed
  against this revision **with the `--workspace` flag the recipe itself
  passes**, the corrected selector reports `matches` for **90** testcases and
  `mismatch` for the remaining 3758, and the 90 are **set-equal** to the census
  of every test name carrying either suffix — no extras, no omissions. The 90
  split **86 in `netsuke-build`** and **4 in `test_support`**, and the suites
  behind them are the eleven `netsuke-build` ones
  (`cli::discovery:: helper_proptests`,
  `cli::discovery::layers::property_tests`, `cli::merge::logging_proptests`,
  `cli_tests::merge_{diag,precedence,targets}_proptests`, `hex::property_tests`,
  `ir::cmd_interpolate::property_tests`, `ir::cycle::property_tests`,
  `ninja_gen::property_tests`, `rfc_stdlib_coverage::markdown::property_tests`)
  plus `test_support`'s `netsuke::locator::tests::locator_property_tests`. The
  `--workspace` flag is not incidental: a `list` without it reports a corpus of
  3807 and 86 matches, and the recipe's own run reports 3924 and 90, so the
  discharge is performed with the same flags the target uses. `make proptest`
  then ran 90/90 and passed at exit 0 with 3834 skipped.

  `--no-tests fail` is added because the plan's premise that "nextest exits 0
  even when a selector matches nothing" is **falsified on this nextest
  version** (0.9.133): a zero-match run exits 4 with `error: no tests to run`
  whether or not the flag is passed. The flag is therefore defence in depth
  rather than the guard the plan expected, and it is the guard on older or
  differently configured nextest. A second, sharper trap was found and fixed in
  the recipe: `$|` is Make's automatic variable for a target's **order-only
  prerequisites**, so an unescaped `($|::)` in the recipe is silently expanded
  to `(::)` — deleting the branch that matches a bare suite name and leaving
  only in-submodule tests selected. The variable is spelled `$$|` and the
  expansion is now checked with `make -n proptest` before the recipe is
  trusted. Affected identifiers: `EP-M2`. Date/Author: 2026-10-13 /
  implementing agent.

- Decision: adjudicate `check-fmt` with the CI-pinned `mdtablefix` (0.6.1),
  installed to a scratch path and passed through the Makefile's `MDTABLEFIX`
  seam, rather than with the host's 0.6.0. Rationale: the two versions have
  different canonical wrap points, and here the difference is a **content**
  change, not a cosmetic one — 0.6.0's `--renumber` rewrote the cardinal in
  "the rebase replayed 18." to "1.", silently falsifying a sentence in this
  plan's own `Progress`. The committed bytes pass under 0.6.1, which is the
  version CI runs, so the host tool is the deviant one and its output must not
  be committed. Options considered: (a) commit the host formatter's output and
  let CI adjudicate; (b) pin the tool locally and revert the host's rewrite.
  (b) was chosen because (a) publishes a false statement in the interim and CI
  would then reject the *wrapping* while the *number* stayed wrong. Note the
  failure is invisible to a local `make fmt && make check-fmt` cycle, which is
  self-consistent under either version. Affected identifiers: none; this is a
  tooling decision, recorded here because it changed what the plan asserts.
  Date/Author: 2026-10-13 / implementing agent.

- Decision: move the header `Status:` from `DRAFT` to `IN PROGRESS`, and
  **decline** to mark roadmap item `4.3.1` complete. Rationale: the two are
  different claims and the evidence supports only the first. The header
  summarizes work done — the approval gate passed on 2026-09-27, `EP-M0` and
  `EP-M1` are complete, `ADR-030` is written and indexed, and three review
  rounds have been discharged — while the roadmap box asserts the *item's*
  deliverable, Proptest coverage for deterministic Ninja emission, which does
  not exist. The branch diff against `origin/main` is four files —
  `docs/adr-030-ninja-emission-determinism-contract.md`, `docs/contents.md`,
  this plan, and `typos.toml` — with no Rust, no `Makefile`, and no
  `.config/nextest.toml` change; `src/ninja_gen/named_action.rs` is absent on
  every ref, `make proptest` is absent from the `Makefile`, and the roadmap's
  three sub-items (insertion-order stability, `default` target ordering,
  `path_key` invariance) are unproven. Checking the box would assert coverage
  that does not exist — the exact defect class this plan has spent eight
  revisions removing, including a retraction published in `3180ccd2`. Options
  considered: (a) check the box on the strength of the plan being complete; (b)
  leave it unchecked and record why. (b) was chosen because roadmap 4.3.1's own
  text names Proptest coverage as the deliverable, not the plan. Affected
  identifiers: none; no obligation, tolerance, or acceptance clause changed.
  Date/Author: 2026-10-13 / implementing agent.

- Decision: reconcile the base **before** implementing `EP-M2`, rather than
  implementing against this branch's tree. Rationale: the sixth revision's
  `EP-M2` seam described `src/ninja_gen/mod.rs` with `src/ninja_gen/` already
  present, but `origin/main` has moved nine of that family's files into
  directory modules and dropped parent prefixes from child filenames. The
  layout contract reports 66 prefix groups and 27 prefixed-beside-directory
  violations on this branch's tree against 0 and 0 on `origin/main`, so the
  pre-rebase tree is not a base the contract accepts and implementing there
  would build on a removed seam. Options considered: (a) implement now and
  rebase later, rejecting conflicts as they appear; (b) rebase first, then
  re-derive the seam. (b) was chosen because the rehearsal is free and proves
  zero conflicts, so (a) buys nothing but rework. Affected identifiers: `EP-M2`
  only; no obligation, tolerance, or acceptance clause changed. Date/Author:
  2026-10-10 / implementing agent.

- Decision: record the `make proptest` selector as an **open question with a
  stated empirical discharge**, rather than asserting the anchored form as
  settled. Rationale: the first attempt at this entry claimed
  `test(/^NAME($|::)/)` was "the accepted spelling" and would match the
  property suites. That was wrong as written. Every accepted selector in
  `nextest_child_cargo_group_invariants.py` is anchored with `^`, while the
  target suites are `src/`-side and nextest lists them under names rooted at
  their module path (`ninja_gen::property_tests::…`), so a `^`-anchored
  module-segment selector may select nothing while exiting 0. The runtime
  verifier cannot be relied on to catch it: it scans only
  `REPO_ROOT / "tests"`, so it never applies its assertion to a `src/`-side
  test name. Discharging this by reading documentation would repeat the
  mistake, so it is discharged by reading `filter-match.status` from
  `cargo nextest list --message-format json` once the suites exist. Preferred
  alternative if a `Makefile` selector cannot work: a nextest test-group in
  `.config/nextest.toml`, which is the mechanism the contracts already police —
  no contract reads a `Makefile` `-E` argument at all. Date/Author: 2026-10-10
  / implementing agent.

- Decision: keep this ExecPlan pre-implementation and approval-gated.
  Rationale: the user stated the plan must be approved before implementation.
  Date/Author: 2026-09-09 / planning agent. **Superseded 2026-09-27**: the
  approval gate was passed when the user directed implementation to proceed,
  and the header status has since moved to `IN PROGRESS`. The entry is retained
  as the record of the decision as it stood on its date.

- Decision: state `P-1`'s non-empty-output clause as a **property-domain
  restriction** rather than a loader guarantee, and exclude empty target names
  from the generated domain. Rationale: the loader genuinely accepts a `name:`
  of `null`, `~`, `""`, or `[]`, lowering it to an output-less edge whose
  emitted Ninja real `ninja` rejects. Establishing the clause as a loader
  invariant would require *adding* validation, which changes user-visible
  behaviour for a manifest that currently loads — a scope increase this plan
  was not approved for, and one that belongs to whichever item owns manifest
  validation. Restricting the domain is the minimal honest statement. The
  alternative — leaving `P-1` as written — would have made every property built
  on it unsound in the same direction. Date/Author: 2026-10-10 / implementation
  agent.

- Decision: withdraw `OBL-ORDER`'s differential arm rather than repair it.
  Rationale: the arm conditioned an assertion on
  `GraphView(g_u) == GraphView(g_v)`, but the end-to-end arm had already been
  narrowed to hold the graph value fixed, and `GraphView` sorts through
  `BTreeMap`/`BTreeSet`. The antecedent therefore holds on every case, so the
  arm degenerates into the metamorphic comparison it was meant to supplement.
  Repair options were to vary the graph value (which the fourth revision had
  already rejected as unsound, since a differing key set is a differing graph)
  or to build an independently computed expected Ninja text (which is
  `OBL-NINJA`'s job). Withdrawing and naming `OBL-NINJA` avoids two obligations
  claiming the same gap. Date/Author: 2026-10-10 / implementation agent.

- Decision: construct, rather than sample, the collector arm's second ordering.
  Rationale: this reverses the fifth revision's disposition of the same concern.
  `prop_shuffle` samples a permutation, so a short input can and does come
  back unchanged; asserting a reorder would reject a valid case, and not
  asserting one leaves "perturbation by construction" false. Deriving two
  orders from one another and re-drawing until they differ gives both a real
  perturbation and no invalid rejection. The lesson is that "the test controls
  the permutation" must be checked against what the combinator actually does,
  not against what the arm's prose says it does. Date/Author: 2026-10-10 /
  implementation agent.

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
  `actions.sort_by_key` in `ordered_actions` is the only guard re-establishing
  a deterministic order over it. Date/Author: 2026-10-10 / implementation agent.
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
- Decision: treat a delegated head-drift verdict as unverified until it quotes
  the text it claims to have found, and re-check every finding by hand. A
  delegated analysis of the second review round reported two findings as
  "demonstrably addressed" at `b68b8289`; both were live, and the agent had
  matched line *numbers* against a revision whose content at those positions
  differed. Rationale: a line number is not evidence that a specific sentence
  was fixed. The check that works is to quote the anchor text and grep for it,
  which is cheap and cannot drift. The same rule already governs gate
  provenance, where a log belongs to a revision rather than a branch.
  Date/Author: 2026-10-10 / implementation agent.

- Decision: place `EP-M2`'s extracted module **inside** `src/ninja_gen/` as
  `named_action.rs`, rather than as the `src/ninja_gen_named_action.rs` sibling
  this plan's earlier wording implied. Rationale: the sibling form violates
  `main`'s module-layout contract on both counts it checks — a prefixed file
  beside a directory module, and membership of the already-oversized
  `('.', 'ninja')` prefix group. The contract is absent from this branch's base
  but is 44 commits ahead, wired into CI, and a rebase is already pending, so
  choosing the shape that survives it costs nothing now and avoids a structural
  rework later. Measured, not inferred: the upstream contract was executed
  against the live tree over four candidate layouts, which is also how the
  clean shape was identified. Date/Author: 2026-10-10 / implementation agent.

- Decision: name the injectivity property
  `path_key_is_injective_on_keyable_paths`, not the
  `path_key_is_injective_on_validated_paths` the trace link at line 392
  carried, and correct the link to match. The two words are not synonyms here
  and the difference is the obligation's whole point: "validated" would mean
  the property holds for everything `reject_unsupported_path_characters`
  admits, and it does not — an empty path passes that validator and still
  collides with an absent one. "Keyable" names the actual precondition, which
  is stronger than validation: non-empty *and* NUL-free. Naming the property
  after the weaker condition would have advertised a guarantee the suite does
  not hold, since the property's own strategy builds only keyable paths.
  Date/Author: 2026-10-16 / implementation agent.

- Decision: accept the plan's "two directed non-well-formed cases" as two
  *questions* and let `EP-M4` answer them, rather than treating the
  parenthetical as settled fact. It was wrong: the plan said two edges whose
  output lists are permutations of one another are "rejected by `from_manifest`
  but permitted by a direct constructor", and `BuildGraph::insert_edge` refuses
  the second one for exactly the loader's reason — the output *set* is already
  indexed, and a permutation has the same set. So the second case is a boundary
  that holds at every construction path, not a bypass, and the suite asserts
  *that* instead of asserting a collision that cannot occur. The genuinely
  reachable non-well-formed shape is the output-less edge, which is reachable
  through the loader too (empty `name:` → `StringOrList::Empty` → `to_paths` →
  `[]` → a duplicate scan over an empty slice finds nothing), and the suite
  asserts the emitter renders it rather than dropping it. This is the only one
  of the two the plan's own `OBL-NOLOSS` statement actually needs, because a
  dropped output-less edge would be invisible to the output-count assertion.
  Date/Author: 2026-10-16 / implementation agent.

- **The net-lines tolerance is three lines from exhausted, with `EP-M5` through
  `EP-M8` unwritten. This is a proposed deviation, and the plan is `BLOCKED`
  until it is accepted or the budget is re-scoped.** *(Resolved 2026-10-17 by
  the "Widen the cap" entry below, which supersedes this one's status. The
  measurements and options are retained as the record of how the breach was
  found.)* The `Scope` tolerance permits 34 files and 2,600 net added lines.
  Measured at `8a213218` against the merge base `e1df2ead` (which is also the
  hook's diff base, so the figure is not a stale-ref artefact):

  ```plaintext
  excluding docs/execplans/   add=3137  del=540  net=2597  / 2600
  including docs/execplans/   add=8352  del=540  net=7812
  files changed               28 / 34
  ```

  The count is given both ways because the tolerance's wording — "stop if net
  added lines exceed 2,600" — does not say whether the ExecPlan is inside or
  outside its own budget, and the two readings differ by a factor of three. The
  conservative reading is the one asserted above: 2,597. The generous reading
  has already breached, which is exactly the ambiguity a tolerance should not
  have, and it is recorded rather than resolved unilaterally.

  Where the budget went, by area: `ADR-030` 497, `src/ninja_gen/determinism/`
  1,792 (the `EP-M3` strategy and the `EP-M4` suites), other `src/ninja_gen/`
  58, `tests/kani_mutation_evidence_tests/` 140, elsewhere
  110. `EP-M3` and `EP-M4` therefore account for roughly 69% of a budget that
  has to cover four further milestones, including `EP-M8`'s Ninja oracle and
  documentation sweep. The file count is not the binding constraint — 28 of 34,
  and the artefact list budgets 12 test and strategy files against 6 written —
  so a re-scope has line room to move in on that axis.

  Options, with the trade-off each carries. **Widen the cap** (for example to
  4,000) and record why: the 2,600 number was chosen against the 4.2.3
  precedent, and that precedent's failure mode was under-delivery rather than
  overspend, so the cap may be measuring the wrong axis. **Narrow scope**: drop
  `OBL-NINJA` or move `EP-M7` out, both of which were already considered and
  declined once — the second because the measured `-n` finding shows the oracle
  covers the sidecar path nothing else reaches, so this option re-opens a
  settled question. **Split the plan** at the `EP-M4` plateau, shipping
  `EP-M0`–`EP-M4` as one PR and `EP-M5`–`EP-M8` as a successor.

  The third is the one this agent would recommend, and it is the one the
  milestone structure already supports: every milestone must end in a coherent
  validated state, `EP-M4` closes two complete obligations, and the branch's
  Proptest coverage would then be partial-but-real rather than
  partial-and-overrunning. It is not taken unilaterally because the user
  directed the wide scope and declined the split once already, which makes
  re-proposing it a decision for them rather than for this agent. No
  implementation work is blocked in the meantime — the compiler pool is
  unavailable, so no milestone can advance regardless — and this entry exists
  so the breach is visible before the budget is breached rather than after.
  Date/Author: 2026-10-16 / implementation agent.

- **The `Scope` tolerance is widened, by user direction, to 56 files and 6,000
  net added lines. The plan is no longer `BLOCKED` on it.** On 2026-10-17 the
  user answered the deviation above with "Widen the cap", declining both the
  scope cut and the split at the `EP-M4` plateau. The widening is therefore
  accepted, and the derivation below is recorded so the new number is a
  measured bound rather than a round figure someone felt comfortable with.

  The figure still excludes `docs/execplans/`, which resolves the ambiguity the
  previous entry had to give both ways. The ExecPlan is the document the
  tolerance protects the reader from, not part of the change under budget; the
  branch spends 8,352 added lines there and 3,138 everywhere else, so including
  it would make the cap meaningless as a measure of product scope.

  Spent at HEAD, measured against merge base `e1df2ead`:

  ```plaintext
  add=3138  del=541  net=2597   28 of 56 files
  ```

  Still to land, from the artefacts `EP-M5` through `EP-M8` name. Nine new
  suites, each sized by the nearest measured comparable rather than guessed:
  `order.rs` (two properties, the largest at ~400), `defaults/mod.rs` plus
  `defaults/generator.rs` (~200 together), `declaration.rs` (~250),
  `ninja_oracle.rs` (~400), `hashmap_boundary.rs` (~250),
  `action_hash_property_tests.rs` (~200), `process_tests.rs` (~300),
  `duplicates.rs` (~250), `cycles.rs` (~300). That is ~2,550 lines. Add seven
  mutation patches (`MUT-EDGESORT`, `MUT-ACTIONSORT`, `MUT-DEFSORT`,
  `MUT-DEFPOS`, `MUT-HASHMETA`, `MUT-DUPWITHIN`, `MUT-CYCLEDEPTH`) at ~14 lines
  each — the two `EP-M4` patches measure 12 and 14 — for ~100, two module
  wiring files at ~30, and `EP-M8`'s documentation set at ~250. Remaining
  inventory is therefore ~3,000 lines, and `2597 + 3000 = 5,597` sits inside
  6,000 with a margin of just over 400.

  The file axis moves from 34 to 56 for the same inventory: 28 spent, plus nine
  suites, two wiring files, seven patches and eight edited documents is 54, and
  56 keeps a two-file margin. The margin is deliberately narrow on both axes. A
  tolerance with slack invites the same drift that made 34 files look generous
  and then bind.

  One consequence is recorded rather than silently absorbed: `EP-M8` now owns
  more documentation than its section currently lists, including the users'-
  guide guarantee and the six translated READMEs decision. Those were already
  named there; the widening makes room for them rather than changing what they
  are. Date/Author: 2026-10-17 / implementation agent, on the user's explicit
  instruction.

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

- 2026-10-13 (eighth revision, status reconciliation): **no milestone or
  obligation changed**, and no design decision was revisited. Five inputs were
  re-derived against the live repository. The header `Status:` moves from
  `DRAFT` to `IN PROGRESS` after eight revisions of drift: the approval gate
  passed on 2026-09-27 and `EP-M1` has since delivered `ADR-030`, so neither
  `DRAFT` nor `APPROVED` described the document. The stale "Awaiting approval"
  sentence beneath it is removed for the same reason. Roadmap `4.3.1` is
  **deliberately left unchecked**: its deliverable is Proptest coverage, which
  does not exist, and the branch diff against `origin/main` is four
  documentation files. CI run `38056865899` is confirmed **fully green at the
  branch tip `7ec291a9`**, superseding the earlier `20e5f480` evidence and
  finally attributing the green result to the revision it describes. Finally,
  `origin/main` has moved again to `b14bde20` (12 commits), and re-deriving the
  seam shows `EP-M2` unaffected — `src/ninja_gen/mod.rs` is still 386 lines,
  the mutation-evidence file still 390, and the `ir`-root assertion still at
  line 212 — with one rebase hazard recorded for later: `main`'s `Makefile` now
  runs the shared CV-005 contract checker and pins `CV005_CONTRACTS_REF`, so a
  rebase must merge that hunk rather than let `EP-M2`'s new `proptest` target
  displace it. The compile-admission pool has cleared (longest wait 16 s
  against 5593 s), so the local Rust gates are runnable again. Two document
  defects were corrected in passing: the `Progress` claim of "**50** plans" is
  really **49** (the contract's `execplan_names` enumerates the same 49, and no
  plan has ever been deleted), and the header field itself had drifted eight
  revisions. One new hazard is recorded: the host's `mdtablefix` is **0.6.0**
  while CI pins **0.6.1**, and the host version's `--renumber` silently rewrote
  a factual "18." to "1." in this plan's prose — so `check-fmt` is now
  adjudicated with the pinned binary.

- 2026-10-10 (seventh revision, pre-`EP-M2` base reconciliation): no milestone
  or obligation changed. Three planning inputs were re-derived against the live
  repository, and one falsified premise is corrected. The branch was 44 commits
  behind `origin/main`, which has restructured `src/ninja_gen*` into directory
  modules and added `tests/workflow_contracts/rust_module_layout_test.py`.
  Applying that contract's logic gives 66 prefix-group violations and 27
  prefixed-beside-directory violations on this branch's tree, and 0 and 0 on
  `origin/main`; so `EP-M2`'s seam is re-derived against `origin/main`, not
  against the tree the sixth revision described. The `make proptest` selector
  is corrected: the two suffixes in use are `proptests` and `property_tests`,
  and `propert` is a prefix of only the second (`property` and `proptest`
  diverge at index four), so a single substring cannot match both and the
  selector carries both anchored forms. The third finding falsifies a premise:
  `supplemental_property_location`'s existential-resolution design was
  justified by flat and directory module forms coexisting, and `origin/main`
  has eliminated that coexistence, leaving zero collisions. The generalization
  is still required — `origin/main` still hard-codes an `ir` root — but it
  becomes a single-form rewrite, and the collision-witness rationale is
  withdrawn rather than carried forward.

- 2026-10-10 (sixth revision, `EP-M1` review round 3): a confirming CodeRabbit
  pass returned six findings — one major, five minor. All six were verified
  against the tree before anything changed; all six were **accepted**. Four
  were correctness defects in the plan's own reasoning rather than wording.

  The major finding is a **reversal of the previous round's disposition**, and
  it is the reviewer who was right. The fifth revision accepted that
  `prop_shuffle` may return its input order but held that the collector arm was
  exempt because it "shuffles an explicitly constructed `Vec` whose permutation
  the test controls". Re-reading the arm as it actually stood — "feeds each
  helper an explicitly `prop_shuffle`d `Vec` … genuinely permuted by
  construction" — shows the exemption was misapplied: `prop_shuffle` *samples*
  a permutation and cannot be made to return a different order, so "by
  construction" was a claim about the caller's intent, not about the case. The
  two arms are now genuinely disjoint: the collector arm constructs two orders
  that are *checked to be distinct* (re-drawing on coincidence rather than
  asserting), and only the end-to-end arm samples. The same correction is
  applied to `ADR-030`.

  Accepted — the differential-oracle finding, in a stronger form than raised.
  The finding said `GraphView` equality selects cases but supplies no
  independent expected output. Reading `src/graph_view/mod.rs:97` shows the
  antecedent is not merely uninformative but **tautological**: the end-to-end
  arm now holds the graph value fixed, and `GraphView` sorts through
  `BTreeMap`/`BTreeSet`, so `GraphView(g_u) == GraphView(g_v)` holds on every
  case. The arm is withdrawn and `OBL-NINJA`'s real-`ninja` oracle is named as
  the thing that actually closes the content gap.

  Accepted — findings 2, 4, and 6, three places where the plan's own text
  contradicted itself or the loader. The five-key minimum is now stated as a
  sampling threshold, not an unreachability result (divergence *is* reachable
  at 2–4 keys; it is simply unlikely, at 23/200 two-key trials). Acceptance
  item 3 no longer requires "no other test" to fail, which contradicted the
  mutation table's own two-obligation rows. And `P-1`'s non-empty-output clause
  is restated as a **property-domain restriction**: a probe against the built
  binary over five `name:` shapes showed that `null`, `~`, `""`, and `[]` all
  lower to an output-less edge and emit `build : <hash>`, which real `ninja`
  rejects — while an absent `name:` is a deserialization error, since the field
  has no `#[serde(default)]`. Finding 1 removed the only first-person pronoun
  in the ADR, which the style guide bans outside `README.md`.

  *Provenance note:* this pass reviewed `ea571de8`, not the `c91e96c2` it was
  briefed against, because four commits landed between dispatch and execution.
  Findings were therefore re-anchored by quoted content rather than line
  number; the ADR was byte-identical across all five revisions, so two findings
  applied unchanged. The lesson is the one already recorded for drift verdicts,
  applied to the dispatch itself: pin the revision and *stop writing to the
  tree* until the pass returns.
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
  qualifies. The acceptance criterion is now phrased as "Success is
  observable", per the second-person ban at
  `docs/documentation-style-guide.md:39`.

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

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

You can observe success without reading code. Run `make proptest` and see the
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
   (`src/ir/from_manifest_support.rs:46`) hashes the `Action` with
   `crate::hasher::ActionHasher::hash` and uses the hexadecimal digest as the
   map key.
2. Duplicate outputs are rejected. `find_duplicates`
   (`src/ir/from_manifest_support.rs:291`) fails the lowering if any output
   path is claimed twice, across targets or within one target. So no two
   distinct edges in a graph built this way share an output path.
3. A multi-output edge is stored once per output.
   `insert_edge_for_outputs` (`src/ir/from_manifest_support.rs:129`) clones the
   edge under each explicit output. An edge with no explicit outputs is
   silently not inserted — but `register_action` has already run, so its rule
   block is still emitted with no `build` statement referencing it.
4. `register_action` hard-codes
   `depfile: None, deps_format: None, pool: None, restat: false`. No manifest
   can currently populate those fields.

`src/ninja_gen/` emits the text. `generate` (`src/ninja_gen/mod.rs:106`) is the
simple path and rejects graphs needing staged serial lowering. `generate_bundle`
(`src/ninja_gen/dyndep.rs:91`) is what production uses and returns
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
(`src/ninja_gen/mod.rs:385`). Emitted bytes are therefore a function of
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
- A new architecture decision record, referred to throughout as **`ADR-NNN`**.
  The number is deliberately not allocated: `adr-021-*` is already claimed on
  four open branches, and `adr-020` on two. Allocate the number in a single
  pass as the final commit of the plan, after checking remote branches again.

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
FV-CONTRACT  -> ADR-NNN     -> EP-M1 -> docs/adr-NNN-ninja-emission-determinism-contract.md
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
    **and** within a total of 200 explicit outputs. The second bound is the one
    that governs cost: `insert_edge_for_outputs` stores an edge once per
    output, so the emitter's sort sees `targets.len()`, not the edge count.
    Without it, per-case cost swings 13-fold.
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
  the end-to-end variant re-materializes until the two iteration orders differ,
  failing the case if they cannot within a bounded number of attempts.
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
- **Regression seeds in `tests/` are inert.** *Severity: moderate. Likelihood:
  high.* Proptest's default `SourceParallel` persistence walks up from
  `file!()` for a `lib.rs`/`main.rs`; from an integration-test crate it finds
  neither, so three of the five committed `tests/*.proptest-regressions` files
  are probably never replayed — including two carrying careful retention
  comments. *Mitigation:* `EP-M0` question 5 verifies empirically and widens to
  the existing files; this plan places its properties library-side, where
  persistence works.
- **Counter-examples are unreadable.** *Severity: moderate. Likelihood: high.*
  Proptest prints the input's `Debug` regardless of the assertion message; a
  50/100 `GraphSpec` is tens of kilobytes. *Mitigation:* a handwritten compact
  `Debug` is part of `EP-M3`, not a later refinement.
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
  - *Covers.* Per-case `prop_assert!` that the shuffle actually reordered
    (skipping length < 2). Classes: empty list, singleton, already-sorted,
    reverse-sorted, repeated elements. Counts recorded.
  - *Witnesses.* Two directed tests. `path_key(["a","b"]) == path_key(["a\0b"])`
    shows the NUL precondition is load-bearing;
    `path_key([]) == path_key([""])` shows the non-emptiness precondition is.
    Both are stated as *disjunctions*: either the encoding admits the collision,
    in which case the corresponding precondition is load-bearing, or the
    encoding is injective without it. A future length-prefixed encoding would
    be a strict improvement and must not be blocked by these tests.
  - *Mutation.* `MUT-PATHKEY` deletes `parts.sort_unstable()`.

______________________________________________________________________

**OBL-NOLOSS** — *every distinct edge is emitted exactly once; no edge is
silently dropped.*

Statement: for every well-formed graph, the number of `build` statements in the
emitted text equals the number of distinct edges in the graph, and every
explicit output appears exactly once on the left-hand side of a `build` line.

This replaces the first draft's `OBL-GUARD`, which asserted a *mechanism*
("validation runs before sorting") as a proxy for the failure that matters. The
failure that matters is the `seen` set dropping a real edge. Asserting the
outcome survives refactors that legitimately move the validator, and it also
catches the silent drop of output-less edges that `insert_edge_for_outputs`
performs — which the first draft recorded as a surprise and left untested.

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
    multi-output edge (the case where the map holds several clones). Directed
    cases pin the two bypasses.
  - *Mutation.* `MUT-GUARD` moves `reject_unsupported_path_characters` after
    the edge sort, which under a NUL-bearing path collapses two edges into one
    and drops a `build` line.

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
every sidecar's path, content, and position. Because `RandomState` is outside
the seed, it re-materializes up to eight times until `targets.keys()` and
`actions.keys()` actually differ between the two graphs, and fails the case if
they cannot. That makes the predicate near-deterministic and removes the need
for a run-level vacuity floor entirely.

- **Method:** Proptest, metamorphic.
- **Rationale:** `RM-4.3.1.a`.
- **Domain:** `GraphSpec` values with 1 to 50 actions, 1 to 100 edges, and at
  most 200 explicit outputs in total. Both `DependencyOrder` variants; serial
  edges get 0 to 4 implicit dependencies so dyndep staging is genuinely
  exercised.
- **Oracle:** the second emission — metamorphic. Additionally, a *differential*
  arm asserts that whenever
  `GraphView::from_build_graph(g_u) == GraphView::from_build_graph(g_v)`, the
  bundles are equal. `GraphView` derives its ordering through `BTreeMap`/
  `BTreeSet` iteration rather than explicit `sort_by_key`, so it is an
  independent reference, not a reimplementation. Metamorphic testing alone
  cannot catch two emissions being wrong in the same way; this arm can.
- **Artefact:** `src/ninja_gen/determinism/order.rs`.
- **Non-vacuity:**
  - *Covers.* Per-case: the shuffle reordered; the two iteration orders
    differed. Recorded classes must include the *intersection* multi-output ∧
    phony ∧ dyndep-staged ∧ non-empty defaults, not only the marginals, and all
    four combinations of `BuildEdge::always` and `Action::restat` — `DisplayEdge`
    emits `restat` only when `edge.always && !action_restat`
    (`src/ninja_gen/display_edge.rs:37`).
  - *Mutation.* `MUT-EDGESORT` deletes `edges.sort_by_key` in both paths;
    `MUT-ACTIONSORT` deletes `actions.sort_by_key`. Each must fail the core
    property, which pinpoints the helper rather than the whole emission.

______________________________________________________________________

**OBL-NOHASH** — *no unordered collection is iterated inside the emitter.*

Statement: no source file under `src/ninja_gen/` iterates a
`std::collections::HashMap` or `HashSet` (`.iter()`, `.keys()`, `.values()`,
`.drain()`, or a `for` loop over the collection). Membership-only `HashSet` use
is exempt, with the two existing `seen` sets named as the documented exemptions.

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
  failure; the exemption list is asserted to be exactly the two known sets, so
  adding a third requires a deliberate edit.

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
processes, produces byte-identical `build.ninja`.

- **Method:** a directed `assert_cmd` integration test, not a property.
- **Rationale:** this is the statement `EP-M1`'s ADR publishes and the one users
  read, and **nothing in the repository tests it**. The eight
  `tests/snapshots/ninja/*.snap` fixtures are single-run. Every other
  obligation here runs two emissions in one process. Roughly twenty lines close
  the gap between what is promised and what is checked; without it the plan
  publishes a guarantee it does not verify.
- **Domain:** two or three existing fixture manifests, run twice each, plus one
  run under a different `TMPDIR` and locale set with `Command::env` so
  `Constraint 5` holds.
- **Oracle:** byte equality between runs.
- **Artefact:** `tests/ninja_determinism_process_tests.rs`.
- **Non-vacuity:** the test is validated by `MUT-DEFSORT`, which makes two runs
  diverge whenever the fixture declares more than one default.

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
  strictly lower indices, so acyclicity is structural. The cyclic arm adds
  exactly one back edge with generated endpoints. The third arm adds
  dependencies outside the namespace.
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

*Prototype; scratch commits, not merged.* Seven questions. Three are already
answered and recorded in `Artefacts and notes`; four remain.

1. Do two insertion permutations of a 50/100 graph actually produce different
   iteration orders, and how often? Also: how many re-materialization attempts
   does the bounded loop in `OBL-ORDER` need in practice?
2. Does `OBL-E2E` hold today? Build a manifest, permute `targets`, lower and
   emit both, compare bytes. Repeat for `actions`. **This gates `EP-M1`'s ADR
   content**, so it runs first.
3. *(Answered.)* Per-case cost.
4. Does a 50/100 counter-example shrink to something readable within the
   30-second `max_shrink_time`?
5. Do regression seeds persist for a library-side property? Widen the check to
   the three existing `tests/*.proptest-regressions` files suspected inert.
6. *(Answered.)* Is `adr-021` claimed?
7. *(Answered.)* Can a `src/`-side `#[cfg(test)]` module receive a
   `netsuke`-typed value from `test_support`?

*Acceptance:* every question has a recorded answer. *Conformance check:* if
question 2 answers "no", stop and escalate before `EP-M1`. *Recovery:* discard
the scratch commits.

### EP-M1 — state the determinism contract

*Assigned:* `FV-CONTRACT`, `ADR-NNN`.

Write `docs/adr-NNN-ninja-emission-determinism-contract.md` in the repository's
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
  `default_targets` emitted twice; output-less edges dropped while their rule
  block is still emitted; duplicate rule names last-writer-wins;
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
`path_key_is_permutation_invariant`; `MUT-GUARD` fails
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
`MUT-DEFSORT` fails `OBL-E2E` and `OBL-PROCESS`. The directed duplicate-rule
test demonstrates the excluded class genuinely diverges. *Conformance check:* if
`EP-M0` question 2 answered "no", `ADR-NNN` must already have been amended in
`EP-M1`; this milestone does not start otherwise.

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
`docs/netsuke-design.md` §5.5 referencing `ADR-NNN` and correcting its
unqualified determinism claim; a developers'-guide subsection; an annotation on
`ADR-004` recording its deferred obligations as discharged; the roadmap marked
done; and allocation of the real ADR number after re-checking remote branches.

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

| Handle           | Falsifies                               | Mutation                                                 |
| ---------------- | --------------------------------------- | -------------------------------------------------------- |
| `MUT-PATHKEY`    | `OBL-PATHKEY`                           | Delete `parts.sort_unstable()` in `path_key`.            |
| `MUT-GUARD`      | `OBL-NOLOSS`                            | Move the path-character validator after the edge sort.   |
| `MUT-EDGESORT`   | `OBL-ORDER`                             | Delete `edges.sort_by_key` in both paths.                |
| `MUT-ACTIONSORT` | `OBL-ORDER`                             | Delete `actions.sort_by_key`.                            |
| `MUT-DEFSORT`    | `OBL-DEFAULT`, `OBL-E2E`, `OBL-PROCESS` | Delete `defs.sort()`.                                    |
| `MUT-DEFPOS`     | `OBL-DEFAULT`, `OBL-NINJA`              | Emit `default` before edge rendering.                    |
| `MUT-HASHMETA`   | `OBL-ACTION`                            | Make the hasher skip `pool`.                             |
| `MUT-DUPWITHIN`  | `OBL-DUP`                               | Disable the within-one-target half of `find_duplicates`. |
| `MUT-CYCLEDEPTH` | `OBL-CYCLE`                             | Cap cycle traversal depth at 4.                          |

Nine, not the first draft's eleven. Two were cut because the existing nightly
`cargo-mutants` job (`.github/workflows/mutation-testing.yml`, 03:05 UTC over
`src/` with `--all-features`) already generates sort-deletion mutants
automatically. Handwritten patches earn their keep on faults `cargo-mutants`
cannot generate — the *reordering* ones, `MUT-GUARD` and `MUT-DEFPOS`. `EP-M8`
additionally records a scoped
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
6. Running the built binary twice over a fixture yields identical bytes, and
   `MUT-DEFSORT` breaks that.
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

### Answers already obtained (2026-09-09)

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
issues 592, 643, 644 and 646. `adr-020` is claimed twice. Hence `ADR-NNN`.

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

`EP-M0` questions 1, 2, 4, 5; `EP-M2` timings; per-obligation classification
counts; per-mutation transcripts; final gate logs; CodeRabbit outcomes.

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
- [ ] Approval gate: await explicit user approval before any implementation.
- [ ] `EP-M0`: answer questions 1, 2, 4, 5. Question 2 gates `EP-M1`.
- [ ] `EP-M1`: write `ADR-NNN`.
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
- (2026-09-09) Three of the five committed `tests/*.proptest-regressions` files
  are probably never replayed, because Proptest's default persistence finds no
  `lib.rs`/`main.rs` above an integration-test crate. Two carry careful
  retention comments that may have never had effect.
- (2026-09-09) `register_action` hard-codes `depfile`, `deps_format`, `pool`,
  and `restat`, so no manifest can populate them. `OBL-ORDER`'s direct-graph
  strategy is the only obligation reaching those emitter branches, which makes
  `OBL-E2E` *not* strictly stronger, contrary to the first draft.
- (2026-09-09) `process_rules` is last-writer-wins on duplicate rule names and
  no `DuplicateRule` error exists, so `OBL-E2E`'s unrestricted form is false.
- (2026-09-09) An output-less target still registers its action, so its `rule`
  block is emitted with no `build` statement referencing it.
- (2026-09-09) `make fmt`'s `mdtablefix --renumber` converted a wrapped line
  beginning "72." into an ordered-list item, truncating the sentence before it.
  No gate caught it; it was found by a reviewer reading the prose.

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

- Decision: use `ADR-NNN` as a placeholder and allocate the number in the final
  commit. Rationale: `adr-021` is claimed on four open branches and `adr-020`
  on two; the repository already has three historical collisions. Date/Author:
  2026-09-09 / planning agent.

- Decision: state `OBL-E2E` only for manifests with distinct rule names, and
  add a directed test reaching the excluded class. Rationale: `process_rules`
  is last-writer-wins and the unrestricted claim is false; a generator that
  allocates unique names would have passed vacuously on exactly the class where
  the contract fails. Date/Author: 2026-09-09 / planning agent.

- Decision: follow the obligation-driven ExecPlan style of `4-2-3`, with
  sentence-case back-matter headings. Rationale: it is the most recent
  precedent and the closest structural analogue. Date/Author: 2026-09-09 /
  planning agent.

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
- Three committed `tests/*.proptest-regressions` files are probably inert. This
  plan's properties are library-side, where persistence works; the existing
  files deserve a separate fix.
- `src/ninja_gen/mod.rs:178` clones a key that is dead immediately after
  (`seen.insert(key.clone())`), where `dyndep.rs:161` already moves it. A
  trivial follow-up, not taken here under `Constraint 1`.
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

- 2026-09-09 (first draft): established the obligation set, the determinism
  statements, the shared strategy, mutation-driven red, and eight milestones.
- 2026-09-09 (this revision): rewritten after a six-lens design review. The
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

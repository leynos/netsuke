# 4.3.1. Add Proptest coverage for deterministic Ninja emission

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT — AWAITING APPROVAL

## Purpose / big picture

Netsuke reads a `Netsukefile` manifest and writes a `build.ninja` file that the
Ninja build tool then executes. `docs/netsuke-design.md` states the promise
plainly: "Netsuke's pipeline is **deterministic**. Given the same `Netsukefile`
and environment variables, the generated `build.ninja` will be byte-for-byte
identical." The README makes a weaker public promise about "a reproducible,
fully static dependency graph".

That promise is currently held up by three explicit sort calls buried inside
the emitter, and by an invariant established a layer away in the manifest-to-IR
lowering. Nothing in the test suite stops a future change from removing a sort,
reordering validation, or introducing a new collection whose iteration order
leaks into the output. The existing coverage is snapshot tests over eight fixed
manifests and a handful of narrow property tests over single-edge graphs.

This plan closes that gap. After this work, a maintainer who deletes any one of
the emitter's ordering guarantees will see a named property test fail with a
shrunk counter-example naming the two graphs that disagreed and the first byte
at which their emitted Ninja text diverges. A maintainer who introduces a new
`HashMap` into the emission path and iterates it directly will see the same
failure.

The work also discharges two obligations that earlier roadmap items explicitly
deferred here. Roadmap item `4.2.1` and `ADR-004` both record that Kani proves
duplicate-output rejection and cycle rejection only for one to three nodes, and
that "the larger-N graph property is handed off to the future Proptest roadmap
item `4.3.1`". This plan is that hand-off.

Finally, the plan settles a contract question that
`docs/formal-verification-methods-in-netsuke.md` flags but no roadmap item
owns: whether byte-for-byte stable Ninja output is a public guarantee or an
implementation detail. Verifying an unstated property is verification without a
specification. This plan states the contract in a new ADR and in the
user-facing documentation first, then verifies exactly that statement.

You can observe success without reading any code. Run `make proptest` and see
the determinism suite pass. Apply any one of the recorded mutation patches under
`docs/verification/mutations/`, run `make proptest` again, and see a specific
named property fail with a minimized counter-example. Revert the patch. Read
`docs/users-guide.md` and find a stated determinism guarantee that matches what
the tests check.

## Context and orientation

### What Netsuke is

Netsuke is a Rust command-line build front end. A user writes a `Netsukefile`
in YAML. Netsuke parses it, expands control constructs (`foreach`, `when`),
renders string fields with the MiniJinja templating engine, lowers the result
into an intermediate representation (IR) called a *build graph*, and then emits
a `build.ninja` file. Ninja, a separate program, reads that file and runs the
build. Netsuke does not run compilers itself.

The crate is published as `netsuke-build`; the library target is `netsuke`.
Nothing in this plan changes the compiled behaviour of the shipped binary.

### The emission pipeline this plan targets

Three files matter. Read them before writing any assertion.

`src/ir/graph.rs` defines the build graph. Its shape is short enough to quote:

```rust
pub struct BuildGraph {
    pub actions: IrHashMap<String, Action>,
    pub targets: IrHashMap<Utf8PathBuf, BuildEdge>,
    pub default_targets: Vec<Utf8PathBuf>,
}
```

`IrHashMap<K, V>` is a plain `std::collections::HashMap` in ordinary builds
(`src/ir/graph.rs:28`); it swaps to a bounded bespoke map only under
`#[cfg(kani)]`. An `Action` is a recipe plus Ninja rule metadata. A `BuildEdge`
is one `build` statement: an action identifier, inputs, implicit dependencies,
explicit and implicit outputs, order-only dependencies, and two booleans.

`src/ir/from_manifest.rs` builds that graph from the manifest. Three facts from
it are load-bearing for everything below:

1. Actions are interned by content hash. `register_action`
   (`src/ir/from_manifest_support.rs:46`) hashes the `Action` value with
   `crate::hasher::ActionHasher::hash` and uses the resulting hexadecimal
   string as the map key. Equal actions therefore collapse to one key, and the
   key is a pure function of the action's content.
2. Duplicate outputs are rejected. `find_duplicates`
   (`src/ir/from_manifest_support.rs:291`) fails the lowering if any output
   path is claimed twice, whether by two different targets or twice within one
   target. Consequently *no two distinct edges in a well-formed graph share an
   output path*.
3. A multi-output edge is stored once per output.
   `insert_edge_for_outputs` (`src/ir/from_manifest_support.rs:129`) clones the
   edge into `targets` under each of its explicit outputs. An edge with no
   explicit outputs is silently not inserted at all.

`src/ninja_gen/` emits the text. There are two entry points. `generate`
(`src/ninja_gen/mod.rs:106`) is the simple path and rejects any graph needing
staged serial-dependency lowering. `generate_bundle`
(`src/ninja_gen/dyndep.rs:91`) is the path production actually uses; it returns
a `GeneratedNinja { build_file: String, dyndep_files: Vec<GeneratedDyndep> }`
and can lower serial dependency lists into Ninja `dyndep` sidecars.

Both paths do the same four things in the same order:

1. `reject_unsupported_path_characters` — rejects any path containing `$`,
   `:`, `|`, or any Unicode control character, including NUL
   (`src/ninja_gen/path_syntax.rs:52`).
2. `reject_reserved_paths` — rejects paths under `.netsuke/serial` or
   `.netsuke/dyndep` (`src/ninja_gen/dyndep.rs:346`).
3. `write_action_rules` — collects `graph.actions` into a `Vec`, sorts it by
   the map key, and writes one `rule` block per non-dependency-only action
   (`src/ninja_gen/mod.rs:216`).
4. Edge rendering — collects `graph.targets.values()` into a `Vec`, sorts it by
   `path_key(&edge.explicit_outputs)`, deduplicates on the same key, and writes
   one `build` statement per surviving edge (`src/ninja_gen/mod.rs:173` and
   `src/ninja_gen/dyndep.rs:155`).

Then, if `graph.default_targets` is non-empty, both paths clone it, call
`sort()`, and write a single `default` line.

`path_key` itself is five lines (`src/ninja_gen/mod.rs:242`):

```rust
pub(crate) fn path_key(paths: &[Utf8PathBuf]) -> String {
    let mut parts: Vec<String> = paths.iter().map(|p| p.as_str().to_owned()).collect();
    parts.sort_unstable();
    parts.join(&char::from(0).to_string())
}
```

It maps a list of paths to a canonical string by sorting the paths and joining
them with a NUL byte.

### What "deterministic" means here, precisely

Three different statements are easy to confuse. This plan keeps them apart.

The **process-level statement** is what the design document already claims: the
same manifest and environment give byte-identical output. This is what a user
cares about and what caching depends on.

The **graph-level statement** is what roadmap `4.3.1` actually names: emission
is invariant under `HashMap` insertion order. Two `BuildGraph` values that are
*equal as values* but whose maps were populated in different sequences must
emit identical bytes. This is the property that fails if a sort is removed,
because `std::collections::HashMap` uses `RandomState`: each map instance draws
a fresh hash key, so two maps built from the same pairs in one process will
generally iterate differently once they hold more than a couple of entries.

The **declaration-level statement** is stronger and not stated anywhere yet:
permuting the order in which targets are declared in the manifest does not
change the emitted bytes. It follows from the graph-level statement plus the
content-hash interning of actions and the sorting of `default_targets`, but
only if the lowering itself does not smuggle declaration order into any edge.
Confirming that is worth doing, because it is what a user would actually
predict, but it is a hypothesis to be measured before it is asserted.

One thing is explicitly *not* claimed. Within a single edge, the order of
`explicit_outputs`, `inputs`, `implicit_deps`, and `order_only_deps` is
preserved verbatim into the emitted `build` line by `DisplayEdge`
(`src/ninja_gen/display_edge.rs:22`). `path_key` sorts internally, so two edges
whose output lists are permutations of each other share a `path_key` but would
render differently. In a well-formed graph such a pair cannot exist, because
duplicate outputs are rejected — but this is an invariant of the *lowering*,
not of the emitter, and `BuildGraph`'s fields are public. That gap is the
reason obligation `OBL-GUARD` below exists.

### What Proptest is and how this repository already uses it

Proptest generates random values from a *strategy*, checks a property against
each, and on failure *shrinks* the input to a minimal counter-example. It is
already a dev-dependency at version `1.11.0` (the current release), and
`test_support` depends on it at the same version.

The house conventions, all confirmed against existing code, are:

- Use the `proptest! { ... }` macro block. No file in this repository
  implements the `Strategy` or `Arbitrary` traits directly, and neither
  `proptest-derive` nor `test-strategy` is a dependency. Strategies are
  functions returning `impl Strategy<Value = T>`, composed from
  `prop::collection::vec`, `prop_oneof!`, regex string strategies, `.prop_map`,
  and `prop_compose!`.
- Assert with `prop_assert!` and `prop_assert_eq!`, never `assert!` or
  `unwrap`, so shrinking is preserved.
- Set case counts inline with `#![proptest_config(...)]`. Existing counts range
  from 8 to 256 and scale inversely with per-case cost; 128 is the count used
  by the existing Ninja property suite.
- Library-side property tests live in a sibling file wired from the production
  module with `#[cfg(test)] #[path = "..."] mod property_tests;`. There is no
  central registry in `src/lib.rs`.
- Regression seed files are committed. Library-side seeds land under
  `proptest-regressions/<module path>.txt`; integration-test seeds land beside
  the test as `tests/<stem>.proptest-regressions`. Seeds retained for reasons
  other than a live defect carry a comment explaining why.

The existing Ninja property suite is `src/ninja_gen_property_tests.rs`, wired
from `src/ninja_gen/mod.rs:392`, with two submodules under
`src/ninja_gen_property_tests/`. One of them, `ninja_oracle.rs`, is important
prior art: it probes for a real `ninja` binary, and if one is present it writes
generated files to a temporary directory and runs `ninja -t commands` to check
that Ninja agrees with what Netsuke thinks it emitted. Because it must skip
when Ninja is absent, it drives `TestRunner::run` directly rather than using the
`proptest!` macro. This plan reuses that pattern.

`test_support/src/ninja_gen.rs` already exports
`paths_strategy(prefix, size_range) -> impl Strategy<Value = Vec<Utf8PathBuf>>`,
the only cross-crate strategy in the repository. There is no
arbitrary-`BuildGraph` strategy today; each property test hand-rolls the graph
shape it needs. Building a shared, well-formed, bounded graph strategy is the
central new artefact of this plan.

### Terms used in this plan

- **Build graph / IR** — the `BuildGraph` value described above.
- **Edge** — one `BuildEdge`, rendered as one Ninja `build` statement.
- **Action** — one `Action`, rendered as one Ninja `rule` block.
- **Well-formed graph** — a `BuildGraph` satisfying the invariants that
  `from_manifest` establishes: every edge has at least one explicit output;
  explicit output sets are pairwise disjoint and internally duplicate-free;
  every `action_id` referenced by an edge exists in `actions`; every edge value
  stored under key `k` has `k` among its explicit outputs; and no path contains
  a character the emitter rejects.
- **Insertion-order permutation** — building two `BuildGraph` values from the
  same set of key-value pairs by inserting those pairs in two different
  sequences. The resulting values are equal; their maps' iteration orders
  generally differ.
- **Metamorphic property** — a property relating the outputs of two runs on
  *related* inputs, rather than checking one output against a fixed expected
  value. "Permuting insertion order does not change the bytes" is metamorphic.
- **Oracle** — an independent source of truth. Here, the real `ninja` binary.
- **Non-vacuity** — evidence that a property could actually fail. A property
  that passes because its generator never produces an interesting input proves
  nothing.
- **Mutation patch** — a committed `.patch` file that deliberately breaks one
  production behaviour, used to demonstrate that a named test detects it.

## Signposts: documentation and skills

Read these before starting. They are listed in the order they become useful.

Repository documentation:

- `docs/roadmap.md` §4.3 — the item being delivered, and §4.2 for the deferred
  obligations this plan inherits.
- `docs/formal-verification-methods-in-netsuke.md` §"Proptest for determinism
  and manifest semantics" — the design basis, referred to below as `FV-DET`.
  Also its §"Determinism contract", which asks for the contract decision this
  plan makes.
- `docs/adr-004-bound-kani-ir-harnesses-to-small-n.md` — records the Kani bound
  and the hand-off to this item.
- `docs/netsuke-design.md` §1.2 (the determinism claim), §5.4 (Ninja file
  synthesis), and §5.5 (design decisions, including the sorting rationale).
- `docs/developers-guide.md` §"Property-based testing with proptest" — the
  house rules, and the two cited canonical examples.
- `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md`,
  `docs/snapshot-testing-in-netsuke-using-insta.md`,
  `docs/rstest-bdd-users-guide.md`, and `docs/rust-doctest-dry-guide.md` — the
  testing idioms this repository expects.
- `docs/documentation-style-guide.md` — required for every documentation edit.
- `AGENTS.md` — the binding rules: 400-line file ceiling, 80-column Markdown
  wrap, en-GB-oxendict spelling, the ban on in-process environment mutation,
  and the gate commands.
- `docs/adr-011-use-ninja-dyndep-for-serial-dependency-ordering.md` — why the
  dyndep bundle path exists at all.

Deliberate non-reference: `docs/rfcs/0012-netsukefile-property-testing.md`
describes a *manifest-author-facing* property-testing feature for Netsuke users
(roadmap phase 10). It is unrelated to this plan, which adds internal Rust
property tests. Do not conflate them.

Agent skills to load:

- `execplans` — this document's format and its living-section discipline.
- `rust-router`, then `rust-verification` for adversary selection, then
  `proptest` for strategy design, the filtering trap, and shrinking discipline.
- `rust-unit-testing` for assertion and helper shape.
- `hexagonal-architecture` for the boundary question in
  `Interfaces and dependencies`.
- `codegraph-mcp` for navigating callers and impact before editing.
- `arch-decision-records` for the ADR this plan writes.
- `en-gb-oxendict` for all prose.

External references consulted while drafting: the Ninja manual v1.13.1
(<https://ninja-build.org/manual.html>), specifically its statement that "A
default target statement must appear after the build statement that declares
the target as an output file", which the emitter satisfies by writing `default`
last; and the Proptest book's guidance on composing rather than filtering
strategies.

## Conformance basis

There is no Terms of Reference document in this repository. The upstream
artefacts are:

- `docs/roadmap.md`, item 4.3.1 and its three sub-items, referred to as
  `RM-4.3.1.a` (insertion-order stability), `RM-4.3.1.b` (`default` ordering),
  and `RM-4.3.1.c` (`path_key` invariance).
- `docs/roadmap.md`, item 4.2.1, whose first and third sub-items each record
  that "4.3.1 closes the larger-N Proptest coverage". These inherited
  obligations are referred to as `RM-4.2.1.dup` (duplicate-output rejection at
  larger N) and `RM-4.2.1.cyc` (cycle rejection at larger N).
- `docs/formal-verification-methods-in-netsuke.md`, §"Proptest for determinism
  and manifest semantics" — the design basis, `FV-DET`. Note that `FV-DET`
  lists a fourth bullet the roadmap omits: "action-hash stability for
  field-preserving permutations". This plan discharges it as `OBL-ACTION`.
- `docs/formal-verification-methods-in-netsuke.md`, §"Determinism contract" —
  `FV-CONTRACT`, the requirement to decide and document what is guaranteed.
- `docs/adr-004-bound-kani-ir-harnesses-to-small-n.md` — `ADR-004`, which
  records the hand-off.
- A new architecture decision record, `ADR-021`, written by this plan, stating
  the determinism contract.

Trace links:

```plaintext
RM-4.3.1.a   -> FV-DET      -> EP-M3 -> ninja_gen::determinism_property_tests::emission_is_insertion_order_invariant
RM-4.3.1.a   -> FV-DET      -> EP-M3 -> ninja_gen::determinism_property_tests::bundle_is_insertion_order_invariant
RM-4.3.1.b   -> FV-DET      -> EP-M3 -> ninja_gen::determinism_property_tests::default_line_is_the_ascending_sort
RM-4.3.1.c   -> FV-DET      -> EP-M2 -> ninja_gen::determinism_property_tests::path_key_is_permutation_invariant
RM-4.3.1.c   -> FV-DET      -> EP-M2 -> ninja_gen::determinism_property_tests::path_key_is_injective_on_validated_paths
FV-DET       -> FV-DET      -> EP-M2 -> ninja_gen::determinism_property_tests::validation_precedes_path_key_ordering
FV-DET       -> FV-DET      -> EP-M4 -> ir::action_hash_property_tests::action_hash_is_a_function_of_content
RM-4.2.1.dup -> ADR-004     -> EP-M5 -> ir::graph_property_tests::duplicate_outputs_are_rejected_at_larger_n
RM-4.2.1.cyc -> ADR-004     -> EP-M5 -> ir::graph_property_tests::cycles_are_rejected_at_larger_n
RM-4.2.1.cyc -> ADR-004     -> EP-M5 -> ir::graph_property_tests::acyclic_graphs_with_missing_deps_are_accepted
FV-CONTRACT  -> ADR-021     -> EP-M6 -> docs/adr-021-ninja-emission-determinism-contract.md
FV-CONTRACT  -> ADR-021     -> EP-M6 -> docs/users-guide.md (determinism guarantee)
RM-4.3.1.a   -> ADR-021     -> EP-M4 -> ninja_gen::determinism_property_tests::declaration_order_does_not_change_emission
RM-4.3.1.a   -> ADR-021     -> EP-M7 -> tests/ninja_determinism_oracle_tests.rs
```

Roadmap item 4.3.2 declares `Requires 4.1.1`, not this item, so nothing
downstream is blocked by the scope choices made here. Roadmap 4.4.1 owns the
README's *command placeholder* section; `ADR-021` and the determinism sentence
this plan adds are a different subject and do not collide with it.

## Constraints

These are hard invariants. Violating any of them is an escalation, not a
judgement call.

1. No change to the observable behaviour of the shipped binary. This plan adds
   tests, documentation, one ADR, and Make targets. If a property fails against
   current production code, that is a discovery to be recorded and escalated
   under `Tolerances`, not silently patched.
2. No widening of the public API of `netsuke::ir` or `netsuke::ninja_gen`.
   `path_key` is `pub(crate)` and must stay so; library-side property tests can
   reach it through `super::`. If a test genuinely cannot be written without
   widening, stop and escalate.
3. No new `[dependencies]`, `[dev-dependencies]`, or `[build-dependencies]`
   entries. `proptest 1.11.0`, `rstest 0.26.1`, `googletest 0.14.3`,
   `pretty_assertions 1.4.1`, and `insta 1` are already present and are
   sufficient. In particular, do not add `proptest-derive` or `test-strategy`;
   the house style is handwritten strategy functions.
4. No file may exceed 400 lines (`AGENTS.md`). The new strategy and property
   code must be split across sibling files from the outset rather than
   retrofitted.
5. No in-process environment mutation in tests. There is no
   `EnvLock`/`serial_test` escape hatch. The only exemption is subprocess
   isolation via `assert_cmd` or `Command::env`, which the Ninja oracle already
   uses.
6. Strategies must construct valid values, not filter for them.
   `prop_filter` and `prop_assume!` interact badly with shrinking and exhaust
   the rejection budget. Disjoint output sets, unique action identifiers, and
   acyclic edge sets must all be produced by construction from the seed.
7. Every property must be validated by a mutation. A property that still passes
   after the production behaviour it names is deliberately broken is a
   falsified property, not a passing one. Each mutation is recorded as a
   literal patch file under `docs/verification/mutations/`, not as prose.
8. Regression seed files are committed, and any seed retained for a reason
   other than a live defect carries a comment saying why.
9. Assertions inside `proptest!` bodies use `prop_assert*`. No `unwrap`, no
   `assert!`, no `panic!`.
10. Graph size stays within the roadmap bound: at most 50 actions and at most
    100 edges per generated graph.
11. All prose is en-GB-oxendict, wrapped at 80 columns, with code blocks
    wrapped at 120. Markdown must be `mdtablefix`-canonical; run `make fmt`
    and compare table *cells*, never rendered rows.
12. `make check-fmt`, `make typecheck`, `make lint`, `make doc-coverage`,
    `make test`, `make markdownlint`, and `make nixie` must pass at every
    milestone boundary.

## Tolerances (exception triggers)

Stop and escalate when any of these thresholds is reached. Do not work around
them.

- **Scope.** If implementation needs to touch more than 16 files beyond this
  ExecPlan, stop. The expected set is: `Makefile`; `.github/workflows/ci.yml`;
  `test_support/src/ninja_gen.rs` and any new sibling under
  `test_support/src/ninja_gen/`; `src/ninja_gen/mod.rs` (test-module wiring
  only); new files under `src/ninja_gen_determinism_property_tests/`;
  `src/ir/mod.rs` or `src/ir/graph.rs` (test-module wiring only); new files
  under `src/ir/`; `tests/ninja_determinism_oracle_tests.rs`;
  `docs/adr-021-ninja-emission-determinism-contract.md`; `docs/contents.md`;
  `docs/netsuke-design.md`; `docs/developers-guide.md`; `docs/users-guide.md`;
  `README.md`; `docs/roadmap.md`; the mutation patch files; and the committed
  regression seed files.
- **Production change.** If any property cannot be made to pass without
  changing production code, stop immediately. Record the counter-example in
  `Surprises & discoveries` and escalate. This is the most likely and most
  valuable failure mode in the whole plan; treat it as a finding, not an
  obstacle.
- **API widening.** If a test needs a symbol made more public than
  `pub(crate)`, stop and present options.
- **Runtime.** If the light property tier adds more than 45 seconds of wall
  time to `make test` on the reference machine (six-core Rocky 10, 64 GB RAM),
  stop and split the suite into light and heavy tiers as described in `EP-M1`.
  If the heavy tier exceeds 10 minutes, stop and reduce either the case count
  or the graph bound, and record the reduction in `Decision log`.
- **Shrinking.** If any property produces counter-examples that do not shrink
  to a small, readable graph within `max_shrink_iters`, stop and redesign the
  strategy. A 50-action counter-example that will not minimize is an unusable
  test.
- **Rejection rate.** If any strategy needs `prop_filter` or `prop_assume!` on
  a structural condition — as opposed to a genuinely rare edge — stop and
  redesign it to construct valid values. Record generator classification
  evidence showing the rejection rate is under 5%.
- **Mutation discipline.** If any property still passes after its matching
  mutation patch is applied, stop and redesign the property.
- **Lint friction.** If Clippy or the Whitaker Dylint suite cannot be satisfied
  without a broad `#[allow(...)]` umbrella, stop and escalate before adding
  one. Narrowly scoped suppressions with a `reason = "..."` clause are
  acceptable; blanket ones are not.
- **Gates.** If `make check-fmt`, `make lint`, `make typecheck`, or `make test`
  fails after two focused fix attempts, stop and escalate with the captured
  `/tmp` log paths.
- **Review.** If `coderabbit review --agent` raises unresolved correctness,
  testing, or documentation concerns, do not proceed until they are addressed
  or explicitly waived.
- **Contract disagreement.** If writing `ADR-021` reveals that the determinism
  guarantee cannot be stated without qualifying it in a way that contradicts
  the README or `docs/netsuke-design.md` §1.2, stop and escalate before editing
  either document.

## Risks

- **A property fails against current production code.** *Severity: high.
  Likelihood: moderate.* The most likely candidates are the declaration-order
  property (`OBL-E2E`), which depends on the lowering never smuggling
  declaration order into an edge, and the reserved-path or validation ordering
  in the dyndep bundle path. *Mitigation:* `EP-M0` runs each candidate property
  as a throwaway spike before any milestone commits to it. A confirmed failure
  is escalated under `Tolerances`, not fixed silently.
- **Weak strategy makes a strong property look strong.** *Severity: high.
  Likelihood: moderate.* A generator that only ever produces single-output
  edges with no implicit dependencies would pass every property here while
  proving almost nothing. *Mitigation:* every obligation carries an explicit
  non-vacuity clause naming the equivalence classes it must reach, checked with
  `proptest::prop_assert` on classification counters recorded in `EP-M0` and
  re-measured at `EP-M7`.
- **Insertion-order permutation does not actually change iteration order.**
  *Severity: high. Likelihood: low but real.* For very small maps, two
  `HashMap`s built from the same pairs may happen to iterate identically, which
  would make the property vacuous on those cases. *Mitigation:* the property
  records, per case, whether the two maps' raw iteration orders differed, and
  the suite asserts that a material fraction of cases exhibited a genuine
  difference. Cases where they did not are still checked but are not counted as
  evidence.
- **Test-suite runtime regresses the inner loop.** *Severity: moderate.
  Likelihood: moderate.* Fifty-action graphs emitted twice per case at 128
  cases is far more work than the existing properties do. *Mitigation:* `EP-M1`
  measures before committing, and the light/heavy split is pre-planned rather
  than retrofitted.
- **The 400-line file ceiling forces awkward splits late.** *Severity: low.
  Likelihood: high.* The strategy code alone will not fit in one file with the
  properties. *Mitigation:* the file layout in `Interfaces and dependencies` is
  decided up front.
- **Real-Ninja oracle is flaky or absent in CI.** *Severity: low. Likelihood:
  moderate.* *Mitigation:* reuse the existing `NinjaCommandOracle::try_create`
  skip-when-absent pattern verbatim; the oracle tier never gates the light
  suite.
- **Regression seeds do not persist for integration tests.** *Severity: low.
  Likelihood: moderate.* Two existing integration tests override
  `failure_persistence` with an explicit `FileFailurePersistence::Direct`,
  citing seeds being neither written nor replayed; three others work without
  it. The discrepancy is unexplained. *Mitigation:* `EP-M0` verifies
  persistence empirically for the new integration test by forcing a failure and
  confirming a seed file appears; if it does not, mirror the explicit override
  and record why in `Surprises & discoveries`.
- **ADR number collision.** *Severity: low. Likelihood: moderate.* The
  repository already has duplicate ADR numbers at 003, 004, and 014.
  *Mitigation:* before allocating `021`, enumerate remote branches and open
  pull requests for an existing `adr-021-*` file, exactly as the RFC-numbering
  caution requires.

## Verification plan

### Axioms (assumed, not proved)

- **AXIOM-PROPTEST.** Proptest 1.11.0 generates values from strategies and
  shrinks counter-examples correctly. Its internals are out of scope.
- **AXIOM-HASHMAP.** `std::collections::HashMap` with the default `RandomState`
  provides no iteration-order guarantee, and distinct map instances within one
  process generally draw distinct hash keys. This is what makes the
  insertion-order property testable in-process rather than requiring separate
  processes. It is also why the property must *measure* whether orders actually
  differed rather than assume it.
- **AXIOM-CAMINO.** `Utf8PathBuf`'s derived `Ord` is the lexicographic ordering
  of the underlying UTF-8 string. `Vec::sort` on `Vec<Utf8PathBuf>` therefore
  produces a total, content-determined order. Camino's internals are out of
  scope.
- **AXIOM-HASHER.** `crate::hasher::ActionHasher::hash` is a deterministic pure
  function of the `Action` value's canonical serialization. The cryptographic
  strength of the digest is out of scope; only its determinism is relied upon,
  and `OBL-ACTION` checks that reliance rather than assuming it.
- **AXIOM-NINJA.** The `ninja` binary parses `build.ninja` per its published
  manual, including the rule that a `default` statement must appear after the
  build statement declaring the target. Ninja's own correctness is out of
  scope; only Netsuke's use of it is verified.
- **AXIOM-SORT.** Rust's `slice::sort_by_key` is a stable sort and
  `sort_unstable` is not. Stability matters here: the edge sort is stable, so
  if the sort key were ever non-unique, ties would fall back to `HashMap`
  iteration order. `OBL-GUARD` exists precisely because that fallback is
  invisible until the key stops being unique.

### Obligations

______________________________________________________________________

**OBL-PATHKEY** — *`path_key` is invariant under permutation of its argument,
and injective over path lists that the emitter's validator accepts.*

Statement: for every list of paths `p`, and every permutation `q` of `p`,
`path_key(p) == path_key(q)`. Further, for any two lists `p` and `r` in which
no path contains a NUL byte, `path_key(p) == path_key(r)` if and only if `p` and
`r` are permutations of one another.

- **Method:** Proptest over generated path lists, plus a directed
  counter-example test for the injectivity precondition.
- **Rationale:** this is `RM-4.3.1.c`. Permutation invariance is what makes
  `path_key` a canonical key for an output *set*. Injectivity is what makes it
  safe to use as a deduplication key: if two distinct output sets could share a
  key, the emitter's `seen` set would silently drop a real edge.
- **Domain:** lists of 0 to 8 paths drawn from a generated pool, permuted with
  `prop_shuffle`. The injectivity half draws two independent lists from a
  shared pool so that collisions are reachable rather than astronomically
  unlikely.
- **Oracle:** the permutation half compares `path_key` against itself on a
  shuffled input — a metamorphic relation, not a reimplementation. The
  injectivity half compares against a sorted-multiset equality computed
  directly in the test, which is structurally different from `path_key`'s
  join-with-NUL implementation.
- **Artefact:** `src/ninja_gen_determinism_property_tests/path_key.rs`,
  properties `path_key_is_permutation_invariant` and
  `path_key_is_injective_on_validated_paths`.
- **Evidence:** `make proptest` reports both properties passing; the shrunk
  counter-example for the deliberate mutation names two short path lists.
- **Non-vacuity:**
  - *Covers.* The generator must reach: the empty list; a single-element list;
    a list already in sorted order; a list in reverse order; and a list
    containing repeated paths. Record the classification counts in
    `Artefacts and notes`. A run in which the repeated-path class is never
    reached is a failed run, not a passing one.
  - *Witness.* The injectivity property is guarded by a precondition. Exhibit
    at least one concrete pair `p = ["a", "b"]`, `r = ["a\0b"]` for which
    `path_key(p) == path_key(r)` despite `p` and `r` not being permutations,
    as a directed `#[test]` documenting that the precondition is load-bearing
    and not vacuous. This is the case `OBL-GUARD` shows is unreachable in
    practice.
  - *Mutation.* `MUT-PATHKEY` deletes
    `parts.sort_unstable()`. `path_key_is_permutation_invariant` must fail.

______________________________________________________________________

**OBL-GUARD** — *the path-character validator runs before any
`path_key`-derived ordering, on both emission paths.*

Statement: for every `BuildGraph` containing a path with a NUL byte (or any
other character `unsupported_character` rejects), both `generate` and
`generate_bundle` return `Err(NinjaGenError::UnsupportedPathCharacter { .. })`,
and do so without having emitted any output.

- **Method:** Proptest over graphs seeded with one adversarial path, plus a
  directed ordering test.
- **Rationale:** `OBL-PATHKEY`'s injectivity has a precondition. This
  obligation discharges that precondition for the only callers that matter. It
  is the obligation that would catch a future refactor moving validation after
  sorting — a change that would compile, pass every existing test, and
  reintroduce a silent edge-dropping bug.
- **Domain:** well-formed graphs of 1 to 6 edges, into exactly one of which a
  path containing a generated control character is injected, at a generated
  position among explicit outputs, implicit outputs, inputs, implicit
  dependencies, and order-only dependencies.
- **Oracle:** the expected error variant, determined by the test from the
  injected character, not by calling `unsupported_character`.
- **Artefact:** `src/ninja_gen_determinism_property_tests/guard.rs`, property
  `validation_precedes_path_key_ordering`.
- **Evidence:** `make proptest` passes; the mutation below fails it.
- **Non-vacuity:**
  - *Covers.* The injected character must reach each of `$`, `:`, `|`, NUL, and
    at least one non-NUL control character, and each of the five path
    positions. Classification counts recorded.
  - *Mutation.* `MUT-GUARD` moves the
    `reject_unsupported_path_characters` call in `src/ninja_gen/mod.rs` and
    `src/ninja_gen/dyndep.rs` to after the edge sort.
    `validation_precedes_path_key_ordering` must fail, and the failure must
    name the NUL case specifically.

______________________________________________________________________

**OBL-ORDER** — *emission is invariant under `HashMap` insertion order.*

Statement: for every well-formed graph specification `s` and every pair of
insertion permutations `(u, v)` of its actions and edges, the graphs `G_u` and
`G_v` built by inserting in those orders satisfy
`generate_bundle(G_u) == generate_bundle(G_v)`, comparing the whole bundle: the
`build_file` string, and the `dyndep_files` vector including each sidecar's
path, contents, and position. When neither graph requires dyndep staging, the
same holds for `generate`.

- **Method:** Proptest, metamorphic, over generated well-formed graphs bounded
  to 50 actions and 100 edges.
- **Rationale:** this is `RM-4.3.1.a`, the heart of the item. It is the
  property that fails if any of the three sorts is removed, if a new unsorted
  collection enters the emission path, or if a sort key stops being unique.
- **Domain:** `GraphSpec` values (see `Interfaces and dependencies`) with 1 to
  50 actions and 1 to 100 edges, output namespaces allocated without
  replacement so disjointness holds by construction, and each edge's
  `action_id` drawn from the generated action identifiers. Both
  `DependencyOrder::Parallel` and `DependencyOrder::Serial` are generated, and
  serial edges are given between 0 and 4 implicit dependencies so that the
  dyndep staging path is genuinely exercised.
- **Oracle:** the second emission of the same specification — a metamorphic
  relation. No reimplementation of the emitter exists or is permitted.
- **Artefact:** `src/ninja_gen_determinism_property_tests/insertion_order.rs`,
  properties `emission_is_insertion_order_invariant` and
  `bundle_is_insertion_order_invariant`.
- **Evidence:** `make proptest` passes. On failure the assertion reports the
  first differing byte offset and a windowed excerpt of both outputs, not two
  50-kilobyte strings.
- **Non-vacuity:**
  - *Covers.* Cases must reach: graphs whose two maps genuinely iterate in
    different orders (recorded per case and asserted to exceed a floor across
    the run — this is the anti-vacuity check that matters most here); graphs
    with at least one multi-output edge; graphs with at least one
    dependency-only (`phony`) action; graphs requiring dyndep staging; graphs
    with a non-empty `default_targets`; and all four combinations of
    `BuildEdge::always` and `Action::restat`, because `DisplayEdge` emits the
    `restat` flag only when `edge.always && !action_restat`
    (`src/ninja_gen/display_edge.rs:37`), an edge-to-action interaction a naive
    generator would never vary. A run in which the
    "iteration orders actually differed" count is zero must be treated as a
    failure of the test, not a pass of the property.
  - *Mutation.* Three patches, each of which must fail this property:
    `MUT-EDGESORT` (delete
    `edges.sort_by_key` in both `src/ninja_gen/mod.rs` and
    `src/ninja_gen/dyndep.rs`);
    `MUT-ACTIONSORT` (delete
    `actions.sort_by_key` in `write_action_rules`); and
    `MUT-FIRSTOUT` (replace the
    `path_key` sort key with the edge's first explicit output, making the key
    non-unique for multi-output edges and thereby exposing the stable-sort
    fallback described in `AXIOM-SORT`).

______________________________________________________________________

**OBL-DEFAULT** — *the emitted `default` line is exactly the ascending sort of
`default_targets`, and is emitted after every `build` statement.*

Statement: for every well-formed graph with non-empty `default_targets`, the
emitted text contains exactly one line beginning `default`, that line's
operands equal `default_targets` sorted ascending with duplicates preserved,
and that line's byte offset is greater than the offset of every `build` line.

- **Method:** Proptest over generated default sets, including permutations and
  deliberate duplicates.
- **Rationale:** this is `RM-4.3.1.b`. The ordering half is the stated
  requirement. The positional half encodes `AXIOM-NINJA`: the Ninja manual
  requires a `default` statement to follow the build statement declaring its
  target, so emitting `default` last is a correctness requirement, not a
  stylistic one, and nothing else in the suite pins it.
- **Domain:** graphs of 1 to 20 edges whose `default_targets` is a generated
  sub-multiset of the declared explicit outputs, permuted with `prop_shuffle`,
  with a generated number of deliberate repetitions between 0 and 3.
- **Oracle:** a sorted clone computed in the test with `Vec::sort`, compared
  against the parsed operands of the emitted line. This is structurally the
  same call the production code makes, so the *ordering* half is weak on its
  own; its strength comes from the permutation-invariance half, which compares
  emissions from two different input permutations and cannot be satisfied by a
  copied implementation.
- **Artefact:** `src/ninja_gen_determinism_property_tests/defaults.rs`,
  property `default_line_is_the_ascending_sort`.
- **Evidence:** `make proptest` passes.
- **Non-vacuity:**
  - *Covers.* Cases must reach: an empty `default_targets` (asserting no
    `default` line is emitted at all); a single target; a set already sorted; a
    set in strictly descending order; and a set containing duplicates. Record
    the counts. The duplicate class is what pins the documented decision that
    `Vec::sort` does not deduplicate.
  - *Mutation.* `MUT-DEFSORT` deletes
    `defs.sort()` in both emission paths;
    `MUT-DEFPOS` moves the
    `default` block before edge rendering. Both must fail this property, the
    second on the positional half.

______________________________________________________________________

**OBL-ACTION** — *an action's interned identifier is a function of the action's
content, and is unchanged by field-preserving permutations of the manifest that
produced it.*

Statement: for every pair of `Action` values `a` and `b`,
`ActionHasher::hash(a) == ActionHasher::hash(b)` if and only if `a == b`, over
the generated domain. Consequently, two manifests that declare the same actions
in different orders intern them to the same identifiers.

- **Method:** Proptest over generated `Action` values.
- **Rationale:** this is the fourth `FV-DET` bullet, which the roadmap's
  sub-items omit. It is a genuine prerequisite for `OBL-ORDER`: the action sort
  key's uniqueness, and hence the absence of a stable-sort tie, depends on the
  interning being content-determined. Without this obligation, `OBL-ORDER`
  rests on an unchecked assumption.
- **Domain:** `Action` values with generated recipes (scalar command,
  command list, and dependency-only variants), and generated `Option<String>`
  metadata for `description`, `depfile`, `deps_format`, and `pool`, plus the
  `restat` flag. Pairs are drawn so that both the equal and unequal cases are
  reachable: one arm generates `(a, a.clone())`, the other generates two
  independent values, and a third mutates exactly one field of a base value.
- **Oracle:** structural equality of the `Action` values, computed by the
  derived `PartialEq`, which is independent of the hasher's serialization path.
- **Artefact:** `src/ir/action_hash_property_tests.rs`, property
  `action_hash_is_a_function_of_content`.
- **Evidence:** `make proptest` passes.
- **Non-vacuity:**
  - *Covers.* All three arms must fire, and the single-field-mutation arm must
    reach each of the six `Action` fields. Record the counts. A run in which
    the unequal arms never produce a hash difference means the generator is
    producing degenerate actions.
  - *Mutation.* `MUT-HASHMETA`
    makes the hasher skip the `pool` field. The single-field-mutation arm must
    fail with `pool` named in the shrunk counter-example.

______________________________________________________________________

**OBL-E2E** — *permuting the declaration order of a manifest's targets does not
change the emitted bytes.*

Statement: for every generated manifest that lowers successfully, and every
permutation of its `targets` list (and independently of its `actions` list and
`rules` list), lowering and emitting the permuted manifest yields byte-identical
`GeneratedNinja` output.

- **Method:** Proptest, metamorphic, end-to-end from the manifest AST through
  `BuildGraph::from_manifest` to `generate_bundle`.
- **Rationale:** this is the statement a user would actually predict from the
  README, and it is the one `ADR-021` will commit to. It is strictly stronger
  than `OBL-ORDER` because it also covers the lowering: it fails if
  `from_manifest` ever smuggles declaration order into an edge's field order,
  into `default_targets` in a way the sort does not erase, or into an interned
  identifier.
- **Domain:** generated manifest ASTs with 1 to 12 rules and 1 to 30 targets,
  built directly as typed `NetsukeManifest` values rather than as YAML text, so
  that parsing and Jinja rendering (owned by roadmap 4.3.2 and 4.3.3) are out
  of scope. Output namespaces are allocated without replacement so lowering
  succeeds by construction; dependency edges are drawn only from
  already-allocated outputs with a strictly increasing index so acyclicity
  holds by construction.
- **Oracle:** the emission of the permuted manifest — metamorphic.
- **Artefact:**
  `src/ninja_gen_determinism_property_tests/declaration_order.rs`, property
  `declaration_order_does_not_change_emission`.
- **Evidence:** `make proptest` passes. **This obligation is contingent on
  `EP-M0`.** If the spike shows it does not currently hold, the failure is
  escalated under `Tolerances` and this obligation is either narrowed with a
  recorded justification or removed, with `ADR-021` weakened to match. Do not
  weaken the test to fit the code without recording the decision.
- **Non-vacuity:**
  - *Covers.* Cases must reach: manifests where the permutation genuinely
    reorders targets; manifests with at least two targets sharing an identical
    recipe (so interning collapses them and the permutation could otherwise
    change which one "wins"); manifests with declared `defaults`; and manifests
    with cross-target dependencies. Record the counts.
  - *Mutation.* `docs/verification/mutations/defaults-not-sorted-at-emit.patch`
    (reuse of the `OBL-DEFAULT` mutation) must fail this property too, since
    `default_targets` is the one field that carries declaration order into the
    graph. Additionally,
    `MUT-IDINDEX` appends the
    target's declaration index to the interned action identifier; this
    property must fail while `OBL-ORDER` continues to pass, demonstrating that
    `OBL-E2E` is genuinely stronger and not a restatement.

______________________________________________________________________

**OBL-DUP** — *duplicate outputs are rejected at larger N.*

Statement: for every generated manifest containing at least one output path
claimed twice — whether by two different targets or twice within one target —
`BuildGraph::from_manifest` returns `Err(IrGenError::DuplicateOutput { .. })`,
and the reported path list contains the colliding path. For every generated
manifest whose output paths are pairwise distinct, lowering does not return
`DuplicateOutput`.

- **Method:** Proptest over generated manifests with a deliberately injected
  collision, paired with a control arm that injects none.
- **Rationale:** this is `RM-4.2.1.dup`, the obligation `ADR-004` deferred here.
  Kani proves it for one to three nodes with fixed minimal manifests; this
  extends it to the generated range up to 30 targets, and adds the negative
  arm, which Kani's fixed manifests do not cover.
- **Domain:** manifests of 2 to 30 targets over an allocated output namespace,
  with a generated collision mode (`across targets`, `within one target`, or
  `none`) and generated positions.
- **Oracle:** the injected collision is known to the test by construction; the
  expected error variant and the expected colliding path are both computed
  without calling `find_duplicates`.
- **Artefact:** `src/ir/graph_property_tests/duplicates.rs`, property
  `duplicate_outputs_are_rejected_at_larger_n`.
- **Evidence:** `make proptest` passes.
- **Non-vacuity:**
  - *Covers.* All three collision modes must fire, including `none`. Without
    the `none` arm the property would be satisfied by a lowering that rejects
    every manifest. Record the counts, and assert that the `none` arm produces
    a successful lowering rather than merely a non-`DuplicateOutput` error.
  - *Mutation.*
    `MUT-DUPWITHIN`
    removes the within-one-target half of `find_duplicates`. The
    `within one target` arm must fail.

______________________________________________________________________

**OBL-CYCLE** — *dependency cycles are rejected at larger N, and missing
dependencies do not create false cycles.*

Statement: for every generated manifest whose target dependency relation
contains a cycle, `BuildGraph::from_manifest` returns
`Err(IrGenError::CircularDependency { .. })`. For every generated manifest
whose relation is acyclic, lowering succeeds, even when some declared
dependencies name paths that no target produces.

- **Method:** Proptest over two generator arms, one acyclic by construction and
  one with an injected back edge.
- **Rationale:** this is `RM-4.2.1.cyc`. `ADR-004` records that Kani covers
  self-edges and two- to three-node cycles, and that "existing unit tests keep
  cycle path canonicalization and missing-dependency reporting covered until
  roadmap item `4.3.1` expands generated graph coverage".
- **Domain:** target sets of 2 to 30 nodes. The acyclic arm draws each target's
  dependencies only from strictly lower indices, so acyclicity holds by
  construction with no filtering. The cyclic arm takes an acyclic graph and
  adds exactly one edge from a lower index to a higher one, with generated
  endpoints, guaranteeing a cycle by construction. A third arm adds
  dependencies on paths outside the allocated namespace, exercising the
  missing-dependency path.
- **Oracle:** cyclicity is known by construction from the generator's own
  index discipline, not computed by calling the production cycle detector.
- **Artefact:** `src/ir/graph_property_tests/cycles.rs`, properties
  `cycles_are_rejected_at_larger_n` and
  `acyclic_graphs_with_missing_deps_are_accepted`.
- **Evidence:** `make proptest` passes.
- **Non-vacuity:**
  - *Covers.* Cases must reach: self-edges; two-node cycles; cycles of length
    at least 8 (the range Kani cannot reach, which is the entire point of this
    obligation); acyclic graphs with no missing dependencies; and acyclic
    graphs with at least one missing dependency. Record the counts. If the
    long-cycle class count is zero, the obligation has not been discharged.
  - *Mutation.* `MUT-CYCLEDEPTH`
    caps the cycle detector's traversal depth at 4. The long-cycle class must
    fail while the short-cycle class continues to pass, demonstrating the
    obligation reaches strictly beyond the Kani bound.

______________________________________________________________________

**OBL-NINJA** — *permuted emissions are not merely byte-identical but
semantically accepted by real Ninja.*

Statement: for every generated well-formed graph, when a `ninja` binary is
available, the emitted `build.ninja` (with sidecars written alongside) parses
without error, and `ninja -t commands` over the graph's default targets returns
the same command list for both insertion permutations.

- **Method:** Proptest driven through `TestRunner::run` (not the `proptest!`
  macro, so the whole property can be skipped when Ninja is absent), using the
  existing `NinjaCommandOracle` pattern.
- **Rationale:** byte equality could in principle be preserved by an emitter
  that produced identical *garbage*. This obligation closes that gap with an
  independent oracle, and simultaneously discharges the `AXIOM-NINJA`
  interaction — that `default` really does have to come last, and that
  Netsuke's escaping of generated paths survives a real parse.
- **Domain:** smaller graphs than `OBL-ORDER` — 1 to 8 actions and 1 to 12
  edges — because each case spawns a subprocess. Paths are restricted to
  characters that are legal on both POSIX and Windows filesystems.
- **Oracle:** the `ninja` binary. Structurally independent of Netsuke.
- **Artefact:** `tests/ninja_determinism_oracle_tests.rs`.
- **Evidence:** the test reports the number of cases actually run; when Ninja
  is absent it reports a skip. A run reporting zero cases *and* no skip is a
  failure.
- **Non-vacuity:**
  - *Covers.* Cases must reach graphs with multi-output edges, phony edges, and
    a non-empty `default` line. Record the counts.
  - *Mutation.* `MUT-DEFPOS`
    (reused) must cause real Ninja to reject the file, confirming the oracle
    detects a semantic break that byte comparison alone would not.

### Residual gaps

These are stated so they are not mistaken for coverage:

- Proptest samples; it does not prove. The obligations above hold over the
  generated domain at the configured case counts, not over all inputs. Where an
  exhaustive small-N result exists, it is the Kani harnesses from 4.2.x, and
  the two layers are complementary by design.
- Manifest parsing, `foreach`/`when` expansion, and MiniJinja rendering are out
  of scope; they belong to roadmap items 4.3.2 and 4.3.3. `OBL-E2E` starts from
  a typed `NetsukeManifest`, not from YAML text.
- Cross-process determinism is not tested. The properties run two emissions in
  one process, which is sufficient to defeat `RandomState` (per
  `AXIOM-HASHMAP`) but does not exercise, for example, locale- or
  environment-dependent formatting. The existing snapshot tests cover the
  fixed-manifest cross-run case.
- Path *content* determinism on Windows is not covered beyond what
  `OBL-NINJA`'s restricted alphabet reaches. `ADR-017` already governs UTF-8
  invocation paths.
- The `graph` subcommand's renderer (`src/graph_view/`) also sorts at the IR
  boundary and also promises deterministic output. It is a different emitter
  and is out of scope here; note it in `ADR-021` as adjacent but unverified by
  this item.

## Plan of work

### Stage A — measure and confirm (no production changes)

Establish that the properties are worth writing and that the strategies can
reach the classes they claim. Nothing is committed to `main` from this stage
except measurements recorded in this document.

### Stage B — red

Add each property in a form that fails for the intended reason before the
supporting strategy or wiring exists, and record the failure. Because the
production code is expected to be correct already, "red" here means the test
fails to compile or fails on a deliberately mutated build — not that production
behaviour is broken. Each obligation's mutation patch is written *before* its
property is declared complete, and the red evidence is the mutated run.

### Stage C — green: obligations discharged in dependency order

Discharge `OBL-PATHKEY` and `OBL-GUARD` first, because `OBL-ORDER` depends on
their conclusions. Then `OBL-ACTION`, then `OBL-ORDER` and `OBL-DEFAULT`, then
`OBL-E2E`, then the inherited `OBL-DUP` and `OBL-CYCLE`, then `OBL-NINJA`.

### Stage D — contract, documentation, and wider validation

Write `ADR-021`, update the design document, developers' guide, users' guide,
README, and roadmap, and run the full gate set plus a CodeRabbit review.

## Milestones and plateaus

Each milestone ends in a state that is correct, gated, and safe to stop at.

### EP-M0 — feasibility and measurement spike (prototyping)

*Assigned:* de-risking for all obligations. *Prototype; not merged as-is.*

Write throwaway tests, on a scratch commit, answering six questions:

1. Does an insertion-order permutation of a 50-action, 100-edge graph actually
   produce different `HashMap` iteration orders, and in what fraction of cases?
   Measure by collecting `graph.targets.keys()` into a `Vec` for each
   permutation and comparing.
2. Does `OBL-E2E` hold today? Build a small manifest by hand, permute its
   `targets`, lower and emit both, and compare bytes. Repeat for `actions` and
   `rules`.
3. What does one case of the largest planned property cost in wall time?
   Measure `generate_bundle` twice on a 50/100 graph.
4. Does a generated 50/100 counter-example shrink to something readable?
   Deliberately break a sort locally and observe the shrunk output.
5. Do regression seeds persist for a new `tests/*.rs` proptest without an
   explicit `failure_persistence` override? Force a failure and look for the
   file.
6. Is `adr-021-*` already claimed on any remote branch or open pull request?

*End state:* a scratch branch, deleted afterwards, and six measurements written
into `Artefacts and notes`. *Acceptance:* every question has a recorded answer.
*Conformance check:* if question 2 answers "no", stop and escalate before
`EP-M1`. *Recovery:* discard the scratch commit; nothing is merged.

### EP-M1 — measured test tiering and the `proptest` Make target

*Assigned:* the tiering decision the user asked to be measured, not assumed.

Using the `EP-M0` cost figure, add a `proptest` Make target that runs the
property suites through `cargo nextest` with a filter expression, capturing
output under `/tmp` in the house style. Measure `make proptest` and measure the
delta it adds to `make test`. Then decide, and record the decision with its
numbers:

- If the light suite adds under 45 seconds to `make test`, keep one tier: all
  properties run under `make test`, and `make proptest` is a convenience filter
  for the same tests.
- Otherwise, split. Light properties keep low case counts and run under
  `make test`; heavy properties (the 50/100 graphs, `OBL-E2E`, and the
  inherited IR obligations at their full range) move behind a
  `PROPTEST_HEAVY=1` environment gate read at test start via an injected
  provider — never by mutating the harness environment — and run under a new
  `make proptest-heavy` target wired into a CI job alongside `kani-smoke`.

*End state:* `make proptest` exists, is documented in `make help`, and the
tiering decision is recorded with measurements. *Acceptance:* `make proptest`
runs and reports the existing property tests passing; `make test` timing before
and after is recorded. *Conformance check:* `Constraint 5` — the heavy gate
must not mutate the process environment. *Recovery:* the target is additive;
delete it to revert.

### EP-M2 — `path_key` canonicality and the validator ordering

*Assigned:* `RM-4.3.1.c`, `OBL-PATHKEY`, `OBL-GUARD`.

*End state:* `src/ninja_gen_determinism_property_tests/` exists with `mod.rs`,
`path_key.rs`, and `guard.rs`, wired from `src/ninja_gen/mod.rs` with
`#[cfg(test)] #[path = ...] mod determinism_property_tests;`. The directed
NUL-collision witness test is present and documents why `OBL-GUARD` is
load-bearing. Two mutation patches are committed and demonstrated. *Acceptance:*
`make proptest` passes; applying `MUT-PATHKEY` fails
`path_key_is_permutation_invariant`; applying `MUT-GUARD` fails
`validation_precedes_path_key_ordering`; both revert cleanly. *Conformance
check:* `path_key` is still `pub(crate)`; no file exceeds 400 lines.
*Recovery:* the whole directory is additive and can be deleted.

### EP-M3 — insertion-order and `default` ordering

*Assigned:* `RM-4.3.1.a`, `RM-4.3.1.b`, `OBL-ORDER`, `OBL-DEFAULT`.

*End state:* the shared bounded graph strategy lives in `test_support` (see
`Interfaces and dependencies`), and `insertion_order.rs` and `defaults.rs` are
present. The insertion-order property records, and asserts a floor on, the
fraction of cases in which the two maps genuinely iterated differently.
*Acceptance:* `make proptest` passes; each of the four mutation patches for
these two obligations fails its named property and only its named property.
*Conformance check:* graphs stay within 50 actions and 100 edges; the
classification counts from `EP-M0` question 1 are reproduced within tolerance.
*Recovery:* additive.

### EP-M4 — action-hash stability and declaration-order invariance

*Assigned:* the `FV-DET` action-hash bullet, `OBL-ACTION`, `OBL-E2E`.

*End state:* `src/ir/action_hash_property_tests.rs` and `declaration_order.rs`
are present. The manifest strategy generating typed `NetsukeManifest` values
lives beside the graph strategy in `test_support`. *Acceptance:*
`make proptest` passes; `MUT-HASHMETA` fails `OBL-ACTION`; `MUT-IDINDEX` fails
`OBL-E2E` while `OBL-ORDER` still passes. *Conformance check:* if `EP-M0`
question 2 answered "no", this milestone must not be started until the
escalation is resolved. *Recovery:* additive.

### EP-M5 — inherited larger-N IR obligations

*Assigned:* `RM-4.2.1.dup`, `RM-4.2.1.cyc`, `OBL-DUP`, `OBL-CYCLE`.

*End state:* `src/ir/graph_property_tests/` with `mod.rs`, `duplicates.rs`, and
`cycles.rs`, wired from `src/ir/mod.rs`. The cycle generator's long-cycle class
is confirmed to fire. *Acceptance:* `make proptest` passes; `MUT-DUPWITHIN`
fails only the within-target arm; `MUT-CYCLEDEPTH` fails only the long-cycle
class. *Conformance check:* `ADR-004`'s deferred obligations are now
discharged; the ADR is annotated to say so in `EP-M6`. *Recovery:* additive.

### EP-M6 — the determinism contract and documentation

*Assigned:* `FV-CONTRACT`, `ADR-021`.

Write `docs/adr-021-ninja-emission-determinism-contract.md` in the repository's
Y-Statement style, stating precisely which of the three determinism statements
in `Context and orientation` is a public guarantee, which is an internal
invariant, and which is neither. Record the `graph` subcommand's separate
guarantee as adjacent and out of scope. Then:

- Add the guarantee to `docs/users-guide.md` in user-facing terms, with the
  caching and source-control consequences that motivate it.
- Add one sentence to `README.md` immediately following the existing
  reproducibility statement, as `FV-DET`'s determinism-contract section asks.
- Update `docs/netsuke-design.md` §5.5 to reference `ADR-021` and to state that
  the sorting decisions are now property-verified, naming the test module.
- Add a "Determinism property tests" subsection to `docs/developers-guide.md`
  under the existing proptest section, describing the shared graph strategy,
  the light/heavy tiering decided in `EP-M1`, the mutation-patch discipline,
  and the regression-seed convention.
- Annotate `ADR-004` to record that its deferred larger-N obligations are
  discharged by this item.
- Index `ADR-021` in `docs/contents.md`.

*End state:* the contract is stated before it is claimed to be verified.
*Acceptance:* `make markdownlint`, `make nixie`, and `make check-fmt` pass; the
users' guide sentence and the tests agree in substance. *Conformance check:*
en-GB-oxendict throughout; 80-column wrap; `mdtablefix`-canonical. *Recovery:*
documentation-only; revert individually.

### EP-M7 — real-Ninja oracle, mutation sweep, and final validation

*Assigned:* `OBL-NINJA`, and the non-vacuity evidence for every obligation.

*End state:* `tests/ninja_determinism_oracle_tests.rs` exists and skips
gracefully when Ninja is absent. Every mutation patch has been applied and
reverted once more against the final tree, and the results are tabulated in
`Artefacts and notes`. Classification counts for every obligation are recorded.
The roadmap entry for 4.3.1 is marked done with its sub-items, and the two
`4.2.1` sub-items that deferred work here are annotated as discharged.
*Acceptance:* `make check-fmt`, `make typecheck`, `make lint`,
`make doc-coverage`, `make test`, `make proptest` (and `make proptest-heavy` if
`EP-M1` split the suite), `make markdownlint`, and `make nixie` all pass, with
logs captured under `/tmp`. `coderabbit review --agent` returns no unresolved
findings. *Conformance check:* every trace link in `Conformance basis` resolves
to a test that exists and passes. *Recovery:* additive.

## Interfaces and dependencies

### Architectural note: where the boundary sits

Netsuke's emission path already has the shape hexagonal architecture asks for,
and this plan must not disturb it. `BuildGraph` is the domain model: it has no
I/O, no framework types, and no knowledge of Ninja. `src/ninja_gen/` is an
outbound adapter that renders the domain model into one specific backend's text
format. `generate_into` writes to a `W: Write`, so the adapter does not own the
sink either.

The obligations respect that boundary. `OBL-PATHKEY`, `OBL-ORDER`,
`OBL-DEFAULT`, and `OBL-GUARD` are properties of the adapter and are tested
against the adapter's own entry points. `OBL-DUP`, `OBL-CYCLE`, and
`OBL-ACTION` are properties of the domain and its lowering, and are tested
against `BuildGraph::from_manifest` without touching the adapter. `OBL-E2E`
deliberately spans both, which is the only place a composed property is
justified: the user-facing determinism claim is about the composition, not
either half.

`OBL-NINJA` is the one obligation that crosses out of the process, and it is
therefore the one that lives in `tests/` rather than `src/`, uses a subprocess,
and skips when its dependency is absent. That placement is the boundary showing
through, not an inconsistency.

The temptation to resist is introducing a "deterministic collection port" —
swapping `IrHashMap` for an ordered map to make determinism structural rather
than emergent. That would be a production change, is forbidden by
`Constraint 1`, and would in any case make the properties tautological. The
determinism must remain a checked property of the adapter, not a type-system
consequence. If a future maintainer wants to make it structural, `ADR-021` is
where that argument belongs; note it there as a considered and rejected option.

### New files

Shared strategies, in `test_support` so both library-side and integration tests
can use them, split to respect the 400-line ceiling:

- `test_support/src/ninja_gen/graph_strategy.rs` — the `GraphSpec` type and its
  strategy.
- `test_support/src/ninja_gen/manifest_strategy.rs` — the typed
  `NetsukeManifest` strategy for `OBL-E2E`, `OBL-DUP`, and `OBL-CYCLE`.
- `test_support/src/ninja_gen/classification.rs` — the classification counters
  used for non-vacuity evidence.

Library-side property tests, wired from `src/ninja_gen/mod.rs`:

- `src/ninja_gen_determinism_property_tests/mod.rs`
- `src/ninja_gen_determinism_property_tests/path_key.rs`
- `src/ninja_gen_determinism_property_tests/guard.rs`
- `src/ninja_gen_determinism_property_tests/insertion_order.rs`
- `src/ninja_gen_determinism_property_tests/defaults.rs`
- `src/ninja_gen_determinism_property_tests/declaration_order.rs`

Library-side IR property tests, wired from `src/ir/mod.rs`:

- `src/ir/action_hash_property_tests.rs`
- `src/ir/graph_property_tests/mod.rs`
- `src/ir/graph_property_tests/duplicates.rs`
- `src/ir/graph_property_tests/cycles.rs`

Integration test:

- `tests/ninja_determinism_oracle_tests.rs`

Documentation and evidence:

Mutation patches follow the existing house convention in
`docs/verification/mutations/`: each file is named after the *test it
falsifies*, with `__` standing in for the module separator, not after the
mutation it applies. Note that
`ir__from_manifest__verification__duplicate_output_always_rejected.patch`
already exists in that directory from the 4.2.1 Kani work and flips `||` to
`&&` in `find_duplicates`. The new duplicate-output patch below is deliberately
more surgical, disabling only the within-one-target half, so that `OBL-DUP`'s
three collision modes can be told apart. Both patches are retained.

- `docs/adr-021-ninja-emission-determinism-contract.md`

Eleven mutation patches under `docs/verification/mutations/`. Each is referred
to in this plan by a short handle; the filename follows the house convention of
naming a patch after the test it falsifies, with `__` for the module separator.

| Handle           | Filename (under `docs/verification/mutations/`)                                                   | Falsifies                  | Mutation                                                        |
| ---------------- | ------------------------------------------------------------------------------------------------- | -------------------------- | --------------------------------------------------------------- |
| `MUT-PATHKEY`    | `ninja_gen__determinism_property_tests__path_key_is_permutation_invariant.patch`                  | `OBL-PATHKEY`              | Delete `parts.sort_unstable()` in `path_key`.                   |
| `MUT-GUARD`      | `ninja_gen__determinism_property_tests__validation_precedes_path_key_ordering.patch`              | `OBL-GUARD`                | Move `reject_unsupported_path_characters` after the edge sort.  |
| `MUT-EDGESORT`   | `ninja_gen__determinism_property_tests__emission_is_insertion_order_invariant.patch`              | `OBL-ORDER`                | Delete `edges.sort_by_key` in both emission paths.              |
| `MUT-ACTIONSORT` | `ninja_gen__determinism_property_tests__bundle_is_insertion_order_invariant.patch`                | `OBL-ORDER`                | Delete `actions.sort_by_key` in `write_action_rules`.           |
| `MUT-FIRSTOUT`   | `ninja_gen__determinism_property_tests__emission_is_insertion_order_invariant_multi_output.patch` | `OBL-ORDER`                | Sort edges by first explicit output, making the key non-unique. |
| `MUT-DEFSORT`    | `ninja_gen__determinism_property_tests__default_line_is_the_ascending_sort.patch`                 | `OBL-DEFAULT`, `OBL-E2E`   | Delete `defs.sort()` in both emission paths.                    |
| `MUT-DEFPOS`     | `ninja_gen__determinism_property_tests__default_line_position.patch`                              | `OBL-DEFAULT`, `OBL-NINJA` | Emit the `default` block before edge rendering.                 |
| `MUT-HASHMETA`   | `ir__action_hash_property_tests__action_hash_is_a_function_of_content.patch`                      | `OBL-ACTION`               | Make `ActionHasher::hash` skip the `pool` field.                |
| `MUT-IDINDEX`    | `ninja_gen__determinism_property_tests__declaration_order_does_not_change_emission.patch`         | `OBL-E2E`                  | Append the declaration index to the interned action identifier. |
| `MUT-DUPWITHIN`  | `ir__graph_property_tests__duplicate_outputs_are_rejected_at_larger_n.patch`                      | `OBL-DUP`                  | Disable only the within-one-target half of `find_duplicates`.   |
| `MUT-CYCLEDEPTH` | `ir__graph_property_tests__cycles_are_rejected_at_larger_n.patch`                                 | `OBL-CYCLE`                | Cap the cycle detector's traversal depth at 4.                  |

Commit the regression seed files these properties produce, under
`proptest-regressions/` for the library-side properties and beside the test as
`tests/ninja_determinism_oracle_tests.proptest-regressions` for the integration
test.

### The shared graph strategy

This is the plan's central new artefact, and its design decides how much the
properties are worth. It generates a *specification*, not a graph, so that the
same specification can be materialized twice in two different insertion orders.

```rust
/// A bounded, well-formed build-graph specification and two insertion orders.
pub struct GraphSpec {
    /// Unique action identifiers paired with their actions.
    actions: Vec<(String, Action)>,
    /// Edges whose explicit output sets are pairwise disjoint.
    edges: Vec<BuildEdge>,
    /// Default targets, drawn from the declared explicit outputs.
    default_targets: Vec<Utf8PathBuf>,
    /// Insertion order for the first materialisation.
    first_order: Vec<usize>,
    /// Insertion order for the second materialisation.
    second_order: Vec<usize>,
}
```

Well-formedness is achieved by construction, never by filtering
(`Constraint 6`):

- Output paths are drawn from a pre-numbered namespace `out/NNNN`. Each edge is
  assigned a contiguous, non-overlapping slice, so disjointness is structural.
  The namespace avoids `.netsuke/`, so `reject_reserved_paths` never fires.
- Action identifiers are `act-NNNN`, unique by index. For `OBL-ORDER` these
  stand in for production's content hashes; `OBL-ACTION` separately verifies
  that the real interning is content-determined, so the substitution is
  justified rather than assumed.
- Every edge's `action_id` is drawn from the generated identifier list by
  index, so `MissingAction` is unreachable.
- Inputs, implicit dependencies, and order-only dependencies are drawn from
  already-allocated outputs at strictly lower indices, plus a generated number
  of external paths outside the namespace. The index discipline means no cycle
  is generated, and the external paths exercise the missing-dependency path.
- `default_targets` is a generated sub-multiset of the allocated outputs,
  shuffled, with a generated repetition count so `OBL-DEFAULT`'s duplicate
  class is reachable.
- `first_order` and `second_order` are independent shuffles of `0..n`.

Materialization inserts into fresh `HashMap`s in the given order, including the
per-output cloning that `insert_edge_for_outputs` performs, so the two graphs
are equal as values but were built differently.

Because a strategy this size can hide gaps, `classification.rs` records, per
case, which equivalence classes were reached, and each property asserts a floor
on the classes its `Non-vacuity` clause names.

### Existing interfaces relied upon

- `netsuke::ninja_gen::{generate, generate_bundle}` — public; unchanged.
- `netsuke::ninja_gen::path_key` — `pub(crate)`; reached from library-side tests
  via `super::`. Must not be widened.
- `netsuke::ir::BuildGraph::from_manifest` — public; unchanged.
- `netsuke::hasher::ActionHasher::hash` — reached from `src/ir/` tests.
- `test_support::ninja_gen::{paths_strategy, ninja_integration_setup}` —
  existing; extended, not replaced.
- The `NinjaCommandOracle` pattern in
  `src/ninja_gen_property_tests/ninja_oracle.rs` — copied in shape, including
  its skip-when-absent behaviour.

### Test-wiring contract

`tests/integration_test_wiring_tests.rs` enforces two rules. A new top-level
`tests/*.rs` file needs no registration; Cargo's autodiscovery handles it and
the contract test confirms Cargo saw it. Only a new *subdirectory* under
`tests/` containing a `mod.rs` needs an explicit `mod` declaration from some
top-level test file. This plan adds one top-level file and no test
subdirectory, so no manual wiring is required — but run `make test` and
confirm, rather than assuming.

Library-side modules are wired locally from their parent production module.
`src/lib.rs` declares no test modules and must not start.

## Concrete steps

### One-time setup

```bash
cd /path/to/netsuke/worktree
git branch --show-current   # must be 4-3-1-proptests-for-deterministic-ninja-emission
git fetch origin --prune
git log --oneline -1 origin/main
```

Confirm the worktree base is not stale before starting. Do not create an
isolated Cargo cache; use the shared default cache and let Cargo's
package-cache lock serialize access.

### Running the new suite

```bash
make proptest 2>&1 | tee /tmp/proptest-netsuke-$(git branch --show-current).out
```

If `EP-M1` split the suite:

```bash
make proptest-heavy 2>&1 | tee /tmp/proptest-heavy-netsuke-$(git branch --show-current).out
```

Run a single property while iterating:

```bash
cargo nextest run --all-features -E 'test(emission_is_insertion_order_invariant)'
```

Widen the search without recompiling:

```bash
PROPTEST_CASES=4096 cargo nextest run --all-features -E 'test(determinism_property_tests)'
```

Do not lower a case count to make a failure go away. A property that fails at
case 500 and passes at 256 has found something.

### Applying and reverting a mutation patch

```bash
git apply MUT-EDGESORT
make proptest 2>&1 | tee /tmp/mutation-edges-unsorted.out   # must FAIL
git apply -R MUT-EDGESORT
git diff --quiet && echo "tree restored"
```

Record, for each patch, which properties failed and which continued to pass. A
patch that fails *every* property is too coarse to be evidence; narrow it.

### Ordinary gates

Run sequentially, never in parallel — the build cache is shared with other
agents on this machine.

```bash
B=$(git branch --show-current)
make check-fmt   2>&1 | tee /tmp/check-fmt-netsuke-$B.out
make typecheck   2>&1 | tee /tmp/typecheck-netsuke-$B.out
make lint        2>&1 | tee /tmp/lint-netsuke-$B.out
make doc-coverage 2>&1 | tee /tmp/doc-coverage-netsuke-$B.out
make test        2>&1 | tee /tmp/test-netsuke-$B.out
make markdownlint 2>&1 | tee /tmp/markdownlint-netsuke-$B.out
make nixie       2>&1 | tee /tmp/nixie-netsuke-$B.out
```

Prefer delegating full gate runs to the `scrutineer` subagent, which runs them
sequentially, captures each log, and returns a bounded report. When it reports
a failure, read the cited log rather than re-running the gate.

### Commits and review

Commit at every milestone boundary, and more often within a milestone whenever
the tree is green. Subjects are imperative and under 50 characters; bodies wrap
at 72 columns. Never commit a tree that fails a gate.

Request a `coderabbit review --agent` pass at `EP-M3`, `EP-M5`, and `EP-M7`.

## Validation and acceptance

### Red-green-refactor evidence to record

Ordinary red-green-refactor does not apply cleanly here: the production code is
expected to be correct, so a new property should pass immediately. Writing a
test that passes on first run proves nothing about the test. This plan
substitutes mutation-driven red, which is stronger:

- **Red.** Apply the obligation's mutation patch and run the property. Record
  the failure output, including the shrunk counter-example. If the property
  passes, it is not yet a test — redesign it before writing anything else.
- **Green.** Revert the patch and run the property. Record the pass.
- **Refactor.** Extract shared strategy code, keep every file under 400 lines,
  rerun the property and the wider gates.

Record both transcripts per obligation in `Artefacts and notes`. This
substitution is deliberate and is recorded in `Decision log`; it is the
`Constraints` -mandated observable substitute for a red stage that cannot
otherwise exist.

### Acceptance, phrased as behaviour

1. Running `make proptest` on a clean tree passes and reports the property
   tests by name.
2. Applying `MUT-EDGESORT` and running
   `make proptest` fails, and the failure names
   `emission_is_insertion_order_invariant` and prints a shrunk pair of graphs
   small enough to read. Reverting the patch restores a passing run.
3. The same holds for each of the other ten mutation patches, each failing its
   named property and not the whole suite.
4. Running `make test` on a clean tree passes, and the wall-time delta against
   `origin/main` is within the figure recorded in `EP-M1`.
5. If Ninja is installed,
   `cargo nextest run -E 'test(ninja_determinism_oracle)'` passes and reports a
   non-zero case count. If Ninja is absent, it reports a skip rather than a
   pass.
6. Reading `docs/users-guide.md` yields a stated determinism guarantee, and
   `docs/adr-021-ninja-emission-determinism-contract.md` explains which of the
   three determinism statements is public and which is internal.
7. `docs/roadmap.md` item 4.3.1 and its three sub-items are marked done, and the
   two `4.2.1` sub-items that deferred work here are annotated as discharged.
8. `make check-fmt`, `make typecheck`, `make lint`, `make doc-coverage`,
   `make test`, `make markdownlint`, and `make nixie` all pass.

### Quality criteria

- Every property uses `prop_assert*`, never `assert!` or `unwrap`.
- No strategy uses `prop_filter` or `prop_assume!` for a structural condition.
- Every obligation's classification floors are asserted, not merely printed.
- Counter-examples shrink to graphs a human can read in a terminal.
- No file exceeds 400 lines.
- All prose is en-GB-oxendict; all Markdown is `mdtablefix`-canonical.
- Regression seed files are committed, with a comment on any seed retained for
  a reason other than a live defect.

## Idempotence and recovery

Every step is re-runnable. The property suites are pure and hold no state
between runs beyond the committed regression seeds, which are replayed
deterministically. `make proptest` can be interrupted and re-run.

If a mutation patch is left applied by accident, `git diff` will show it and
`git apply -R` removes it; `EP-M7` re-verifies a clean tree explicitly. If a
patch stops applying after a production refactor, that is a signal, not a
nuisance: re-derive the patch against the new code in the same commit as the
refactor, per `Constraint 7`.

If a milestone must be abandoned, every artefact it adds is additive — new
files, new Make targets, new documentation — so reverting is a matter of
deleting files and the wiring lines that reference them. No milestone
introduces a compatibility shim, an alias, or a dual implementation, and none
is permitted to: there is no external consumer of any interface touched here,
and every new surface is test-only or `pub(crate)`.

If the shared Cargo cache is locked by another agent's build, wait for the lock
rather than creating a separate cache. If `/tmp` or the disk fills, stop and
report it.

## Artefacts and notes

To be filled in during implementation. The required contents are:

- `EP-M0` measurements: answers to all six spike questions, with figures.
- `EP-M1` timings: `make test` before and after, `make proptest` alone, and the
  tiering decision with its threshold comparison.
- Per-obligation classification counts, showing every named equivalence class
  was reached.
- Per-mutation transcripts: which properties failed, which passed, and the
  shrunk counter-example for each failure.
- Final gate logs, by `/tmp` path.
- The `coderabbit review --agent` outcome at each requested point.

## Progress

- [x] (2026-09-09T00:00:00Z) Renamed the branch to
      `4-3-1-proptests-for-deterministic-ninja-emission` and pushed it with
      upstream tracking.
- [x] (2026-09-09T00:00:00Z) Loaded the `codegraph-mcp`, `rust-router`,
      `hexagonal-architecture`, `execplans`, `proptest`, and
      `rust-verification` skills.
- [x] (2026-09-09T00:00:00Z) Ran a four-agent Wyvern reconnaissance team over
      the Ninja emission internals, the repository's proptest conventions, the
      documentation and gate tooling, and the ExecPlan house style.
- [x] (2026-09-09T00:00:00Z) Researched the Ninja manual v1.13.1 for `default`
      statement and build-statement semantics, and confirmed current crate
      versions for `proptest`, `proptest-derive`, `test-strategy`, and
      `proptest-state-machine`.
- [x] (2026-09-09T00:00:00Z) Read `src/ninja_gen/mod.rs`,
      `src/ninja_gen/dyndep.rs`, `src/ninja_gen/display_edge.rs`,
      `src/ninja_gen/path_syntax.rs`, `src/ir/graph.rs`,
      `src/ir/from_manifest.rs`, and `src/ir/from_manifest_support.rs`, and
      established the well-formedness invariant the strategies must reproduce.
- [x] (2026-09-09T00:00:00Z) Confirmed the three scope decisions with the user:
      include the inherited larger-N IR obligations, settle the determinism
      contract in this item, and add a measured `proptest` Make target with a
      light/heavy split if the measurement justifies one.
- [x] (2026-09-09T00:00:00Z) Drafted this approval-gated ExecPlan.
- [ ] Community-of-experts design review of this plan, and revision.
- [ ] Approval gate: await explicit user approval before any implementation.
- [ ] `EP-M0`: feasibility and measurement spike; record six answers.
- [ ] `EP-M1`: measured tiering and the `proptest` Make target.
- [ ] `EP-M2`: `OBL-PATHKEY` and `OBL-GUARD`.
- [ ] `EP-M3`: `OBL-ORDER` and `OBL-DEFAULT`.
- [ ] `EP-M4`: `OBL-ACTION` and `OBL-E2E`.
- [ ] `EP-M5`: `OBL-DUP` and `OBL-CYCLE`.
- [ ] `EP-M6`: `ADR-021` and the documentation set.
- [ ] `EP-M7`: `OBL-NINJA`, mutation sweep, roadmap update, final validation.

## Surprises & discoveries

- (2026-09-09) The emitter's edge sort is *stable* and its key,
  `path_key(&edge.explicit_outputs)`, is only unique because `from_manifest`
  rejects duplicate outputs. If that invariant were ever weakened, ties would
  silently fall back to `HashMap` iteration order and the deduplication would
  drop a real edge non-deterministically. This is the single most valuable
  thing the reconnaissance found, and it is why `OBL-GUARD` and `OBL-ACTION`
  exist as separate obligations rather than being folded into `OBL-ORDER`.
- (2026-09-09) `path_key` joins with a NUL byte, so it is injective only
  because `reject_unsupported_path_characters` — which runs first — rejects
  control characters. The concrete collision
  `path_key(["a", "b"]) == path_key(["a\0b"])` is real and reachable if that
  ordering is ever changed. It is recorded as a directed witness test rather
  than left implicit.
- (2026-09-09) `process_defaults` uses `Vec::extend` and the emitter uses
  `Vec::sort`, neither of which deduplicates, so a manifest listing the same
  default twice emits it twice. This is deterministic and probably harmless,
  but it is undocumented; `ADR-021` should say whether it is intended.
- (2026-09-09) An edge with no explicit outputs is silently dropped by
  `insert_edge_for_outputs` rather than rejected. Out of scope here, but worth
  noting in `ADR-021` as an unstated precondition.
- (2026-09-09) The repository has duplicate ADR numbers at 003, 004, and 014,
  so allocating 021 requires checking remote branches first rather than
  trusting the local maximum.
- (2026-09-09) Two existing integration tests override `failure_persistence`
  explicitly, claiming regression seeds are otherwise inert in integration-test
  crates, while three others rely on the default and do have working seed
  files. The discrepancy is unexplained and is `EP-M0` question 5.

## Decision log

- Decision: keep this ExecPlan pre-implementation and approval-gated.
  Rationale: the user stated the plan must be approved before it is
  implemented. Date/Author: 2026-09-09 / planning agent.

- Decision: include the larger-N duplicate-output and cycle-rejection
  obligations inherited from roadmap `4.2.1` and `ADR-004`, and add matching
  sub-items to roadmap `4.3.1`. Rationale: the user chose the wide scope when
  asked. Both upstream artefacts state in terms that `4.3.1` closes this
  coverage, so leaving it out would leave an accepted ADR undischarged with no
  owner. The bounded manifest strategy needed for `OBL-E2E` also serves these
  obligations, so the marginal cost is one milestone. Date/Author: 2026-09-09 /
  planning agent.

- Decision: settle the determinism contract in this item, in a new `ADR-021`
  plus the users' guide and one README sentence. Rationale: the user chose this
  when asked. `docs/formal-verification-methods-in-netsuke.md` asks for the
  decision and no roadmap item owns it, and verifying an unstated property is
  verification without a specification. Date/Author: 2026-09-09 / planning
  agent.

- Decision: add a `proptest` Make target, measure it in `EP-M1`, and split into
  light and heavy tiers only if the measurement justifies it, with the heavy
  tier gated in CI. Rationale: the user asked for measurement rather than an
  assumption. The repository has no proptest target today and all suites run
  under `make test`; adding a tier speculatively would diverge from convention
  without evidence. The threshold is stated in `Tolerances` as 45 seconds added
  to `make test`. Date/Author: 2026-09-09 / planning agent.

- Decision: substitute mutation-driven red for conventional red-green-refactor.
  Rationale: the production code is expected to be correct, so a new property
  passes on first run and its red stage cannot be observed the usual way.
  Applying a recorded mutation patch and observing a named failure is a
  stronger observable substitute, and is the discipline `ADR-004` and roadmap
  `4.2.2` already established for the Kani harnesses. Date/Author: 2026-09-09 /
  planning agent.

- Decision: generate a `GraphSpec` and materialize it twice, rather than
  generating two graphs and asserting they are equal. Rationale: generating two
  graphs independently would make the property depend on the generator
  producing equal values, which is fragile and shrinks badly. Materializing one
  specification under two insertion permutations makes value equality
  structural and isolates the variable under test. Date/Author: 2026-09-09 /
  planning agent.

- Decision: use synthetic `act-NNNN` identifiers in the emission-level
  strategies rather than real content hashes, and verify the real interning
  separately in `OBL-ACTION`. Rationale: the emitter only reads the identifier
  as an opaque sort key, so synthesizing it isolates the emitter's behaviour;
  hashing 50 actions per case would also dominate the runtime budget. The
  substitution is justified rather than assumed because `OBL-ACTION` checks the
  property the substitution relies on. Date/Author: 2026-09-09 / planning agent.

- Decision: build `OBL-E2E`'s manifests as typed `NetsukeManifest` values
  rather than as YAML text. Rationale: parsing, `foreach`/`when` expansion, and
  Jinja rendering belong to roadmap items 4.3.2 and 4.3.3. Starting from typed
  values keeps this item's scope to lowering and emission, and avoids
  duplicating coverage that later items will add. Date/Author: 2026-09-09 /
  planning agent.

- Decision: do not introduce an ordered-map collection port to make determinism
  structural. Rationale: it would be a production behaviour change, is
  forbidden by `Constraint 1`, and would make every property in this plan
  tautological. `ADR-021` records it as a considered and rejected option so a
  future maintainer finds the reasoning. Date/Author: 2026-09-09 / planning
  agent.

- Decision: follow the obligation-driven ExecPlan style used by
  `docs/execplans/4-2-3-kani-harnesses-for-command-interpolation.md` rather
  than the earlier narrative style of `4-1-1` and `4-2-1`, and use sentence
  case for the back-matter headings. Rationale: `4-2-3` is the most recent
  precedent and the closest structural analogue, being the plan that hands
  residual coverage to Proptest. The two precedents disagree on heading case;
  the more recent one is followed. Date/Author: 2026-09-09 / planning agent.

## Outcomes & retrospective

To be completed after `EP-M7`. Record: whether any property failed against
current production code and what was found; the measured cost of the suite and
the tiering decision that followed; whether `OBL-E2E` held as hypothesized;
which mutations proved hardest to make specific; and whether the shared graph
strategy is reusable by roadmap items 4.3.2 and 4.3.3 as intended.

## Revision note

- 2026-09-09: Initial draft. Establishes the obligation set, the three
  determinism statements the plan keeps apart, the shared bounded graph
  strategy, the mutation-driven red substitute, and the eight milestones.
  Records the three scope decisions taken with the user: the inherited larger-N
  IR obligations are in scope, the determinism contract is settled here in
  `ADR-021`, and the `proptest` Make target is measured in `EP-M1` before any
  light/heavy split is committed to. Remaining work is the whole of `EP-M0`
  through `EP-M7`, gated on approval.

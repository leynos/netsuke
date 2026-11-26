# Exclude Kani-only graph implementations from ordinary mutation selection (issue #1038)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: COMPLETE

## Purpose / big picture

The ordinary, test-based mutation lane reports six false survivors from four
implementations in [`src/ir/graph/mod.rs`](../../src/ir/graph/mod.rs) that
exist only under `#[cfg(kani)]`. Those implementations are absent from the
`cargo test` binaries the lane uses to judge its mutants, so no mutation inside
them can ever make those tests fail. They are not evidence of a test gap; they
are evidence that the selector is pointing at code the judging runner does not
compile.

The six reported survivors, from `mutation-report-root-0/missed.txt` of run
[40850417835](https://github.com/leynos/netsuke/actions/runs/40850417835), at
`e52d3ef49429392eebeb484aa4d6622ca79782dd`:

| Location                  | Mutant                                                                 |
| ------------------------- | ---------------------------------------------------------------------- |
| `src/ir/graph/mod.rs:77`  | `insert_edge -> EdgeId` with `Default::default()`                      |
| `src/ir/graph/mod.rs:138` | `output_index` with `Box::leak(Box::new(Default::default()))`          |
| `src/ir/graph/mod.rs:152` | `edge_id_for_output -> Option<EdgeId>` with `None`                     |
| `src/ir/graph/mod.rs:152` | `edge_id_for_output -> Option<EdgeId>` with `Some(Default::default())` |
| `src/ir/graph/mod.rs:194` | `target_for_output` with `None`                                        |
| `src/ir/graph/mod.rs:194` | `target_for_output` with `Some((Box::leak(...), Box::leak(...)))`      |

The report prints each as one `file:line:col: replace …` record inside
`mutation-report-root-0/missed.txt`. The type signatures are elided above only
to keep the table readable; the positions and the mutated constructs are
verbatim.

The caller already excludes whole Kani-only *files* through `exclude-globs`,
but that mechanism is path-scoped and cannot exclude four items inside a file
that also holds the production counterparts which must stay selected.

After this change, a maintainer runs the pinned cargo-mutants inventory:

```sh
cargo mutants --list --all-features --file src/ir/graph/mod.rs
```

and sees none of the six above, while the `#[cfg(not(kani))]` counterpart
mutants and the shared, ordinally compiled graph logic remain listed. The
repository gains a selection regression that fails if a future edit excludes
all same-named functions, or the wrong same-named variant, rather than the four
proof-only ones.

This work does **not** strengthen the proofs, and it does not turn any survivor
into a killed mutant. It removes noise from a lane that was never able to judge
that code, and it records the boundary between ordinary test-based mutation
selection and proof-specific verification.

## Definition of done

- The four Kani-only implementations carry item-level skips the pinned
  cargo-mutants version honours, each with a comment saying why.
- `cargo mutants --list --all-features --file src/ir/graph/mod.rs` shows none of
  the six false survivors.
- The `#[cfg(not(kani))]` counterparts of `insert_edge`, `edge_id_for_output`,
  and `target_for_output` remain listed, and `replace_edge_for_output`,
  `insert_canonical_edge`, `index_output_aliases`, `index_output`,
  `duplicate_output`, `edge_count`, `output_count`, `edges`, and `output_paths`
  remain listed.
- `output_index` leaves the ordinary lane entirely. It is a Kani-only accessor
  with no `cfg(not(kani))` twin — the production duplicate-output scan reads
  the target map directly — so unlike the other three there is no counterpart
  to keep.
- A focused selection regression test uses same-named Kani and production
  variants, so a broad name-based exclusion cannot pass unnoticed.
- The Kani proof lane, the proof inventory, and the mutation-evidence checks
  still run and pass unchanged.
- The ordinary-versus-proof verification boundary is documented in the
  developers' guide, with a matching workflow contract assertion.
- Applicable gates pass: `make check-fmt`, `make lint`, `make doc-coverage`,
  `make test`, `make test-workflow-contracts`, `make markdownlint`.

## Constraints

- Use the exact CI-pinned cargo-mutants version for the inventory evidence.
  The shared workflow `mutation-cargo.yml` at
  `0606ca899ce2995a0c4bc78e660e80832e6e8855` declares `cargo-mutants-version`
  default `27.1.0`, and `cargo-mutants --version` locally reports `27.1.0`, so
  local evidence and CI agree by measurement rather than by assumption.
- Do not exclude the whole graph file, the whole `impl BuildGraph` block, or
  every function sharing these names. The production counterparts stay selected.
- Keep `replace_edge_for_output` and the other shared executable graph logic
  eligible.
- Preserve the Kani proof lane, the proof inventory, and the mutation-evidence
  compile gate. This change must not touch `docs/verification/mutations/`,
  `tests/kani_mutation_evidence_tests/`, or the harness modules.
- Do not describe this as improved proof strength or as newly killed mutants.
- Preserve existing workflow resource limits and scheduling: the
  `exclude-globs`, `--all-features`, `install-mold`, and `setup-commands`
  inputs, the daily cron, the per-ref concurrency, and the job permissions all
  stay as they are.
- Dependency requirements are caret requirements, per the repository's
  dependency policy. A new dependency needs its ownership boundary recorded.

## Tolerances (exception triggers)

- Scope: if the repair needs changes outside `src/ir/graph/mod.rs`, one new
  selection-regression test (plus its fixture), the workflow contract test, the
  caller comment in `.github/workflows/mutation-testing.yml`, the developers'
  guide, and documentation indexes, stop and re-plan.
- Mechanism: if the chosen skip mechanism is not honoured by cargo-mutants
  27.1.0 as measured by `--list`, stop and reconsider rather than falling back
  to a name-based `--exclude-re` that cannot distinguish same-named variants.
- Kani compatibility: if adding the skip makes the Kani lane fail to compile,
  stop; the proof lane is a hard constraint and this change must not weaken it.
- Iterations: if two mechanisms in a row fail their `--list` probe, stop and
  re-plan rather than trying a third by guesswork.

## Risks

- **Kani compilation.** The skip must compile in three configurations:
  ordinary `cargo test`, `cargo check` with `--cfg kani` (which is what the
  mutation-evidence gate's negative control exercises), and the real
  `cargo kani` frontend. A skip that expands to an unresolved crate breaks the
  second and third.
- **Mechanism misreading.** The issue text states that cargo-mutants honours a
  nested skip "without evaluating its condition". That is true of the nested
  attribute's *presence* being honoured, but a probe shows the accepted
  condition spellings are narrower than that sentence suggests. This plan
  resolves the mechanism by measurement rather than by reading.
- **Regression test cost.** The regression must prove the *selection* rule, not
  merely that the source contains some token. A test that greps for
  `mutants::skip` would pass on a broad exclusion; it has to observe the
  inventory or the resolved per-item decision.
- **Doc drift.** The developers' guide currently describes `exclude-globs` as
  the whole Kani story for mutation selection. Leaving that sentence alone
  would document the pre-change world.

## Progress

- [x] (2026-11-25) Confirmed the branch has no existing `issue-1038` remote
      counterpart, so the rename is safe; renamed the local branch and set the
      Lody session title.
- [x] (2026-11-25) Captured the baseline inventory: 31 mutants listed for
      `--file src/ir/graph/mod.rs --all-features`, including the six false
      survivors at 77, 138, 152 (twice), and 194 (twice).
- [x] (2026-11-25) Verified the pinned tool version: `cargo-mutants 27.1.0`
      installed locally; the shared workflow at the pinned SHA declares
      `cargo-mutants-version` default `27.1.0`.
- [x] (2026-11-25) Probed the mechanisms against a scratch crate. Results are
      in `Surprises & discoveries`; they decided the approach.
- [x] (2026-11-25) Milestone 1: added the `mutants` dependency and the four
      item-level skips. `--list` drops exactly the six false survivors, adds
      nothing, and keeps every production counterpart and shared function. The
      ordinary `cargo check --all-features --all-targets` under `-D warnings`
      is clean, so the dependency raises no `unused_dependencies` warning.
- [x] (2026-11-25) Milestone 2: `tests/mutation_selection_contract_tests.rs`
      (7 tests) plus the `tests/data/mutation_selection/` fixture. Verified
      non-vacuous by seeding two defects: a skip on the production half of
      `edge_id_for_output` fails two tests, and dropping the `output_index`
      skip fails a third. Both seeds were reverted.
- [x] (2026-11-25) Milestone 3: the developers' guide boundary section, the
      workflow's explanatory comment, and two new assertions in
      `tests/workflow_contracts/mutation_testing_test.py`.
- [x] (2026-11-25) `make check-fmt`, `make lint` (Clippy, Whitaker, Ruff,
      Pylint, Interrogate, df12 lints, ambrleaks, yamllint, actionlint),
      `make doc-coverage` (98.37%), `make test` (6620 nextest tests plus every
      doctest), and `make test-workflow-contracts` all pass.
- [x] (2026-11-25) CodeRabbit review returned three findings; all three were
      verified against the pinned cargo-mutants source and the filesystem, and
      all three were valid:
  - **Enclosing-scope skips were not modelled.** `visit_item_impl` and
    `visit_file` in cargo-mutants 27.1.0 both return early on
    `attrs_excluded`, so a skip on an `impl` block — or a file-level
    `#![mutants::skip]` — suppresses every method inside it. The reader had
    inspected only each method's own attributes, so it would have reported a
    method as selected while the tool skipped it. The reader now carries the
    enclosing decision down, and three new controls cover it: an inline `impl`
    probe, an inline file-level probe, and a committed fixture block. All three
    were checked to fail against the pre-fix reader and pass after it.
  - **ADR links resolved from the wrong directory.** Both now use `../adrs/`,
    and every relative link in the plan was re-checked to resolve.
  - **The contract test exceeded 400 lines.** The parser and selection logic
    moved to `tests/mutation_selection/reader.rs`, so parser maintenance is
    separate from the selection requirements. The duplicated workflow
    assertion was also removed: `mutation_exclusion_scope_test.py` already
    holds it against the parsed `with:` block, and asserting one contract in
    two languages buys nothing.
- [x] (2026-11-25) The second CodeRabbit pass returned one finding, verified
      against both the cargo-mutants source and the pinned tool: it matches
      `mutants::skip` on the attribute *path*, which `syn` exposes for every
      metadata form, so `#[mutants::skip()]` and `#[mutants::skip(..)]` are
      honoured too. The reader recognized only `Meta::Path`. Measured against
      cargo-mutants 27.1.0: an `impl` marked `#[mutants::skip()]` yields no
      mutants, beside one with the bare spelling, while an unmarked `impl`
      still does. The path check now runs first, and a probe covers the
      parenthesized spelling on an enclosing block. Seeding the exact pre-fix
      reader failed that probe alone — nine of ten passed — so the control is
      specific rather than incidental.
- [x] (2026-11-25) The third review pass returned two findings, both valid.
      One was a modelling gap: `#[mutants::skip]` is not the only exclusion
      cargo-mutants applies — its `attrs_excluded` also matches a bare
      `#[cfg(test)]` at `impl` and method scope — so counting such a node as
      selected overstated what the ordinary lane judges. The reader now models
      it, and a probe covers it. The rule's edges were measured, and the first
      assertion got them wrong: only a *top-level* `test` in a `cfg` excludes a
      node, so `#[cfg(all(test, unix))]` and
      `#[cfg(any(test, feature = "x"))]` are still mutated. The assertion was
      corrected to the measured behaviour rather than the code being widened.
      Seeding a reader without the gate fails that probe alone.
- [x] (2026-11-25) The other third-pass finding was a real documentation
      inaccuracy: the guide and this plan's definition of done both claimed
      every proof-only name keeps a selected counterpart, which is false for
      `output_index`. Both now name the three paired methods and state
      separately that `output_index` leaves the ordinary lane entirely.
- [x] (2026-11-25) Whitaker enforces a 400-line module limit on Rust, contrary
      to an earlier assumption recorded here; that was measured, not guessed,
      when the gate rejected `mutation_selection` at 410 lines. The parser
      probes moved to `tests/mutation_selection/tests.rs`, a child module of the
      reader, so each file is comfortably inside the limit and the probes sit
      beside the rules they check.
- [x] (2026-11-25) The fourth review pass returned three findings about reader
      fidelity, all verified against the pinned source and measured against the
      tool before being acted on. Rather than patch each one, the reader now
      models the walker's exclusion rules as a set:
  - **Inline modules were not traversed.** `visit_item_mod` returns early on
    `attrs_excluded`, so a skipped inline module suppresses everything inside
    it. The reader visited only top-level `impl` items, so it did not even
    record those methods. It now recurses through inline modules and inherits
    the exclusion.
  - **Eligibility was not modelled.** The walker also skips an `unsafe fn`, a
    constructor named `new`, and a method with an empty body. Measured: all
    three produce no mutants, while an ordinary neighbour does.
  - **A selected `cfg(kani)` implementation counted as a production twin.**
    `Selection` gains `selected_production`, and the counterpart check uses it,
    so moving a production counterpart behind the Kani gate now fails.
- [x] (2026-11-25) Three probes were added for these, and each was checked
      against a seeded reader: dropping module recursion and eligibility fails
      their two probes, and counting every selection as production fails the
      third alongside them. With the reader restored, all 14 tests pass.
- [x] (2026-11-25) The fifth review pass returned three findings on the same
      theme — a `kani_only` classification that was too shallow — and all three
      were valid:
  - **Enclosing Kani gates were not inherited.** `#[cfg(kani)] impl Demo { .. }`
    puts every method behind the gate, but the reader read only the method's own
    attributes, so those methods counted as production.
  - **Compound gates were misclassified.** A bare `kani` token scan treats
    `cfg(any(kani, unix))` as proof-only, though `unix` alone satisfies it and
    the item does compile in the ordinary lane. The reader now evaluates the
    predicate: `all(kani)` and `all(kani, unix)` require Kani, `any(kani, unix)`
    and `not(kani)` do not.
  - **The shared-logic check accepted a Kani-only implementation.** It now
    requires `selected_production`.
- [x] (2026-11-25) Scope decisions are carried as one `Scope` value rather than
      a second boolean parameter. Two probes cover the compound forms and the
      enclosing gate, and both were checked against a seeded reader: the naive
      token scan and the non-inheriting scope each fail their probe. The
      shared-logic fix was checked separately by moving `output_count` behind
      `cfg(kani)` — the pre-fix check passed, the fixed one fails it — before
      the graph module was restored.
- [x] (2026-11-25) The probes were split into `tests.rs` (attribute-level rules)
      and `scope_tests.rs` (inheritance and eligibility), because the combined
      file reached exactly 400 lines.
- [x] (2026-11-25) The sixth review pass returned two findings, both valid:
  - **Nested negation inverted the wrong thing.** `cfg(not(not(kani)))` is
    `kani`, but a flat "a negation is never a gate" rule called it production.
    The evaluator now carries a polarity: negation flips the comparator, and De
    Morgan flips the combinator, so `not(all(..))` becomes a disjunction of
    negated conjuncts and `not(any(..))` a conjunction. A probe covers both
    `not(not(kani))` and `not(any(kani, unix))`, and seeding the pre-fix
    evaluator fails that probe alone.
  - **"Exactly once" did not check for more than one.** The assertion rejected
    only `skipped == 0`, so a name with two skips passed a test whose name
    claims exactness. It now requires exactly one, and the message says why
    both directions matter.
- [x] (2026-11-25) The cfg-predicate evaluator moved to
      `tests/mutation_selection/cfg_predicate.rs`; with the negation handling
      and its probes the reader had reached 387 lines, close enough to the
      400-line limit to leave no room for the next change.
- [x] (2026-11-26) The seventh review pass found that the polarity model itself
      was wrong, and it was right to. `not(unix)` holds whenever `unix` is
      false, and `not(any(kani, unix))` is `all(not(kani), not(unix))`, which
      holds when both atoms are false; both are satisfiable without Kani, so
      neither is a gate. My previous rule made `not(<anything>)` a gate.
      The evaluator now models the question as satisfiability: each
      subexpression maps to the pair of truth values it *can* take with `kani`
      false, `kani` alone is pinned to false-only, negation swaps the pair, and
      `all`/`any` combine the pairs. A predicate is a gate exactly when it
      cannot be true. That is correct by construction rather than by case
      analysis, and it collapses the polarity threading entirely.
      The test was also wrong and is corrected: `not(any(kani, unix))` moved to
      the production group, and a `not(unix)` control was added. Seeding the
      previous negation rule fails that one test while the other fifteen pass.
- [x] (2026-11-26) The eighth review pass returned six findings. They had one
      root cause between them, plus two independent corrections:
  - Four were the same defect in different clothes: the model answered a
    two-valued question (*is this a Kani gate?*) when correctness needs three —
    proven production, proven Kani-only, or **undecidable** — and it silently
    chose "production" for anything it could not read. The cases were a
    `cfg_attr` expanding to `cfg`, a Boolean operand such as `cfg(any(true))`,
    two atoms correlated across a predicate, and two conditions correlated
    across nested scopes. The model is now exact within a bound: it collects
    the distinct atoms, enumerates their assignments with `kani` pinned false,
    and evaluates the whole conjoined predicate under each. Nested scopes are
    conjoined as predicates before evaluation rather than combined as separate
    verdicts, which is what makes the cross-scope case come out right.
    Anything still unreadable is reported as `Undecidable`, and a new contract
    assertion makes an undecidable node in the graph module a failure rather
    than a quiet pass.
  - The workflow comment named the test file as the file holding both halves,
    which made "do not add the file here" ambiguous. It now names
    `src/ir/graph/mod.rs` and describes the test only as the contract.
  - Two private helpers were imported from one collected test module by
    another. They moved to `mutation_caller_support.py`, which both import.
- [x] (2026-11-26) One expectation was wrong and corrected rather than the code
      bent to match it: `any(kani, all(unix, not(unix)))` is *decidable* under
      exact enumeration — no assignment satisfies `unix` and its negation — so
      it is a gate, not an unknown. The probe asserts that. Seeding the
      lost-conjunction defect fails two probes; all 19 pass when restored.
- [x] (2026-11-26) The ninth review pass returned two identity findings, both
      valid, and both the same shape as the earlier ones — a key that was
      shorter than the thing it identified:
  - **Selections were keyed by bare method name.** Two types can each declare
    `output_index`, so a skip on one would satisfy a check about the other and
    the protected method would go on producing false survivors. Selections are
    now keyed `Owner::method`, and every assertion names its owner.
  - **A `name = "value"` cfg atom was collected under one key and looked up
    under another**, so `cfg(feature = "alpha")` read as undecidable rather
    than production. Collection and evaluation now share `lookup_key`.
- [x] (2026-11-26) Two probes cover them: a wrong-owner control (two types with
      the same method name, one skipped) and a feature-value control. Seeding
      the bare-name key or the mismatched lookup each fails its probe; all 21
      pass when restored. The ownership and identity probes moved to
      `identity_tests.rs` when `scope_tests.rs` crossed 400 lines.
- [x] (2026-11-26) The tenth review pass returned four findings; all four were
      verified against the pinned tool or the language reference first, and all
      four were valid:
  - **A raw identifier was treated as a different atom.** `#[cfg(r#kani)]` is
    the same atom to rustc — measured: it needs no separate `check-cfg`
    declaration — so it must pin `kani`. Paths are now compared through
    `IdentExt::unraw`, in both the pinning test and the atom key.
  - **A module path was missing from the owner key.** Two modules can each
    declare the same type name, and cargo-mutants prints the module path, so
    the key now carries the lexical path: `left::BuildGraph::insert_edge` and
    `right::BuildGraph::insert_edge` are distinct.
  - **`impl Default` was not excluded.** The walker returns from
    `visit_item_impl` for any `Default` implementation, so nothing in it can be
    a production counterpart. The reader skips the whole block.
  - **The guide overstated what `--exclude-re` can do.** `allows_mutant`
    filters on `Mutant::name(true)`, and the `true` is the line-and-column
    flag, so a location-qualified pattern *can* separate a same-named pair.
    The guide, the plan, and the graph module's comment now say that, while
    explaining why such a pattern is still the wrong tool: a name-only pattern
    hits both implementations, and a location-qualified one is pinned to line
    numbers any edit invalidates.
- [x] (2026-11-26) Three controls cover the first three; seeding each defect
      fails its own probe, and all 23 pass when restored.
- [x] (2026-11-26) The eleventh review pass returned two more normalization
      findings, both valid and both the same class as the previous round:
  - `#[r#cfg(kani)]` is `#[cfg(kani)]` to rustc, but the attribute name was
    compared without normalizing, so a raw-spelled `cfg` bypassed the model
    entirely and the item read as production. Attribute names now compare
    through `IdentExt::unraw`, so a raw-spelled `cfg_attr` also reaches the
    conservative fallback.
  - The owner key used only the last segment of the `impl` type, so
    `impl other::BuildGraph` merged with `impl BuildGraph` at the same lexical
    level. The key now keeps the whole qualified path.
- [x] (2026-11-26) Two controls cover them. The first draft of the qualified
      type control used a nested `mod`, which the module path already
      distinguishes, so it passed against the defect; it was rewritten to the
      path-qualified `impl` shape the finding describes, and then failed
      correctly against the seed.
- [x] (2026-11-26) The twelfth review pass returned three findings that closed a
      whole class rather than adding cases, so they were fixed at the root:
  - **The owner key dropped information the tool keeps.** Two rounds had
    already shortened it — first dropping the module, then the qualification.
    The identity is now the *rendered self type*, plus the lexical module path,
    plus the implemented trait when there is one. That covers generic arguments
    (`Demo<u8>` vs `Demo<u16>`) and trait implementations
    (`<impl OtherTrait for Demo>`) in one change instead of one per round.
    Rendering uses `quote::ToTokens`, now a named dev-dependency; it was
    already in the graph through `syn`.
  - **The cfg search explored worlds rustc cannot produce.** Rust assigns
    `target_os` exactly one value, so `all(not(kani), target_os = "linux",
    target_os = "windows")` can never hold. Enumerating those atoms as
    independent would answer about an impossible target. A predicate naming two
    values of a single-valued built-in option is now reported undecidable
    rather than searched; `feature` stays multi-valued and is unaffected.
- [x] (2026-11-26) Four controls cover them, and each was checked against its
      seed: shortening the key fails all three identity probes, and removing the
      single-valued guard fails the cfg probe. All 27 pass when restored.
- [x] (2026-11-26) The thirteenth review pass returned three findings; all three
      were checked against the Rust reference before being acted on, and the
      `target_family` pair pulled in opposite directions — which is why the
      reference decided them:
  - **`target_family` is not single-valued.** The reference lists a target with
    both `"unix"` and `"wasm"` at once, so my conflict guard was returning
    undecidable for a valid ordinary predicate. It is removed from the
    single-valued set, which now holds only the options Rust sets once.
  - **`unix` and `windows` are derived from `target_family`.** The reference
    defines `unix` as set exactly when `target_family = "unix"` is set, so the
    two are not independent atoms and `all(unix, not(target_family = "unix"))`
    describes no target. A predicate mixing them is now declined rather than
    searched.
  - **The module path sat inside the trait wrapper.** `m::<impl Trait for
    Demo>` and `<impl Trait for m::Demo>` are different implementations, and my
    key built them identically. The implementation component is now built first
    and the module path prepended, which is the order the tool prints.
- [x] (2026-11-26) Three controls cover them, and each was checked against its
      seed: restoring `target_family` to the single-valued set fails the
      multi-family probe, and wrapping the qualified type fails the namespace
      probe. All 30 pass when restored.
- [x] (2026-11-26) The fourteenth review pass returned two findings. Rather
      than add a fifth guard to the same partial model, both were closed by
      removing the partial model:
  - **An invalid built-in value was certified as production.**
    `target_endian = "middle"` cannot hold on any target, but the search
    assigned it freely. The previous two rounds had each added a guard for one
    facet of the same problem — conflicting values, then derived families — and
    each guard needed a value domain this model does not have. The rule is now
    one line: a `name = "value"` atom on a built-in option is declined outright,
    because Rust's domains are closed and version-dependent. `feature` is
    exempt, being Cargo's open-valued key. This removed both earlier guards and
    their tables.
  - **String rewriting corrupted literals.** `compact` collapsed `" :: "` in the
    *rendered text*, so `Demo<{"a :: b".len()}>` and `Demo<{"a::b".len()}>`
    became one key. Rendering is now token-by-token over the `TokenStream`, so
    punctuation joins without touching a literal's contents.
- [x] (2026-11-26) Both controls were checked against seeds. The literal case
      merges to a single key under the old rendering and separates under the
      new one. The trade the first change makes is recorded rather than hidden:
      a valid `target_family` predicate is now declined too, and its probe
      asserts that, because a visible decline is the safe direction and a
      guessed verdict is not.
- [ ] Milestone 4: push, re-review, and the draft PR.

Two gate findings were real and are fixed, not suppressed:

- Clippy's `panic-in-result-fn` refused `assert_eq!` inside the two
  `Result`-returning fixture tests; they now return the failure through
  `ensure!`. `nonminimal_bool` was folded into an `is_none_or` predicate.
- Whitaker's capability-based filesystem policy refused
  `std::fs::read_to_string` in the new test. All three source reads now go
  through one helper that opens the crate root with `cap_std` and resolves each
  path beneath it, matching `tests/resolver_telemetry_boundary_tests.rs`.

A third finding was mine to correct rather than the code's: the first draft of
`each_proof_only_name_keeps_a_selected_implementation` demanded a production
twin for all four names, but `output_index` has no `cfg(not(kani))` twin — the
production duplicate-output scan reads the target map directly. The test was
split so the twin property covers only the three paired names, and adding a
production method purely to satisfy a test was rejected.

The fixtures were also checked against the repository's own file sweeps: a
`.rs` file under `tests/data/` is not a Cargo target, does not enter the
Nextest discovery corpus as a test, and does not trip the Kani proof-scope
scan, which looks only for `#[kani::proof]`. `tests/ui/` is the established
precedent for non-compiled Rust fixtures.

One edit was needed to stay inside the Python 400-line limit: the two new
workflow assertions moved to their own colocated module,
`tests/workflow_contracts/mutation_exclusion_scope_test.py`, which imports the
`with:`-block parsing helpers from `mutation_testing_test` rather than
duplicating them.

## Surprises & discoveries

The mechanism probe is the most important finding, because the issue text
understates the constraint. Measured against a scratch crate with
`cargo-mutants 27.1.0` `--list`:

- `#[mutants::skip]` applied directly to a `#[cfg(kani)]` item **is** honoured.
  The skip is seen even though the item is `cfg`-disabled in the ordinary
  build, so `--list` does not emit mutants for it. This is the mechanism that
  works.
- `#[cfg_attr(<bare path>, mutants::skip)]` such as
  `#[cfg_attr(test, mutants::skip)]` is honoured, matching the book's example.
- `#[cfg_attr(any(), mutants::skip)]` is **not** honoured. An argument list is
  not a path, and `attr.parse_nested_meta` rejects it, so the skip is silently
  dropped. A "condition that can never hold" is therefore not a way to make the
  attribute free.
- `#[cfg_attr(kani, mutants::skip)]` on a `#[cfg(kani)]` item is honoured, but
  it is the wrong tool here: it is honoured *because* cargo-mutants ignores the
  condition, which makes the condition decorative and the intent unclear.
- Assigning `RUSTFLAGS="--cfg kani"` to force the ordinary build to compile the
  Kani items fails: the `kani` crate is not a dependency, so `#[kani::proof]`
  harnesses do not resolve. This confirms, by measurement, that the issue's
  claim is right that merely setting the cfg does not turn `cargo test` into a
  proof runner.

A further pair of measurements fixes the compilation story:

- In an ordinary build, `#[cfg(kani)]` strips the item before the
  `mutants::skip` path is resolved, so a crate with no `mutants` dependency
  compiles cleanly. The dependency is therefore not needed for `cargo test`,
  `cargo check`, `cargo clippy`, or rustdoc.
- In a build where `cfg(kani)` holds, the path *is* resolved, and without the
  dependency rustc reports `E0433: cannot find module or crate mutants`.
  Declaring `mutants` as a regular dependency makes that build pass.

So the `mutants` crate is a real direct dependency whose only consumer is the
Kani lane. It is a five-line MIT proc-macro crate from the same author as
cargo-mutants, and the repository's dependency policy requires its presence to
be justified and its ownership recorded.

One consequence to measure rather than assume: because the dependency is
unreferenced in ordinary builds, Cargo's `unused_dependencies` lint may warn
there. The plan measures the real build's warnings and handles what actually
appears.

A separate negative result closes off the obvious alternatives. The mutant
names emitted for the Kani-only `target_for_output` and its production
counterpart differ only by line and column: the baseline inventory lists the
same text at line 181 and line 194. cargo-mutants 27.1.0 matches `--exclude-re`
and `mutants::exclude_re` against `Mutant::name(true)`, which includes that
location, so a pattern *could* separate them. It is still the wrong tool: a
pattern naming only the function matches both implementations, which is the
failure this issue describes, and a location-qualified pattern is pinned to
line numbers that any edit above it invalidates. The source attribute needs
neither and cannot drift.

## Decision log

- **Mechanism: `#[mutants::skip]` on each of the four items, plus a `mutants`
  dependency.** Chosen because it is the only spelling of the three probed that
  is both honoured by 27.1.0 and honest about intent. `exclude_re` is rejected
  because a name regex cannot distinguish a Kani-only `target_for_output` from
  its production counterpart in the same file, which is exactly the defect
  being fixed. `exclude-globs` is rejected because it is file-scoped. A
  name-based `--exclude-re` in the config file is rejected for the same reason
  as `mutants::exclude_re`.
- **Scope the skips to the four items, not the `impl` block.** A block-level
  skip would cover the production counterparts, which must stay selected.
- **Keep the regression at the selection boundary.** The test resolves the
  actual per-item decision for a same-named pair rather than grepping for a
  token.

## Conformance basis

- `AGENTS.md` — dependency policy (caret requirements), doc-comment rules,
  400-line file limit, en-GB-oxendict spelling, quality gates, and the rule
  that a change needing a new abstraction records its ownership boundary.
- `AGENTS.md` — docs maintenance: update the developers' guide for internally
  facing practice, and index any new document from `docs/contents.md`.
- [ADR-0073](../adrs/adr-0073-split-the-kani-mutation-compile-gate.md) — the
  Kani mutation compile gate's shape and its negative control, which this
  change must not weaken.
- [ADR-0039](../adrs/adr-0039-run-the-kani-proofs-on-a-pull-request-only-when-their-inputs-change.md)
  — the proof lane's trigger policy, preserved unchanged.
- `docs/developers-guide.md`, "Mutation-testing workflow contract tests" — the
  section this change amends.

## Verification plan

Selection evidence (the primary acceptance command):

```sh
cargo mutants --list --all-features --file src/ir/graph/mod.rs
```

Passes when none of the four skipped functions' mutants appear, and every
counterpart and shared function above still does.

Gate set, run sequentially:

```sh
make check-fmt
make lint
make doc-coverage
make test
make test-workflow-contracts
make markdownlint
```

The proof lane and the mutation compile gate are two different things, and an
earlier draft of this section conflated them. It claimed
`make test-kani-mutations` showed the proof lane was unchanged. It does not:
that target runs the mutation prewarm, a compile-rejection negative control,
and the patched-tree compile guard. It compiles proof-adjacent code; it never
runs a proof harness, so a pass there says nothing about whether any property
still holds.

Both are run, and their results are recorded separately:

- `make kani-full` runs the proof harnesses themselves, which is the only
  evidence that the proofs still pass after the `mutants` dependency and the
  four `#[mutants::skip]` attributes joined the crate. This is the command the
  definition of done's "the Kani proof lane … still run[s] and pass[es]" clause
  refers to.
- `make test-kani-mutations` remains the mutation compile gate that ADR-0073
  describes: every committed patch under `docs/verification/mutations/` is
  applied, compiled through the Kani frontend, and reverted. It is unchanged by
  this work and is not evidence about the harnesses.

Measured on 2026-11-25: `make kani-full` reports
`Complete - 17 successfully verified harnesses, 0 failures, 17 total`. That is
the whole proof inventory the developers' guide lists, so adding the `mutants`
dependency and the four attributes left every property intact — including the
`cfg(kani)` graph code the attributes now annotate.

Neither command is modified by this change.

The proof-lane result is the one that matters here, and it is green. The
mutation compile gate is reported honestly rather than assumed. It timed out
twice at the prewarm's 900 s allowance: 326 admission waits with a queue depth
peaking at 216 and **zero crates compiled** in the second run. That is host
saturation, not a verdict about the code, and the same host was carrying
several other agents' builds throughout.

The one way this change could genuinely have broken that gate is by shifting
line numbers in `src/ir/graph/mod.rs` far enough to displace the committed
patches, so that was checked directly rather than inferred: all 30 files under
`docs/verification/mutations/` still pass `git apply --check`. Two of them
touch the graph module, and both apply.

Combined with `make kani-full` verifying all 17 harnesses against the same tree
— which is what exercises the `mutants` dependency and the four attributes
through the real Kani frontend — the proof lane is established as intact. The
compile gate's timeout is recorded as an environment limitation, not as a pass.

## Outcomes & retrospective

The issue's own acceptance criteria are met. The six reported false survivors
are gone from the pinned-tool inventory, nothing was added, and every
production counterpart plus the shared graph logic remains selected. The proof
lane is unchanged and verified: `make kani-full` reports all 17 harnesses
verified.

### What the work actually cost, and why

The source change is 30 lines. The contract around it is roughly 2,400 lines
across nine files. That ratio is the honest headline: the four skipped items
were trivial to write and expensive to *prove correct*, because the reader that
certifies the exclusion has to model cargo-mutants' selection rules faithfully.

Thirteen review passes found three classes of defect in that reader, each
recurring several times:

1. **A key shorter than the thing it identified.** The owner key was shortened
   to the bare method name, then to the type's last path segment, then lost its
   module path. Each round the reviewer found the next distinction it had
   dropped. Fixing it at the root — render the whole self type, prepend the
   module path, wrap trait implementations outside it — closed the class.
2. **A two-valued answer to a three-valued question.** "Is this a Kani gate?"
   has a third answer, *undecidable*, and the first model silently chose
   "production" for anything it could not read. Making `Undecidable` a real
   value, and a failure rather than a quiet pass, closed that class.
3. **A boolean search over impossible worlds.** Enumerating cfg atoms as
   independent considers assignments rustc cannot make. Exact enumeration fixed
   the single-predicate cases; the cross-scope case needed the predicates
   conjoined and evaluated as one formula; the built-in-option cases needed the
   reference to say which options are single-valued and which, like
   `target_family`, are not.

The lesson worth carrying: when a test certifies another tool's behaviour, the
test's fidelity *is* the work. A contract that models a rule approximately will
pass every seed written against its own approximation, and only an adversarial
review reading the tool's source finds the gap. Several of these rounds were
found by the reviewer's citation to a specific line of `visit.rs` or
`options.rs`, which is the standard the contract should have been held to from
the start.

### What was deliberately not done

- `make test-kani-mutations` never completed. It timed out twice at its 900 s
  prewarm under heavy host saturation, with zero crates compiled in the second
  run. The one thing this change could have broken — line shifts displacing the
  committed proof patches — was checked directly instead: all 30 still apply.
  Recording a timeout as a pass would have been dishonest.
- No proof-aware mutation lane was built. Judging these implementations by
  mutation needs a runner that executes `cargo kani` per mutant with real
  harness verification and negative controls. That does not exist here, and the
  guide records its absence rather than implying the exclusion is equivalent to
  proof.
- One assertion was corrected rather than the code being changed to match it:
  `any(kani, all(unix, not(unix)))` *is* decidable under exact enumeration, and
  is a gate. Bending the model to accept the original expectation would have
  introduced a real defect to satisfy a wrong test.

### Residual risk

The reader is a model, not the tool. `MAX_ATOMS` bounds the search, and a
predicate beyond that bound is reported undecidable rather than answered. That
is the safe direction: an undecidable node in the graph module fails
`no_graph_implementation_has_an_undecidable_cfg`, so a future predicate the
model cannot handle surfaces as a test failure rather than as a silent
misclassification.

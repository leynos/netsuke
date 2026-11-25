# Exclude Kani-only graph implementations from ordinary mutation selection (issue #1038)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: IN PROGRESS

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

A separate negative result closes off the alternative mechanism. The mutant
names emitted for the Kani-only `target_for_output` and its production
counterpart are *identical* — the baseline inventory lists the same two strings
at line 181 and line 194 — so a name-based `--exclude-re` or
`mutants::exclude_re` cannot separate them. Excluding by name would either
leave the false survivors in place or remove the production mutants too. The
source attribute is the only mechanism that can distinguish the two.

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

The Kani lane is exercised by `make test-kani-mutations`, which compiles every
committed proof mutation patch through the Kani frontend. It is run to show the
proof lane is unchanged; it is not modified.

## Outcomes & retrospective

To be completed.

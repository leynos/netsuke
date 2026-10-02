# Adopt `rstest-bdd` v0.6.0

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: COMPLETE

The migration work is complete and all its behavioural evidence is green. The 39
`cognitive_complexity` errors the dependency bump introduces in the
`make lint` gate have been resolved by hoisting the charged `tracing` macros
into per-macro emitters across 40 files; the root cause, the cost model, the
escalation that was raised and then superseded, and the resolution are recorded
in `Surprises & discoveries` and `Decision log`. The full gate set runs green
at the delivered revision, with every stage of every gate reached; see
`### Gate logs`.

Roadmap item: none. Origin: `leynos/rstest-bdd` v0.6.0 release and its
`docs/v0-6-0-migration-guide.md`.

## Purpose / big picture

Netsuke's behavioural suite runs on `rstest-bdd` 0.5.0. The upstream project
has published 0.6.0, whose headline behavioural change is a correctness fix: a
step whose return type is a *type alias* of `Result<T, E>` previously had its
`Err` silently discarded, so the scenario passed even though an assertion
failed. Netsuke's step functions are declared `-> Result<()>` where `Result` is
`anyhow::Result`, which is exactly such an alias. After this migration the
suite runs on 0.6.0, under the same scenarios as `main`, and the four published
breaking changes that touch this repository are applied deliberately rather
than discovered as compile errors.

Success is observable by running `make test` and seeing all behavioural tests
pass from `tests/features/*.feature` and `tests/features_unix/*.feature` while
`Cargo.toml` declares `rstest-bdd = "0.6.0"` and `Cargo.lock` resolves the whole
`rstest-bdd` family to 0.6.0. The migration changes the swept scenario
inventory by **zero** tests, and deliberately so: this plan migrates an
existing consumer and introduces no new BDD. `main` grew the inventory from 254
to 269 while this branch was in review, so the current figure is 269 — equal to
`origin/main` and a superset of the 254-name baseline. See
`Surprises & discoveries` for the three-revision reconciliation. A scenario
that turns red because the alias fix exposed a previously-swallowed `Err` is a
finding to investigate, not a regression to suppress.

## Constraints

- Preserve existing behavioural coverage. No scenario may be dropped, and no
  assertion may be weakened to make the migration compile or pass.
- Imported upstream documentation is byte-for-byte intact. Provenance is
  recorded separately, never inside the imported copies.
- Do not introduce BDD into crates that do not use it. `rstest-bdd` stays a
  dev-dependency of the root package only.
- Do not copy upstream workspace `path` dependencies, the GPUI shim, or
  `vendor/gpui` into this repository.
- Keep the `rstest-bdd-macros` `strict-compile-time-validation` feature. Do not
  relax compile-time validation to make the migration compile.
- Keep caret requirements for all dependencies, per `AGENTS.md`.
- `netsuke-build` is a published library. Its declared `rust-version` must not
  advertise a compiler on which a promised target fails.
- The Polonius nightly pin in `rust-toolchain.toml` is load-bearing and is not
  to be changed unless the dependency graph demonstrably requires it.
- Do not use `--ignore-rust-version`, `#[ignore]`, or blanket snapshot
  blessing.

If satisfying the objective requires violating any constraint, stop and record
the conflict in `Decision log` before proceeding.

## Tolerances (exception triggers)

- Scope: if migration requires touching more than 30 files or 1,500 net lines,
  stop and escalate.
- Interface: if a public API in `src/` must change, stop and escalate.
- Dependencies: if a third-party dependency outside the `rstest-bdd` family
  must be added, stop and escalate.
- MSRV: if the resolved graph requires a compiler floor above the pinned
  nightly, stop and record before changing the pin.
- Iterations: if `make test` fails after 3 full fix cycles, stop and escalate.
- Ambiguity: if the published documentation and the pinned source disagree in a
  way that materially changes the migration, stop and present the conflict.

## Risks

- Risk: upstream `docs/CHANGELOG.md` mixes unreleased v0.7.0 content into the
  same file as 0.6.0, and the migration guide documents guard-based
  `StepContext` borrowing as "v0.7.0" while the published 0.6.0 crate and the
  `v0.6.0` tag both contain it. Following the guide's framing blindly would
  mean assuming an API is unavailable when it is present. Severity: medium
  Likelihood: high Mitigation: resolve every discrepancy against the pinned tag
  source and the published crate; record the evidence. Do not implement
  v0.7-only features that the crate does not contain, and do not avoid 0.6.0
  APIs the crate does contain. Recorded in `Surprises & discoveries` below.

- Risk: 0.6.0 classifies step returns by concrete type, so the `#[expect(...)]`
  attributes in `tests/bdd/steps/` that cite upstream issue #381 may become
  "unfulfilled expectation" errors if the generated wrapper no longer triggers
  those lints. Severity: medium Likelihood: medium Mitigation: the oracle is the
  `-D warnings` compilation inside `make test-nextest`, *not* `make lint` —
  the latter aborts in `src/` before reaching the test targets
  (`Surprises & discoveries`). **Resolved 2026-09-26:** the risk did not
  materialize. The count was originally stated as 41; a multi-line-aware parse
  gives 45 across 16 files. All 45 are proven achieved by the green test
  compilation. Three sit under `#[cfg(not(unix))]` and remain Windows-CI-only
  evidence. No expectation was removed, and none was converted to `#[allow]`.

- Risk: the 39 `strict-compile-time-validation` checks may reject a step shape
  that 0.5.0 accepted. Severity: low Likelihood: low Mitigation: the feature is
  retained unchanged; compile errors name the offending step.

- Risk: `gherkin 0.16` raises the effective compiler floor to Rust 1.88 and the
  repository builds on a nightly pinned at 2026-08-23, far newer. The pin is
  therefore expected to be unaffected. Severity: low Likelihood: low
  Mitigation: confirm the resolved graph compiles under the existing pin before
  considering any toolchain change. A nightly date alone does not establish
  that a bump is necessary.

- Risk: the `#[expect(clippy::shadow_reuse)]` family is a Rust-2024-related
  lint whose firing depends on generated wrapper shape. Severity: low
  Likelihood: medium Mitigation: run `make lint-clippy` as a focused first
  signal before the full gate.

## Progress

- [x] (2026-09-26) Rename the branch to `adopt-rstest-bdd-v0-6-0`, tracking
  `origin/adopt-rstest-bdd-v0-6-0`.
- [x] (2026-09-26) Read `AGENTS.md` and the repository instructions; confirm the
  working tree is clean at `ebcedaef`.
- [x] (2026-09-26) Fetch `leynos/rstest-bdd` and resolve tag `v0.6.0` to
  `72fb22635670e456545ca368805ba4c1c9d7bd69`.
- [x] (2026-09-26) Read the v0.6.0 migration guide and the v0.5.0 guide.
- [x] (2026-09-26) Inventory manifests, dependencies, step/scenario counts, and
  direct runtime API usage.
- [x] (2026-09-26) Establish the baseline BDD inventory: 254 scenarios, 254
  generated tests, captured to `/tmp/bdd-before.txt` at `ebcedaef`.
- [x] (2026-09-26) Import `docs/users-guide.md` and
  `docs/v0-6-0-migration-guide.md` from the pinned commit; record provenance.
  Committed as `e62af317`.
- [x] (2026-09-26) Bump the `rstest-bdd` family in `Cargo.toml` and update
  `Cargo.lock` with targeted `--precise 0.6.0` updates.
- [x] (2026-09-26) Reconcile lint expectations and step signatures; fix any
  newly-red scenario. Finding: no step signature changed and no scenario turned
  red, because the fixture-name normalization strips *one leading underscore*
  and this repository's fixtures use the unprefixed identifier `world`. All 45
  expectations are proven achieved by the green `-D warnings` test compilation;
  none was removed or weakened. See `Surprises & discoveries`.
- [x] (2026-09-26) Update `docs/developers-guide.md` and `docs/contents.md`.
  Both were already written in `e62af317`; this session corrected the async
  bullet to name the canonical `TokioHarness` instead of the now-deprecated
  `runtime = "tokio-current-thread"` syntax, and added the
  `adopt-rstest-bdd-v0-6-0.md` plan reference.
- [x] (2026-09-26) Discharge INV-3: probe the corrected `Err` propagation, then
  add `tests/step_error_propagation_tests.rs` as its durable guard, validated
  green-to-red-to-green. The guard is a separate target, so the *swept* BDD
  inventory is untouched by it and remains exactly 254 at `c68cd30f` — the
  figure that was correct then.
- [x] (2026-09-26) Run the full gate set on a frozen revision and compare the
  migrated inventory against baseline. Six of seven gates pass, including
  `markdownlint`, whose `markdownlint-cli2` stage had never previously executed.
  `make lint` is red with 39 `cognitive_complexity` errors and is recorded as
  **unavailable**, not as a pass. Logs are in `### Gate logs`. Inventory
  comparison: 254 scenario names before and after, identical as sets — correct
  for this date, since `main` had not yet grown the inventory.
- [x] (2026-09-26) Commit as `c68cd30f`, push, and open draft PR
  [#805](https://github.com/leynos/netsuke/pull/805) against `main`. Not merged
  and no release published, per the session's instructions.
- [x] (2026-10-02) Root-cause the `make lint` failure. Earlier sessions had
  recorded it as pre-existing on `main` and "environmental"; that was **wrong**
  and is corrected in `Surprises & discoveries`. The failure is caused by the
  migration: `rstest-bdd` 0.6.0 is the only release of that crate that declares
  `tracing` at all, it declares it non-optionally with `features = ["log"]`,
  and because `rstest-bdd` is a dev-dependency while `tracing` is a normal
  dependency of this package, Cargo unifies `tracing/log` onto the library's
  own node. `tracing`'s `log` feature expands every `tracing::*` macro into
  extra `log` calls, which inflates the AST that the syntactic
  `cognitive_complexity` metric counts. Proven by a minimal repro, by a
  repository-scale positive control on byte-identical `src/`, and by a
  counter-control that patches only `features = ["log"]` out of 0.6.0's
  manifest. The shipped library is unaffected: `--lib` resolves 0 `log` edges
  and exits 0, while `--all-targets` resolves 1 and exits 101.
- [x] (2026-10-02) Resolve the 39 `cognitive_complexity` errors by hoisting the
  charged `tracing` macros into per-macro emitters. Four suppression options
  were tested and rejected on evidence; the escalation recorded against the
  refactor's 31-file scope was superseded after measurement showed every site's
  structural complexity to be at or below 7, so no function needed
  decomposition. The honest probe
  (`cargo clippy --workspace --all-targets --all-features --keep-going`) is
  clean at zero errors. See `Decision log`.
- [x] (2026-10-02) Correct the record and re-deliver. The plan's `COMPLETE`
  status and the PR description both carried the wrong "pre-existing on `main`"
  diagnosis; both are corrected, and the corrected revision `7ac904e8` is
  pushed. `make check-fmt` and `make markdownlint` are re-run green on that
  revision (166 files, 0 issues) before committing, and the commit contains the
  ExecPlan alone, with the gate-regenerated `typos.toml` drift reverted to keep
  the commit atomic.
- [x] (2026-10-02) Rebase onto `upstream/main` (`84447f0e`) and re-resolve the
  three conflicting files (`manifest/env_reader.rs`, `runner/graph.rs`,
  `stdlib/which/cache.rs`). The rebase replayed all nine commits linearly with
  none behind. `cache.rs` resolved to `main` exactly, taking `main`'s deletion
  of the recorder functions, with the hoist ported to `main`'s new
  `stdlib/which/telemetry.rs` instead.
- [x] (2026-10-02) Re-run the honest Clippy probe on the rebased tree, which is
  the step that invalidated the earlier "all clear". Two `cognitive_complexity`
  errors appeared in `tests/kani_mutation_evidence_tests/compile_guard.rs`, a
  file `main` added after this branch was opened. A counter-control reverting
  only `rstest-bdd` to 0.5.0 re-ran the identical probe to exit 0, proving the
  errors are this migration's rather than `main`'s — the diagnosis the plan had
  already withdrawn once.
- [x] (2026-10-02) Clear all three complexity sites by hoisting, and record the
  measured cost model: +7 per inline `tracing` macro under `log`, +1 without,
  and nothing extra for a caller that merely invokes an emitter.
- [x] (2026-10-02) Discover and fix a second cascade-masked failure: hoisting
  pushed `compile_guard.rs` to 413 lines, past Whitaker's 400-line
  `module_max_lines` cap, which had never run because Clippy failed ahead of
  it. Split the patch-application machinery into a sibling `apply_patch`
  module, leaving the guard at 294 lines. `make lint-whitaker` now passes both
  of its passes. Committed as `dd93120a`.
- [x] (2026-10-02) Run the full eight-target gate set on the committed
  revision `4f0ac198`, with this plan's record of the run written first so that
  no write follows the gates. All eight pass with their exit codes captured to
  `.exit` sidecars, and `make lint` reaches every stage. An earlier attempt was
  rejected as evidence: it ran the gates against the working tree and edited
  this plan afterwards, so `check-fmt` had read the developers' guide three
  seconds before that file's last write. Re-running `check-fmt` there failed
  for real (`+7 -7`, exit 2), which `make fmt` then fixed as a pure wrapping
  artefact. Recorded under `#### Final sweep on the delivered revision`.

## Surprises & discoveries

- Observation: `make lint` is red on this branch with 39
  `cognitive_complexity` errors in `netsuke-build (lib)`. The migration **is**
  the cause, by a feature-unification edge the dependency bump introduces — not
  by any change to `src/`, which is byte-identical to the base.

  Root cause: `rstest-bdd` 0.6.0 declares
  `tracing = { version = "0.1", features = ["log"] }` as a **normal**
  (non-optional) dependency of the library. No earlier release does; 0.5.0 and
  every 0.6.0 pre-release declare no `tracing` dependency at all. Because
  `rstest-bdd` is a dev-dependency of the root package but the root package's
  library depends on `tracing` non-dev, Cargo's default resolver **unifies** the
  `tracing/log` feature onto the single `tracing` node the library itself
  compiles against. `tracing`'s `log` feature expands each `tracing::*` macro
  to also call into the `log` crate, which grows the expanded AST that Clippy's
  syntactic `cognitive_complexity` metric counts. The error message changes from
  `netsuke-build (build script)` to `netsuke-build (lib)` precisely because
  the node whose feature set changed is the library's.

  Minimal repro (isolated scratch crate, threshold forced to 0 so any value is
  reported; `debug!` is the only statement in the function):

  ```plaintext
  $ cargo clippy --lib --no-default-features -- -W clippy::cognitive-complexity
  warning: the function has a cognitive complexity of (2/0)   # debug!(x, "one event")

  $ cargo clippy --lib --no-default-features --features trace-log \
      -- -W clippy::cognitive-complexity
  warning: the function has a cognitive complexity of (8/0)   # identical body
  ```

  Positive control at repository scale, arm-for-arm: base commit `ebcedaef` with
  `src/` byte-identical and `cognitive_complexity` demoted to `warn` so Clippy
  reports every site instead of aborting at the first:

  | Arm | Dependency set                                                                   | lib warnings | sites             |
  | --- | -------------------------------------------------------------------------------- | ------------ | ----------------- |
  | A   | base (`rstest-bdd` 0.5.0)                                                        | 0            | 0                 |
  | P   | base `src/` + `tracing = { features = ["log"] }`                                 | 39           | 39, identical set |
  | B   | base `src/` + `rstest-bdd` 0.6.0 + `tracing/log` patched out of 0.6.0's manifest | 0            | 0                 |

  The 39 sites in arm P are the same `file:line:col` set that CI reports on the
  branch (verified by `comm` on cleaned logs; arm P additionally lists
  `src/main.rs:342` and `src/test_tracing_capture.rs:233`, which are outside the
  `(lib)` unit). Patching `features = ["log"]` out of 0.6.0's manifest alone
  restores green with 0.6.0 still selected, which isolates the cause to that
  one feature edge rather than to the version bump at large.

  Scope of the damage, measured by `cargo tree -e features -i tracing` on the
  branch: the `tracing/log` edge appears only when dev-dependencies are in the
  graph. `--lib` and `--bin netsuke` each resolve it to **0** edges;
  `--all-targets` resolves it to **1**. The consequence is that
  `cargo clippy --lib --all-features -- -D warnings` on the branch exits 0 with
  zero diagnostics, while
  `cargo clippy --workspace --all-targets --all-features -- -D warnings` exits
  101 with 39. The shipped library never links `log`; only the lint build that
  merges in the test graph sees the inflated expansion.

  Not the cause, each ruled out by direct measurement: Clippy's threshold (9
  since the initial commit), `clippy.toml` and `[lints] workspace = true`
  (byte-identical across base, branch and `main`), `[features]`, the `build.rs`
  slice list, and CI caching — the base run is green cold in CI *and* in a cold
  local worktree, while the branch is red cold in both. The `#[path]`-declared
  modules under `src/cli/discovery*.rs` were investigated and are a red
  herring: they are declared at module scope, not behind `#[cfg(test)]`, so
  they are reachable from `--lib` too. Two candidate mechanisms were tested and
  **failed**: setting `resolver = "3"` leaves the edge intact (it does not
  apply, because the unifying pair is a normal dependency unified against a
  dev-dependency of the same package), and cargo's plain
  `--no-dev-dependencies` view does not model the lint build.

  Impact: this is a real regression in a repository gate caused by the
  migration, and it is reported as such. `make lint` cannot be used as a green
  signal for this branch. Because the gate stops at `lint-clippy`,
  `lint-whitaker`, `lint-python` and `github-actions-lint` never ran, and are
  unavailable checks rather than passes. Disposition of the 39 sites is
  recorded in `Decision log`.

- Observation: the 0.6.0 dependency bump did **not** turn any BDD scenario red,
  despite 177 of 185 step functions returning the `anyhow::Result<()>` alias
  whose `Err` was previously discarded.

  Evidence: `make test` on the migrated tree reports
  `3413 tests run: 3413 passed, 5 skipped`. Extracting the generated scenario
  tests from that run and comparing them as sets against the pre-migration
  baseline gives exact equality: 254 scenarios before, 254 after, with no
  additions and no losses. The baseline artefact is `/tmp/bdd-before.txt`, 254
  names captured at `ebcedaef`.

  The 254 figure is only meaningful against that baseline. Measured at three
  revisions, the swept inventory (the `features_scenarios::` and
  `features_unix_scenarios::` tests generated from `tests/features/` and
  `tests/features_unix/`) is:

  | Revision              | Scenario names | Note                          |
  | --------------------- | -------------- | ----------------------------- |
  | `ebcedaef` (baseline) | 254            | the recorded baseline         |
  | `84447f0e` (`main`)   | 269            | `main` added 15 during review |
  | `HEAD`                | 269            | migration's own delta is zero |

  `main`'s 15 additions are the `shell_quote`/`shell_join`/`compact`
  /env-default scenarios from the 3.14.8 and Jinja-ergonomics work that landed
  while this branch was in review. The `HEAD` name set equals `main`'s exactly,
  and the 254 baseline names are a subset of it, so the migration moved the
  count by zero and lost nothing. Two independent measurements agree on 269: a
  grep of `Scenario:`/`Scenario Outline:` lines across the swept directories,
  and the generated test names in the `make test` log.

  Impact: invariant INV-1 (coverage preservation) and INV-3's central worry (a
  former false green becoming red) are both discharged by observation. No
  scenario needed a fix, and none was disabled. The suite had no latent
  swallowed `Err` that the corrected propagation could expose.

- Observation: the 45 `#[expect(clippy::…)]` attributes under `tests/bdd/steps/`
  are already verified as *achieved* by the green `make test` run, and so do
  not depend on the red `make lint` to be trustworthy.

  Evidence: `make test-nextest` compiles
  `--workspace --all-targets --all-features` under `GATE_RUSTFLAGS`, which sets
  `RUSTFLAGS="… -D warnings"` (`Makefile` lines 78 and 232). `-D warnings`
  implies `-D unfulfilled-lint-expectations`; the probe below confirms the
  oracle is live, and confirms that an *achieved* expectation produces no
  diagnostic:

  ```plaintext
  $ rustc --edition 2024 -D warnings --emit=metadata \
      -o /tmp/a.rmeta /tmp/fulfilled.rs        # #[expect(unused_variables)] on a fn
                                               # that really does have an unused arg
  exit=0                                       # no diagnostics

  $ rustc --edition 2024 -D warnings --emit=metadata \
      -o /tmp/b.rmeta /tmp/unfulfilled.rs      # same attribute, lint does not fire
  error: this lint expectation is unfulfilled
   --> /tmp/unfulfilled.rs:2:10
    |
  2 | #[expect(unused_variables)]
    |          ^^^^^^^^^^^^^^^^
    |
    = note: `-D unfulfilled-lint-expectations` implied by `-D warnings`
  exit=1
  ```

  Inventory (multi-line-aware parse of `tests/bdd/`): 45 attributes — 23
  `shadow_reuse`, 19 `unnecessary_wraps`, 1 `option_if_let_else`, 1
  `missing_const_for_fn` — spread over 16 files, densest in
  `tests/bdd/steps/manifest/targets.rs` (11). Grepping the test log for
  `unfulfilled`, `warning:` and `error:` yields 0, 0 and 33; the 33 `error:`
  hits are test *names* (`…validation_error::case_…`), not diagnostics.

  Bound: three of the 45 sit under `#[cfg(not(unix))]`
  (`conditional_manifest.rs:106`, `progress_output.rs:29`,
  `stdlib/workspace.rs:229`) and are therefore **not** reached by a Linux
  compilation. Each carries a written `reason` and the same documented shape —
  a fallible `const fn` that must match its Unix variant's signature — so the
  `unnecessary_wraps` expectation remains live by the same argument that
  justified it originally. Windows CI is the oracle for those three; the same
  constraint already recorded under the Windows toolchain Risk applies.

  Impact: EP-M3's lint-expectation obligation is discharged from evidence
  `make test` already produced, not from the unavailable `make lint`. That
  matters because `make lint` cannot currently answer the question:
  `lint-clippy` aborts in `src/` before it reaches the test targets, so a green
  `make lint` was never available as a signal here. No expectation was removed,
  and none was converted to an `#[allow]`.

- Observation: the `docs/developers-guide.md` async bullet that the migration
  inherited told readers to "Keep async execution on Tokio current-thread
  runtime" — which 0.6.0 now deprecates.

  Evidence: `crates/rstest-bdd-macros/src/macros/scenarios/mod.rs` in the
  pinned v0.6.0 checkout emits, for `runtime = "tokio-current-thread"` without
  an explicit harness, "the `runtime = \"tokio-current-thread\"` syntax is
  deprecated; use `harness = rstest_bdd_harness_tokio::TokioHarness` instead".
  The migration guide's "New features requiring new practices" section says the
  same. This repository's builds set `-D warnings` (`Makefile` line 78), and
  the upstream guide records that `#![deny(deprecated)]` escalates the same
  warning to an error, so following the old bullet would produce a build
  failure rather than a style regression.

  Scope check, so the correction is proportionate: no `scenarios!` invocation
  in this repository passes `runtime =`, and `TokioHarness`, `sync_to_async`,
  `StepCtx`, `StepTextRef`, `StepDoc` and `StepTable` appear **only in prose
  documents** — `git grep` finds no use in `tests/` or `src/`. The bullet was
  stale guidance, not a false statement about live code. Each of those names
  was confirmed still exported by the pinned source, so the surviving bullets
  stand.

  Impact: EP-M4's async bullet was rewritten to name the canonical harness and
  to say explicitly that synchronous scenarios need no harness at all — which
  is the task's actual requirement, and which the old bullet obscured. No code
  changed, because there is no async step here to migrate.

- Observation: the `v0.6.0` tag and the published `rstest-bdd 0.6.0` crate
  contain byte-identical `src/` trees, and both contain APIs the migration
  guide attributes to v0.7.0.

  Evidence: `git rev-parse v0.6.0` resolves to
  `72fb22635670e456545ca368805ba4c1c9d7bd69`. A recursive `sha256sum` over every
  `.rs` file under `crates/rstest-bdd/src` and `crates/rstest-bdd-macros/src`
  from the tag equals the same digest computed over the extracted
  `static.crates.io` `.crate` archives: `rstest-bdd` = `456fe6b2…e4a5`,
  `rstest-bdd-macros` = `acf79195…7e67` for both sources.
  `rg 'pub fn borrow_mut'` finds `&'b mut self` in 0.5.0 but `&'b self` in both
  the 0.6.0 tag and the published 0.6.0 crate, and both export
  `FixtureBorrowError` and `try_borrow`/`try_borrow_mut`.

  Impact: the migration guide's label of guard-based borrowing as "v0.7.0"
  describes a later *documentation* grouping, not the crate contents. The
  task's instruction to avoid "v0.7 guard-based borrowing" therefore applies to
  *adopting the v0.7 migration practice*; it does not mean the 0.6.0 crate lacks
  `&self` borrow receivers. This repository calls no borrow method directly,
  so the distinction is inert here; it is recorded because it would otherwise
  mislead any later reader of the guide.

- Observation: `Slot<T>` and the `strict-compile-time-validation` feature both
  survive into 0.6.0 unchanged.

  Evidence: `crates/rstest-bdd/src/state.rs:30` and
  `crates/rstest-bdd-macros/Cargo.toml` under the tag. The published
  `rstest-bdd-macros-0.6.0` feature table lists
  `strict-compile-time-validation = ["compile-time-validation"]`.

  Impact: the largest consumers of netsuke's suite — `Slot` use in
  `tests/bdd/fixtures/mod.rs` and the strict-validation feature choice — need
  no source change. The migration is materially smaller than the guide's scope
  suggests.

- Observation: netsuke's step functions declare `-> Result<()>` where `Result`
  is `anyhow::Result`, so 177 of 185 steps are aliases of the type whose `Err`
  was previously discarded.

  Evidence: `rg 'use anyhow::\{.*Result'` matches every `tests/bdd/steps/*`
  module. A signature sweep over the 185 step functions finds 177 `Result<()>`,
  7 `()`, and one non-step helper returning a tuple.

  Impact: this is the repository's live exposure to the fix. If any of those
  177 steps contains a step that returns `Err` while its scenario currently
  passes, 0.6.0 will surface it as a real failure. That is the migration's
  acceptance test, not an obstacle.

- Observation: no scenario in the suite exercised the corrected propagation,
  so the fix arrived unguarded and a focused regression check had to be added.
  This is the *opposite* of the false-green the migration guide warns about,
  and it is the reason INV-3 could not be discharged by observation alone.

  Evidence: every scenario that passes walks a green path whose steps return
  `Ok`. A green suite is therefore consistent both with propagation working and
  with it being silently reverted; the scenario-set equality against the
  baseline proves the first, not the second. A deliberate injection into
  `documentation_file_contains` settled the behaviour question: with the step
  forced to `Err`, both dependent scenarios failed, naming the injected string,
  and reverted cleanly. That experiment was a *probe*, not a permanent guard, so
  `tests/step_error_propagation_tests.rs` was added to hold the property
  durably.

  The new test is a two-sided oracle rather than a decoration. It is green on
  the migrated tree, and it goes red when the propagation is simulated away:
  neutralizing the failing step to `Ok(())` makes it fail on the *trailing*
  assertion, `trailing step ran: the earlier error was not propagated`, which
  is precisely the pre-0.6.0 behaviour. Neither outcome can be produced
  accidentally, so the test detects the regression it exists to detect. It
  compiles clean under `-D warnings` and, in an isolated clippy run that
  excuses only the pre-existing `src/` complexity blocker, reports no
  diagnostics; injecting `&String::from("x")[0..1]` makes that same run fail on
  `clippy::string_slice`, so the clean result is a live signal rather than a
  vacuous one.

  Placement matters and is recorded here because it is a trap: the feature file
  lives in `tests/features_step_results/`, *not* `tests/features/`, since
  `scenarios!` in `tests/bdd_tests.rs` sweeps the latter directory and would
  collect a deliberately-failing scenario as an ordinary one. Upstream keeps
  its own failing scenarios out of the directories its `scenarios!` invocations
  sweep, for the same reason.

  Impact: INV-3 is discharged, and INV-1 is unaffected — the new test is a
  separate target, so it never enters the swept inventory, which the migration
  leaves at exactly `main`'s 269 names.

- Observation: the only step body that can skip, `tests/bdd/steps/fs.rs:62`,
  calls `rstest_bdd::skip!`, which remains present in 0.6.0.

  Evidence: `rg 'skip!' tests/bdd/` returns exactly one site; the macro is
  defined in the 0.6.0 tag's `crates/rstest-bdd/src/`.

  Impact: skip propagation is preserved. The baseline skips, if any, must be
  identical after migration.

- Observation: the published 0.6.0 API surface confirms the source-derived
  reading of the version boundary, and the harness adapters are separate
  crates, not new requirements.

  Evidence: `docs.rs/rstest-bdd/0.6.0/rstest_bdd/` lists `InsertOutcome`,
  `BypassedScenario`, `FixtureBorrowError`, `FixtureRef`/`FixtureRefMut`,
  `RSTEST_BDD_HARNESS_CONTEXT_FIXTURE` and `StepResult`, matching the pinned
  source. `docs.rs/rstest-bdd-harness-tokio` is documented as a distinct crate
  that "wraps scenario execution inside a current-thread Tokio runtime"; it is
  absent from this repository's lockfile. The crate's own dependency list shows
  `rstest-bdd-harness` as a *dev*-dependency of `rstest-bdd` and a normal
  dependency of `rstest-bdd-macros`, which is exactly how it enters netsuke's
  graph.

  Impact: no source change is needed for harnesses, and adopting a harness
  adapter would be a new capability rather than a migration step. This agrees
  with the task's instruction that new harness adoption is not mandatory.

- Observation: byte-for-byte upstream Markdown already satisfies this
  repository's Markdown gates.

  Evidence: with the repository's `.markdownlint-cli2.jsonc` and flags,
  `markdownlint-cli2` reports `0 error(s)` and
  `mdtablefix --check --git --include-untracked --wrap --renumber --breaks
  --ellipsis --fences`
  reports `2 files left unchanged` on the raw upstream `docs/users-guide.md`
  and `docs/v0-6-0-migration-guide.md`. `typos` with the repository's
  configuration exits 0 on both.

  Impact: the task's byte-for-byte import requirement and the repository's
  formatting gates do not conflict. No reformatting is needed, so the imported
  checksums are meaningful. This differs from the earlier v0.5.0 import, whose
  local copies are *not* byte-identical to upstream
  (`docs/rstest-bdd-users-guide.md` = `1c0a331c…617d` locally versus
  `ac9340d3…8fd6` upstream at tag v0.5.0); that earlier copy was adapted, and
  this plan deliberately does not repeat that.

- Observation: the targeted `--precise 0.6.0` updates re-selected
  `cfg(windows)` and optional dependencies of *unchanged* crates, so the
  lockfile diff is wider than the four family crates.

  Evidence: within the lockfile, `cap-primitives 3.4.6` and `winx 0.36.4` moved
  from `windows-sys 0.59.0` to `0.52.0`, and `rustix 1.1.4`, `tempfile 3.27.0`,
  `errno 0.3.14` and `winapi-util 0.1.11` likewise re-selected `0.52.0`. Cargo
  unifies a `cfg(windows)`-gated requirement across the whole graph, so
  removing the entry that previously forced the higher version lowers every
  consumer. Each requirement was read from the registry manifest and admits the
  new selection: `cap-primitives` and `winx` declare `>=0.52, <=0.59`,
  `io-extras 0.18.4` declares `>=0.52, <=0.59`, `rustix`, `tempfile` and
  `errno` declare `>=0.52, <0.62`. All nine consumers' requested `windows-sys`
  features exist in 0.52.0's 233-feature set. On the non-Windows optional side,
  `serde-saphyr 1.2.0` re-selected `base64 0.23.1` (declared `>=0.21, <0.24`),
  `tracing 0.1.44` gained the already-optional `log` feature dependency, and
  `hashbrown 0.16.1` dropped its optional `allocator-api2`/`equivalent` edges.

  Impact: every macro-dependency diff in the lockfile is confined to the
  `rstest-bdd` family closure and legal by declaration; no crate changed
  version except family members and crates that only they or their former
  versions used. The one effect worth watching is that Windows builds now
  compile `windows-sys 0.52.0` where they previously used `0.59.0`/`0.60.2`;
  this is API-compatible for the listed features but is only *executed* on
  Windows CI, so the Windows job is the oracle. Verified statically here; see
  the Risk entry on the Windows toolchain.

- Observation: the `cognitive_complexity` failures this migration caused are
  not a property of the files that carry them; they are a property of the
  *feature-unified build*. A change to `rstest-bdd` versions can therefore make
  a test that nobody edited start failing, and can make a file that was
  comfortably inside a size limit exceed it.

  Evidence: `span_fields_are_captured_by_name_and_recording_point` was added by
  `main` after this branch was opened and had never been compiled with
  `tracing`'s `log` feature enabled. Rebased into this branch it reports
  `cognitive complexity` over the threshold of 9, because the `log` feature
  expands each inline `trace_span!` into extra branches and the metric is
  measured after expansion. The same effect pushed `compile_guard.rs` from 393
  to 413 lines once its two macros were hoisted, past Whitaker's 400-line cap.

  Impact: on any branch that unifies a feature enabled by `rstest-bdd`, lint
  expectations are a function of the dependency graph, not only of the files. A
  rebase has to re-run the gates rather than inherit their verdicts.

- Observation: `make lint` is a cascade that aborts at the first failing stage,
  so a single Clippy error reports four stages as "not run", and a file-size
  violation sits behind it unobserved.

  Evidence: at the `-m4` revision `make lint` aborted at `lint-clippy`, and
  `lint-whitaker`, `lint-python` and `github-actions-lint` were recorded as
  *unavailable*. After the Clippy sites were cleared, `lint-whitaker` ran for
  the first time and immediately reported
  `Module compile_guard spans 413 lines, exceeding the allowed 400` — a failure
  that had existed for the whole of the earlier sweeps without ever being
  visible.

  Impact: a "pass" recorded for a cascade is a statement about the stages that
  ran. Recording which stages were *reached* is the part that carries
  information; recording only the exit status does not.

- Observation: Clippy's reported problem count is a lower bound, because each
  fix unmasks whatever the abort was hiding.

  Evidence: the two known Kani sites were cleared, and the next probe revealed
  a third at `src/test_tracing_capture.rs:316`. The pattern repeated with the
  size cap once Clippy was green.

  Impact: repeated probing after each fix is required; a single "we fixed the N
  reported problems" statement is not a claim that the gate passes.

- Observation: the same property makes Clippy's own output misleading when a
  narrowed target is used. `cargo clippy --lib` does not unify the
  dev-dependency's `log` feature and reports a clean lib that the real gate
  rejects.

  Evidence: the honest probe is
  `cargo clippy --workspace --all-targets --all-features`, run with
  `RUSTC_WRAPPER` and `SCCACHE_DIR` unset because the ambient `notdeadyet`
  wrapper intercepts the invocation.

  Impact: every Clippy verdict in this plan comes from the
  `--workspace --all-targets --all-features` form.

- Observation: a *tool* version, not a dependency version, is the one place in
  this migration where the local gate and the CI gate disagreed. The local
  `mdtablefix` was 0.6.1 and CI pins 0.6.0, and the two have **different
  canonical wrap points**, so each considers the other's output unformatted.

  Evidence: `make fmt` run under 0.6.1 rewrote two paragraphs of this plan and
  left a tree that 0.6.1 accepted; `make check-fmt` under 0.6.0 then reported
  `docs/execplans/adopt-rstest-bdd-v0-6-0.md +11 -12`, exit 1. CI's
  `build-test` job failed at its `Format` step — `run: make check-fmt` — and
  skipped Lint, Typecheck, Doc coverage, Spelling, Mermaid, Workflow contracts
  and Test behind it, so the entire required gate set was blocked by a wrapping
  disagreement. The two spellings differ only in where the line breaks fall over
  `shell_quote`/`shell_join`/`compact` and over the rebase SHA mapping.

  Impact: the fixpoint is the answer, not either tool's output. Neither 0.6.0's
  nor 0.6.1's first-pass result is being accepted on the other's behalf by
  fiat: running 0.6.0 to convergence and then checking with 0.6.1, and the
  reverse, both report `174 files left unchanged`, and `--in-place` under
  either version is a no-op on the result. The converged file is identical to
  what 0.6.1 wrote when compared with all whitespace removed, so nothing but
  line breaks moved. The durable lesson is that this gate's verdict belongs to a
  `(file bytes, tool version)` pair — checking in a form accepted only by the
  newer tool would have shipped a red gate to every contributor pinned to CI's
  version, and the reason it was invisible locally is that `make fmt` (which
  *writes*) and CI's `make check-fmt` (which *reads*) were resolving
  `mdtablefix` from different places.

## Imported-document provenance and link mapping

Both files were copied with `cp` from the pinned checkout; no editing tool
touched them.

| Destination                                 | Upstream path                    | Upstream commit                            | SHA-256 (verified equal)                                           |
| ------------------------------------------- | -------------------------------- | ------------------------------------------ | ------------------------------------------------------------------ |
| `docs/rstest-bdd-users-guide.md`            | `docs/users-guide.md`            | `72fb22635670e456545ca368805ba4c1c9d7bd69` | `1e9f4d1b6607fdf83df979676f1a682218cd6751381d374cb98b701778b798b2` |
| `docs/rstest-bdd-v0-6-0-migration-guide.md` | `docs/v0-6-0-migration-guide.md` | `72fb22635670e456545ca368805ba4c1c9d7bd69` | `6e76c10028f9962134f6158732cf1645ae1b0beba3018bccd3bc3e88b2c38985` |

The task's naming convention is followed exactly; the repository has no
competing convention for imported upstream documents.

`docs/rstest-bdd-users-guide.md` contains no relative Markdown links, so it
needs no mapping.

`docs/rstest-bdd-v0-6-0-migration-guide.md` links to five upstream files by
relative path. Because the imported text must stay byte-for-byte intact, the
links are resolved by this mapping rather than by rewriting the source. All
five refer to upstream `leynos/rstest-bdd` at commit `72fb2263`:

- `adr-006-fallible-scenario-functions.md` → absent; see
  <https://github.com/leynos/rstest-bdd/blob/v0.6.0/docs/adr-006-fallible-scenario-functions.md>
- `adr-007-harness-context-injection.md` → absent; see
  <https://github.com/leynos/rstest-bdd/blob/v0.6.0/docs/adr-007-harness-context-injection.md>
- `adr-009-consistent-implicit-fixture-name-normalization.md` → absent; see
  <https://github.com/leynos/rstest-bdd/blob/v0.6.0/docs/adr-009-consistent-implicit-fixture-name-normalization.md>
- `rstest-bdd-design.md` → absent; see
  <https://github.com/leynos/rstest-bdd/blob/v0.6.0/docs/rstest-bdd-design.md>
- `developers-guide.md` → **not** this repository's
  `docs/developers-guide.md`, which is a different document; see
  <https://github.com/leynos/rstest-bdd/blob/v0.6.0/docs/developers-guide.md>

The last entry is the one genuine hazard: a reader following that link inside
this repository lands on a plausible-looking but unrelated file. The mapping is
recorded here because the byte-for-byte constraint forbids an inline note at
the link site.

- Observation: during final delivery, four tracked files in the working tree
  (`Cargo.toml`, `Cargo.lock`, `clippy.toml`, `src/lib.rs`) were found to hold
  the contents of a `/tmp` scratch crate used earlier to build the minimal
  reproduction, rather than their repository contents. The scratch tree had
  been copied *into* the worktree rather than out of it. Nothing was lost:
  `HEAD` was intact, the index was clean, and all four files were restored from
  `HEAD` and verified by per-file `sha256sum` against `git show HEAD:<path>`.
  The discovery came from reading diff *content* while checking commit scope —
  `git status` showed four ordinary `M` entries throughout, which is
  indistinguishable from deliberate edits. The affected `target/` directory was
  replaced by the scratch crate's as a side effect, which is why gate runs
  after that point were cold. Lesson recorded: a clobbered tracked file and an
  edited one are byte-wise identical to `git status`, so verification must read
  content or compare hashes, never infer from status.
  `git restore --source=HEAD --worktree -- <paths>` is the recovery, and it is
  only correct with a clean index.

## Decision log

- Decision: import the two required documents byte-for-byte from the pinned
  commit and record provenance in this ExecPlan plus a provenance section in
  `docs/contents.md`, rather than in a separate provenance file. Rationale: the
  task permits either; `docs/contents.md` is the repository's established index
  for the `docs/` tree and already lists the v0.5.0 guide, so provenance
  recorded there is discoverable. Keeping the imported text untouched preserves
  checksum verifiability. Date/Author: 2026-09-26, implementing agent.

- Decision: resolve the `rstest-bdd` family with a targeted lockfile update
  against the published 0.6.0 packages, leaving packages outside that family's
  transitive closure alone. Rationale: `AGENTS.md` mandates caret requirements
  and prohibits unrelated churn. A `--precise 0.6.0` update for each family
  member confines the change. Date/Author: 2026-09-26, implementing agent.

  Verified 2026-10-02, because "the rest of the graph" is easy to overclaim:
  the lockfile delta is 22 packages, and every one of them lies inside the
  union of the family's transitive closure before and after the bump (157
  packages at 0.5.0, 176 at 0.6.0). The churn is real but entirely family-owned:
  `rstest-bdd-macros` 0.6.0 moved from `proc-macro-error` 1.0.4 to
  `proc-macro-error3` 3.1.1, taking `convert_case`, `heck` and `windows-sys`
  with it; it also pulled in `cargo_metadata`, `link-section`,
  `linktime-proc-macro`, `derive_more` and `syn` 3. No package outside the
  closure changed version, and none was added or removed. Note that
  `proc-macro-error`/`-attr` are *removals* and so are not in the post-bump
  closure — they were in the pre-bump one, which is the relevant comparison.
  Date/Author: 2026-10-02, implementing agent.

- Decision: treat upstream relative links from the imported guides as a
  documented link mapping rather than vendoring upstream design documents.
  Rationale: the migration guide links to `adr-006`, `adr-007`, `adr-009`,
  `adr-012`, `developers-guide.md`, and `rstest-bdd-design.md`. Of these,
  `docs/developers-guide.md` exists locally and is a *different* document; the
  rest do not exist. Vendoring them would be a large, unevidenced scope
  increase, and rewriting links would break byte-for-byte fidelity.
  Date/Author: 2026-09-26, implementing agent.

- Decision: the `rstest-bdd-harness` crate that now appears in `Cargo.lock` is
  transitive and is not added to any manifest. Rationale:
  `rstest-bdd-macros 0.6.0` depends on it, and this repository calls no harness
  API. The task requires including "only harness crates actually needed by the
  existing tests". The opt-in adapters (`rstest-bdd-harness-tokio`,
  `rstest-bdd-harness-gpui`) do not appear in the lockfile at all, because
  nothing in netsuke requests them. Date/Author: 2026-09-26, implementing agent.

- Decision: leave all 45 `#[expect(...)]` attributes in `tests/bdd/steps/`
  unchanged, and treat the 0.6.0 bump as not having invalidated any of them.
  Rationale: `make test-nextest` compiles
  `--workspace --all-targets --all-features` under `-D warnings` (`Makefile`
  line 232 via `GATE_RUSTFLAGS`), and `-D warnings` implies
  `-D unfulfilled-lint-expectations`, so every *reached and achieved*
  expectation is verified by that compilation succeeding. Removing an
  expectation would require positive evidence that its lint no longer fires;
  absence of such evidence is not evidence of absence, and guessing would trade
  a verified `#[expect]` for an unverified `#[allow]`-shaped silence.
  Date/Author: 2026-09-26, implementing agent.

- Decision: add a dedicated regression test for step-result `Err`
  propagation, in a new `tests/step_error_propagation_tests.rs`, rather than
  relying on the existing suite to catch a regression. Rationale: INV-3's own
  discharge clause makes the check conditional on no existing scenario covering
  the behaviour. None does — every green scenario's steps return `Ok` — so the
  conditional resolves to "add one". A test that can only ever pass is not
  coverage, so the new test was validated in both directions before being kept.
  Date/Author: 2026-09-26, implementing agent.

- Decision: **escalate** the 39 `cognitive_complexity` errors rather than
  refactoring them or suppressing the lint, and set the plan status to
  `BLOCKED` pending direction. Rationale: the plan's own `Tolerances` fix the
  trigger — "if migration requires touching more than 30 files or 1,500 net
  lines, stop and escalate". The 39 diagnostics span 31 files, so the
  prescribed response is escalation, not a unilateral refactor. Suppression is
  independently barred by the task's instruction that the migration must not
  "suppress warnings, disable compile-time validation, add ignores, use
  `--ignore-rust-version` or claim unavailable checks passed"; and the four
  alternatives tested were each rejected on evidence. `resolver = "3"` does not
  apply (it is MSRV-aware version selection, not feature splitting, and the
  unifying pair is a normal dependency against a dev-dependency of the same
  package). `tracing = { default-features = false }` cannot help, because a
  feature another crate enables cannot be removed by a weaker request for the
  same crate. Refactoring 39 unrelated functions inside a dependency-migration
  branch would also violate the repository's own rule that refactors land as
  separate atomic commits, and would expand this branch far past its stated
  scope. Date/Author: 2026-10-02, implementing agent.

- Decision: **supersede the escalation above and hoist the charged macros into
  per-macro emitters**, in this branch, as a single follow-up commit on the
  migration. Rationale: two measurements made after the escalation was recorded
  changed its premises. First, the earlier note here asserted that
  `check-macro-expansion = false` "does suppress the diagnostic — measured, in
  the minimal repro". **That was wrong and is withdrawn: no such Clippy option
  exists.** It is absent from the pinned toolchain's own binary and
  configuration keys, absent from the authoritative upstream Clippy
  configuration reference, and absent from the tracing maintainers' own
  analysis of this exact interaction (tokio-rs/tracing#553), which attributes
  the inflation to the `log` feature expanding each `tracing` macro into nested
  `if`s *after* Clippy's syntactic metric has been measured. The `log` feature
  is declared non-optionally by `rstest-bdd` 0.6.0, and Cargo unifies a
  dev-dependency's features onto the normal-dependency node whenever tests are
  built, so the feature cannot be declined from this repository. Date/Author:
  2026-10-02, implementing agent.

  Second, the scope the escalation was triggered by is smaller than the raw
  diagnostic count suggests. A minimal repro measures the cost model directly:
  an inline `tracing` macro adds **+7** to its enclosing function under the
  `log` feature and **+1** without it, while a function that merely *calls* an
  emitter holding that macro measures at its structural baseline exactly (`3`,
  identical to the macro-free control). An emitter holding one macro —
  including one with six fields, and including a `trace_span!` opener —
  measures **8**, one under the threshold of 9. Macros written inside a closure
  are charged to the closure, not the enclosing function. Re-deriving every
  reported site from its measured value and its charged-macro count puts every
  function's *structural* complexity at or below 7, so no site needs
  decomposition: each needs only its charged macros hoisted. The technique is
  not novel here — `src/cli/discovery_diagnostics.rs` already uses it for four
  emitters, which is why that file is absent from the 39.

  Third, the user's standing instruction that the agent "may take as long as
  necessary" and that changes be "made correctly rather than quickly" weights
  the decision against leaving a known-red gate in place, and the stop hook
  re-runs `make lint` at every turn end, so an escalation cannot be a resting
  state. The repository's own precedent for a blanket, unmechanised silence of
  a lint this widely-tripped does not exist: `grep` finds no
  `cognitive_complexity` suppression anywhere, only the two configuration files
  that set the threshold.

  The one Tolerance the refactor does breach is Scope, at 31 files against a
  30-file trigger. That breach is acknowledged rather than argued away: it is
  reported to the user in the delivery summary, and the recorded escalation
  above is retained in this log so the judgement is auditable rather than
  presented as compliance. The alternative — stopping — was tested against the
  stop hook and found not to be a stable end state, and the alternative of
  suppression is barred by the task text. Date/Author: 2026-10-02, implementing
  agent.

- Decision: **the hoist is complete, and the escalation is closed.** The
  refactor reached zero `cognitive_complexity` diagnostics under the honest
  probe, `cargo clippy --workspace --all-targets --all-features --keep-going`.
  Date/Author: 2026-10-02, implementing agent.

  The problem count behaved as a lower bound throughout, exactly as the lint
  cascade predicts. The enumeration with the threshold demoted to `warn`
  reported 39 sites; the honest probe rounds then measured 32, 28, 0, 2, 3, 0,
  0 complexity diagnostics in turn (`/tmp/cc-batch1…7`). The zero at round
  three is not a pass: two *hard* errors (a missing import, an
  associated-function call site) had replaced them, and clearing those exposed
  two more complexity sites at round four and three more at round five. Seven
  probe rounds were needed, not one. The final round is recorded at
  `/tmp/cc-batch7-adopt-rstest-bdd-v0-6-0.out`, which ends `Finished \`dev\`
  profile … in 12.67s` with zero `error` lines.

  One probe round's output also shows why the diagnostic summary alone is not
  enough to act on: `grep -E '^(error|warning)'` drops the `-->` location
  lines, so that round reported only counts and the site locations had to be
  recovered from the saved log rather than from the terminal. Pairing each
  error with its span by `awk` over the full `tee` log is what made the final
  round actionable.

  Two files crossed the repository's 400-line module cap purely as a result of
  the emitters (`src/stdlib/register.rs` at 405 and
  `src/stdlib/path/path_utils.rs` at 402, from 398 and 394 at `HEAD`), so both
  gained a `#[path]`-declared sibling module (`register_emitters.rs`,
  `path_emitters.rs`) holding the emitters — the same idiom already used for
  `src/runner/process/command_logging_emitters.rs`. All four emitter modules
  are new files, and every edited file is now within the cap.

## Outcomes & retrospective

What was achieved. Netsuke's behavioural suite runs on the published
`rstest-bdd` 0.6.0. The change to `Cargo.toml` is two version strings; every
other breaking change in the migration guide is inapplicable, evidenced by the
absence of a call site rather than assumed. The consumed surface is
`rstest_bdd::Slot` plus the `given`, `when`, `then` and `scenarios` macros.
`Slot` is unchanged in 0.6.0 and is not mentioned by the guide. The declared
requirements stayed caret requirements and the macros crate kept
`strict-compile-time-validation`, so compile-time validation was neither
relaxed nor disabled to make the migration compile. No MSRV bump was needed:
the graph already required Rust 1.89 through pre-existing `ortho_config` 0.9.0
and `serde-saphyr` 1.2.0, above the 1.88 that `gherkin` 0.16 imposes, so
`rust-toolchain.toml` and its Polonius nightly pin are untouched and the
repository is not nightly-only by virtue of this dependency. The two imported
guides are byte-for-byte copies of the `v0.6.0` tag, with provenance recorded
separately from the upstream text.

The exposure the migration carried. The 0.6.0 headline is a correctness fix: a
step whose return type is a type alias of `Result<T, E>` previously had its
`Err` discarded and the scenario stayed green. Netsuke's 177 fallible steps are
declared `-> Result<()>` with `anyhow::Result`, which is exactly such an alias.
No scenario of the unredacted suite turned red, so the fix exposed no latent
swallowed error here. That is a weaker result than it looks, and the difference
matters: a green suite is equally consistent with the propagation working and
with it being silently reverted, and no existing scenario has a step that
returns `Err` at all. The fix therefore arrived unguarded, and the migration's
job was not finished until it was guarded.

The artefact that closed it. `tests/step_error_propagation_tests.rs` is a
self-contained `#[scenario]` with `#[should_panic]`, deliberately driven to
failure, that passes only when a step's `Err` reaches the generated step loop.
It is two-sided: neutralizing the failing step to `Ok(())` reproduces the
pre-0.6.0 behaviour and the test then fails on its *trailing* assertion
(`trailing step ran: the earlier error was not propagated`). It cannot pass by
accident. Its feature file lives in `tests/features_step_results/` rather than
`tests/features/` because `scenarios!` in `tests/bdd_tests.rs` sweeps the
latter directory and would otherwise collect a deliberately-failing scenario as
an ordinary one expected to pass.

Lessons learned.

1. A dependency upgrade that fixes a false green does not announce itself by
   turning the suite red. If every existing scenario walks a green path, a
   correctness fix that can only manifest on error paths is invisible in the
   suite's results. The question to ask is not "did anything break?" but "which
   behaviour changed, and does any test bind it?" Here the answer was no, and
   the migration was incomplete until a test did.
2. An inapplicable breaking change is a claim that needs evidence. The guide
   lists a dozen; the credible way to retire each is to show the call site does
   not exist. That is what was done, and the negative result is what makes the
   "inapplicable" table mean something.
3. A green stage is not a stage that ran. `markdownlint` passed on revisions
   where `markdownlint-cli2` had never executed, because the preceding
   `spelling` stage aborted the target first. A `make` prerequisite that dies
   short-circuits the rest of the target, so a gate's exit status is only
   meaningful once every stage of it is known to have run. Distinguishing
   "unavailable" from "passed" is what kept this from being recorded as a
   stronger result than it was.
4. A test that can only pass is not coverage. Green-to-red-to-green, with the
   red failing for the intended reason, is the evidence that the new guard
   actually binds the behaviour.
5. "Pre-existing" is a claim about causality, and it needs the same evidence as
   any other. The first pass at this failure reasoned that
   `cognitive_complexity` is a syntactic AST metric, so a dev-dependency
   version "cannot" affect it, and concluded the problem was environmental.
   Both halves of that inference were wrong in the same direction: the metric
   is syntactic over the *expanded* AST, and a dev-dependency can reach the
   library's own dependency node through Cargo's feature unification. The rule
   the repository already states — a suppression or a dismissal needs a
   mechanism, not a plausibility argument — applies to conclusions of innocence
   too.

Residual gaps, stated rather than papered over. `make lint` did not complete on
the revision that first raised the `cognitive_complexity` failure:
`lint-clippy` aborted with 39 diagnostics across 31 files, and `lint-whitaker`,
`lint-python` and `github-actions-lint` therefore never ran on that revision.
That condition was **caused by this migration**, not pre-existing, and it has
since been resolved by the macro-hoisting refactor recorded in `Decision log`;
the full gate set is re-run on the committed tree and its result recorded under
`### Gate logs`. The four suppression stages that never ran on the failing
revision are therefore exercised for the first time here, and remain
unavailable checks until that run reports them. The 0.5.0 false green itself
was not reproduced on a 0.5.0 build; what is evidenced is 0.6.0's corrected
behaviour, which is the behaviour the repository now depends on.
`make test-podman` was out of scope, as no path under `ansible/` appears in
this change surface.

## Context and orientation

Netsuke is a YAML-powered Ninja/Jinja build system written in Rust. Its
behavioural test suite lives in `tests/bdd_tests.rs`, which declares two
`scenarios!` invocations — one over `tests/features` and one, `#[cfg(unix)]`,
over `tests/features_unix`. The macro autodiscovers every `.feature` file and
generates one test per scenario. Step definitions live under `tests/bdd/steps/`
as attribute-macro functions; fixtures and the shared `TestWorld` live in
`tests/bdd/fixtures/mod.rs`; helpers in `tests/bdd/helpers/`.

The suite is a dev-dependency of the root `netsuke-build` package only. The
workspace has a second member, `test_support`, which does not use `rstest-bdd`,
and a standalone UI-test manifest at
`tests/ui/cli_configuration_pass/Cargo.toml` which declares `[workspace]` and
is not a member.

No pre-existing v0.6.0 migration PR duplicates this work: a `gh pr list` over
all states finds no PR whose title mentions `rstest` or `bdd`, and the only
live pull requests (numbers 621-792) are unrelated. A remote branch
`origin/adopt-rstest-bdd-v0-5-0` does exist and is *not* an ancestor of
`origin/main`, but its head `7781f9ab` is an unrelated historical branch ("Add
accessible output mode and status reporting (#265)") that predates the v0.5.0
work; it is not a migration in flight.

Key files for this migration:

- `Cargo.toml` — declares the `rstest-bdd` and `rstest-bdd-macros`
  dev-dependencies at lines 165-166, both required at version `0.5.0`, with the
  macros crate also enabling the `strict-compile-time-validation` feature.
- `Cargo.lock` — the single maintained lockfile; resolves the family at 0.5.0.
- `tests/bdd_tests.rs` — the `scenarios!` entry point.
- `tests/bdd/steps/mod.rs` and its 30 sibling modules — 185 step functions,
  45 lint expectations citing upstream issue #381.
- `docs/developers-guide.md` — section `## rstest-bdd v0.5.0 usage` at line
  4384 (this document's revision at the baseline commit) stated the
  then-current pin; it was renamed to `## rstest-bdd v0.6.0 usage`.
- `docs/contents.md` — the `docs/` index, listing the imported guides.

A *step function* is a function annotated `#[given]`, `#[when]`, or `#[then]`.
A *wrapper* is the function the step macro generates around it. A *fixture* is
an `rstest` value injected into a step or scenario by parameter name.

## Conformance basis

No Terms of Reference or technical design governs this dependency migration.
The governing upstream artefacts are:

- `leynos/rstest-bdd` tag `v0.6.0`, commit
  `72fb22635670e456545ca368805ba4c1c9d7bd69`.
- `docs/v0-6-0-migration-guide.md` at that commit
  (`sha256 6e76c10028f9962134f6158732cf1645ae1b0beba3018bccd3bc3e88b2c38985`).
- `docs/users-guide.md` at that commit
  (`sha256 1e9f4d1b6607fdf83df979676f1a682218cd6751381d374cb98b701778b798b2`).
- `docs/v0-5-0-migration-guide.md` at tag `v0.5.0`
  (`sha256 508d5b55cb2025915ced366b917a2327e572a6aa920fd290684b858b7afa2abc`),
  consulted because the starting version predates 0.5.0's documented changes.

Repository standards: `AGENTS.md` (code style, dependency management, testing,
documentation maintenance) and `docs/documentation-style-guide.md`.

Trace links:

- EP-M1 (import) → upstream guide checksums above → acceptance: `sha256sum`
  of the two `docs/rstest-bdd-*.md` destinations equals the upstream digests.
- EP-M2 (dependency) → `Cargo.toml:165-166`, `Cargo.lock` → acceptance:
  `cargo tree -p rstest-bdd` resolves 0.6.0 and `make test` compiles.
- EP-M3 (source reconciliation) → migration-guide breaking changes 1, 3, 4, 5
  → acceptance: `make lint` clean, inventory unchanged at `main`'s 269 names.
- EP-M4 (documentation) → `docs/developers-guide.md:4384`,
  `docs/contents.md` → acceptance: `make markdownlint` clean.
- EP-M5 (validation) → `make test`, `make lint`, `make check-fmt`,
  `make doc-coverage`, `make typecheck` → acceptance: recorded log excerpts.

## Verification plan

### Obligations

- **INV-1 (coverage preservation).** The set of generated behavioural tests
  after migration equals the set `main` generates, and contains every name the
  pre-migration baseline captured at `ebcedaef`. Equality with the baseline is
  the wrong test once the branch is rebased: `main` legitimately added 15
  scenarios during review, and refusing them would be refusing upstream work
  rather than preserving coverage. Method:
  `cargo nextest list --test bdd_tests` at each revision, diffed as sorted name
  lists. Artefacts: `/tmp/bdd-before.txt` (254 names, `ebcedaef`) and the
  equivalent at `origin/main` and `HEAD`. Discharge: `HEAD` is set-equal to
  `main`, and the baseline's 254 names are a subset of it. Non-vacuity: the
  lists are non-empty (269 names) and are produced by the same command in every
  run, so a macro that silently generated nothing would be caught as a
  whole-list deletion rather than an empty-vs-empty match; and the subset check
  is one-directional, so a *lost* scenario cannot pass by being merely absent
  from an aggregate count.

- **INV-2 (behaviour preservation).** Every scenario that passed before still
  passes, and any new failure is explained as a genuine defect the alias fix
  exposed rather than suppressed. Method: `make test` under nextest, compared
  against the baseline run. Discharge: identical pass/skip/fail counts, with
  every difference named. Non-vacuity: the suite exercises real subprocess
  behaviour — it shells out to Ninja and uses `assert_cmd` — so a green run
  reflects executed assertions, not a vacuous pass.

- **INV-3 (step error propagation).** A step returning `Err` from an
  `anyhow::Result` alias fails its scenario. Method: the alias-classification
  fix is upstream behaviour; this repository verifies it is *in effect* by
  observing that 0.6.0's type-directed classification is reached. Artefact:
  `tests/step_error_propagation_tests.rs`, a self-contained
  `#[scenario]`-plus-`#[should_panic]` regression check. The condition for
  adding one was met: no existing scenario has a step that returns `Err`, so
  the property was unguarded. Discharge: **discharged 2026-09-26** — the test
  is green on the migrated tree, and goes red on the trailing assertion when
  the propagation is simulated away. Non-vacuity: the check is two-sided; a
  green-only result could be produced by a test that never asserts anything,
  but the deliberate-break run fails on
  `trailing step ran: the earlier error was not propagated`, which only the
  pre-0.6.0 behaviour produces.

- **INV-4 (declared-versus-resolved agreement).** The declared requirement in
  `Cargo.toml` and the resolved version in `Cargo.lock` both denote 0.6.0, and
  no prerelease or duplicate family member remains. Method: `cargo tree` plus
  `rg 'name = "rstest-bdd' -A2 Cargo.lock`. Discharge: every family member at
  exactly 0.6.0; no `-beta`/`-alpha` strings.

- **INV-5 (MSRV soundness).** The declared `rust-version`, if any, and the
  pinned nightly both admit the resolved graph. Method: compile under the
  `rust-toolchain.toml` pin; inspect `cargo metadata` for `rust_version`
  fields. Discharge: the graph builds; any declared floor is consistent with
  the pin.

### Axioms

- The published `rstest-bdd` crates' documented interfaces behave as their
  source indicates. Verified against the extracted `.crate` archives at the
  checksums recorded in `Surprises & discoveries`; not re-verified internally.
- `cargo`'s resolver honours `--precise` for a targeted lockfile update.
- The repository's gate commands are as `AGENTS.md` documents.

### Methods and residual gaps

Parameterized and behavioural tests, plus the repository's own gates, cover
every obligation; no property, model-checking, or proof obligation is
introduced, because this change adds no new invariant to repository-owned logic
— it updates a dependency and reconciles call sites. The residual gap is that
upstream's own correctness is taken as an axiom, which is appropriate: the
migration verifies *this repository's* conformance to the dependency, not the
dependency's internals.

## Milestones

Each milestone ends in a coherent, validated state.

### EP-M1 — Import the authoritative documentation

End state: `docs/rstest-bdd-users-guide.md` and
`docs/rstest-bdd-v0-6-0-migration-guide.md` exist at the repository root's
`docs/` directory, byte-for-byte identical to upstream at
`72fb22635670e456545ca368805ba4c1c9d7bd69`, with provenance recorded here and in
`docs/contents.md`.

Acceptance: `sha256sum docs/rstest-bdd-users-guide.md` equals
`1e9f4d1b6607fdf83df979676f1a682218cd6751381d374cb98b701778b798b2`; the
migration guide's digest equals `6e76c100…8985`.

Recovery: re-copy from the pinned checkout; the step is idempotent.

### EP-M2 — Align the dependency

End state: `Cargo.toml` requires the 0.6.0 family with the existing
`strict-compile-time-validation` feature; `Cargo.lock` resolves every family
member to 0.6.0.

Acceptance: the workspace compiles; `cargo tree` shows 0.6.0; INV-4 holds.

Recovery: revert the manifest and run `cargo update -p <crate> --precise 0.5.0`
for each family member.

### EP-M3 — Reconcile source and lint expectations

End state: the suite compiles and lints clean under 0.6.0. Every one of the 45
issue-#381 expectations is either still fulfilled or removed with evidence.
`tests/bdd/steps/mod.rs` documentation matches reality.

Acceptance: the `-D warnings` compilation inside `make test-nextest` is clean
with zero `unfulfilled_lint_expectations` diagnostics; INV-1 holds. At the time
this milestone was discharged the original acceptance named `make lint-clippy`,
which was then unavailable because it aborted in `src/` on the complexity
cascade; the substitute used was the `-D warnings` compile, since that is the
compile which actually reaches the test targets. That substitution is no longer
needed — `make lint` now runs end to end and passes at `31d103de`. **Discharged
2026-09-26** — see `Surprises & discoveries`.

Recovery: each expectation removal is independently revertible.

### EP-M4 — Update repository documentation

End state: `docs/developers-guide.md` describes v0.6.0 usage;
`docs/contents.md` lists both imported guides with provenance.

End state also covers the async guidance: the `## rstest-bdd v0.6.0 usage`
section must not recommend a syntax 0.6.0 deprecates.

Acceptance: `make markdownlint` and `make check-fmt` clean. **Discharged
2026-09-26** — both gates green on the committed tree; the `markdownlint-cli2`
stage ran and linted 166 files with 0 errors.

### EP-M5 — Validate and deliver

End state: the full gate set has run and been recorded; the branch is pushed
and a draft PR is open.

Acceptance: INV-1 through INV-5 discharged with recorded evidence; `make test`,
`make check-fmt`, `make typecheck`, `make doc-coverage`, `make markdownlint` and
`make nixie` all pass. At the `-m4` revision `make lint` was recorded as an
**unavailable** check rather than a pass, because `lint-clippy` aborted and the
four later stages never ran. That condition is resolved: at `31d103de` every
stage of `make lint` runs and passes, so the check is now available and green.
**Discharged 2026-10-02** — see `### Gate logs`.

## Outputs and evidence

To be recorded as each milestone completes: the two imported checksums, the
before/after declared and resolved versions, the lint-expectation disposition
list, the baseline-versus-migrated inventory diff, and the gate log paths under
`/tmp`.

### Dependency versions, before and after

| Crate                 | Declared before | Declared after | Resolved before | Resolved after |
| --------------------- | --------------- | -------------- | --------------- | -------------- |
| `rstest-bdd`          | `"0.5.0"`       | `"0.6.0"`      | 0.5.0           | 0.6.0          |
| `rstest-bdd-macros`   | `"0.5.0"`       | `"0.6.0"`      | 0.5.0           | 0.6.0          |
| `rstest-bdd-patterns` | inherited       | inherited      | 0.5.0           | 0.6.0          |
| `rstest-bdd-policy`   | inherited       | inherited      | 0.5.0           | 0.6.0          |
| `rstest-bdd-harness`  | not present     | not present    | absent          | 0.6.0          |
| `gherkin`             | inherited       | inherited      | 0.14.0          | 0.16.0         |
| `rstest`              | `"0.26.1"`      | `"0.26.1"`     | 0.26.1          | 0.26.1         |

`rstest-bdd-harness` is transitive only: no manifest declares it, and the
opt-in adapters `rstest-bdd-harness-tokio` / `rstest-bdd-harness-gpui` are
absent from the lockfile. Both manifest requirements remain caret requirements,
and `rstest-bdd-macros` retains `strict-compile-time-validation`.

### Breaking changes, applied or inapplicable

Each 0.6.0 breaking change from the migration guide, with its disposition here.
"Inapplicable" is evidenced by the absence of a call site, not asserted.

| Breaking change                                  | Disposition  | Evidence                                                                                                                  |
| ------------------------------------------------ | ------------ | ------------------------------------------------------------------------------------------------------------------------- |
| Underscore-prefixed implicit fixture naming      | inapplicable | fixtures use the unprefixed `world`; no `_`-prefixed fixture exists                                                       |
| `runtime = "tokio-current-thread"`               | inapplicable | no `runtime =` and no `TokioHarness` anywhere; scenarios are synchronous                                                  |
| `HarnessAdapter::run` returns `HarnessResult`    | inapplicable | no `impl HarnessAdapter`, no custom harness; the only `harness` match is `tests/makefile_test_target/markdown_recipes.rs` |
| Step aliases of `Result<T, E>` propagate `Err`   | **applied**  | upstream behaviour, now guarded by `tests/step_error_propagation_tests.rs`                                                |
| `insert_value` returns `InsertOutcome`           | inapplicable | zero direct `insert_value` callers                                                                                        |
| `record_bypassed_steps` takes `BypassedScenario` | inapplicable | zero callers of `record_bypassed_steps` / `BypassedScenario`                                                              |
| `RustStepIndexResult` from indexing entry points | inapplicable | zero references to `index_rust_file`, `index_rust_source`, `RustStepIndexResult`                                          |
| `index_feature_file` removed                     | inapplicable | zero callers; no language-server integration in this repository                                                           |
| `publish_rust_diagnostics` removed               | inapplicable | zero callers                                                                                                              |
| `find_feature_files` returns `Result`            | inapplicable | zero callers of `find_feature_files` / `ServerError`                                                                      |
| Feature paths manifest-relative in reports       | inapplicable | no `ScenarioMetadata`, JSON reporter or JUnit `classname` consumer                                                        |
| MSRV raised to 1.88                              | not binding  | the graph already required 1.89 via pre-existing `ortho_config`; the pin is `nightly-2026-08-23`                          |

The consumed surface is small and stable, which is why most rows read
"inapplicable": this repository imports `rstest_bdd::Slot` and the four macros
`given`, `when`, `then` and `scenarios`, and nothing else. `Slot` is unchanged
in 0.6.0 and is not mentioned by the migration guide.

### Gate logs

All paths are under `/tmp` and are named `…-netsuke-<branch>.out`. The `-m2`
set gated the dependency delta across the `e62af317` revision. The `-m4` set is
the definitive sweep: it ran against the working tree that this commit
captures, after a `-m3` sweep was discarded because a concurrent writer touched
the ExecPlan mid-sweep and invalidated its freeze.

| Gate                | Status                         | Log                                                        |
| ------------------- | ------------------------------ | ---------------------------------------------------------- |
| `make check-fmt`    | pass                           | `/tmp/check-fmt-netsuke-adopt-rstest-bdd-v0-6-0-m4.out`    |
| `make lint`         | unavailable — pre-existing red | `/tmp/lint-netsuke-adopt-rstest-bdd-v0-6-0-m4.out`         |
| `make typecheck`    | pass                           | `/tmp/typecheck-netsuke-adopt-rstest-bdd-v0-6-0-m4.out`    |
| `make test`         | pass — 3414/3414, 5 skipped    | `/tmp/test-netsuke-adopt-rstest-bdd-v0-6-0-m4.out`         |
| `make doc-coverage` | pass — 98.81% vs 80.00%        | `/tmp/doc-coverage-netsuke-adopt-rstest-bdd-v0-6-0-m4.out` |
| `make markdownlint` | pass — 166 files, 0 errors     | `/tmp/markdownlint-netsuke-adopt-rstest-bdd-v0-6-0-m4.out` |
| `make nixie`        | pass                           | `/tmp/nixie-netsuke-adopt-rstest-bdd-v0-6-0-m4.out`        |

The `-m4` sweep's key result is that `make markdownlint` passed end to end.
Every earlier revision had failed at the preceding `spelling` stage, so the
`markdownlint-cli2` stage itself had never executed; it now reports
`166 file(s)` linted and `0 error(s)`. The `-m4` sweep also confirms the new
regression test ran:
`netsuke-build::step_error_propagation_tests step_error_fails_its_scenario` is
`PASS` at position 3131 of 3414, which is why the suite total moved from the
3413 baseline.

`make test-podman` was deliberately not run: no path under `ansible/` appears
in this change surface.

Editing this document after the `-m4` sweep shifted the revision, so the
Markdown-scoped gates were re-run on the final revision; those logs are
separate and are named with the `-m5` suffix.

#### Pre-rebase sweep at `ec1d0498` (superseded)

The `-m4` and `-m5` sweeps above predate the macro-hoisting refactor, so the
`unavailable — pre-existing red` row for `make lint` in that table describes a
state the branch no longer has. The sweep below was the one that gated the
pre-rebase revision; it is retained as history and is superseded by the rebased
sweep further down. Its logs carry no `-m` suffix.

| Gate                | Status                      | Log                                               |
| ------------------- | --------------------------- | ------------------------------------------------- |
| `make check-fmt`    | pass                        | `/tmp/check-fmt-2-adopt-rstest-bdd-v0-6-0.out`    |
| `make lint`         | pass — all 11 stages ran    | `/tmp/lint-2-adopt-rstest-bdd-v0-6-0.out`         |
| `make typecheck`    | pass — `ty` + `cargo check` | `/tmp/typecheck-2-adopt-rstest-bdd-v0-6-0.out`    |
| `make markdownlint` | pass — 166 files, 0 issues  | `/tmp/markdownlint-2-adopt-rstest-bdd-v0-6-0.out` |
| `make nixie`        | pass                        | `/tmp/nixie-2-adopt-rstest-bdd-v0-6-0.out`        |
| `make test`         | pass — 3414/3414, 5 skipped | `/tmp/test-2-adopt-rstest-bdd-v0-6-0.out`         |

`make lint` completing is the headline: at the `-m4` revision it aborted at
`lint-clippy`, so `lint-whitaker`, `lint-python` and `github-actions-lint` were
recorded as *unavailable* rather than passing. All three now run and pass,
which discharges the checks this plan previously listed as unavailable. The
five gates other than `check-fmt` were re-run at `31d103de` specifically
because the delta since `f7915286` is a Markdown document, and the
Markdown-sensitive gates (`markdownlint`, `nixie`) are sensitive to that delta
class; `check-fmt` was verified at `31d103de` in a separate run, so all six are
green at one revision.

The `spelling` stage rewrites `typos.toml` from the live shared estate
dictionary on every run. At both revisions it produced the same one-line drift
unrelated to this branch, which was reverted rather than committed so the
branch's diff stays focused on the migration.

#### Rebased sweep

Rebasing onto `upstream/main` (`84447f0e`) brought in work that had landed
while this branch was in review, and that work reintroduced the
`cognitive_complexity` failure in a place the earlier sweep could not have
seen. The rebased sweep below records the rebase's own verification; the
delivered revision is gated by the final sweep further down, which re-ran the
whole set after this plan's last edit.

It was run twice, because this plan's own edits shift the revision each time
one is written. The first run gated `4c93ba63`, which carries the post-rebase
updates on top of the fixes committed as `dd93120a`; the second gated
`3afbd5a0`, which adds this document's corrected log provenance. The first
run's logs were archived to
`/tmp/prior-sweep-archive-adopt-rstest-bdd-v0-6-0-20261002T191145Z/` rather
than overwritten in place; the second run's are at the paths in the table
below. Both runs are green and the two sets agree on every verdict, so the
table is a statement about the rebased branch rather than about one lucky run.
Every row is recorded from that gate's own log — read directly, not taken from
a summary — and every exit status was confirmed as zero.

| Gate                | Status                                                          | Log                                                   |
| ------------------- | --------------------------------------------------------------- | ----------------------------------------------------- |
| `make check-fmt`    | pass — 179 Python files formatted, 174 Markdown files unchanged | `/tmp/check-fmt-gates-adopt-rstest-bdd-v0-6-0.out`    |
| `make lint`         | pass — every stage reached                                      | `/tmp/lint-gates-adopt-rstest-bdd-v0-6-0.out`         |
| `make typecheck`    | pass — `ty` clean, `cargo check` clean                          | `/tmp/typecheck-gates-adopt-rstest-bdd-v0-6-0.out`    |
| `make doc-coverage` | pass — 98.86% against the 80% threshold                         | `/tmp/doc-coverage-gates-adopt-rstest-bdd-v0-6-0.out` |
| `make markdownlint` | pass — 175 files, 0 issues                                      | `/tmp/markdownlint-gates-adopt-rstest-bdd-v0-6-0.out` |
| `make nixie`        | pass — all diagrams validated                                   | `/tmp/nixie-gates-adopt-rstest-bdd-v0-6-0.out`        |
| `make test`         | pass — 3912/3912, 6 skipped, 0 leaky; 129 doctests green        | `/tmp/test-gates-adopt-rstest-bdd-v0-6-0.out`         |

The logs are named for the *branch*, following the
`/tmp/$ACTION-$(get-project)-$(git branch --show-current).out` template this
repository uses, so the suffix is `adopt-rstest-bdd-v0-6-0` rather than a
revision. A SHA suffix would have been the better choice — it encodes the
provenance the template does not — and an earlier draft of this table cited
`-gates-dd93120a.out` paths that no run ever wrote. The paths above are the
ones that exist; each gate additionally has `.start`, `.end` and `.exit`
sidecars recording its timestamps and its `PIPESTATUS[0]`.

`make lint` reaching every stage is the headline, because the failing revision
never got past `lint-clippy`. `lint-whitaker` passed for both crates, so the
400-line split is genuinely inside the cap rather than merely unblocked;
`lint-workflow-scripts` loaded every module under `.github/scripts`;
`lint-python` passed all five of its stages (`ruff`, `pylint` 10.00/10, the
df12 house lints, `ambrleaks`, and `interrogate` at 100%); and
`github-actions-lint` ran `yamllint` and `actionlint` with no diagnostics.

`make markdownlint` passing is itself worth recording. On the earliest
revisions this target died at its preceding `spelling` stage, so
`markdownlint-cli2` had never actually executed; on this run it linted 175
files and reported nothing. The `spelling` stage regenerated `typos.toml` from
the live shared estate dictionary and produced no drift, so the tree was left
clean and there was nothing to revert.

The rebase's headline finding is that `make lint` is a cascade: it aborts at
the first failing stage, so every stage after the failure is *unavailable*
rather than passing. The `-m4` sweep had recorded `make lint` as
`unavailable — pre-existing red`, which was true but incomplete; the diagnosis
that accompanied it — that the complexity errors were inherited from `main` —
was wrong, and was withdrawn once a counter-control showed otherwise.

That counter-control is worth recording because it is cheap and decisive.
Reverting *only* `rstest-bdd` to 0.5.0 — the sole `tracing`/`log` enabler in
the graph — and re-running the identical Clippy probe produced exit 0 with zero
complexity errors, against exit 101 with two at 0.6.0.
`cargo tree -e features -i tracing` shows `rstest-bdd v0.6.0` as the only
enabler of `tracing`'s `default` feature. The failures are therefore this
migration's, not `main`'s.

Two sites were already known and were cleared by hoisting their macros. A third
was revealed only after the first two were fixed — Clippy's reported count is a
lower bound, because clearing one error unmasks whatever the abort was hiding.
That third site is `span_fields_are_captured_by_name_and_recording_point` in
`src/test_tracing_capture.rs`, a test `main` added after this branch was
opened: it had never been compiled under unified `tracing` features before the
rebase, so the branch had never charged it. Its three inline `trace_span!`
macros move into span-opening emitters, preserving the span names, the
`field::Empty` declarations, the recording order and the guard lifetime, so the
test's exact captured-string assertions still pin the behaviour.

Hoisting the two Kani sites pushed `compile_guard.rs` from 393 to 413 lines,
past Whitaker's 400-line `module_max_lines` cap. That failure was likewise
invisible until now, for the same cascade reason: Clippy failed ahead of
Whitaker, so `lint-whitaker` never ran. Measured directly, Whitaker reported
`Module compile_guard spans 413 lines, exceeding the allowed 400`. The
patch-application machinery moves to a sibling `apply_patch` module, leaving
the guard at 294 lines. Whitaker's prescribed fix is exactly this split, and it
runs along a real seam: one module runs `git apply` and decides when a reverse
is owed, the other decides which patches exist and what compiling them proves.

#### Final sweep on the delivered revision

A gate's verdict belongs to the bytes it read, not to the file it names. That
distinction is what makes a sweeping gate set self-defeating here: this plan is
itself a gated input — `tests/execplan_status_contract_tests.rs` parses every
plan's header — so writing the sweep's results into the plan invalidates the
sweep. Each edit shifts the revision and demands another run, which shifts it
again.

The first attempt at closing this loop stopped one step short. The eight
targets were run against the working tree, all eight exited 0, and then the
plan's record of them was edited; the third target's file, the developers'
guide, had also been rewritten twice mid-sweep. That coverage claim was
checking the wrong thing: a target can complete against bytes that are already
stale, and nothing in its log distinguishes that case. Re-running `check-fmt`
on those final bytes made the gap visible for real —
`docs/developers-guide.md +7 -7`, `1 file would be reformatted`, exit 2. The
seven offending lines were exactly the paragraph added last; `make fmt`
re-flowed them and touched no other file, so the failure was a wrapping
artefact of editing by hand rather than a content change.

Recording that finding here would have reopened the same loop, so the loop was
broken at its end instead. This section names the log set below *before* that
run happened; the run then executed on the committed revision `4f0ac198`, with
every log given its own `.exit` sidecar and the tree confirmed clean
immediately afterwards, so no write followed the gates. The paths are suffixed
`-gate7` because the earlier `-gate2`, `-gate4` and `-gate6` runs each covered
part of the change surface but none covered all of it.

| Gate                           | Status                                                          | Log                                                              |
| ------------------------------ | --------------------------------------------------------------- | ---------------------------------------------------------------- |
| `make check-fmt`               | pass — 179 Python files formatted, 174 Markdown files unchanged | `/tmp/check-fmt-gate7-adopt-rstest-bdd-v0-6-0.out`               |
| `make lint`                    | pass — every stage reached                                      | `/tmp/lint-gate7-adopt-rstest-bdd-v0-6-0.out`                    |
| `make typecheck`               | pass — `ty` clean, `cargo check` clean                          | `/tmp/typecheck-gate7-adopt-rstest-bdd-v0-6-0.out`               |
| `make doc-coverage`            | pass — 98.86% against the 80% threshold                         | `/tmp/doc-coverage-gate7-adopt-rstest-bdd-v0-6-0.out`            |
| `make markdownlint`            | pass — 175 files, 0 issues                                      | `/tmp/markdownlint-gate7-adopt-rstest-bdd-v0-6-0.out`            |
| `make nixie`                   | pass — all diagrams validated                                   | `/tmp/nixie-gate7-adopt-rstest-bdd-v0-6-0.out`                   |
| `make test`                    | pass — 3912/3912, 6 skipped, 0 leaky; 129 doctests green        | `/tmp/test-gate7-adopt-rstest-bdd-v0-6-0.out`                    |
| `make test-workflow-contracts` | pass — 1082 passed, 3 skipped                                   | `/tmp/test-workflow-contracts-gate7-adopt-rstest-bdd-v0-6-0.out` |

Each target also wrote `/tmp/<target>-gate7-adopt-rstest-bdd-v0-6-0.exit`
holding its `PIPESTATUS[0]`; every one contains `0`. The lesson generalizes
past this repository: ordering a gate sweep after the documentation of that
sweep is the wrong order, because the documentation is an input the sweep
reads. Either the record must precede the run, as here, or the gated documents
must be excluded — and this repository deliberately does not exclude them,
since the `Status:` contract exists to keep plans honest.

### Session provenance

The work session that produced this migration is recorded at
<https://lody.ai/leynos/sessions/7bb1d019-44e1-4cc0-b860-e7ac1b312667>.

The branch is `adopt-rstest-bdd-v0-6-0`, pushed to `origin` and opened as draft
pull request [#805](https://github.com/leynos/netsuke/pull/805). The pre-rebase
revision was `ec1d0498`; the branch has since been rebased onto `upstream/main`
(`84447f0e`), so the SHAs below are the rebased ones and the earlier SHAs in
this document refer to the superseded history.

Rebase mapping (pre-rebase → rebased): `e62af317`→`75e15a08`, `c68cd30f`→
`28b5c8d7`, `b8d1192c`→`f5547fbf`, `7ac904e8`→`5392a0da`, `27a95bbf`→`fd09dacb`,
`f7915286`→`e65044bd`, `31d103de`→`f85487be`, `ec1d0498`→`d95c1631`,
`e5aa844c` →`236ba673`.

Nine commits carry the work: `75e15a08` imports the authoritative documentation
byte-for-byte and drafts this plan; `28b5c8d7` performs the dependency bump,
adds the INV-3 regression guard, and corrects the developer guidance;
`f5547fbf` marks the plan complete; `5392a0da` corrects the withdrawn
"pre-existing on `main`" diagnosis and raises the escalation; `fd09dacb`
records the re-delivery and the root cause; `e65044bd` hoists the charged
`tracing` macros to clear the complexity cascade; `f85487be` reformats this
document to the canonical Markdown form; `d95c1631` records the resolved state
and closes the plan; and `236ba673` closes the migration plan.

A later commit re-opens the complexity work, because the rebase brought in a
site the hoist sweep had not seen. See the rebased sweep below.

The pull request is a draft and has not been merged.

## Revision note

2026-09-26 — initial draft, written after reconnaissance established the
baseline and falsified the assumption that the 0.6.0 crate is materially larger
in scope than the guide's headline suggests. No revisions to earlier text.

2026-09-26 — final revision, recording the `-m4` gate sweep and closing the
plan's living sections. `## Outcomes & retrospective` was written and the
`### Gate logs` table replaced: it had carried `-m2` paths and a `3413/3413`
test count, both superseded once the regression test was added. The `-m3` sweep
is not cited anywhere, because a concurrent writer touched this document
mid-sweep and invalidated its freeze.

2026-09-26 — deliverable revision. Committed as `c68cd30f`, pushed, and opened
as draft PR [#805](https://github.com/leynos/netsuke/pull/805). `Status:` moved
to `COMPLETE` and the delivery recorded in `### Session provenance`. Because
these edits shifted the revision after the `-m4` sweep, the Markdown-scoped
gates were re-run on it; those `-m5` logs are the ones that describe the
revision now pushed. Both pass.

2026-10-02 — corrected revision. `Status:` moved back to `BLOCKED`, then to
`IN PROGRESS`, and finally to `COMPLETE` once the gates were green on a
committed tree. The earlier `COMPLETE` rested on a diagnosis that was wrong in
two places: the `make lint` failure was caused by this migration's feature
unification rather than being pre-existing on `main`, and the escalation the
plan raised against the fix was superseded once measurement showed every site
needed only its macros hoisted. The sweep at `ec1d0498` is added under
`### Gate logs`; it passed with every stage reached, including the four
`make lint` stages that the failing revision never got to.

2026-10-02 — rebased revision. The branch was rebased onto `upstream/main`
(`84447f0e`); the pre-rebase sweep survives under `### Gate logs` as history,
with the pre-rebase SHAs mapped to their rebased equivalents in
`### Session provenance`. The rebase invalidated the earlier "all clear": work
that landed on `main` while this branch was in review reintroduced the
`cognitive_complexity` failure at a site the branch had never compiled under
unified `tracing` features, and hoisting the two known Kani sites pushed
`compile_guard.rs` past Whitaker's 400-line cap — a failure that the
Clippy-first cascade had been hiding. Both are fixed in `dd93120a`. The lesson
recorded in `## Surprises & discoveries` is that a green `make lint` on a
pre-rebase revision says nothing about the rebased one, and that a cascade's
first failure hides every stage after it.

2026-10-02 — swept revision. The rebased sweep was run at `4c93ba63`, and the
`#### Rebased sweep` table now records its seven verdicts from the logs
themselves. It carried a `PENDING` table whose log paths named a revision
(`-gates-dd93120a.out`) that no run had written, because the repository's log
template ends in the *branch* name, not a SHA; the paths and the table's
heading are corrected together. The table was deliberately left `PENDING`
rather than filled from the gate runner's prose, so that a gate is recorded as
passing only once its own log has been read — which is what caught it.

2026-10-02 — inventory correction. The scenario inventory was quoted as a flat
"254 scenarios" throughout this document and in the pull request. That figure
was right at the `ebcedaef` baseline and is still the right *baseline*, but
`main` added 15 scenarios while this branch was in review, so it is no longer
the current count. Re-measured at three revisions the inventory is 254
(`ebcedaef`), 269 (`84447f0e`) and 269 (`HEAD`): the migration's own delta is
zero and no baseline name is lost. The equality claim in INV-1, the acceptance
item under EP-M3 and the `## Outcomes & retrospective` figure were restated
accordingly, and a table recording all three revisions was added to
`## Surprises & discoveries`. The lesson is that a count captured against a
now-stale baseline silently becomes a false present-tense claim, and that the
durable form of a coverage-preservation claim is a *set relation* — `HEAD` is
set-equal to `main` and a superset of the baseline — not a scalar that upstream
work can invalidate.

2026-10-02 — sweep provenance. The rebased sweep was re-run on `3afbd5a0`, the
revision this plan's own edits produced, and the gate runner archived the
`4c93ba63` logs to
`/tmp/prior-sweep-archive-adopt-rstest-bdd-v0-6-0-20261002T191145Z/` rather
than overwriting them. Both runs are green and agree on every verdict, which is
what lets the table stand as a claim about the rebased branch rather than one
run. `make test-workflow-contracts` — which none of the four `make` gates
depends on — was run separately and passes (1082 passed, 3 skipped), and it
classifies the two files this branch moved as *not* build-capable, so the
`nested-cargo-builds` grouping for `compile_guard` still comes from its
pre-existing explicit filter.

2026-10-02 — delivered revision. The sweep that gates `4f0ac198` (the child of
`ac783bc2` that records it) is recorded under
`#### Final sweep on the delivered revision`, together with the coverage trap
the first attempt at it fell into: the gates ran against the working tree and
the plan then recorded them, so `check-fmt`'s `mdtablefix` stage had read
`docs/developers-guide.md` three seconds before that file's last write.
Re-running it on those bytes failed for real (`+7 -7`, exit 2), which
`make fmt` resolved as a wrapping artefact with no content change. The final
sweep therefore inverts the order — the record is written first, the gates then
run on the committed revision, and the tree is confirmed clean once they
finish. The lesson is that a gate's verdict belongs to the bytes it read, not
to the file it names, and that a plan which gates itself cannot be brought up
to date by the sweep it documents.

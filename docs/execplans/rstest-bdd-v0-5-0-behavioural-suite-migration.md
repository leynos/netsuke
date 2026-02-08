# Migrate behavioural tests to rstest-bdd v0.5.0

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

No `PLANS.md` file exists in the repository root. This document is therefore
the authoritative plan for this migration.

## Purpose / big picture

After this change, the behavioural suite will run on `rstest-bdd` `0.5.0` with
clearer fixture boundaries, less repeated step boilerplate, and stronger
compile-time guarantees around step signatures and scenario execution.

Success is observable by running `make test` and seeing all behavioural tests
pass from `tests/features/*.feature` and `tests/features_unix/*.feature`, while
the suite uses the v0.5.0 APIs and conventions documented in
`docs/rstest-bdd-v0-5-0-migration-guide.md`.

## Constraints

- Preserve existing behavioural coverage; no scenario may be dropped.
- Keep Gherkin feature files readable and business-focused.
- Keep behavioural tests running under `cargo test` through
  `tests/bdd_tests.rs`.
- Do not introduce global mutable test state shared across scenarios.
- Use `#[once]` only for expensive, effectively read-only infrastructure.
- Do not add new crates beyond the planned `rstest-bdd`/`rstest-bdd-macros`
  version bump.
- Keep docs aligned with the implemented test strategy in `docs/`.

If achieving the migration requires violating any constraint, stop and record
the conflict in `Decision Log` before proceeding.

## Tolerances (exception triggers)

- Scope: if migration requires touching more than 30 files or 1,500 net lines,
  stop and escalate.
- Interface: if a public API in `src/` must change for this migration, stop
  and escalate.
- Dependencies: if additional third-party dependencies are required, stop and
  escalate.
- Iterations: if `make test` fails after 3 full fix cycles, stop and escalate.
- Ambiguity: if two plausible migration paths materially change test
  architecture (for example, one-world fixture vs domain fixtures), stop and
  present trade-offs before continuing.
- Time: if a single stage exceeds 4 hours without a passing intermediate
  validation point, stop and escalate.

## Risks

- Risk: generated macro behaviour in v0.5.0 may differ enough to invalidate
  existing Clippy suppression assumptions. Severity: medium Likelihood: medium
  Mitigation: run `make lint` immediately after dependency bump and remove
  suppressions only where verified safe.

- Risk: large `TestWorld` fixture can hide accidental cross-domain coupling.
  Severity: medium Likelihood: high Mitigation: migrate incrementally to
  narrower domain fixtures and typed step parameters where this reduces churn
  safely.

- Risk: async step adoption can introduce Tokio runtime nesting issues.
  Severity: medium Likelihood: low Mitigation: keep async execution on
  `tokio-current-thread`, avoid creating runtimes inside async scenarios, and
  use sync steps where async adds no value.

- Risk: migration may preserve behaviour but leave strategy documentation
  inconsistent. Severity: medium Likelihood: medium Mitigation: update
  `docs/developers-guide.md` and keep it aligned with final implementation
  details before completion.

## Progress

- [x] (2026-02-08 18:20Z) Gather repository context and migration guidance from
  `docs/rstest-bdd-v0-5-0-migration-guide.md` and
  `docs/rstest-bdd-users-guide.md`.
- [x] (2026-02-08 18:25Z) Inventory current behavioural suite layout in
  `tests/bdd_tests.rs`, `tests/bdd/`, and feature directories.
- [x] (2026-02-08 18:35Z) Draft ExecPlan with staged migration and validation
  gates.
- [x] (2026-02-08 19:05Z) Execute stage A baseline validation and capture
  before-migration evidence.
- [x] (2026-02-08 19:10Z) Execute stage B dependency upgrade to
  `rstest-bdd` `0.5.0`.
- [x] (2026-02-08 19:20Z) Execute stage C step refactors for inferred patterns
  and typed parameters.
- [x] (2026-02-08 19:25Z) Execute stage D documentation updates for the new
  behavioural test usage.
- [x] (2026-02-08 19:40Z) Execute stage E final quality gates.

## Surprises & Discoveries

- Observation: `docs/developers-guide.md` did not exist at planning time.
  Evidence: repository file inventory and attempted read reported no file.
  Impact: migration work must create this document and treat it as the source
  of truth for testing strategy.

- Observation: behavioural tests are discovered via `scenarios!` in
  `tests/bdd_tests.rs`; there are currently no hand-written `#[scenario]`
  functions in the suite entry point. Evidence: `tests/bdd_tests.rs` contains
  `scenarios!("tests/features", ...)` and
  `scenarios!("tests/features_unix", ...)`. Impact: scenario return-type
  migration concerns mostly apply to any new explicit `#[scenario]` tests
  introduced during this migration.

- Observation: `qdrant-find` is not available in this environment.
  Evidence: `/bin/bash: qdrant-find: command not found`. Impact: project-memory
  retrieval could not be performed; in-repo documentation was used as the
  working context.

- Observation: dependency bump to `rstest-bdd` `0.5.0` did not introduce
  compile-time breakage in the existing behavioural suite. Evidence:
  `cargo test --test bdd_tests` passed immediately after updating the lockfile.
  Impact: migration effort focused on adopting new v0.5.0 usage patterns for
  clarity and reduced boilerplate.

## Decision Log

- Decision: perform an incremental migration rather than a single large
  refactor. Rationale: this keeps behavioural parity measurable at each step
  and reduces rollback risk if macro expansion behaviour changes under v0.5.0.
  Date/Author: 2026-02-08 (Codex)

- Decision: treat documentation updates as a first-class deliverable, not a
  follow-up. Rationale: the request requires cohesive developer guidance
  reflecting actual testing practice. Date/Author: 2026-02-08 (Codex)

- Decision: target v0.5.0 features that improve this suite directly: optional
  async steps where useful, explicit isolation rules, inferred step patterns
  for simple steps, and stable async wrapper import paths. Rationale: these
  reduce boilerplate and improve clarity without changing product behaviour.
  Date/Author: 2026-02-08 (Codex)

- Decision: use typed step parameters directly in step function signatures
  rather than accepting `&str` and converting inside each step. Rationale:
  implementing `FromStr` for string-backed wrappers in `tests/bdd/types.rs`
  removes repeated conversion code and strengthens step-level type semantics.
  Date/Author: 2026-02-08 (Codex)

- Decision: adopt inferred step patterns for simple no-argument `Then` steps in
  `tests/bdd/steps/cli.rs`. Rationale: this exercises new v0.5.0 inference
  behaviour and removes repetitive literal annotations. Date/Author: 2026-02-08
  (Codex)

## Outcomes & retrospective

Migration complete. Implemented outcomes:

- Updated `Cargo.toml` and `Cargo.lock` to `rstest-bdd`/`rstest-bdd-macros`
  `0.5.0`.
- Added `FromStr` and `From<&str>` support for string-backed wrappers in
  `tests/bdd/types.rs`.
- Refactored CLI and manifest-command step modules to consume typed parameters
  directly instead of local `&str` conversion boilerplate.
- Applied inferred `#[then]` patterns for simple no-argument CLI step
  definitions.
- Updated `docs/developers-guide.md` to reflect active v0.5.0 usage policy.
- Revalidated quality gates: `make check-fmt`, `make lint`, and `make test`
  passing after migration.

Retrospective:

- The dependency upgrade itself was low risk in this repository because the
  existing suite already aligned with v0.5.0 contracts.
- The most valuable migration work was explicit adoption of clearer step
  signatures and reduced repetitive annotations.

## Context and orientation

The behavioural suite currently lives in:

- `tests/bdd_tests.rs`: suite entry point using `rstest_bdd_macros::scenarios`.
- `tests/features/` and `tests/features_unix/`: Gherkin specifications.
- `tests/bdd/steps/`: step-definition modules.
- `tests/bdd/fixtures/mod.rs`: shared `TestWorld` fixture and helper traits.
- `tests/bdd/types.rs`: typed wrappers for step parameters.

The project is now pinned to `rstest-bdd = "0.5.0"` and
`rstest-bdd-macros = "0.5.0"` in `Cargo.toml`.

Migration requirements and target usage are documented in:

- `docs/rstest-bdd-v0-5-0-migration-guide.md`
- `docs/rstest-bdd-users-guide.md`

## Plan of work

### Stage A: baseline and migration inventory (no behaviour changes)

Run baseline quality gates and capture the current behavioural test inventory.
Record where the suite uses patterns likely to change in v0.5.0, especially:
fixture isolation assumptions, skip handling, async wrappers, and macro-related
Clippy suppressions.

Go/no-go: proceed only if baseline `make check-fmt`, `make lint`, and
`make test` are green.

### Stage B: dependency migration to rstest-bdd v0.5.0

Update `Cargo.toml` dev-dependencies to `rstest-bdd = "0.5.0"` and
`rstest-bdd-macros` version `0.5.0` with `strict-compile-time-validation`
enabled. Regenerate lockfile entries and resolve compile-time changes
introduced by the new macro/runtime contracts.

Go/no-go: proceed only when the suite compiles and strict compile-time
validation passes.

### Stage C: exploit v0.5.0 functionality for clarity and reduced boilerplate

Apply focused refactors that use v0.5.0 capabilities to improve this suite:

- Prefer inferred step patterns (`#[given]`, `#[when]`, `#[then]` without
  explicit text) for no-argument steps whose function names map directly to
  feature wording.
- Replace any legacy `rstest_bdd::sync_to_async` imports with
  `rstest_bdd::async_step::sync_to_async` and aliases (`StepCtx`,
  `StepTextRef`, `StepDoc`, `StepTable`) where wrappers are needed.
- Introduce async steps only in domains where asynchronous work is natural and
  improves coverage (for example, stdlib HTTP/process behaviours), using Tokio
  current-thread runtime and avoiding nested runtime creation.
- Reassess fixture shape in `tests/bdd/fixtures/mod.rs` and extract
  domain-focused fixtures where this removes `RefCell<Option<_>>` boilerplate
  and narrows step parameter types safely.
- Remove obsolete macro-related lint suppressions only where v0.5.0 generated
  code proves they are no longer required.

Go/no-go: proceed only if behaviour remains unchanged at feature level.

### Stage D: expand behavioural coverage where v0.5.0 enables it

Use the migration to add missing high-value behavioural checks while keeping
feature files concise:

- Add scenarios covering previously under-tested async-capable behaviour paths
  where relevant.
- Add assertions around skipped scenario behaviour where the suite uses
  `rstest_bdd::skip!`, using v0.5.0 skip assertion helpers where appropriate.
- Ensure scenario isolation is explicit and documented (what is per-scenario vs
  `#[once]` infrastructure).

Go/no-go: proceed only if added coverage does not introduce flaky ordering
dependencies.

### Stage E: docs, hardening, and final validation

Update `docs/developers-guide.md` and any related docs to reflect final usage,
including fixture strategy, async usage policy, and authoring conventions. Run
all quality gates and retain concise evidence logs.

## Concrete steps

All commands run from `/home/user/project`.

1. Baseline inventory and quality:

    set -o pipefail
    make check-fmt 2>&1 | tee /tmp/rstest-bdd-v050-baseline-check-fmt.log
    make lint 2>&1 | tee /tmp/rstest-bdd-v050-baseline-lint.log
    make test 2>&1 | tee /tmp/rstest-bdd-v050-baseline-test.log

2. Dependency bump and lock update:

    set -o pipefail
    rg -n "rstest-bdd" Cargo.toml
    cargo update -p rstest-bdd -p rstest-bdd-macros
    rg -n "rstest-bdd" Cargo.toml Cargo.lock

3. Focused refactors and suite updates by module:

    set -o pipefail
    cargo test --test bdd_tests 2>&1 | tee /tmp/rstest-bdd-v050-bdd-tests.log

4. Documentation and Markdown validation after docs edits:

    set -o pipefail
    make fmt 2>&1 | tee /tmp/rstest-bdd-v050-fmt.log
    make markdownlint 2>&1 | tee /tmp/rstest-bdd-v050-markdownlint.log
    make nixie 2>&1 | tee /tmp/rstest-bdd-v050-nixie.log

5. Final quality gates:

    set -o pipefail
    make check-fmt 2>&1 | tee /tmp/rstest-bdd-v050-final-check-fmt.log
    make lint 2>&1 | tee /tmp/rstest-bdd-v050-final-lint.log
    make test 2>&1 | tee /tmp/rstest-bdd-v050-final-test.log

Expected success signals:

- `make check-fmt` exits `0` with no formatting diffs required.
- `make lint` exits `0` with no Clippy warnings.
- `make test` exits `0` and includes passing `bdd_tests`.

## Validation and acceptance

The migration is complete when all of the following are true:

- `Cargo.toml` and `Cargo.lock` use `rstest-bdd` and `rstest-bdd-macros`
  version `0.5.0`.
- Behavioural tests in `tests/features/` and `tests/features_unix/` execute
  via `tests/bdd_tests.rs` and pass.
- The suite uses v0.5.0 conventions where applicable:
  - explicit scenario isolation policy (per-scenario fixtures by default),
  - stable async wrapper import paths,
  - reduced repetitive step boilerplate for simple steps.
- `docs/developers-guide.md` describes the strategy in active use.
- `make check-fmt`, `make lint`, and `make test` pass.

## Idempotence and recovery

- Every stage is designed to be re-runnable. If a command fails, fix the issue
  and rerun the same command; logs may be overwritten safely.
- If dependency bump causes broad breakage, revert only the migration-specific
  edits and rerun baseline gates before retrying.
- Keep changes staged by domain module so partial rollbacks are small and
  deterministic.

## Artifacts and notes

Collect and keep short excerpts from:

- `/tmp/rstest-bdd-v050-baseline-*.log`
- `/tmp/rstest-bdd-v050-final-*.log`
- `cargo test --test bdd_tests` output proving behavioural coverage remained
  green through migration.

When failures occur, record the exact error message and affected step/scenario
pair in this plan's `Surprises & Discoveries` section before retrying.

## Interfaces and dependencies

- Dependencies:
  - `rstest-bdd = "0.5.0"` in `Cargo.toml` dev-dependencies.
  - `rstest-bdd-macros = { version = "0.5.0", features =
    ["strict-compile-time-validation"] }`.
- Behavioural suite entry point remains `tests/bdd_tests.rs` with
  `scenarios!`-driven discovery.
- Fixture/state interfaces to preserve or improve:
  - `tests/bdd/fixtures::TestWorld` (or successor domain fixtures),
  - typed step wrappers in `tests/bdd/types.rs`,
  - shared helpers in `tests/bdd/helpers/`.
- Runtime policy:
  - sync scenarios by default,
  - async scenarios only where needed with Tokio current-thread runtime.

## Revision note (required when editing an ExecPlan)

- 2026-02-08: Initial draft created to plan migration of behavioural tests to
  `rstest-bdd` `0.5.0`, including staged execution, risk controls, and explicit
  validation gates.
- 2026-02-08: Updated status to `COMPLETE` after implementing dependency
  migration, typed step refactors, inferred step patterns, and final gate
  validation.

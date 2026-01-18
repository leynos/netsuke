# Externalize user-facing strings into Fluent (roadmap 3.7.1)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DONE

PLANS.md is not present in the repository.

## Purpose / Big picture

Netsuke should render all user-facing text (help, status, warnings, errors)
from Fluent `.ftl` bundles so translations can be managed without touching
code. This item delivers three concrete outcomes:

- All user-facing strings move into `locales/*/messages.ftl` and are rendered
  through `ortho_config::Localizer`.
- A compile-time audit fails the build when a referenced message key is
  missing, preventing silent regressions in CI.
- Spanish (`es-ES`) is included as a non-English example locale.

Success is observable when common failures (missing manifest, invalid CLI
flags, manifest parse errors) are displayed in English by default, in Spanish
when `--locale es-ES` (or `NETSUKE_LOCALE=es-ES`) is set, and CI fails if a
referenced Fluent key is missing.

## Constraints

- All user-facing copy must originate from Fluent resources under
  `locales/` and be rendered via `ortho_config::Localizer`.
- Locale resolution behaviour must not expand beyond the current sources
  (`--locale` and `NETSUKE_LOCALE`); roadmap item 3.7.2 is out of scope.
- English (`en-US`) remains the fallback locale when translations are missing.
- New modules must start with a `//!` doc comment and remain under 400 lines.
- No `unsafe` code; use `Result`-based error handling, not panics.
- All new tests use `rstest` and `rstest-bdd` (`rstest-bdd` v0.3.2).
- Documentation updates must follow `docs/documentation-style-guide.md`
  (British English, 80-column wrapping, no fenced code blocks inside this plan
  file).

## Tolerances (Exception triggers)

- Scope: more than 30 files changed or more than 1,500 net new lines.
- Interface: any public API signature change (for example `runner::run`).
- Dependencies: introducing a new crate not already in the workspace.
- Test iterations: more than 2 failed full-gate runs (`make check-fmt`,
  `make lint`, `make test`) without a clear fix.
- Ambiguity: multiple plausible interpretations of “user-facing” messages
  that materially change which strings are externalized.

Escalate immediately if a tolerance is hit.

## Risks

- Risk: Scope creep from the large surface area of error strings.
  Severity: high. Likelihood: medium. Mitigation: define a clear “user-facing”
  inventory in Stage A and stick to it; defer internal-only logs to later
  roadmap items.

- Risk: Localization of `miette` diagnostics may require structural changes
  to error types. Severity: medium. Likelihood: medium. Mitigation: use a small
  adapter layer that formats diagnostics from Fluent without changing public
  APIs unless strictly necessary.

- Risk: Spanish translations drift from English semantics.
  Severity: medium. Likelihood: low. Mitigation: keep translations short, avoid
  idioms, and add tests that assert key Spanish phrases.

- Risk: BDD step ambiguity (inventory-order differences in `rstest-bdd`).
  Severity: medium. Likelihood: low. Mitigation: avoid overlapping step
  patterns and follow the guidance in the qdrant notes for `rstest-bdd` step
  wording.

## Progress

- [x] (2026-01-17 00:00Z) Draft ExecPlan and capture initial context.
- [x] (2026-01-17) Inventory user-facing strings and define Fluent key taxonomy.
- [x] (2026-01-17) Add compile-time Fluent key audit.
- [x] (2026-01-17) Externalize user-facing strings and add Spanish translations.
- [x] (2026-01-17) Add unit and behavioural tests.
- [x] (2026-01-17) Update documentation and roadmap; run quality gates.

## Surprises & Discoveries

- Proc-macro ingestion of `tests/features/*.feature` does not always trigger a
  rebuild locally; touching `tests/bdd_tests.rs` forces the scenarios to
  regenerate when feature text changes.

## Decision Log

- Decision: Use `ortho_config::Localizer` as the single rendering interface
  for all user-facing copy, keeping the Fluent resources under `locales/`.
  Rationale: aligns with existing CLI localisation and the OrthoConfig guide.
  Date/Author: 2026-01-17 (Terry)

- Decision: Add a compile-time audit in `build.rs` to validate Fluent keys.
  Rationale: ensures missing keys fail CI before runtime. Date/Author:
  2026-01-17 (Terry)

- Decision: Centralize Fluent key names in `src/localization/keys.rs` and treat
  them as the source of truth for the audit and message rendering. Rationale:
  keeps key usage consistent and makes audits deterministic. Date/Author:
  2026-01-17 (Terry)

## Outcomes & Retrospective

- Fluent keys now cover CLI copy, diagnostics, and stdlib errors with Spanish
  as the reference translation and English fallback.
- The compile-time Fluent key audit prevents missing-key regressions in CI.
- Unit + BDD coverage updated for localisation-aware messages; quality gates
  (`make check-fmt`, `make lint`, `make test`) pass.

## Context and orientation

Relevant files and modules:

- `src/main.rs` builds the CLI localiser and prints runner errors.
- `src/cli_localization.rs` builds a Fluent-backed localiser with an
  English fallback and Spanish resources.
- `src/cli_l10n.rs` localises clap usage, help, and subcommand copy.
- `locales/en-US/messages.ftl` and `locales/es-ES/messages.ftl` currently
  contain help text and clap error strings.
- User-facing error strings are embedded in `thiserror`/`miette` derives in
  `src/runner/error.rs`, `src/manifest/diagnostics/*`, `src/ir/graph.rs`,
  `src/ninja_gen.rs`, and `src/stdlib/**/error.rs`.
- Design expectations for localisation are in
  `docs/netsuke-cli-design-document.md` and `docs/ortho-config-users-guide.md`.
- Testing conventions and fixtures are documented in
  `docs/rust-testing-with-rstest-fixtures.md` and
  `docs/rstest-bdd-users-guide.md`.

## Plan of work

### Stage A: Inventory and key taxonomy (no code changes)

1. Enumerate user-facing strings by searching for `#[error("...")]`,
   `Diagnostic` messages, and other `Display`/`Context` strings that reach the
   CLI output. Keep this list in the plan notes while implementing.
2. Define a Fluent key taxonomy in the design document, for example:
   `cli.*`, `error.runner.*`, `error.manifest.*`, `error.ir.*`,
   `error.stdlib.*`, `status.*`.
3. Decide which strings are strictly user-facing and which are internal logs.
   Log-only strings stay as-is for now to avoid scope creep.

### Stage B: Localisation infrastructure and key audit

1. Create a dedicated localisation module (for example
   `src/localization/mod.rs`) with a `//!` doc comment. This module should
   define:
   - A typed way to refer to message keys (constants or an enum).
   - A helper that renders a key with `LocalizationArgs` via
     `ortho_config::Localizer`, with a fallback string for resilience.
2. Add a compile-time audit in `build.rs` that parses
   `locales/en-US/messages.ftl` (and optionally `locales/es-ES/messages.ftl`)
   and verifies that every key referenced in code exists. Fail the build on
   missing keys. Keep the audit fast and deterministic.
3. Ensure the audit is deterministic in CI and does not require network
   access. If an extra crate is needed for Fluent parsing, pause and escalate
   before adding it (tolerance: new dependency).

### Stage C: Externalize user-facing strings

1. Replace user-facing `thiserror` messages and `miette` diagnostics with
   Fluent key lookups. Preserve structured data (paths, target names, counts)
   as Fluent variables rather than interpolating in code.
2. Update error types to carry the data needed for localisation while keeping
   public signatures stable. Prefer adding helper methods to format errors
   rather than changing core return types.
3. Extend `locales/en-US/messages.ftl` with the new keys and add matching
   `es-ES` translations. Keep key names snake_case where they map to clap
   argument IDs.
4. Ensure the CLI output path uses the localiser when rendering diagnostics
   (for example, when mapping `RunnerError` and manifest diagnostics to the
   final stderr output).

### Stage D: Tests, docs, and finalisation

1. Add `rstest` unit tests for the localisation helpers and at least one
   error mapping that uses variables (for example, manifest not found).
2. Add `rstest-bdd` scenarios that assert:
   - English output for a default error (no locale specified).
   - Spanish output when `--locale es-ES` or `NETSUKE_LOCALE=es-ES` is set.
   Include both a happy path (help output) and an unhappy path (error).
3. Update `docs/users-guide.md` to describe the expanded localisation
   behaviour and any new CLI output differences.
4. Record design decisions in `docs/netsuke-cli-design-document.md` (or the
   most relevant design doc) covering the key taxonomy and compile-time audit.
5. Run formatting and lint gates, then mark roadmap item 3.7.1 as done in
   `docs/roadmap.md`.

## Concrete steps

All commands are run from the repository root (`/root/repo`). Use `tee` with
`set -o pipefail` to preserve exit codes as required by `AGENTS.md`.

1. Inventory user-facing strings:

    rg -n "#\\[error\(" src
    rg -n "Diagnostic" src
    rg -n "Context\(|bail!\(|anyhow!\(" src

2. Run formatting and Markdown checks after doc edits:

    set -o pipefail
    make fmt 2>&1 | tee /tmp/netsuke-fmt.log
    make markdownlint 2>&1 | tee /tmp/netsuke-markdownlint.log
    make nixie 2>&1 | tee /tmp/netsuke-nixie.log

3. Run quality gates after code changes:

    set -o pipefail
    make check-fmt 2>&1 | tee /tmp/netsuke-check-fmt.log
    make lint 2>&1 | tee /tmp/netsuke-lint.log
    make test 2>&1 | tee /tmp/netsuke-test.log

## Validation and acceptance

Quality criteria (done means all of these are true):

- All user-facing strings are sourced from Fluent bundles under `locales/`.
- A compile-time audit fails the build if a referenced Fluent key is missing.
- Spanish (`es-ES`) translations exist for every new key used by the CLI.
- `rstest` unit tests cover localised rendering (happy and unhappy paths).
- `rstest-bdd` scenarios cover English and Spanish CLI output.
- `docs/users-guide.md` documents the localisation behaviour.
- The design document records localisation decisions.
- `make check-fmt`, `make lint`, and `make test` all pass.
- `docs/roadmap.md` marks 3.7.1 as done.

## Idempotence and recovery

- The Fluent audit and localisation helpers are safe to re-run; rebuild after
  any `.ftl` changes.
- If a gate fails, fix the underlying issue and re-run the same command,
  overwriting the log file to keep evidence current.

## Artifacts and notes

Keep these logs as evidence of success:

- `/tmp/netsuke-check-fmt.log`
- `/tmp/netsuke-lint.log`
- `/tmp/netsuke-test.log`

## Interfaces and dependencies

- Localisation helper module (new): `src/localization/mod.rs` (exact name to
  be decided during implementation).
- Compile-time audit: `build.rs` should validate Fluent keys against the list
  referenced in code and the `locales/en-US/messages.ftl` bundle.
- Fluent resources: update `locales/en-US/messages.ftl` and
  `locales/es-ES/messages.ftl` with new keys for errors and status output.
- Error mapping: `src/runner/error.rs`, `src/manifest/diagnostics/*`,
  `src/ir/graph.rs`, `src/ninja_gen.rs`, and `src/stdlib/**/error.rs` should
  route user-facing messages through Fluent.
- CLI output: `src/main.rs` should render diagnostics using the localiser so
  errors appear in the selected locale.

## Revision note (required when editing an ExecPlan)

- 2026-01-17: Initial ExecPlan drafted for roadmap item 3.7.1.

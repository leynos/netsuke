# Implement locale resolution for command-line interface (CLI) and runtime

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DONE

No `PLANS.md` file exists in this repository.

## Purpose / big picture

Netsuke should resolve the active locale from command-line flags, environment
variables, configuration files, and system defaults so both CLI help/errors and
runtime diagnostics render in the correct language. Users should be able to set
`--locale`, `NETSUKE_LOCALE`, or `locale = "..."` in configuration files, and
the application should fall back to system defaults when none are supplied. If
a requested locale is unsupported, the system must fall back to `en-US`.

Success is observable by running the CLI with different locale inputs and
seeing Fluent messages rendered in the expected language, and by verifying that
locale resolution honours the documented precedence order with unit and
behavioural tests.

## Constraints

- Keep the `--locale` flag name and semantics intact.
- Use OrthoConfig's layered configuration model (defaults < file < env < CLI)
  for locale data and do not introduce ad-hoc config loaders.
- Maintain localized clap help and error output by continuing to use
  `ortho_config::Localizer` in CLI parsing.
- Fall back to `en-US` when a translation bundle is absent or invalid.
- Avoid new files exceeding 400 lines; split modules if needed.
- Do not add `unsafe` code; prefer dependency injection and mockable traits for
  system/environment interactions.
- Ensure documentation uses en-GB-oxendict spelling and wraps at 80 columns.

## Tolerances (exception triggers)

- Scope: if the change requires edits to more than 12 files or more than
  600 net new lines, stop and escalate.
- Dependencies: if any new external dependency beyond upgrading to
  `rstest-bdd` 0.4.0 and adding a single system-locale helper crate is
  required, stop and escalate.
- Interfaces: if a public application programming interface (API) signature
  must change in `src/cli/mod.rs` or `src/localization/mod.rs`, stop and
  escalate.
- Tests: if `make test` still fails after two investigation cycles, stop and
  escalate.
- Ambiguity: if locale precedence or system-default behaviour remains unclear
  after reviewing docs, stop and request clarification before coding.

## Risks

- Risk: system locale strings often include encodings or separators
  (`en_US.UTF-8`) that are not valid Best Current Practice (BCP) 47 tags.
  Severity: medium Likelihood: high Mitigation: implement a normalization
  helper and cover representative cases with unit tests.

- Risk: upgrading `rstest-bdd` to 0.4.0 may require API adjustments to step
  macros or fixtures. Severity: medium Likelihood: medium Mitigation: review
  crate changelog, run tests early, and update step definitions incrementally.

- Risk: global localizer state introduces test flakiness if not properly
  serialized. Severity: medium Likelihood: medium Mitigation: reuse
  `test_support::localizer_test_lock` and avoid leaking localizer guards across
  tests.

## Progress

- [x] (2026-01-28) Drafted ExecPlan for locale resolution work.
- [x] (2026-01-29) Received approval to proceed with implementation.
- [x] (2026-01-29) Reviewed locale handling and captured current flow.
- [x] (2026-01-29) Chose system-locale detection and updated design docs.
- [x] (2026-01-29) Added unit and behavioural tests for locale resolution.
- [x] (2026-01-29) Implemented locale resolution and wired CLI/runtime usage.
- [x] (2026-01-29) Updated user/design docs and marked roadmap entry done.
- [x] (2026-01-29) Ran `make check-fmt`, `make lint`, and `make test` via tee.

## Surprises & discoveries

- Observation: behaviour-driven development (BDD) filesystem scenarios now
  skip when no block devices are available. Evidence: `tests/bdd/steps/fs.rs`
  uses `rstest_bdd::skip!` to avoid environment-specific failures. Impact:
  tests remain stable across minimal environments.

## Decision log

- Decision: Introduce a dedicated locale-resolution module that normalizes
  locale tags and enforces precedence. Rationale: Centralizing locale logic
  avoids duplicated precedence handling between CLI parse and runtime
  diagnostics. Date/Author: 2026-01-28 (Codex)

- Decision: Use a system-locale helper crate (e.g. `sys-locale`) plus
  normalization logic rather than platform-specific code. Rationale:
  Cross-platform defaults are otherwise error-prone and would require operating
  system (OS)-specific implementations. Date/Author: 2026-01-28 (Codex)

- Decision: Introduce lightweight environment and system-locale provider
  traits instead of adding `mockable` to production dependencies. Rationale:
  This preserves dependency-injection for tests while keeping the runtime
  dependency footprint within the tolerance limits. Date/Author: 2026-01-29
  (Codex)

## Outcomes & retrospective

- Implemented locale resolution with normalized tags and precedence-aware
  resolution for startup and runtime locales.
- Added unit (rstest) and behavioural (rstest-bdd 0.4.0) coverage for happy
  and unhappy locale paths, plus unsupported locale fallback behaviour.
- Documented locale precedence and design decisions, and marked roadmap
  entry 3.7.2 as done.

## Context and orientation

Locale handling is currently split across several modules:

- `src/main.rs` builds a localizer before parsing CLI arguments using
  `cli::locale_hint_from_args` and `NETSUKE_LOCALE`, then rebuilds a runtime
  localizer after `cli::merge_with_config`.
- `src/cli_l10n.rs` extracts `--locale` from raw arguments to localize clap
  help and errors before full parsing.
- `src/cli_localization.rs` builds Fluent localizers, providing an English
  fallback and optional Spanish resources.
- `src/cli/mod.rs` defines `Cli` (with `locale: Option<String>`) and provides
  OrthoConfig merging via `merge_with_config`.
- Tests in `tests/cli_tests`, `tests/localization_tests.rs`, and BDD features
  validate existing localization behaviour.

The new work should consolidate locale resolution so precedence across
`--locale`, `NETSUKE_LOCALE`, configuration files, and system defaults is
consistent. The outcome must update both CLI help localization and runtime
localization for diagnostics.

## Plan of work

Stage A: understand and propose (no code changes)

- Review `src/main.rs`, `src/cli_l10n.rs`, `src/cli_localization.rs`, and
  `src/cli/mod.rs` to map the current locale flow.
- Decide the exact precedence order and document it in
  `docs/netsuke-design.md` under the CLI design decisions section.
- Choose the system default provider (`sys-locale` or equivalent) and record
  its normalization requirements (e.g. strip encoding suffixes, replace
  underscores with hyphens). If a different approach is required, update the
  Decision Log and revisit tolerances.

Stage B: scaffolding and tests (small, verifiable diffs)

- Upgrade test dependencies to `rstest-bdd` 0.4.0 (and macros) in
  `Cargo.toml`, respecting caret requirements, and update `Cargo.lock`.
- Add unit tests (rstest) for a new locale-resolution helper covering:
  - CLI > env > config > system precedence.
  - Normalization of `en_US.UTF-8`, `es_ES`, and already-normal tags.
  - Fallback to `en-US` when preferred locales are unsupported.
- Add behavioural tests (rstest-bdd) that exercise the locale resolution
  behaviour using step definitions and a feature file. Include:
  - A happy path where config supplies the locale and no overrides exist.
  - An override path where `NETSUKE_LOCALE` or `--locale` wins over config.
  - An unhappy path where an unsupported locale falls back to English.
- Ensure tests use dependency injection and mockable environment helpers rather
  than mutating global state directly.

Stage C: implementation (minimal change to satisfy tests)

- Add a dedicated module (e.g. `src/locale_resolution.rs`) with:
  - A `LocaleResolution` struct or functions that accept:
    - CLI locale hint (raw args),
    - environment locale (`NETSUKE_LOCALE`),
    - configuration locale (from merged `Cli`),
    - system default locale from the helper crate,
    - and return a normalized `Option<String>` for startup and runtime.
  - A normalization helper that strips encodings, replaces `_` with `-`, and
    validates via `ortho_config::LanguageIdentifier`.
  - Dependency injection interfaces for environment/system providers so unit
    tests can supply `MockEnv` and stub system locales.
- Update `src/main.rs` to:
  - Resolve an initial locale for CLI parsing (CLI hint, env, system),
    build the localizer from it, and set it before parsing.
  - After `cli::merge_with_config`, resolve the runtime locale using the merged
    config value and system default, then set the runtime localizer.
- Update any helper functions (e.g. BDD CLI parsing helpers) to use the new
  locale-resolution module so tests exercise real behaviour.

Stage D: hardening, documentation, cleanup

- Update `docs/users-guide.md` to describe the new locale resolution order,
  how to set `locale` in configuration files, and the system-default fallback.
- Update `docs/netsuke-design.md` to record the final precedence rules and any
  normalization decisions for system locale strings.
- Mark roadmap item `3.7.2` as done in `docs/roadmap.md`.
- Run formatting and linting gates, plus documentation tooling:
  - `make fmt` (after doc edits), `make markdownlint`, `make nixie`.
  - `make check-fmt`, `make lint`, `make test`.

## Concrete steps

All commands should be run from `/home/user/project`. For long-running commands
use `set -o pipefail` and `tee` to capture logs, for example:

    set -o pipefail
    make test 2>&1 | tee /tmp/netsuke-make-test.log

Concrete implementation steps:

1. Inspect locale-related modules (`src/main.rs`, `src/cli_l10n.rs`,
   `src/cli_localization.rs`, `src/cli/mod.rs`) and update this plan if new
   constraints emerge.
2. Update `Cargo.toml` dev-dependencies for `rstest-bdd`/macros to 0.4.0 and
   add the chosen system-locale dependency with a caret version. Run
   `cargo update -p rstest-bdd -p rstest-bdd-macros` if required.
3. Add unit tests under `tests/` (or `src/` `#[cfg(test)]`) that exercise the
   locale-resolution helper using `rstest` and mockable environment helpers.
4. Add a new `.feature` file under `tests/features/` and matching step
   definitions under `tests/bdd/steps/` for locale-resolution scenarios.
5. Implement the locale-resolution module and integrate it into `src/main.rs`
   and any test helpers that build CLI localizers.
6. Update documentation (`docs/users-guide.md`, `docs/netsuke-design.md`) and
   mark `docs/roadmap.md` item 3.7.2 as done.
7. Run formatting, linting, and tests with `tee` logs:

    set -o pipefail
    make fmt 2>&1 | tee /tmp/netsuke-make-fmt.log
    make markdownlint 2>&1 | tee /tmp/netsuke-markdownlint.log
    make nixie 2>&1 | tee /tmp/netsuke-nixie.log
    make check-fmt 2>&1 | tee /tmp/netsuke-check-fmt.log
    make lint 2>&1 | tee /tmp/netsuke-lint.log
    make test 2>&1 | tee /tmp/netsuke-test.log

## Validation and acceptance

Acceptance requires observable locale selection across all sources.

- Tests must demonstrate that `--locale`, `NETSUKE_LOCALE`, config file values,
  and system defaults are honoured in precedence order.
- Running `make test` should show new unit tests failing before the change and
  passing after, and BDD scenarios passing via `rstest-bdd` 0.4.0.
- Unsupported locales must fall back to English in both unit and behavioural
  tests.

Quality criteria (what "done" means):

- Tests: `make test` passes and includes new rstest and rstest-bdd scenarios.
- Lint/typecheck: `make check-fmt` and `make lint` pass with no warnings.
- Documentation: `make fmt`, `make markdownlint`, and `make nixie` pass after
  documentation updates.

Quality method (how we check):

- Run each make target with `set -o pipefail` and inspect the log outputs for
  errors or warnings.

## Idempotence and recovery

All steps are re-runnable. If a test or formatting step fails, fix the issue
and rerun the same command. If `Cargo.lock` changes unexpectedly during
dependency updates, re-run `cargo update -p <crate>` to ensure only intended
crates are modified.

## Artifacts and notes

Expected new/updated artefacts include:

- `src/locale_resolution.rs` (or equivalent) for centralized locale logic.
- Updated `Cargo.toml` and `Cargo.lock` for dependency changes.
- New/updated tests in `tests/` and `tests/bdd/`.
- Updated documentation in `docs/users-guide.md`, `docs/netsuke-design.md`,
  and `docs/roadmap.md`.

## Interfaces and dependencies

- New module: `src/locale_resolution.rs` providing:
  - `resolve_startup_locale(args: &[OsString], env: &impl Env,
    system: &impl SystemLocale) -> Option<String>`
  - `resolve_runtime_locale(merged: &Cli, system: &impl SystemLocale)
    -> Option<String>`
  - `normalize_locale_tag(raw: &str) -> Option<String>`
- Trait: `SystemLocale` (testable wrapper returning `Option<String>`).
- Dependency: a single system-locale helper crate (e.g. `sys-locale`) with a
  caret version in `Cargo.toml`.
- Tests should reuse `mockable::MockEnv` and any existing test locks in
  `test_support` to avoid global state contamination.

## Revision note

2026-01-29: Updated status to IN PROGRESS, recorded approval, and logged
implementation progress plus dependency-injection decisions. This keeps the
plan aligned with the current code changes.

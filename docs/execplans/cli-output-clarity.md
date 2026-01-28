# Refine CLI Output With OrthoConfig Localized Help

This ExecPlan is a living document. The sections `Progress`,
`Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must
be kept up to date as work proceeds.

No `PLANS.md` exists in the repository root at the time of writing.

## Purpose / Big Picture

Users should see clear, descriptive CLI help and intuitive command feedback
when running Netsuke. Configuration should be layered ergonomically via
`ortho_config` so defaults, config files, environment variables, and CLI flags
behave consistently. Localized help strings should be used for CLI usage and
error output. Success is observable by running `netsuke --help`, subcommand
help, and core commands (build, clean, graph, manifest) and seeing clear
messages, plus passing the new unit and behavioural tests that assert these
outputs and configuration precedence.

## Progress

- [x] (2026-01-02 00:00Z) Drafted ExecPlan skeleton and captured repository
  context.
- [ ] Inventory current CLI output and help messages (help text, errors,
  subcommand feedback) and record gaps.
- [x] Implement OrthoConfig-backed CLI configuration and localized help
  plumbing.
- [x] Refine user-facing CLI output and update docs.
- [x] Add unit tests and rstest-bdd behavioural tests for happy/unhappy paths.
- [x] (2026-01-02 00:00Z) Run formatting, lint, and test gates; mark the
  roadmap entry as done.

## Surprises & Discoveries

- Observation: None yet.
  Evidence: N/A.

## Decision Log

- Decision: Use OrthoConfig merge composition to layer defaults, config files,
  environment variables, and CLI overrides while treating clap defaults as
  absent. Rationale: Preserves deterministic precedence and avoids masking
  file/env values with clap defaults. Date/Author: 2026-01-02 (Codex)

- Decision: Provide Fluent-backed localization with English defaults and a
  Spanish catalogue layered as a consumer resource. Rationale: Validates
  localized help/error support while keeping fallback behaviour intact.
  Date/Author: 2026-01-02 (Codex)

## Outcomes & Retrospective

- Outcome: Complete.
  Notes: OrthoConfig configuration layering, localization resources (including
  Spanish), updated CLI output, and tests are in place; `make check-fmt`,
  `make lint`, and `make test` now pass.

## Context and Orientation

Key runtime entry points and CLI definitions live in these files:

- `src/cli/mod.rs` defines the clap CLI (flags, subcommands, help text) and
  default command behaviour.
- `src/main.rs` parses the CLI, configures logging, and dispatches to the
  runner.
- `src/runner/mod.rs` and `src/runner/process/` implement subcommand behaviour
  and user-visible status logs.
- `src/cli_policy.rs` derives network policy from CLI settings.
- Tests cover CLI parsing and runner behaviour in `tests/cli_tests.rs` and
  `tests/runner_tests.rs`, plus behavioural steps in `tests/bdd/steps/cli.rs`
  and `tests/bdd/steps/process.rs`.

OrthoConfig is wired in. The user guide for it is
`docs/ortho-config-users-guide.md`, which explains configuration layering,
localized help via Fluent, and error localization helpers. Design expectations
for CLI behaviour are in `docs/netsuke-design.md` and the roadmap entry in
`docs/roadmap.md` (Phase 3 → “CLI and Feature Completeness”).

Testing guidance for fixtures, dependency injection (DI), and behaviour-driven
development (BDD) lives in:

- `docs/rust-testing-with-rstest-fixtures.md`
- `docs/reliable-testing-in-rust-via-dependency-injection.md`
- `docs/behavioural-testing-in-rust-with-cucumber.md` (applies to Gherkin
  structure, even though `rstest-bdd` is used instead)
- `docs/rust-doctest-dry-guide.md` (for any new public API docs)

## Plan of Work

1. Audit current CLI output and help. Capture current `--help` output for the
   root command and each subcommand, plus error messages from invalid flags and
   missing values. Review `tracing` info logs emitted by
   `src/runner/process/mod.rs` and `src/runner/mod.rs` to identify copy that
   needs to be clarified. Document gaps in `Surprises & Discoveries` and update
   `Decision Log` if scope changes.

2. Introduce an OrthoConfig-backed configuration layer for CLI data. Add a
   new module (for example `src/cli_config.rs`) that defines `CliConfig` and
   subcommand argument structs using
   `#[derive(OrthoConfig, Deserialize, Serialize, Parser)]` (or the equivalent
   pattern in the guide). Configure a prefix such as `NETSUKE` and ensure
   fields map to orthographic CLI flags, env vars, and config file keys. Use
   OrthoConfig helpers (`ConfigDiscovery`, `compose_layers`,
   `merge_from_layers`, or `SubcmdConfigMerge`) so configuration files and
   environment variables are layered beneath explicit CLI overrides. Add
   explicit handling for missing required values and make sure clap defaults do
   not incorrectly override config layers (use `cli_default_as_absent` where
   needed).

3. Localize CLI help and clap errors. Create Fluent resources (for example
   `locales/en-US/messages.ftl` and a CLI-specific bundle) and wire a
   `FluentLocalizer` into CLI parsing. Follow the existing `locales/` layout
   for project translations. Use `command().localize(&localizer)` before
   parsing and `localize_clap_error_with_command` on errors. Ensure the
   fallback path preserves stock clap output when localization fails.

4. Refine CLI output messages. Update docstrings and help text in
   `src/cli/mod.rs` (or the new config module) to be plain language and
   action-oriented. Review `tracing::info!` messages for build/manifest/graph
   flows and update wording to align with the user guide. Ensure stderr/stdout
   separation remains correct and messages are consistent across subcommands.
   If necessary, introduce a small output helper module to centralize user
   message formatting.

5. Add tests. Extend unit tests in `tests/cli_tests.rs` with `rstest` fixtures
   that validate:

   - OrthoConfig precedence (defaults < file < env < CLI), using
     `MergeComposer` or `compose_layers` to avoid touching process state.
   - Localized help contains expected copy, and clap errors are localized when
     possible.
   - Unhappy paths (invalid schemes, invalid jobs count, missing required
     values) return the correct error kinds and messages.

   Add behavioural coverage with `rstest-bdd` in `tests/features` and step
   definitions in `tests/bdd/steps/cli.rs` to exercise real CLI invocations.
   Use distinct step wording to avoid the `rstest-bdd` pattern ambiguity
   pitfalls noted in prior gotchas. Cover at least one happy path and one
   unhappy path for help output and configuration layering.

6. Documentation updates. Update `docs/users-guide.md` to explain the new
   configuration layering, localized help, and any changes to CLI output.
   Record design decisions in `docs/netsuke-design.md`. Mark the “Refine all
   CLI output…” item as done in `docs/roadmap.md` once tests pass.

## Concrete Steps

All commands are run from the repository root (`/root/repo`). Use `tee` with
`set -o pipefail` to preserve exit codes, as required by `AGENTS.md`.

1. Capture current CLI output for reference:

    set -o pipefail
    cargo run -- --help 2>&1 | tee /tmp/netsuke-help.log
    cargo run -- build --help 2>&1 | tee /tmp/netsuke-build-help.log
    cargo run -- clean --help 2>&1 | tee /tmp/netsuke-clean-help.log
    cargo run -- graph --help 2>&1 | tee /tmp/netsuke-graph-help.log
    cargo run -- manifest --help 2>&1 | tee /tmp/netsuke-manifest-help.log

2. Implement OrthoConfig integration and localization. Update or add the
   relevant files and ensure new Fluent resources exist.

3. Update tests and docs.

4. Format and lint documentation if modified:

    set -o pipefail
    make fmt 2>&1 | tee /tmp/netsuke-fmt.log
    make markdownlint 2>&1 | tee /tmp/netsuke-markdownlint.log
    make nixie 2>&1 | tee /tmp/netsuke-nixie.log

5. Run core quality gates:

    set -o pipefail
    make check-fmt 2>&1 | tee /tmp/netsuke-check-fmt.log
    make lint 2>&1 | tee /tmp/netsuke-lint.log
    make test 2>&1 | tee /tmp/netsuke-test.log

## Validation and Acceptance

- Running `netsuke --help` (via `cargo run -- --help`) prints localized,
  plain-language descriptions for every flag and subcommand.
- Subcommand help (`build`, `clean`, `graph`, `manifest`) is descriptive and
  matches the user guide. Error output for invalid CLI inputs is localized when
  a translation exists.
- Configuration precedence is verified by unit tests: defaults < config file
  < environment variables < CLI overrides.
- Behavioural tests exercise at least one happy path and one unhappy path that
  assert CLI output clarity and config layering.
- `make check-fmt`, `make lint`, and `make test` pass.
- `docs/users-guide.md` reflects the updated CLI behaviour and configuration
  model, and `docs/roadmap.md` marks the CLI output item as done.

## Idempotence and Recovery

- OrthoConfig and localization changes are safe to re-run; regenerate
  configuration layers and help output as often as needed.
- If localization setup fails, fall back to default clap strings and record
  the failure in `Surprises & Discoveries` with the error output.
- If tests fail mid-run, fix the underlying issue and re-run the same command
  with the same log path, overwriting the log file to keep evidence current.

## Artifacts and Notes

Keep the following short transcripts for evidence:

- `netsuke --help` output with localized strings (from
  `/tmp/netsuke-help.log`).
- A failing CLI invocation showing localized error output (record the command
  and a short excerpt of stderr).
- Test summaries from `/tmp/netsuke-test.log` showing the new unit and BDD
  tests passing.

## Interfaces and Dependencies

- Add `ortho_config` as a dependency at the version specified in
  `Cargo.toml`, enabling `yaml`/`json5` features only if required and
  documenting the decision in `docs/netsuke-design.md`.
- Define `CliConfig` in `src/cli_config.rs` (or equivalent) with fields that
  map to existing CLI flags: `file`, `directory`, `jobs`, `verbose`,
  `fetch_allow_scheme`, `fetch_allow_host`, `fetch_block_host`,
  `fetch_default_deny`, plus subcommand args. Ensure
  `#[ortho_config(prefix = "NETSUKE")]` is used for orthographic naming.
- Use `FluentLocalizer` from `ortho_config` and wire it into clap command
  creation. Provide Fluent keys for usage, about, flag help, and
  `clap-error-<kind>` messages.
- Ensure CLI parsing in `src/main.rs` uses the localizer and merges config
  layers before calling `runner::run`.
- If a helper module is introduced (for example `src/cli_output.rs`), keep it
  small, with a single responsibility for formatting user-visible messages.

## Revision note (required when editing an ExecPlan)

- 2026-01-02: Initial ExecPlan created to cover OrthoConfig integration,
  localized help, CLI output clarity, and test/documentation updates.

# Onboarding and defaults (roadmap 3.6)

This ExecPlan is a living document. The sections `Progress`,
`Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must
be kept up to date as work proceeds.

This plan covers all three items in roadmap section 3.6:

- 3.6.1 Ensure default subcommand builds manifest defaults
- 3.6.2 Curate OrthoConfig-generated Clap help output
- 3.6.3 Publish "Hello World" quick-start walkthrough

## Purpose / big picture

Users running `netsuke` for the first time should have a smooth onboarding
experience. When a manifest file is missing, the CLI should emit a clear,
actionable error with a hint rather than a generic file I/O error. All CLI help
text (subcommands and flags) should be plain-language, localizable, and
consistent with the documentation style guide. A step-by-step quickstart
tutorial should demonstrate running Netsuke end-to-end, exercised via an example
build fixture. Success is observable by:

1. Running `netsuke` in an empty directory and seeing:

   ```text
   Error: No `Netsukefile` found in the current directory.

   Hint: Run `netsuke --help` to see how to specify or create a manifest.
   ```

2. Running `netsuke --help` and seeing localized descriptions for all flags.

3. Following `docs/quickstart.md` and successfully building the hello-world
   example.

This behaviour is specified in `docs/netsuke-cli-design-document.md` (lines
40-50) and tracked in `docs/roadmap.md` items 3.6.1, 3.6.2, and 3.6.3.

## Progress

- [x] Draft ExecPlan and capture repository context.
- [x] Implement `ManifestNotFound` error variant with `miette::Diagnostic`.
- [x] Add file existence check in `generate_ninja()` before manifest loading.
- [x] Extend `cli_l10n.rs` to localize flag help strings.
- [x] Add localization keys to Fluent files (`en-US`, `es-ES`).
- [x] Create `docs/quickstart.md` tutorial.
- [x] Create `examples/hello-world/` fixture with working manifest.
- [x] Add BDD scenarios for missing manifest and quickstart example.
- [x] Update `docs/users-guide.md` with new error behaviour and quickstart link.
- [x] Run formatting, lint, and test gates; mark roadmap entries as done.

## Surprises & discoveries

- Observation: The `thiserror` derive macro's `#[error(...)]` attribute captures
  struct fields for formatting, but Clippy's `unused_assignments` lint does not
  recognize this usage, triggering false-positive warnings for fields only used
  in error messages.
  Evidence: `src/runner/mod.rs` required `#![allow(unused_assignments)]` with an
  explanatory comment referencing upstream issue `rust-lang/rust#130021`.

- Observation: Fluent localization keys must use snake_case to match clap's
  `Arg::id()` output, not kebab-case as initially assumed from the flag names.
  Evidence: `localize_arguments()` generates keys like `cli.flag.fetch_allow_scheme.help`
  from `--fetch-allow-scheme`, requiring snake_case keys in `.ftl` files.

- Observation: No other surprises encountered during implementation.
  Evidence: Implementation matched expectations for error handling, help
  localization, and quickstart documentation.

## Decision log

- Decision: Detect missing manifest at runner level (`generate_ninja()`) rather
  than in the manifest loader. Rationale: The runner has CLI context (directory
  option, file path) needed for constructing helpful directory descriptions in
  error messages. Date/Author: 2026-01-08 (Terry)

- Decision: Use `miette::Diagnostic` with static English messages initially;
  full Fluent integration for runtime errors deferred to roadmap item 3.7.
  Rationale: The existing error infrastructure uses `miette` derives with
  compile-time strings, and Fluent integration for `miette` diagnostics requires
  additional infrastructure not yet in place. Date/Author: 2026-01-08 (Terry)

- Decision: Check file existence with `Path::exists()` before calling
  `fs::read_to_string()`. Rationale: Allows differentiation between "file
  missing" (user-friendly error with hint) vs "file unreadable" (permission
  error, which should surface differently). Date/Author: 2026-01-08 (Terry)

- Decision: Extend `cli_l10n.rs` to localize flag help strings using a
  `localize_arguments()` helper. Rationale: Follows the existing pattern for
  subcommand about text localization and keeps all clap localization logic in
  one module. Date/Author: 2026-01-08 (Terry)

- Decision: Use text processing (not C compilation) for the hello-world example.
  Rationale: Avoids compiler dependencies, making the quickstart portable across
  systems without a C toolchain. Date/Author: 2026-01-08 (Terry)

- Decision: Create `docs/quickstart.md` as a separate tutorial document rather
  than expanding the user guide. Rationale: Keeps the user guide as a reference
  document while providing a focused, step-by-step onboarding path for new
  users. Date/Author: 2026-01-08 (Terry)

## Outcomes & retrospective

- Outcome: All three roadmap items (3.6.1, 3.6.2, 3.6.3) implemented and PR ready
  for review.
  - Default subcommand now validates manifest existence before loading, producing
    a clear error with actionable hint when the manifest is missing.
  - CLI help text fully localized via `localize_arguments()` helper; Spanish
    translations provided alongside English.
  - Quickstart tutorial (`docs/quickstart.md`) and working example
    (`examples/hello-world/`) created for new user onboarding.
  - BDD scenarios added for missing manifest detection and quickstart example
    validation.
  - All quality gates pass (`make check-fmt`, `make lint`, `make test`).

- Design decisions:
  - Manifest existence check placed in `generate_ninja()` at the runner level to
    access CLI context for directory descriptions in error messages.
  - Used `miette::Diagnostic` with static English messages; full Fluent
    integration for runtime errors deferred to roadmap item 3.7.
  - Hello-world example uses text processing (not C compilation) for portability.

- Known limitations:
  - Module-level `#![allow(unused_assignments)]` required to suppress
    false-positive lint caused by thiserror derive macro (tracked upstream at
    rust-lang/rust#130021).
  - Fluent keys must use snake_case to match clap's `Arg::id()` output.

- Follow-up tasks:
  - Track rust-lang/rust#130021 and remove lint suppression when fixed.
  - Consider Fluent integration for `miette` diagnostics in roadmap item 3.7.

## Context and orientation

Key runtime entry points and relevant files:

- `src/main.rs` parses CLI, merges config layers via OrthoConfig, and dispatches
  to `runner::run()`.
- `src/cli.rs` defines the clap CLI with `#[derive(Parser, OrthoConfig)]` and
  default command behaviour via `.with_default_command()`.
- `src/cli_l10n.rs` contains clap localization logic; currently localizes
  subcommand about/long_about but not flag help strings.
- `src/runner/mod.rs` implements subcommand execution; `generate_ninja()`
  (lines 244-258) resolves the manifest path and loads the manifest.
- `src/manifest/mod.rs` contains `from_path_with_policy()` (lines 179-189) which
  reads and parses the manifest file.
- `locales/en-US/messages.ftl` and `locales/es-ES/messages.ftl` contain
  localized CLI messages.
- `tests/features/` contains BDD feature files; `tests/bdd/steps/` contains step
  definitions using `rstest-bdd` v0.3.2.
- `test_support/src/netsuke.rs` provides `run_netsuke_in()` for CLI integration
  tests.
- `examples/` contains 5 existing example manifests (basic_c.yml, photo_edit.yml,
  visual_design.yml, website.yml, writing.yml) but no step-by-step tutorial.

Design expectations are in `docs/netsuke-cli-design-document.md` (Friendly UX
section, lines 29-82). Testing guidance is in
`docs/behavioural-testing-in-rust-with-cucumber.md` and
`docs/rust-testing-with-rstest-fixtures.md`. Documentation style is in
`docs/documentation-style-guide.md` (British English, Oxford comma).

## Plan of work

### 3.6.1 Missing manifest error handling

1. **Define error type.** Add a `ManifestNotFound` variant to the runner's error
   handling in `src/runner/mod.rs`. Use `thiserror::Error` for the error trait
   and `miette::Diagnostic` for rich diagnostics with a `help` attribute. The
   error should capture the manifest name (e.g., `Netsukefile`), directory
   description (e.g., "the current directory" or "directory `/path`"), and the
   attempted path.

2. **Add existence check in `generate_ninja()`.** Before calling
   `manifest::from_path_with_policy()`, check if the resolved manifest path
   exists using `Path::exists()`. If not, return the `ManifestNotFound` error
   with contextual information derived from CLI options.

3. **Add BDD scenarios.** Create `tests/features/missing_manifest.feature` with
   scenarios for:
   - Running `netsuke` in an empty directory (no manifest).
   - Running `netsuke --file nonexistent.yml` with a custom path that does not
     exist.
   - Running `netsuke -C /tmp/empty` in a specified directory without a
     manifest.
   Each scenario should assert the command fails and stderr contains the
   expected error fragments ("No `Netsukefile` found", "--help").

4. **Add unit tests.** Add `rstest` unit tests to verify that `generate_ninja()`
   returns the correct error type when the manifest file is missing.

### 3.6.2 Curate help output

1. **Audit current help text.** Capture `netsuke --help` and each subcommand
   help to identify gaps in flag descriptions.

2. **Extend `localize_command()`.** Add a `localize_arguments()` helper in
   `src/cli_l10n.rs` to iterate over command arguments and replace help strings
   using pattern `cli.flag.{arg_id}.help` for root flags and
   `cli.subcommand.{cmd}.flag.{arg_id}.help` for subcommand flags.

3. **Add Fluent messages.** Add localization keys for all flags to
   `locales/en-US/messages.ftl`:
   - Root flags: `file`, `directory`, `jobs`, `verbose`, `locale`,
     `fetch-allow-scheme`, `fetch-allow-host`, `fetch-block-host`,
     and `fetch-default-deny`
   - Build subcommand: `emit` and `targets`
   - Manifest subcommand: output file argument

4. **Add Spanish translations.** Add corresponding keys to
   `locales/es-ES/messages.ftl`.

5. **Add BDD test.** Verify help output contains expected localized strings for
   both English and Spanish locales.

### 3.6.3 Hello world quickstart

1. **Create quickstart document.** Write `docs/quickstart.md` with:
   - Prerequisites (Netsuke, Ninja)
   - Step 1: Create project directory
   - Step 2: Create minimal Netsukefile (echo command)
   - Step 3: Run netsuke and see output
   - Step 4: Add real build target (text processing)
   - Step 5: Demonstrate vars, glob, foreach
   - Next steps: link to user guide and examples

2. **Create example fixture.** Create `examples/hello-world/` with:
   - `Netsukefile` — working manifest using text processing
   - `input.txt` — sample input file
   - `README.md` — example documentation

3. **Add BDD scenario.** Create `tests/features/quickstart.feature` to exercise
   the example:
   - Copy workspace from `examples/hello-world/`
   - Run `netsuke`
   - Verify command succeeds
   - Verify expected output file exists

4. **Update documentation.** Update `docs/users-guide.md` section 2 ("Getting
   Started") to:
   - Link to quickstart tutorial
   - Document the new missing manifest error message

5. **Run quality gates and mark roadmap.** Run `make check-fmt`, `make lint`,
   and `make test`. Once all pass, mark roadmap items 3.6.1, 3.6.2, and 3.6.3
   as done in `docs/roadmap.md`.

## Concrete steps

All commands are run from the repository root (`/root/repo`). Use `tee` with
`set -o pipefail` to preserve exit codes, as required by `AGENTS.md`.

1. Verify current behaviour (expect generic error):

   ```sh
   set -o pipefail
   mkdir -p /tmp/empty-workspace && cd /tmp/empty-workspace
   cargo run --manifest-path /root/repo/Cargo.toml 2>&1 | tee /tmp/netsuke-missing-before.log
   cd /root/repo
   ```

2. Capture current help output:

   ```sh
   set -o pipefail
   cargo run -- --help 2>&1 | tee /tmp/netsuke-help.log
   cargo run -- build --help 2>&1 | tee /tmp/netsuke-build-help.log
   cargo run -- manifest --help 2>&1 | tee /tmp/netsuke-manifest-help.log
   ```

3. Implement error type, existence check, and flag localization.

4. Add Fluent messages and BDD scenarios.

5. Create quickstart document and example fixture.

6. Format and lint:

   ```sh
   set -o pipefail
   make fmt 2>&1 | tee /tmp/netsuke-fmt.log
   make markdownlint 2>&1 | tee /tmp/netsuke-markdownlint.log
   ```

7. Run quality gates:

   ```sh
   set -o pipefail
   make check-fmt 2>&1 | tee /tmp/netsuke-check-fmt.log
   make lint 2>&1 | tee /tmp/netsuke-lint.log
   make test 2>&1 | tee /tmp/netsuke-test.log
   ```

8. Verify improved behaviour:

   ```sh
   set -o pipefail
   mkdir -p /tmp/empty-workspace && cd /tmp/empty-workspace
   cargo run --manifest-path /root/repo/Cargo.toml 2>&1 | tee /tmp/netsuke-missing-after.log
   cd /root/repo
   ```

## Validation and acceptance

- Running `netsuke` in an empty directory prints:
  `Error: No \`Netsukefile\` found in the current directory.` followed by a hint
  mentioning `--help`.
- Running `netsuke --file custom.yml` where `custom.yml` does not exist prints a
  similar error with the custom filename.
- The `netsuke --help` output shows localized descriptions for all flags.
- Subcommand help (`netsuke build --help`) displays localized flag text.
- Spanish translations appear when invoking `netsuke --locale es-ES --help`.
- `docs/quickstart.md` exists with step-by-step tutorial.
- `examples/hello-world/` contains working example that builds successfully.
- BDD scenarios pass for missing manifest and quickstart example.
- `make check-fmt`, `make lint`, and `make test` all pass.
- `docs/users-guide.md` links to quickstart and documents error behaviour.
- `docs/roadmap.md` items 3.6.1, 3.6.2, 3.6.3 are marked as done.

## Idempotence and recovery

- The existence check and error emission are safe to re-run; they do not modify
  state.
- Flag localization changes are additive and safe to re-run.
- If tests fail mid-run, fix the underlying issue and re-run the same command,
  overwriting the log file to keep evidence current.
- If localization keys conflict with existing entries, rename them with a unique
  prefix.

## Artifacts and notes

Keep the following transcripts for evidence:

- `/tmp/netsuke-missing-before.log` — current (generic) error output.
- `/tmp/netsuke-missing-after.log` — improved error output with hint.
- `/tmp/netsuke-help.log` — help output with localized flag descriptions.
- `/tmp/netsuke-test.log` — test run showing new BDD and unit tests passing.

## Interfaces and dependencies

- **Error type location**: `src/runner/mod.rs` — add `RunnerError` enum (or
  extend existing error handling) with `ManifestNotFound` variant.
- **Detection location**: `src/runner/mod.rs` function `generate_ninja()` — add
  `Path::exists()` check before `manifest::from_path_with_policy()` call.
- **Flag localization**: `src/cli_l10n.rs` — add `localize_arguments()` helper.
- **Localization**: `locales/en-US/messages.ftl` and `locales/es-ES/messages.ftl`
  — add `error-manifest-not-found`, `error-manifest-not-found-hint`, and
  `cli.flag.{arg_id}.help` keys.
- **BDD tests**: `tests/features/missing_manifest.feature` (new file) and
  `tests/features/quickstart.feature` (new file).
- **Step definitions**: Reuse existing steps from `tests/bdd/steps/process.rs`
  (`the command should fail`, `stderr should contain`) and
  `tests/bdd/steps/manifest_command.rs`; may need new step `an empty workspace`.
- **Quickstart**: `docs/quickstart.md` (new file).
- **Example**: `examples/hello-world/` (new directory with Netsukefile,
  input.txt, README.md).
- **Documentation**: `docs/users-guide.md` section 2.
- **Roadmap**: `docs/roadmap.md` items 3.6.1, 3.6.2, 3.6.3.

## Critical files to modify

| File | Change |
|------|--------|
| `src/runner/mod.rs` | Add `ManifestNotFound` error; existence check in `generate_ninja()` |
| `src/cli_l10n.rs` | Add `localize_arguments()` to localize flag help strings |
| `locales/en-US/messages.ftl` | Add error keys + all flag help descriptions |
| `locales/es-ES/messages.ftl` | Add Spanish translations |
| `tests/features/missing_manifest.feature` | **New** — BDD scenarios for missing manifest |
| `tests/features/quickstart.feature` | **New** — BDD scenario exercising hello-world example |
| `docs/quickstart.md` | **New** — step-by-step tutorial |
| `examples/hello-world/Netsukefile` | **New** — minimal working example |
| `examples/hello-world/input.txt` | **New** — sample input |
| `examples/hello-world/README.md` | **New** — example documentation |
| `docs/users-guide.md` | Link to quickstart; document error behaviour |
| `docs/roadmap.md` | Mark 3.6.1, 3.6.2, 3.6.3 as done |

## Revision note (required when editing an ExecPlan)

- 2026-01-08: Initial ExecPlan created for roadmap section 3.6 covering missing
  manifest error handling (3.6.1), curated help output (3.6.2), and Hello World
  quickstart tutorial (3.6.3).

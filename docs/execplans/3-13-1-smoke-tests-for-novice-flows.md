# 3.13.1. Add smoke tests for novice flows

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

## Purpose / big picture

Roadmap item 3.13.1 is about the very first experience a newcomer has with
Netsuke. Today the repository already has focused coverage for CLI parsing,
missing-manifest handling, manifest generation, progress output, and
localization, but those tests are scattered across different suites and do not
yet prove the complete beginner journey end to end. A regression in one of the
entry points could therefore slip through even if the lower-level tests stay
green.

After this work is complete, the repository will have a small, explicit smoke
suite that proves three newcomer-facing flows with the real binary and the real
configuration/help stack:

1. Running `netsuke` with no arguments in a valid workspace succeeds as a first
   run.
2. Running `netsuke` in an empty workspace fails with the documented guided
   message and `--file` remediation hint.
3. Running `netsuke --help` and `netsuke help` produces newcomer-friendly help
   output through the localized clap/OrthoConfig path.

Observable success means a developer can run a focused smoke slice and see that
the journey described in `docs/users-guide.md` and
`docs/netsuke-cli-design-document.md` still matches the actual CLI.

## Constraints

- Keep this task scoped to smoke coverage for novice journeys. Do not fold in
  broader documentation work from roadmap items 3.13.2 or 3.13.3.
- Use the existing `Cli` type as the OrthoConfig merge root. Do not introduce a
  parallel configuration or help-rendering path just for tests.
- Exercise localized help through the real binary or the existing
  `localize_command()` integration. Do not test a bespoke string builder.
- Add `rstest`-based tests for focused command-level assertions and
  `rstest-bdd` v0.5.0 scenarios for behavioural coverage.
- Reuse the existing BDD fixtures and step infrastructure where practical:
  `tests/bdd/fixtures/mod.rs`, `tests/bdd/steps/manifest_command.rs`, and
  `tests/bdd/steps/progress_output.rs` already cover most of the required
  workspace and process plumbing.
- Keep assertions smoke-level. Prefer high-signal fragments and ordering checks
  over brittle full-output snapshots. Roadmap 3.12.2 already owns reporter
  snapshot coverage.
- Normalize Fluent bidi isolate markers in any test that inspects rendered
  localized output, using `test_support::fluent::normalize_fluent_isolates` or
  the shared BDD assertion helpers.
- Keep files under the 400-line cap from `AGENTS.md`. If a new test module or
  step module grows too large, split it rather than extending an oversized file.
- Record any testing-strategy decisions in the design documentation. This task
  should update `docs/netsuke-design.md` or
  `docs/netsuke-cli-design-document.md` with the final decision about how
  novice-flow UX is validated.
- Update `docs/users-guide.md` if behaviour or wording visible to end users
  changes while aligning runtime output with the documented journey.
- Mark roadmap item 3.13.1 done in `docs/roadmap.md` only after all validation
  gates pass.

## Tolerances

- Scope: if the implementation grows beyond 10 changed files or roughly 500 net
  new lines, stop and reassess before continuing.
- Dependencies: if the work appears to require any new crate dependency, stop
  and escalate. The existing test stack is sufficient.
- Behaviour drift: if the runtime behaviour for first-run success, missing
  manifest wording, or help copy materially disagrees with both
  `docs/users-guide.md` and `docs/netsuke-cli-design-document.md`, stop to
  decide which source is authoritative before writing final assertions.
- Help brittleness: if clap line wrapping or terminal-width behaviour makes the
  help assertions unstable across runs, reduce the assertions to stable
  fragments and ordering checks rather than snapshotting the entire screen.
- Environment isolation: if the smoke suite cannot be made deterministic with
  the existing fake-ninja and `NINJA_ENV` helpers after two focused attempts,
  stop and document the blocker.

## Risks

- Risk: the current missing-manifest runtime text may not exactly match the user
  guide examples. Mitigation: begin by auditing the live output against
  `docs/users-guide.md:61-75` and `docs/netsuke-cli-design-document.md:34-67`.
  If wording differs, decide whether the code or the docs should move, then
  make the smoke tests assert the final documented wording.

- Risk: `run_netsuke_in()` clears `PATH`, so a first-run success scenario will
  fail unless the test deliberately provides `ninja`. Mitigation: reuse the
  existing fake-ninja installation pattern from
  `tests/bdd/steps/progress_output.rs` for BDD scenarios and the existing
  `path_with_fake_ninja()` helper pattern in `tests/assert_cmd_tests.rs` for
  `rstest` integration tests.

- Risk: help output is wrapped by clap and can change if assertions are too
  strict. Mitigation: assert stable fragments such as command names and
  one-line descriptions, plus relative ordering where useful, rather than exact
  spacing or complete text blocks.

- Risk: `.feature` edits may not trigger recompilation in the BDD test binary.
  Mitigation: `touch tests/bdd_tests.rs` before the final `make test` run.

- Risk: localized output may contain invisible Fluent isolate markers that break
  naïve substring assertions. Mitigation: normalize before comparison,
  following the pattern already used in `tests/bdd/helpers/assertions.rs` and
  `tests/cli_tests/locale.rs`.

## Progress

- [x] 2026-03-29: Reviewed roadmap item 3.13.1, the execplans skill, project
      memory, and the referenced testing/configuration/design documents.
- [x] 2026-03-29: Audited existing beginner-facing coverage in
      `tests/features/missing_manifest.feature`, `tests/features/cli.feature`,
      `tests/features/progress_output.feature`, `tests/assert_cmd_tests.rs`,
      and `tests/runner_tests.rs`.
- [x] 2026-03-29: Drafted this ExecPlan.
- [x] 2026-03-30: Stage A completed. Captured live output for empty-workspace
      failure, `--help`, `help`, and first-run success with fake Ninja-backed
      command execution; aligned the documented missing-manifest wording with
      the runtime contract.
- [x] 2026-03-30: Stage B completed via `tests/novice_flow_smoke_tests.rs`
      covering first run, missing manifest guidance, both help entry points,
      and localized Spanish help.
- [x] 2026-03-30: Stage C completed via
      `tests/features/novice_flows.feature`, reusing the existing BDD
      workspace and fake-Ninja plumbing.
- [x] 2026-03-30: Stage D completed by updating
      `docs/users-guide.md`, `docs/netsuke-cli-design-document.md`, and
      `docs/roadmap.md`.
- [x] 2026-03-30: Stage E completed. Ran `make fmt`, `make check-fmt`,
      `make lint`, `make test`, `make markdownlint`, and `make nixie`, all
      with logged output.

## Surprises & Discoveries

- The repo already has the core process fixtures needed for this task.
  `tests/bdd/steps/manifest_command.rs` provides `a minimal Netsuke workspace`,
  `an empty workspace`, `netsuke is run without arguments`, generic stdout and
  stderr assertions, and file-existence checks.
- The existing missing-manifest BDD feature only checks broad fragments such as
  `"not found in"` and `"Ensure the manifest exists"`. It does not yet prove
  that the UX matches the exact newcomer-facing wording shown in the user guide
  and CLI design document.
- `test_support::netsuke::run_netsuke_in()` deliberately clears `PATH`, so any
  success path that invokes `build` must install fake `ninja` support or use a
  helper that sets `NINJA_ENV`.
- The current repository already uses `rstest-bdd = "0.5.0"`.
- It also already enables strict compile-time validation for
  `rstest-bdd-macros`, so this task can follow the current BDD guidance
  directly without a migration step.
- The live missing-manifest contract has moved on from the older
  “No `Netsukefile` found … / Hint: Run `netsuke --help` …” copy still present
  in the docs. The runtime now renders
  `Manifest 'Netsukefile' not found in the current directory.` plus
  `help: Ensure the manifest exists or pass \`--file\` with the correct path.`
  The documentation and smoke suite should pin that newer wording.

## Decision Log

- Decision: validate the novice journey with both `rstest` integration tests
  and `rstest-bdd` scenarios. Rationale: the `rstest` layer gives fast,
  targeted feedback for command-level smoke cases, while the BDD layer
  documents the beginner journey in plain language and proves the real binary
  behaviour end to end. Date/Author: 2026-03-29 / Codex.

- Decision: keep help assertions fragment-based rather than introducing help
  snapshots. Rationale: clap wrapping is not the behaviour under test here. The
  roadmap item asks for smoke coverage of the novice journey, so the tests
  should pin the presence of the important commands and guidance without
  overfitting to whitespace. Date/Author: 2026-03-29 / Codex.

- Decision: treat the documented journey in `docs/users-guide.md` and
  `docs/netsuke-cli-design-document.md` as the acceptance target, then update
  the runtime or the docs so they agree before finalizing tests. Rationale:
  roadmap sub-bullet 3.13.1 explicitly says to confirm the UX matches the
  documented journey. The smoke suite should therefore assert the final
  documented contract, not just the current implementation accident.
  Date/Author: 2026-03-29 / Codex.

- Decision: route novice help coverage through the existing OrthoConfig/clap
  localization surface instead of testing parser-only helpers. Rationale:
  roadmap 3.13.1 is about user-visible help, and the task explicitly calls for
  using `ortho_config` for ergonomic layered configuration with localized help
  support. The tests must therefore exercise the same help/localization path
  that end users hit. Date/Author: 2026-03-29 / Codex.

- Decision: treat the current runtime missing-manifest message as
  authoritative and update the docs to match it. The smoke suite now pins
  `Manifest 'Netsukefile' not found in the current directory.` together with
  `Ensure the manifest exists or pass \`--file\` with the correct path.`
  Rationale: the current message is more specific than the older docs copy and
  reflects the localized diagnostic/help split users actually see. Date/Author:
  2026-03-30 / Codex.

## Outcomes & Retrospective

- Implemented a dedicated `rstest` smoke target in
  `tests/novice_flow_smoke_tests.rs` and a matching behavioural feature in
  `tests/features/novice_flows.feature`.
- Collected behavioural evidence with focused runs of
  `cargo test --test novice_flow_smoke_tests` and
  `cargo test --test bdd_tests novice_flows`; both passed with the new
  assertions.
- Updated the user guide and CLI design document to match the live
  missing-manifest wording, and recorded the decision to keep novice-flow UX
  protected by smoke coverage through the real binary/help stack.
- Marked roadmap item 3.13.1 complete once the implementation and targeted
  evidence were in place.
- Validation completed with the full repository gates plus the documentation
  checks required for Markdown changes.

## Context and orientation

The following repository areas matter for this task.

### Runtime and help-generation paths

- `src/cli/mod.rs`
  - `Cli::with_default_command()` currently converts a missing command into
    `Commands::Build(BuildArgs { emit: None, targets: Vec::new() })`.
  - This is the core behaviour behind the “run `netsuke` with no arguments”
    first-run story.
- `src/cli_l10n.rs`
  - `localize_command()` overrides usage, about text, and argument help, then
    localizes subcommands.
  - This is the user-visible help surface the smoke tests should exercise.
- `src/runner/mod.rs`
  - `generate_ninja()` resolves the manifest path and calls
    `ensure_manifest_exists_or_error(...)` before manifest loading.
  - This is the key code path behind the guided missing-manifest UX.

### Existing binary-test helpers

- `test_support/src/netsuke.rs`
  - `run_netsuke_in()` launches the compiled binary in a working directory and
    captures stdout, stderr, and exit status.
  - It clears `PATH`, so tests that use `build` must provide `ninja`
    deliberately.
- `tests/assert_cmd_tests.rs`
  - Already contains end-to-end command tests using `assert_cmd`,
    `path_with_fake_ninja()`, and temporary workspaces.
  - This is the most natural place either to extend the existing smoke coverage
    or to copy the helper pattern into a new focused smoke module.
- `tests/runner_tests.rs`
  - Already exercises runner behaviour with `rstest`, fake `ninja`, and
    temporary manifests.
  - It is suitable for direct assertions on missing-manifest and successful
    default-build behaviour if the implementation prefers runner-level tests.

### Existing BDD infrastructure

- `tests/features/missing_manifest.feature`
  - Covers missing-manifest failure today, but only with broad fragment checks.
- `tests/features/cli.feature`
  - Covers parser-level default-command and invalid-argument behaviour, but not
    the real binary journey.
- `tests/features/progress_output.feature`
  - Already proves real-binary output and fake-ninja integration patterns.
- `tests/bdd/steps/manifest_command.rs`
  - Reusable steps for `a minimal Netsuke workspace`, `an empty workspace`,
    `netsuke is run without arguments`, `netsuke is run with arguments ...`,
    and generic stdout/stderr assertions.
- `tests/bdd/steps/progress_output.rs`
  - Reusable fake-ninja installation helpers, including the `NINJA_ENV`
    strategy required for `build` smoke scenarios.
- `tests/bdd/steps/mod.rs`
  - Must be updated if a new `novice_flows` step module is introduced.
- `tests/bdd_tests.rs`
  - The BDD test harness that should be `touch`ed after `.feature` edits.

### Documentation that defines the acceptance contract

- `docs/users-guide.md:29-75`
  - Describes the basic first run, missing-manifest guidance, and quick-start
    link.
- `docs/netsuke-cli-design-document.md:34-67`
  - Defines the first-run success story, helpful missing-manifest message,
    newcomer-friendly help output, and the expectation that defaults are
    welcoming.
- `docs/netsuke-design.md`
  - Should record the final testing/design decision taken during this work.
- `docs/roadmap.md`
  - Contains the roadmap checkbox that must be marked done only after all
    validation succeeds.

## Plan of work

### Stage A. Audit and freeze the documented novice journey

Start by comparing the current runtime behaviour with the two documentation
sources above.

1. Capture the current outputs for:
   - `netsuke` in an empty workspace,
   - `netsuke --help`,
   - `netsuke help`,
   - `netsuke` in a minimal workspace with fake `ninja`.
2. Decide which fragments are stable enough to treat as the documented contract
   for smoke tests.
3. If the runtime wording differs from the user guide or CLI design document,
   align them before finalizing tests. Do not leave the smoke suite asserting
   undocumented or stale text.

Acceptance for Stage A:

- There is a short written note in the `Decision Log` describing which wording
  and help fragments are now authoritative.

### Stage B. Add focused `rstest` smoke coverage

Add a small `rstest` suite for fast feedback. Prefer one new focused test file,
such as `tests/novice_flow_smoke_tests.rs`, unless the existing
`tests/assert_cmd_tests.rs` or `tests/runner_tests.rs` remains comfortably
under the size limit after extension.

The suite should include:

1. `first_run_without_args_succeeds_in_minimal_workspace`
   - Create a temp workspace from `tests/data/minimal.yml`.
   - Provide fake `ninja`.
   - Run bare `netsuke`.
   - Assert success.
   - Assert at least one high-signal beginner-visible cue, such as the default
     build path succeeding without an explicit subcommand.
2. `missing_manifest_error_matches_documented_guidance`
   - Run bare `netsuke` in an empty workspace.
   - Assert failure.
   - Assert the final documented error and `--file` remediation hint fragments.
3. `help_entry_points_are_novice_friendly`
   - Parameterize with `rstest` over `["--help"]` and `["help"]`.
   - Assert success.
   - Assert that the help output mentions the core commands (`build`, `clean`,
     `graph`, `manifest`) and the newcomer-facing build description.
4. `localized_help_still_flows_through_cli_localization`
   - Use `--locale es-ES --help` or the equivalent supported path.
   - Assert at least one stable localized fragment.
   - This keeps the novice help coverage aligned with the OrthoConfig/clap
     localization path rather than English-only strings.

Implementation notes:

- Normalize localized output before substring matching.
- Reuse the fake-ninja PATH helper pattern already present in
  `tests/assert_cmd_tests.rs`.
- Prefer helper functions for repeated workspace setup and output assertions.

Acceptance for Stage B:

- The targeted `rstest` suite fails before the implementation is complete and
  passes after the behaviour/docs are aligned.

### Stage C. Add behavioural smoke scenarios with `rstest-bdd`

Add one feature file dedicated to the beginner journey, for example
`tests/features/novice_flows.feature`.

Use three core scenarios:

1. `First run succeeds in a minimal workspace`
   - Given a minimal Netsuke workspace
   - And fake `ninja` is available
   - When netsuke is run without arguments
   - Then the command should succeed
   - And the visible output should show a small number of stable beginner cues
2. `Missing manifest shows guided failure`
   - Given an empty workspace
   - When netsuke is run without arguments
   - Then the command should fail
   - And stderr should contain the final documented error fragment
   - And stderr should contain the `--file` remediation hint
3. `Help output matches the documented journey`
   - When netsuke is run with arguments `"--help"`
   - Then the command should succeed
   - And stdout should contain the core commands and beginner-facing summary

Optional fourth scenario if the help path needs explicit coverage:

1. `The help subcommand matches the flag form`
   - When netsuke is run with arguments `"help"`
   - Then the command should succeed
   - And stdout should contain the same command fragments as `--help`

Reuse existing steps where possible. Only add a new step module if one of the
following is required:

- a reusable fake-ninja setup step for bare `netsuke` smoke scenarios,
- ordered stderr assertions,
- grouped assertions for the documented beginner journey.

If a new module is needed, add it as `tests/bdd/steps/novice_flows.rs` and wire
it into `tests/bdd/steps/mod.rs`.

Acceptance for Stage C:

- A newcomer can read the feature file alone and understand the intended first
  experience with Netsuke.
- The scenarios run via the normal `tests/bdd_tests.rs` harness.

### Stage D. Update design docs, user docs, and roadmap

Make the documentation reflect the final contract and testing strategy.

1. Update `docs/users-guide.md` if the missing-manifest wording, first-run
   guidance, or help wording visible to end users changed.
2. Update `docs/netsuke-design.md` or
   `docs/netsuke-cli-design-document.md` with the testing/design decision that
   novice-flow UX is protected by smoke tests covering first-run success,
   missing manifest, and help entry points.
3. Mark roadmap item 3.13.1 done in `docs/roadmap.md`.

Acceptance for Stage D:

- The docs and the live CLI no longer disagree about the beginner journey.

### Stage E. Validation and evidence

Run focused checks first, then the full gates. Because this repo auto-commits
each turn, do not stop after targeted tests.

Suggested focused commands:

```sh
set -o pipefail
cargo test --test novice_flow_smoke_tests 2>&1 | tee /tmp/novice_flow_smoke_tests.log
```

```sh
set -o pipefail
touch tests/bdd_tests.rs
cargo test --test bdd_tests novice_flows 2>&1 | tee /tmp/novice_flows_bdd.log
```

If the smoke tests extend existing files instead of creating a new test target,
replace the focused command with the relevant target name or filter.

Then run the required repository gates:

```sh
set -o pipefail
make check-fmt 2>&1 | tee /tmp/make-check-fmt.log
```

```sh
set -o pipefail
make lint 2>&1 | tee /tmp/make-lint.log
```

```sh
set -o pipefail
make test 2>&1 | tee /tmp/make-test.log
```

Because this task edits Markdown, also run the documentation gates required by
`AGENTS.md`:

```sh
set -o pipefail
make fmt 2>&1 | tee /tmp/make-fmt.log
```

```sh
set -o pipefail
make markdownlint 2>&1 | tee /tmp/make-markdownlint.log
```

```sh
set -o pipefail
make nixie 2>&1 | tee /tmp/make-nixie.log
```

After `make fmt`, inspect `git diff` and restore any incidental wrap-only
changes in unrelated Markdown files before finalizing.

## Concrete acceptance evidence

The completed implementation should let a reviewer verify the task with the
following observable checks.

### First run success

Running bare `netsuke` in a minimal workspace with fake `ninja` should succeed
without requiring the user to name a subcommand explicitly.

### Missing manifest guidance

Running bare `netsuke` in an empty workspace should fail with the documented
guided message and a next-step hint that points the user to `--help`.

### Help output

Running `netsuke --help` and `netsuke help` should succeed and show the main
commands a novice needs to discover:

- `build`
- `clean`
- `graph`
- `manifest`

If localized help is covered in the final implementation, the localized smoke
assertion should also pass for the chosen stable fragment.

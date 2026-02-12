# Integrate `indicatif::MultiProgress` for six-stage feedback (roadmap 3.9.1)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: IN PROGRESS

No `PLANS.md` file exists in this repository.

## Purpose / big picture

Netsuke currently reports static stage lines only in accessible mode and emits
no progress in standard mode. Roadmap item 3.9.1 requires standard-mode
real-time feedback using `indicatif::MultiProgress`, with six pipeline stages,
persistent stage summaries, and localized labels.

After this change:

- Standard mode can display six persistent stage summaries managed by
  `indicatif::MultiProgress`.
- Accessible mode remains static, textual, and screen-reader friendly.
- Stage labels and status words are localized via Fluent.
- Progress behaviour is configurable through OrthoConfig layering (config
  file, environment variable, CLI), with localized help text.

Observable success:

- Running `netsuke --progress true manifest -` in a valid workspace emits
  six stage summary lines on stderr and the Ninja manifest on stdout.
- Running with `--locale es-ES` localizes stage labels.
- Running with `--accessible true` keeps static textual stage reporting.
- `make check-fmt`, `make lint`, and `make test` pass.

## Constraints

- Keep the pipeline model aligned with the six-stage build flow documented in
  `docs/netsuke-design.md`: Manifest Ingestion, Initial YAML Parsing, Template
  Expansion, Deserialisation & Final Rendering, IR Generation & Validation, and
  Ninja Synthesis & Execution.
- Preserve accessible mode guarantees from roadmap 3.8.1.
- Integrate progress configuration with OrthoConfig conventions and localized
  CLI help.
- Localize all newly introduced user-facing strings in both:
  `locales/en-US/messages.ftl` and `locales/es-ES/messages.ftl`.
- Add/adjust unit tests using `rstest`.
- Add/adjust behavioural tests using `rstest-bdd` v0.5.0.
- Cover happy paths, unhappy paths, and edge cases.
- Update `docs/users-guide.md` for user-visible behaviour changes.
- Record implementation decisions in the design document.
- Mark roadmap entry 3.9.1 as done only when implementation and validation are
  complete.
- Run quality gates before finalizing:
  `make check-fmt`, `make lint`, and `make test`.

## Tolerances (exception triggers)

- Scope: if the implementation requires more than ~18 touched files or
  ~1000 net new lines, stop and confirm scope before continuing.
- API compatibility: if any existing public API must break, stop and escalate.
- Output ordering: if `MultiProgress` causes irreconcilable stream corruption
  with subprocess output, stop and escalate with alternatives.
- Test stability: if progress rendering tests remain flaky after two
  stabilization attempts, stop and escalate with deterministic fallback options.
- Localization drift: if Fluent key expansion requires broad unrelated
  catalogue refactors, stop and isolate the minimal path.

## Risks

- Risk: Netsuke currently reports five stages in `src/status.rs`, not six.
  Mitigation: introduce a six-stage canonical enum and route all reporters
  through it; add tests asserting stage count and order.

- Risk: Manifest pipeline internals are currently collapsed in
  `manifest::from_str_named`, making fine-grained stage reporting difficult.
  Mitigation: refactor manifest loading into explicit pipeline steps and report
  boundaries from runner-controlled orchestration.

- Risk: `indicatif` redraw and subprocess stderr output can interleave.
  Mitigation: keep stage progress updates coarse-grained (stage transitions),
  avoid high-frequency spinner ticks, and test stderr behaviour with stripped
  ANSI assertions.

- Risk: Non-interactive test environments can hide or alter progress drawing.
  Mitigation: add explicit progress config (`--progress`) so behavioural tests
  can force deterministic progress mode without relying on TTY auto-detection.

- Risk: Fluent key additions can miss one locale and fail build-time audits.
  Mitigation: add keys in both locales together and include localization smoke
  tests for new progress messages.

## Progress

- [x] 2026-02-12: Gathered context from roadmap, design docs, status/runner
      modules, localization files, and existing BDD/unit test surfaces.
- [x] 2026-02-12: Drafted this ExecPlan in
      `docs/execplans/3-9-1-integrate-indicatif-multi-progress.md`.
- [x] Implement six-stage progress model and `indicatif::MultiProgress`
      standard reporter.
- [x] Add OrthoConfig-backed progress configuration and localized help.
- [ ] Add unit tests (`rstest`) for reporter logic, stage mapping, and config
      precedence.
- [ ] Add behavioural tests (`rstest-bdd` v0.5.0) for standard/accessible,
      localized, and failure paths.
- [ ] Update `docs/users-guide.md` and design document decisions.
- [ ] Mark roadmap item 3.9.1 done.
- [ ] Run and pass required quality gates:
      `make check-fmt`, `make lint`, `make test`.

## Surprises & Discoveries

- The current status pipeline is explicitly five stages in `src/status.rs`,
  while the design and roadmap target six-stage user-facing reporting.
- `manifest::from_path_with_policy` currently performs multiple pipeline steps
  inside one function, so stage-level reporting needs extraction.
- Standard mode currently uses `SilentReporter`; no `indicatif` dependency is
  present.
- No project-memory MCP resources were available in this environment during
  planning, so repository docs were used as the authoritative source.
- The runtime `manifest` command path needed an explicit completion call after
  synthesis; otherwise an in-progress stage was finalized as failed in the new
  reporter drop path.

## Decision Log

- Decision: adopt the six-stage user-facing model from `docs/netsuke-design.md`
  for progress reporting, even if internal implementation details differ.
  Rationale: roadmap 3.9.1 explicitly asks to surface six stages, and the
  design document already establishes those stages as the user mental model.
  Date/Author: 2026-02-12 / Codex.

- Decision: keep accessible mode as the non-animated fallback path and do not
  regress existing static output semantics. Rationale: roadmap 3.8.1 and
  accessibility requirements remain hard invariants. Date/Author: 2026-02-12 /
  Codex.

- Decision: introduce an OrthoConfig-managed `progress` setting to control
  progress rendering ergonomically via config/env/CLI, with localized help.
  Rationale: this satisfies the explicit requirement to use OrthoConfig and
  provides deterministic behavioural testing in non-TTY environments.
  Date/Author: 2026-02-12 / Codex.

## Outcomes & Retrospective

Pending implementation.

## Context and orientation

Primary implementation surfaces:

- `src/status.rs`: progress abstractions, stage enum, localized descriptions.
- `src/runner/mod.rs`: reporter selection and stage transition calls.
- `src/manifest/mod.rs`: manifest pipeline decomposition for stage boundaries.
- `src/runner/process/mod.rs`: subprocess output streaming interaction surface.
- `src/cli/mod.rs`: OrthoConfig-backed CLI fields and merge filtering.
- `src/cli_l10n.rs`: localized help key mapping.
- `src/localization/keys.rs`: Fluent key constants.
- `locales/en-US/messages.ftl`, `locales/es-ES/messages.ftl`: localized copy.
- `tests/`: unit, integration, and BDD coverage.
- `docs/users-guide.md`: user-visible output/configuration documentation.
- `docs/netsuke-design.md`: design decision updates.
- `docs/roadmap.md`: task completion checkbox updates.

Useful existing patterns:

- `OutputMode::resolve_with` in `src/output_mode.rs` for dependency-injected
  detection.
- Existing `StatusReporter` abstraction in `src/status.rs`.
- BDD command execution and stderr assertions in
  `tests/bdd/steps/manifest_command.rs`.
- Localized CLI help integration in `src/cli_l10n.rs`.

## Plan of work

## Stage A: Add progress configuration through OrthoConfig

Extend CLI configuration to include a progress toggle that follows layered
precedence and localized help text.

Planned changes:

- Add `progress: Option<bool>` to `Cli` in `src/cli/mod.rs`.
- Ensure `Cli::default()` initializes `progress` to `None`.
- Include `progress` in `cli_overrides_from_matches` command-line source
  filtering.
- Add localized flag help key (for example `cli.flag.progress.help`) in:
  `src/localization/keys.rs`, `locales/en-US/messages.ftl`,
  `locales/es-ES/messages.ftl`, and `src/cli_l10n.rs`.
- Update CLI parsing/merge tests to verify OrthoConfig precedence and explicit
  override behaviour.

Acceptance for Stage A:

- CLI accepts `--progress true|false`.
- `NETSUKE_PROGRESS` and config file `progress = true/false` merge correctly.
- Help text is localized for the new option.

## Stage B: Refactor reporting model to six pipeline stages

Replace five-stage reporting with six canonical stages and explicit stage
status transitions.

Planned changes:

- Update `PipelineStage` in `src/status.rs` to six variants aligned with
  `docs/netsuke-design.md`.
- Introduce stage status semantics (`pending`, `running`, `done`, `failed`)
  for persistent summaries.
- Add localized keys for new stage labels/status words as needed.
- Ensure a single source of truth for stage count and ordering.

Acceptance for Stage B:

- Stage count constant is six and verified by unit tests.
- Stage descriptions are localized and deterministic.

## Stage C: Integrate `indicatif::MultiProgress` standard reporter

Implement a standard reporter backed by `indicatif::MultiProgress`, keeping
lines persistent as stages complete.

Planned changes:

- Add `indicatif` dependency to `Cargo.toml` (caret requirement).
- Replace `SilentReporter` usage in standard mode with a new reporter (for
  example `IndicatifReporter`) that: creates six stage lines up front, updates
  stage state transitions, keeps summaries visible after completion, marks
  failure when a stage errors.
- Keep `AccessibleReporter` intact for accessible mode.
- Select reporter in `runner::run` using resolved output mode plus progress
  config.

Acceptance for Stage C:

- Standard mode shows six persistent summary lines.
- Accessible mode remains static text and does not animate.
- Failure paths mark the failing stage clearly.

## Stage D: Expose true six-stage boundaries in runner + manifest

Refactor manifest loading/orchestration so stage transitions match real
pipeline boundaries.

Planned changes:

- Split manifest loading flow in `src/manifest/mod.rs` into explicit steps that
  can be reported individually.
- Update `src/runner/mod.rs` to report stage transitions at the boundary of
  each of the six stages.
- Ensure stage 6 covers synthesis + execution semantics for the active command
  (`build`, `clean`, `graph`, `manifest`) with localized wording.

Acceptance for Stage D:

- Stage transitions occur in the documented order.
- Early failures stop at the correct stage and preserve prior summaries.

## Stage E: Unit tests with `rstest`

Add focused unit coverage for stage modelling, reporter transitions, and
configuration precedence.

Planned test additions:

- `src/status.rs` or `src/runner/tests.rs`:
  stage count/order assertions, localized stage-label resolution checks,
  reporter transition happy and failure-path tests.
- `tests/cli_tests/*`:
  parse + merge cases for `progress` from defaults, config, env, and CLI.
- `tests/localization_tests.rs`:
  resolve newly introduced progress/stage messages in `en-US` and `es-ES`.

Key edge cases:

- `accessible=true` with `progress=true` (accessible must win).
- `progress=false` in standard mode (no multi-progress output).
- failure before stage 6 still yields persistent summaries for completed
  stages.

## Stage F: Behavioural tests with `rstest-bdd` v0.5.0

Add BDD scenarios that exercise real command execution and user-visible output
contracts.

Planned changes:

- Add `tests/features/progress_output.feature`.
- Add `tests/bdd/steps/progress_output.rs` and register it in
  `tests/bdd/steps/mod.rs`.
- Reuse existing command-run steps/helpers for capturing stdout/stderr.

Planned scenarios:

- Happy path: standard mode with progress enabled emits six stage summaries.
- Happy path localized: `--locale es-ES --progress true` emits Spanish labels.
- Unhappy path: invalid manifest reports failure and marks the current stage.
- Edge case: accessible mode overrides standard progress rendering.
- Edge case: progress explicitly disabled suppresses standard progress UI.

## Stage G: Documentation and roadmap updates

Synchronize user and design documentation with implemented behaviour.

Planned changes:

- Update `docs/users-guide.md`:
  standard-mode progress behaviour, six stages, persistent summaries,
  `--progress` / `NETSUKE_PROGRESS` / config key usage and precedence.
- Update design decisions in `docs/netsuke-design.md` to record chosen
  architecture/trade-offs for six-stage reporting and reporter selection.
- Mark roadmap item 3.9.1 and its sub-bullets done in `docs/roadmap.md` once
  implementation/tests are complete.

## Validation and evidence capture

Run all required gates with `tee` and `pipefail`:

    set -o pipefail
    make check-fmt 2>&1 | tee /tmp/3-9-1-check-fmt.log
    make lint 2>&1 | tee /tmp/3-9-1-lint.log
    make test 2>&1 | tee /tmp/3-9-1-test.log

Documentation gates after doc updates:

    set -o pipefail
    make fmt 2>&1 | tee /tmp/3-9-1-fmt.log
    make markdownlint 2>&1 | tee /tmp/3-9-1-markdownlint.log
    make nixie 2>&1 | tee /tmp/3-9-1-nixie.log

Record concise evidence in commit/PR notes:

- command exit codes,
- relevant new/updated test names,
- stderr snippets demonstrating six-stage persistent summaries,
- locale-specific output proof points.

## Rollback / recovery strategy

- If `indicatif` integration causes severe output regressions, retain
  `AccessibleReporter` and a guarded standard fallback path (`progress=false`)
  while keeping stage model and localization changes.
- If pipeline-stage refactor introduces functional regressions, land progress
  reporter scaffolding first behind standard-mode gating, then complete stage
  boundary refactor in a follow-up atomic change.

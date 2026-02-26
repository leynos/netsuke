# Capture per-stage timing metrics in verbose mode (roadmap 3.9.3)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DRAFT (2026-02-25)

No `PLANS.md` file exists in this repository.

## Purpose / big picture

Roadmap item `3.9.3` requires Netsuke to capture per-stage timing metrics and
include them in the completion summary when verbose mode is enabled, while
keeping default output quiet.

After this change:

- `--verbose` enables a timing summary at completion, with per-stage durations
  for the six pipeline stages.
- Default output (without `--verbose`) remains unchanged and does not add
  timing noise.
- The behaviour is still configured through OrthoConfig layering (defaults <
  config file < environment < CLI), reusing `verbose` and localized help
  surfaces.

Observable success:

- Running `netsuke --verbose manifest -` prints the standard completion line
  plus a stage timing summary.
- Running `netsuke manifest -` (without `--verbose`) does not print timing
  lines.
- `make check-fmt`, `make lint`, and `make test` pass.

Expected verbose completion output (illustrative):

```text
# stdout
rule cc
  command = cc -c $in -o $out
...

# stderr
[done] Stage 6/6: Synthesizing Ninja plan and executing Build
Build complete.
Stage timing summary:
- Stage 1/6 Reading manifest file: 12ms
- Stage 2/6 Parsing YAML document: 4ms
- Stage 3/6 Expanding template directives: 7ms
- Stage 4/6 Deserializing and rendering manifest values: 6ms
- Stage 5/6 Building and validating dependency graph: 3ms
- Stage 6/6 Synthesizing Ninja plan and executing Build: 18ms
Total pipeline time: 50ms
```

The summary lines are emitted on `stderr` with other status output. `stdout`
continues to carry command artefacts (for example `manifest -` output) and is
not used for timing diagnostics.

## Constraints

- Implement roadmap item `3.9.3` only:
  capture per-stage timing metrics in verbose mode.
- Include timing metrics in completion summary output.
- Keep default output quiet (no new timing lines unless verbose mode is on).
- On failed runs, suppress timing summary lines entirely (including partially
  collected stage timings) to avoid implying successful completion.
- Reuse OrthoConfig layering; do not add ad-hoc configuration reads.
- Use localized help support for any updated verbose/help wording.
- Localize all new user-facing runtime strings in both:
  `locales/en-US/messages.ftl` and `locales/es-ES/messages.ftl`.
- Preserve existing progress behaviour:
  `--progress false` must still suppress stage/task progress updates.
- Keep accessibility guarantees:
  accessible mode remains static and text-first.
- Add unit tests with `rstest` covering happy, unhappy, and edge paths.
- Add behavioural tests with `rstest-bdd` v0.5.0 covering happy, unhappy, and
  edge paths.
- Record design decisions in `docs/netsuke-design.md`.
- Update `docs/users-guide.md` for user-visible behaviour.
- Mark roadmap item `3.9.3` done in `docs/roadmap.md` only after all gates
  pass.
- Run required quality gates with logged output:
  `make check-fmt`, `make lint`, and `make test`.

## Tolerances (exception triggers)

- Scope: if implementation exceeds 14 touched files or 800 net new lines,
  stop and escalate.
- Public interfaces: if this requires breaking a public API signature, stop
  and escalate with alternatives.
- Configuration: if implementing this requires a new top-level CLI/config key
  (instead of using existing `verbose` layering), stop and escalate.
- Test determinism: if reliable timing assertions cannot be achieved without
  sleeps after two attempts, stop and escalate with a deterministic design.
- File size: if any edited Rust file exceeds 400 lines, split into focused
  submodules before continuing.

## Risks

- Risk: Wall-clock timing can make tests flaky.
  Mitigation: isolate timing capture behind an injectable clock and test with a
  deterministic fake clock.

- Risk: `src/status.rs` is already near the 400-line limit.
  Mitigation: place timing-specific logic in a new focused module (for example
  `src/status_timing.rs`) and keep `status.rs` orchestration-focused.

- Risk: completion summary formatting can diverge between standard and
  accessible reporters. Mitigation: centralize timing-summary rendering in one
  helper and reuse it for all reporters.

- Risk: behaviour-driven development (BDD) step matching may become ambiguous
  (known `rstest-bdd` sharp edge). Mitigation: use unambiguous step text and
  avoid generic patterns that can overlap existing steps.

- Risk: Fluent bidi isolation characters can make brittle string assertions.
  Mitigation: normalize isolates in unit assertions and use stable substrings
  in behavioural assertions.

## Progress

- [x] (2026-02-25 00:00Z) Reviewed roadmap `3.9.3`, current status/reporter
      architecture, runner stage boundaries, and localization surfaces.
- [x] (2026-02-25 00:00Z) Drafted this ExecPlan at
      `docs/execplans/3-9-3-per-stage-timing-metrics.md`.
- [ ] Stage A: design and implement timing recorder/wrapper.
- [ ] Stage B: wire verbose-gated completion summary output.
- [ ] Stage C: add localization and localized verbose/help copy updates.
- [ ] Stage D: add unit + behavioural coverage.
- [ ] Stage E: update docs/design/roadmap and run quality gates.

## Surprises & Discoveries

- `runner::run` always reports stage transitions through `StatusReporter`, but
  completion messages are emitted only on successful paths (`report_complete`).
  Failed flows currently rely on reporter drop logic for failed-stage marking.
- `src/status.rs` is 381 lines; adding timing logic there directly is likely to
  exceed repository file-size guidance.
- Existing progress BDD scenarios already cover standard/accessibility/progress
  toggles and are the right extension point for verbose timing behaviour.

## Decision Log

- Decision: reuse existing `Cli.verbose` (OrthoConfig-managed) as the sole
  feature gate for timing summaries. Rationale: satisfies layered config
  ergonomics and avoids extra flags. Date/Author: 2026-02-25 / Codex.

- Decision: keep timing output in completion summary only (verbose mode), not
  as per-stage streaming output. Rationale: roadmap requires metrics in
  completion summary and explicitly avoiding noise in default output.
  Date/Author: 2026-02-25 / Codex.

- Decision: suppress all timing summary lines on failure, even when verbose
  mode is enabled. Rationale: a completion-oriented summary on failed runs is
  misleading; failure details should remain focused on the failed stage/error.
  Date/Author: 2026-02-25 / Codex.

- Decision: prefer deterministic duration formatting with existing std library
  types and no new dependency. Rationale: reduces churn and keeps tests stable
  and straightforward. Date/Author: 2026-02-25 / Codex.

## Outcomes & Retrospective

Not implemented yet. This section must be updated with outcomes, test evidence,
and lessons learned after execution.

## Context and orientation

Primary implementation surfaces:

- `src/status.rs`: reporter trait and concrete status reporters.
- `src/status_pipeline.rs`: canonical six-stage order and labels.
- `src/runner/mod.rs`: reporter construction and pipeline orchestration.
- `src/cli/mod.rs`: OrthoConfig-derived `verbose` configuration.
- `src/cli_l10n.rs`: localized clap help key mapping.
- `src/localization/keys.rs`: Fluent key constants.
- `locales/en-US/messages.ftl` and `locales/es-ES/messages.ftl`: localized
  runtime/help strings.
- `src/status_tests.rs`: status reporter unit tests.
- `tests/features/progress_output.feature` and
  `tests/bdd/steps/progress_output.rs`: behavioural progress output coverage.
- `tests/localization_tests.rs`: localization key resolution coverage.
- `docs/users-guide.md`: user-visible CLI behaviour documentation.
- `docs/netsuke-design.md`: design rationale and decisions.
- `docs/roadmap.md`: implementation status tracking.

Relevant existing behaviour:

- Six pipeline stages are emitted through `report_pipeline_stage(...)`.
- Stage 6 task progress is already parsed from Ninja status lines.
- Progress rendering can be disabled with `--progress false`.
- `--verbose` currently raises tracing verbosity and should become the timing
  summary gate as well.

## Plan of work

### Stage A: Introduce deterministic per-stage timing capture

Implement timing capture that starts/stops on stage transitions and final
completion, without changing default output behaviour.

Planned changes:

- Add a focused timing module (for example `src/status_timing.rs`) containing:
  - a stage timing recorder state machine,
  - deterministic duration formatter,
  - injectable clock abstraction for tests.
- Add a reporter wrapper (for example `VerboseTimingReporter`) that:
  - delegates to an inner `StatusReporter`,
  - records elapsed time per stage when `report_stage(...)` is called,
  - finalizes the current stage on `report_complete(...)`,
  - exposes rendered summary lines to output path logic.
- Keep implementation additive and avoid breaking existing reporter trait
  callers.

Acceptance for Stage A:

- Recorder correctly measures stage boundaries in order.
- Unit tests verify deterministic elapsed durations using fake time.

### Stage B: Emit timing metrics in verbose completion summary only

Wire timing summary output so it appears when verbose is enabled and remains
silent by default.

Planned changes:

- Update `runner::make_reporter(...)` to accept a verbose flag and wrap the
  selected base reporter with timing support when `cli.verbose` is true.
- Ensure behaviour by mode:
  - non-verbose: unchanged output (no timing summary),
  - verbose: completion summary includes per-stage timings.
- Keep `--progress false` semantics:
  stage/task progress remains suppressed; verbose timing summary may still be
  shown at completion as verbose diagnostics.
- Preserve stream discipline:
  emit summary on stderr in the same channel as other status messages.
- Define failure contract:
  failed runs do not emit timing summary lines (partial or total).

Acceptance for Stage B:

- `--verbose` reliably adds timing summary on successful completion.
- Default output remains unchanged when `--verbose` is absent.
- Failed runs suppress timing summary output entirely.

### Stage C: Localization and OrthoConfig/localized-help updates

Add localized copy for new timing summary lines and align verbose help copy
with the expanded behaviour.

Planned changes:

- Add new Fluent keys for timing summary header/line formatting in:
  - `src/localization/keys.rs`,
  - `locales/en-US/messages.ftl`,
  - `locales/es-ES/messages.ftl`.
- Keep existing key audit compatibility (`build.rs` Fluent key checks).
- Update `cli.flag.verbose.help` copy in both locales to mention that verbose
  mode includes timing metrics in completion output.
- Keep clap localization mapping in `src/cli_l10n.rs` aligned with key names.

Acceptance for Stage C:

- New/updated messages resolve in `en-US` and `es-ES`.
- Localized help output remains functional and consistent with runtime
  behaviour.

### Stage D: Validation with `rstest` and `rstest-bdd` v0.5.0

Add comprehensive tests for happy, unhappy, and edge conditions.

Planned unit tests (`rstest`):

- `src/status_tests.rs` (or new timing test module):
  - happy path: stage durations are captured and rendered in stage order,
  - unhappy path: incomplete/failure flows do not panic and emit no timing
    summary,
  - edge path: non-verbose mode omits timing summary entirely.
- `tests/localization_tests.rs`:
  - verify new timing message keys resolve for both locales.

Planned behavioural tests (`rstest-bdd` v0.5.0):

- Extend `tests/features/progress_output.feature` with scenarios:
  - happy path: `--verbose` includes timing summary in completion output,
  - unhappy path: command failure emits no timing summary lines,
  - edge path: default (non-verbose) runs do not print timing summary.
- Extend `tests/bdd/steps/progress_output.rs` only as needed for assertions.
- Keep step phrases specific to avoid pattern ambiguity.

Acceptance for Stage D:

- New tests are deterministic and pass locally without time-based flakiness.

### Stage E: Documentation, design log, roadmap, and gates

Update docs and close roadmap item only after all tests pass.

Planned changes:

- Update `docs/users-guide.md`:
  - describe verbose timing summary behaviour,
  - clarify that default output stays unchanged.
- Update `docs/netsuke-design.md` Section 8.4:
  - record timing-capture design,
  - document verbose gating and output rationale.
- Mark roadmap item `3.9.3` as done in `docs/roadmap.md` once complete.

Validation commands (required):

```sh
set -o pipefail
make check-fmt 2>&1 | tee /tmp/3-9-3-check-fmt.log
make lint 2>&1 | tee /tmp/3-9-3-lint.log
make test 2>&1 | tee /tmp/3-9-3-test.log
```

Acceptance for Stage E:

- Docs, design, and roadmap are synchronized with implementation.
- All three quality gates pass.

## Validation and acceptance checklist

- Per-stage timing metrics are captured for all six stages.
- Timing metrics appear in completion summary when verbose mode is enabled.
- Default output does not gain timing noise.
- OrthoConfig layering continues to control verbose behaviour
  (`verbose` in config/env/CLI).
- Localized help and runtime timing messages resolve in `en-US` and `es-ES`.
- `rstest` unit tests and `rstest-bdd` behavioural tests cover happy, unhappy,
  and edge paths.
- `make check-fmt`, `make lint`, and `make test` succeed.
- `docs/users-guide.md`, `docs/netsuke-design.md`, and `docs/roadmap.md` are
  updated.

## Idempotence and recovery

- The timing recorder and summary rendering are additive; rerunning tests should
  be deterministic.
- If timing output causes noisy logs, run without `--verbose` to keep baseline
  output while debugging.
- If integration with current reporters proves too invasive, fall back to a
  wrapper-based design that composes around existing reporters instead of
  rewriting them.

## Approval gate

This document is ready for review. Implementation should begin only after
explicit user approval of this plan.

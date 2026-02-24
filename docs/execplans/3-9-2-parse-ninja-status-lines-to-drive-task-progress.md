# Parse Ninja status lines to drive task progress (roadmap 3.9.2)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DONE (2026-02-24)

No `PLANS.md` file exists in this repository.

## Purpose / big picture

Netsuke currently reports six pipeline stages, but Stage 6 does not yet derive
task-level progress from Ninja execution lines. Roadmap item 3.9.2 requires
Ninja status-line parsing, so Netsuke can present task progress during build
execution. The same milestone also requires deterministic textual fallback
updates when stdout is not a teletype terminal (TTY) and whenever accessible
mode is active.

After this change:

- Stage 6 task progress is driven by parsed Ninja status lines.
- Standard interactive mode keeps the existing `indicatif`-based UX, now with
  task counters.
- Non-interactive stdout and accessible mode emit textual task updates.
- Progress control continues through OrthoConfig layering with localized help
  copy (`--progress`, `NETSUKE_PROGRESS`, config file key).

Observable success:

- Running `netsuke --accessible false --progress true build` against a manifest
  that executes multiple Ninja edges shows Stage 6 task advancement.
- Running the same command with stdout redirected still emits textual task
  updates suitable for logs and continuous integration (CI).
- Running with `--accessible true` emits text-only task updates (no animated
  redraw dependency).
- `make check-fmt`, `make lint`, and `make test` pass.

## Constraints

- Implement roadmap item `3.9.2` only: parse Ninja status lines for Stage 6
  task progress plus fallback textual updates.
- Preserve the six-stage model already implemented in `src/status_pipeline.rs`.
- Keep accessible mode text-first and non-animated.
- Use OrthoConfig layering for user control; do not introduce ad-hoc config
  reads. Existing `progress: Option<bool>` remains the controlling switch.
- Keep command-line interface (CLI) help localization wired through Fluent and
  `src/cli_l10n.rs`.
- Maintain public behaviour compatibility for existing commands.
- Add unit tests using `rstest`.
- Add behaviour-driven development (BDD) tests using `rstest-bdd` v0.5.0.
- Cover happy path, unhappy path, and edge conditions.
- Record final design decisions in `docs/netsuke-design.md`.
- Update `docs/users-guide.md` with user-visible changes.
- Mark roadmap item `3.9.2` done only after implementation and validation.
- Run required quality gates with logged output:
  `make check-fmt`, `make lint`, `make test`.

## Tolerances (exception triggers)

- Scope: if implementation needs more than 16 files or 900 net new lines,
  stop and escalate.
- API compatibility: if a public function signature must break (for example
  `runner::run_ninja`), stop and escalate with alternatives.
- Configuration: if this milestone requires a new top-level CLI/config field
  instead of extending `progress`, stop and confirm.
- Output contracts: if preserving subprocess stream behaviour conflicts with
  progress parsing, stop and escalate before altering semantics.
- Tests: if deterministic coverage cannot be achieved after three iterations,
  stop and escalate with specific flake evidence.
- File-size guardrail: if any edited source file would exceed 400 lines, split
  into focused submodules before proceeding.

## Risks

- Risk: Ninja status lines vary by output settings and may be absent or
  malformed. Severity: high Likelihood: medium Mitigation: parse
  conservatively, ignore malformed lines, and keep reporter state monotonic.

- Risk: status parsing and raw output forwarding can interfere with stream
  ordering. Severity: high Likelihood: medium Mitigation: parse in a tee-style
  forwarding path that preserves original bytes and writes unchanged child
  output.

- Risk: fallback mode criteria (`stdout` TTY, accessible mode, hidden draw
  target) can diverge across environments. Severity: medium Likelihood: high
  Mitigation: centralize fallback predicate and cover it with parameterized
  tests using dependency-injected terminal capability inputs.

- Risk: BDD tests that rely on host Ninja binaries can be flaky in CI.
  Severity: medium Likelihood: high Mitigation: run behavioural scenarios with
  deterministic fake Ninja fixtures that emit controlled status lines.

## Progress

- [x] (2026-02-22 17:55Z) Reviewed roadmap, current status/reporter pipeline,
      process streaming code, OrthoConfig integration, and existing test
      surfaces.
- [x] (2026-02-22 17:55Z) Drafted this ExecPlan at
      `docs/execplans/3-9-2-parse-ninja-status-lines-to-drive-task-progress.md`.
- [x] (2026-02-24) Stage A: Added Ninja status parser and forwarding hooks via
      `src/runner/process/ninja_status.rs` and
      `src/runner/process/streaming.rs`; process orchestration in
      `src/runner/process/mod.rs` now accepts an internal status observer.
- [x] (2026-02-24) Stage B: Wired parsed task progress into reporters by
      extending `StatusReporter` with `report_task_progress` and implementing
      task-progress rendering in `IndicatifReporter` and
      `AccessibleReporter`.
- [x] (2026-02-24) Stage C: Added centralized fallback behaviour in
      `src/runner/mod.rs` (`should_force_text_task_updates`) to emit textual
      task updates when stdout is non-TTY or accessible mode is enabled.
- [x] (2026-02-24) Stage D: Added/updated unit and behavioural coverage using
      `rstest` and `rstest-bdd`:
      `src/runner/process/ninja_status.rs`,
      `src/runner/process/streaming.rs`, `src/status_tests.rs`,
      `src/runner/tests.rs`, `tests/features/progress_output.feature`, and
      `tests/bdd/steps/progress_output.rs`.
- [x] (2026-02-24) Stage E: Updated docs and roadmap:
      `docs/users-guide.md`, `docs/netsuke-design.md`, `docs/roadmap.md`;
      validated with `make check-fmt`, `make lint`, and `make test`.

## Surprises & discoveries

- `src/runner/process/mod.rs` is already 421 lines, so adding parser logic in
  that file directly would violate project file-size guidance. This work should
  extract logic into new submodules rather than grow `mod.rs`.
- Current BDD progress scenarios (`tests/features/progress_output.feature`) use
  `manifest -`, which does not invoke Ninja. New scenarios must exercise build
  execution with controlled Ninja output.
- Model Context Protocol (MCP) project-memory tools (`qdrant-find` /
  `qdrant-store`) were not available in this environment; repository docs were
  used as the only source of truth.
- Fluent rendering includes bidi isolation markers in localized strings, so
  brittle literal assertions were replaced with content assertions that strip
  isolation code points in unit tests.
- The initial fake-Ninja fixture shell script used `cat`, which failed under
  test PATH constraints. Replaced with shell built-ins (`read` + `printf`) to
  keep fixtures deterministic.

## Decision log

- Decision: parse bracketed Ninja status tokens of the form `[current/total]`
  at line start for Stage 6 progress, treating malformed lines as non-events.
  Rationale: this is the documented, stable surface from Ninja output and keeps
  parser complexity low. Date/Author: 2026-02-22 / Codex.

- Decision: use the existing OrthoConfig-backed `progress` setting as the sole
  control for stage and task progress output, and update localized help text to
  reflect the expanded behaviour. Rationale: avoids unnecessary configuration
  sprawl while satisfying layered config and localization requirements.
  Date/Author: 2026-02-22 / Codex.

- Decision: emit explicit textual task updates whenever stdout is non-TTY or
  accessible mode is active, regardless of `indicatif` draw target details.
  Rationale: roadmap requirement is about deterministic accessibility/log
  readability and must not depend solely on terminal redraw support.
  Date/Author: 2026-02-22 / Codex.

## Outcomes & retrospective

Implemented outcomes:

- Stage 6 task progress now advances from parsed Ninja status lines in
  `[current/total] description` form, with monotonic filtering to ignore
  malformed and regressive updates.
- Non-TTY stdout and accessible mode now force deterministic textual task
  updates while preserving standard `indicatif` stage rendering elsewhere.
- Unit + BDD coverage now includes parser happy/unhappy cases, monotonic guard
  behaviour, fallback predicate combinations, and behavioural scenarios using
  deterministic fake Ninja output.
- Documentation and roadmap were synchronized:
  `docs/users-guide.md`, `docs/netsuke-design.md`, `docs/roadmap.md`.

Validation summary:

- `make check-fmt`: pass.
- `make lint`: pass.
- `make test`: pass.
- Additional doc gates run after doc updates:
  `make fmt`, `make markdownlint`, `make nixie`.

## Context and orientation

Primary implementation surfaces:

- `src/runner/process/mod.rs`: child process spawning and output forwarding.
- `src/runner/mod.rs`: reporter construction and pipeline orchestration.
- `src/status.rs`: reporter trait and implementations.
- `src/status_pipeline.rs`: six-stage canonical ordering and labels.
- `src/cli/mod.rs`: OrthoConfig-derived CLI configuration.
- `src/cli_l10n.rs`: localized clap help mapping.
- `src/localization/keys.rs`: Fluent message key constants.
- `locales/en-US/messages.ftl` and `locales/es-ES/messages.ftl`: localized
  user-facing strings.
- `tests/features/progress_output.feature` and `tests/bdd/steps/*`: behavioural
  test coverage.
- `src/status_tests.rs` and `src/runner/process/*tests*`: unit coverage.
- `docs/users-guide.md`: user-visible behaviour documentation.
- `docs/netsuke-design.md`: design decisions and rationale.
- `docs/roadmap.md`: implementation status tracking.

Terminology used in this plan:

- Ninja status line: the leading progress token Ninja emits per edge, typically
  `[current/total]`.
- Stage 6: `PipelineStage::NinjaSynthesisAndExecution`.
- Textual fallback: non-animated, line-oriented progress output suitable for
  screen readers, CI logs, and redirected output.

## Plan of work

### Stage A: parser and process plumbing

Create a dedicated Ninja status parser module and integrate it with subprocess
output forwarding in a non-disruptive way.

Planned edits:

- Add `src/runner/process/ninja_status.rs` with:
  - a parsed update type (for example `NinjaTaskProgress`),
  - `parse_ninja_status_line(&str) -> Option<NinjaTaskProgress>`,
  - monotonic update guard logic to avoid regressions.
- Extract/reshape forwarding code from `src/runner/process/mod.rs` so file size
  does not grow beyond guidance.
- Add an internal observer hook for parsed status updates while preserving the
  current public `run_ninja` API contract.

Validation gate:

- Unit tests for parser happy/unhappy cases pass before wiring reporter logic.

### Stage B: reporter integration for Stage 6 task progress

Connect parsed status updates to status reporting so Stage 6 reflects
task-level advancement.

Planned edits:

- Extend `StatusReporter` in `src/status.rs` with a task-progress method
  (default no-op for compatibility).
- Implement task-progress handling in:
  - `IndicatifReporter` (interactive summaries),
  - `AccessibleReporter` (text-first updates),
  - `SilentReporter` (no output).
- Ensure Stage 6 state remains coherent (`running` -> `done`/`failed`) when
  task updates are present or absent.

Validation gate:

- Unit tests verify no reporter panics on malformed/duplicate/regressive
  updates and that state transitions remain correct.

### Stage C: fallback textual updates and OrthoConfig-aligned behaviour

Implement deterministic fallback rules and ensure layered configuration still
controls progress ergonomically.

Planned edits:

- Centralize fallback predicate:
  - fallback when stdout is not a TTY, or
  - accessible mode is active.
- Update localized help copy for `progress` to clarify that it controls stage
  and task progress summaries.
- Keep precedence through OrthoConfig (`config < env < CLI`) with existing
  `progress: Option<bool>` semantics.

Validation gate:

- CLI merge/parsing tests still pass; explicit `--progress false` suppresses
  task progress in all modes.

### Stage D: tests with `rstest` and `rstest-bdd` v0.5.0

Add focused unit and behavioural coverage for happy, unhappy, and edge paths.

Planned unit tests (`rstest`):

- Parser coverage in `src/runner/process/ninja_status.rs` tests:
  - valid bracketed lines,
  - malformed lines,
  - zero/overflow/regression handling,
  - duplicate progress updates.
- Reporter coverage in `src/status_tests.rs`:
  - Stage 6 task update formatting,
  - fallback predicate combinations,
  - failure-path finalization with partial task progress.

Planned behavioural tests (`rstest-bdd` v0.5.0):

- Extend `tests/features/progress_output.feature` with build-execution
  scenarios that use fake Ninja output.
- Add `tests/bdd/steps/progress_output.rs` (new module) to keep
  `tests/bdd/steps/manifest_command.rs` under 400 lines and to isolate progress
  fixtures/steps.
- Include scenarios for:
  - happy path: parsed updates advance task progress,
  - happy path accessible: textual updates appear in accessible mode,
  - unhappy path: malformed status lines are ignored safely,
  - edge case: `--progress false` suppresses progress updates.

Validation gate:

- New BDD scenarios are deterministic and do not depend on host Ninja.

### Stage E: documentation, design record, roadmap, and gates

Synchronize docs and complete quality validation.

Planned edits:

- Update `docs/users-guide.md`:
  - Stage 6 task progress behaviour,
  - fallback textual updates in non-TTY and accessible flows,
  - `progress` configuration semantics.
- Update `docs/netsuke-design.md` (Section 8.4 decisions) with parser approach,
  fallback rules, and configuration rationale.
- Mark roadmap item `3.9.2` and its fallback sub-bullet done in
  `docs/roadmap.md` once all tests and gates pass.

Validation gate:

- `make check-fmt`, `make lint`, and `make test` all succeed.

## Interfaces and dependencies

The implementation should remain dependency-neutral (no new external crates).
Use existing crates already in `Cargo.toml`:

- `indicatif` for standard mode rendering.
- `ortho_config` for layered configuration and localized help integration.
- `rstest` and `rstest-bdd` v0.5.0 for unit and behavioural tests.

Expected new internal interfaces (names may vary, behaviour is required):

- A parser function in `src/runner/process/ninja_status.rs` that converts a
  single output line into an optional task-progress event.
- A process-layer hook that receives parsed task-progress events without
  changing emitted child output bytes.
- A status-reporter method for Stage 6 task updates with safe default no-op
  semantics for reporters that do not display progress.

## Concrete steps

Run all commands from the repository root.

1. Implement parser module and parser unit tests.
2. Wire parser into process forwarding path with observer callbacks.
3. Extend reporter trait/implementations for task-progress updates.
4. Implement fallback predicate and update localized `progress` help strings.
5. Add/extend BDD scenarios and steps using fake Ninja emitters.
6. Update `docs/users-guide.md`, `docs/netsuke-design.md`, and
   `docs/roadmap.md`.
7. Run quality gates with `tee` logs:

    ```sh
    set -o pipefail
    make check-fmt 2>&1 | tee /tmp/3-9-2-check-fmt.log
    make lint 2>&1 | tee /tmp/3-9-2-lint.log
    make test 2>&1 | tee /tmp/3-9-2-test.log
    ```

8. Run documentation gates after doc edits:

    ```sh
    set -o pipefail
    make fmt 2>&1 | tee /tmp/3-9-2-fmt.log
    make markdownlint 2>&1 | tee /tmp/3-9-2-markdownlint.log
    make nixie 2>&1 | tee /tmp/3-9-2-nixie.log
    ```

## Validation and acceptance

Acceptance requires all of the following:

- Parsed Ninja status lines produce Stage 6 task progress updates.
- When stdout is non-TTY, fallback textual task updates are emitted.
- When accessible mode is active, fallback textual task updates are emitted.
- Malformed or missing Ninja status lines do not panic and do not corrupt
  overall stage completion/failure reporting.
- `progress = false` disables stage/task progress output consistently across
  config file, env (`NETSUKE_PROGRESS`), and CLI (`--progress false`).
- Unit tests (`rstest`) and behavioural tests (`rstest-bdd` v0.5.0) cover
  happy, unhappy, and edge conditions.
- `make check-fmt`, `make lint`, and `make test` succeed.
- `docs/users-guide.md`, `docs/netsuke-design.md`, and `docs/roadmap.md` are
  updated and consistent.

## Idempotence and recovery

- Parser and reporter changes are additive and re-runnable; repeated test runs
  should produce the same output assertions.
- If status parsing causes regressions, retain raw output forwarding and gate
  parser callbacks behind `progress` while investigating.
- If fallback behaviour causes noisy logs, disable task updates via
  `progress = false` temporarily and continue with non-progress execution while
  preserving deterministic command output.

## Artefacts and notes

Capture concise evidence for review:

- Parser unit test names and pass/fail status.
- BDD scenario names proving fallback behaviour.
- A short stderr excerpt showing Stage 6 textual updates in non-TTY mode.
- Command exit codes for `make check-fmt`, `make lint`, and `make test`.

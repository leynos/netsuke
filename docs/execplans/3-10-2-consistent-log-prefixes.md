# 3.10.2. Introduce consistent prefixes for log differentiation

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

## Purpose / big picture

Netsuke output currently lacks visual consistency across its three output
channels: accessible text reporter, indicatif progress bars, and verbose timing
summaries. Specifically:

- `AccessibleReporter` prefixes completion with `Success:` but stage
  announcements and task progress have no semantic prefix.
- `IndicatifReporter` emits completion as a bare message (no prefix).
- `VerboseTimingReporter` emits timing summaries with no distinguishing
  prefix.

This task introduces localizable, emoji-aware prefixes so every output channel
uses consistent visual markers for status information, stage progress, and
timing summaries. The `--no-emoji` flag (already implemented) controls whether
Unicode glyphs or plain ASCII labels are used.

After this change:

1. Stage announcements are prefixed with an info marker.
2. Completion messages are consistently prefixed across all reporters.
3. Timing summary headers use a timing-specific prefix.
4. Task progress lines use indentation to show hierarchy under stage output.

## Constraints

Hard invariants that must hold throughout implementation:

- **No new CLI flags**: Reuse the existing `--no-emoji` mechanism via
  `OutputPrefs`.
- **No functional regressions**: All existing Behaviour-Driven Development
  (BDD) scenarios must pass.
- **Localization**: All new prefixes must be defined in Fluent `.ftl` files
  for both `en-US` and `es-ES` locales.
- **File size limit**: No source file may exceed 400 lines.
- **AGENTS.md compliance**: All changes must pass `make check-fmt`,
  `make lint`, and `make test`.
- **Existing API surface**: The `StatusReporter` trait must maintain its
  existing public signatures.

## Tolerances (exception triggers)

- **Scope**: If implementation requires changes to more than 12 files or 400
  lines of code (net), stop and escalate.
- **Dependencies**: If a new external dependency is required, stop and
  escalate.
- **Iterations**: If tests still fail after 3 implementation attempts, stop
  and escalate.

## Risks

- Risk: Existing BDD scenarios assert exact output strings that will change
  with prefix additions. Severity: low Likelihood: high Mitigation: Update BDD
  assertions to include the new prefixes.

- Risk: Indicatif progress bar rendering may clip long prefixed lines.
  Severity: low Likelihood: low Mitigation: Keep prefixes short (2-3 characters
  - label).

## Progress

- [x] Stage A: Add Fluent localization keys and messages
- [x] Stage B: Extend `OutputPrefs` with new prefix methods
- [x] Stage C: Integrate prefixes into reporters
- [x] Stage D: Add/update tests
- [x] Stage E: Update documentation and roadmap

## Surprises & discoveries

- All existing BDD assertions used substring `contains` matching, so adding
  prefixes to output lines did not break any existing scenarios. Only scenario
  titles and a few new assertions needed updating.
- The `IndicatifReporter` needed `OutputPrefs` added to its constructor and
  all downstream call sites (runner, tests) to support the success prefix.

## Decision log

- **2026-03-05**: Decided on three new semantic prefixes:
  - `semantic.prefix.info` — for stage announcements (Unicode: `ℹ`, ASCII:
    `Info:`)
  - `semantic.prefix.timing` — for timing summaries (Unicode: `⏱`, ASCII:
    `Timing:`)
  - Completion messages already use `semantic.prefix.success` — extend
    usage to `IndicatifReporter`.
- **2026-03-05**: Task progress lines will be indented with two spaces to
  show hierarchy under their parent stage. No prefix needed — indentation
  provides sufficient visual grouping.

## Outcomes & retrospective

**Outcome**: SUCCESS

All quality gates pass on first attempt. The implementation introduces
consistent, localizable, emoji-aware prefixes across all three output channels
without breaking any existing functionality.

**What went well**:

- The existing `OutputPrefs` pattern (emoji-gated Fluent select expressions)
  was easily extensible to new prefix types.
- Substring-based BDD assertions meant minimal test churn.
- Parallel implementation across all three reporters was straightforward.

**Artefacts produced**:

- `src/localization/keys.rs`: Two new keys (`SEMANTIC_PREFIX_INFO`,
  `SEMANTIC_PREFIX_TIMING`)
- `locales/en-US/messages.ftl`: Two new Fluent messages
- `locales/es-ES/messages.ftl`: Two new Fluent messages (Spanish)
- `src/output_prefs.rs`: Two new methods (`info_prefix`, `timing_prefix`)
  with unit tests
- `src/status.rs`: `AccessibleReporter` uses info prefix;
  `IndicatifReporter` accepts `OutputPrefs` and uses success prefix
- `src/status_timing.rs`: `VerboseTimingReporter` accepts `OutputPrefs` and
  uses timing prefix with indented detail lines
- `src/runner/mod.rs`: Updated reporter factory
- `tests/features/progress_output.feature`: Updated and expanded scenarios
- `docs/roadmap.md`: Marked 3.10.2 as complete

## Context and orientation

### Key files and modules

1. `src/output_prefs.rs` — `OutputPrefs` struct, emoji resolution, prefix
   methods.
2. `src/status.rs` — `StatusReporter` trait, `AccessibleReporter`,
   `IndicatifReporter`, `SilentReporter`.
3. `src/status_timing.rs` — `VerboseTimingReporter` wrapper.
4. `src/localization/keys.rs` — Fluent message key constants.
5. `locales/en-US/messages.ftl` — English locale messages.
6. `locales/es-ES/messages.ftl` — Spanish locale messages.
7. `tests/features/progress_output.feature` — BDD progress scenarios.
8. `src/runner/mod.rs` — Reporter factory (`make_reporter()`).

### New prefix design

| Channel                    | Current output                 | After change                        |
| -------------------------- | ------------------------------ | ----------------------------------- |
| Stage (accessible)         | `Stage 1/6: ...`               | `ℹ Info: Stage 1/6: ...`            |
| Stage (indicatif, hidden)  | `[pending] Stage 1/6: ...`     | unchanged (summary format)          |
| Task progress (accessible) | `Task 1/2: ...`                | two-space indent before task label  |
| Completion (accessible)    | `✔ Success: Build complete.`   | unchanged                           |
| Completion (indicatif)     | `Build complete.`              | `✔ Success: Build complete.`        |
| Timing header              | `Stage timing summary:`        | `⏱ Timing: Stage timing summary:`   |
| Timing stage line          | `- Stage 1/6: ...`             | two-space indent before stage line  |
| Timing total line          | `Total pipeline time: ...`     | two-space indent before total line  |

## Plan of work

### Stage A: Add Fluent localization keys and messages

1. Add `SEMANTIC_PREFIX_INFO` and `SEMANTIC_PREFIX_TIMING` to
   `src/localization/keys.rs`.
2. Add corresponding messages to `locales/en-US/messages.ftl` and
   `locales/es-ES/messages.ftl`.

### Stage B: Extend `OutputPrefs` with new prefix methods

1. Add `info_prefix()` and `timing_prefix()` methods to `OutputPrefs`.
2. Add unit tests for the new methods in the existing test module.

### Stage C: Integrate prefixes into reporters

1. **`AccessibleReporter`**: Pass `OutputPrefs` to `report_stage()` and
   prepend `info_prefix()`. Indent task progress with two spaces.
2. **`IndicatifReporter`**: Pass `OutputPrefs` to constructor; use
   `success_prefix()` in `report_complete()`.
3. **`VerboseTimingReporter`**: Accept `OutputPrefs`; use `timing_prefix()`
   for the summary header, indent stage and total lines with two spaces.

### Stage D: Add/update tests

1. Update existing BDD scenarios that assert completion messages to include
   the `Success:` prefix for standard mode.
2. Add new BDD scenario verifying info prefix on accessible stage output.
3. Update unit tests for timing summary formatting.

### Stage E: Documentation and roadmap

1. Mark 3.10.2 as complete in `docs/roadmap.md`.
2. Run full quality gates.

### Validation gates

At each stage transition:

1. `make check-fmt` must pass.
2. `make lint` must pass.
3. `make test` must pass.

## Concrete steps

See "Plan of work" above for step-by-step guidance.

## Validation and acceptance

Quality criteria:

- **Tests**: All tests pass including updated prefix assertions.
- **Lint/typecheck**: `make check-fmt` and `make lint` pass.
- **Localization**: Both en-US and es-ES FTL files contain the new keys.

Observable behaviour after completion:

1. `netsuke --accessible true manifest -` shows `Info:` prefixed stage lines.
2. `netsuke --progress true manifest -` shows `Success:` prefixed completion.
3. `netsuke --verbose manifest -` shows `Timing:` prefixed timing header.
4. `netsuke --no-emoji true manifest -` shows ASCII-only prefixes.

## Idempotence and recovery

All stages are repeatable without side effects. If a stage fails:

1. Identify failing tests with `make test 2>&1 | tee test.log`.
2. Review test output for specific failures.
3. If within tolerances, fix and retry.
4. If tolerances exceeded, escalate with documented context.

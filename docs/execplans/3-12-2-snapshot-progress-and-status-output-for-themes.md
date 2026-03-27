# 3.12.2. Snapshot progress and status output for themes

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

## Purpose / big picture

Netsuke's theme system (roadmap 3.12.1) introduced tokenized design tokens for
symbols, spacing, and colours, and wired them through the reporter stack. The
reporters (`AccessibleReporter`, `IndicatifReporter`, `VerboseTimingReporter`)
now draw prefixes and indentation from a resolved `OutputPrefs` value, and unit
tests assert individual prefixes and formatting helpers. However, there is no
automated guarantee that the **full rendered output** of each reporter stays
stable across code changes. A small drift in indentation width, a stray blank
line, or a missing prefix could break visual alignment for users without any
existing test failing.

This change introduces `insta` snapshot tests that capture the complete,
multi-line output of progress and status rendering for both the Unicode and
ASCII themes. After the work is complete:

1. Each reporter's rendering path has at least one snapshot per theme that
   pins its exact output, so any drift in alignment, wrapping, or prefix
   formatting fails the build.
2. Snapshot tests cover the AccessibleReporter (stage announcements, task
   progress, and completion), the VerboseTimingReporter (timing summaries), and
   the output_prefs prefix/spacing accessors.
3. BDD scenarios confirm end-to-end snapshot-level output correctness for both
   `--theme unicode` and `--theme ascii` via the real binary, guarding against
   regressions in the integration between theme resolution, localization, and
   reporter rendering.
4. `make check-fmt`, `make lint`, and `make test` pass.

Observable success means a developer can change reporter formatting and receive
an immediate, diff-friendly failure showing exactly which lines shifted rather
than hunting through substring assertions.

## Constraints

- Do not modify the user-visible output format itself. This plan adds tests
  that pin the current rendering; it does not change how output looks.
- Do not add new crate dependencies. `insta` (with the `yaml` feature) is
  already a dev-dependency in `Cargo.toml`.
- Preserve the existing test infrastructure. New snapshot tests must
  co-exist with the existing `rstest` unit tests and `rstest-bdd` BDD scenarios
  without changing their assertions or structure.
- Keep the 400-line file limit per AGENTS.md. If a new test file approaches
  this limit, split it into a sibling module.
- Do not suppress lints. If Clippy raises new warnings, fix them in the code.
- The Fluent localization system produces invisible bidi isolate markers
  (`U+2068`/`U+2069`). Snapshot tests must normalize these markers before
  comparing, using the existing
  `test_support::fluent::normalize_fluent_isolates` helper. This ensures
  snapshots contain only human-visible characters and remain stable across
  Fluent versions.
- All snapshot names must be descriptive and stable. Use the pattern
  `<reporter>_<theme>_<scenario>` (e.g.,
  `accessible_unicode_stage_announcement`).
- Use en-US locale for all snapshot tests. Locale-dependent rendering is
  already validated by existing BDD scenarios; snapshot tests should pin a
  single locale to keep snapshots deterministic.
- Update `docs/users-guide.md` only if user-facing behaviour changes. Since
  this plan adds regression tests without changing behaviour, the user guide
  update will be limited to noting the existence of snapshot-based regression
  guards in the theme documentation section if appropriate.
- Mark roadmap item 3.12.2 done in `docs/roadmap.md` only after the full
  implementation and validation are complete.

## Tolerances (exception triggers)

- Scope: if implementation requires more than 12 files changed or more than
  600 net new lines, stop and escalate.
- Dependencies: if a new crate dependency is required beyond the existing
  `insta`, `rstest`, `rstest-bdd`, and `test_support`, stop and escalate.
- Interface: if any public API signature in `src/status.rs`,
  `src/status_timing.rs`, or `src/output_prefs.rs` must change to enable
  snapshot capture, stop and escalate.
- Iterations: if `make test` or `make lint` still fail after three focused
  fix-and-rerun cycles, stop and document the blocking failures.
- Ambiguity: if the normalized rendered output is non-deterministic across
  runs (e.g., due to timing jitter or thread ordering), stop and investigate
  before committing snapshots.

## Risks

- Risk: Fluent bidi isolate markers may leak into snapshots, causing them to
  appear correct visually but differ at the byte level across environments.
  Severity: medium Likelihood: high Mitigation: normalize all rendered strings
  through `test_support::fluent::normalize_fluent_isolates` before passing them
  to `insta::assert_snapshot!`. This is the established pattern in
  `src/status_tests.rs` and `src/status_timing_tests.rs`.

- Risk: `IndicatifReporter` writes to a real `MultiProgress` backed by stderr,
  making its output difficult to capture deterministically for snapshots.
  Severity: medium Likelihood: high Mitigation: do not attempt to snapshot
  `IndicatifReporter` directly. Instead, snapshot the shared formatting helpers
  (`stage_summary`, `task_progress_update`, `format_completion_line`) and the
  `AccessibleReporter` (which accepts a `Vec<u8>` writer). The
  `IndicatifReporter` integration is already covered by the existing BDD
  scenarios that capture stderr from the real binary.

- Risk: `rstest-bdd` feature file edits may not trigger recompilation.
  Severity: low Likelihood: medium Mitigation: `touch tests/bdd_tests.rs`
  before the final `make test` run.

- Risk: snapshot files may cause merge conflicts when multiple developers work
  on reporter formatting concurrently. Severity: low Likelihood: low
  Mitigation: keep snapshot files small and named descriptively. Store them
  under `src/snapshots/status/` to co-locate with the source modules they pin.

- Risk: new test files or helpers in shared CLI modules may trigger
  dead-code warnings from `build.rs` compilation. Severity: medium Likelihood:
  low Mitigation: snapshot tests will live in existing test modules
  (`src/status_tests.rs`, `src/status_timing_tests.rs`) or in dedicated sibling
  files included via `#[path = ...]`. No new shared CLI helpers are expected.

## Progress

- [x] Researched existing test infrastructure, reporter API surfaces, and
      insta usage patterns.
- [x] Drafted this ExecPlan.
- [x] Stage A: Add insta snapshot tests for AccessibleReporter output.
- [x] Stage B: Add insta snapshot tests for timing summary rendering.
- [x] Stage C: Add insta snapshot tests for prefix and spacing accessors.
- [x] Stage D: Add BDD scenarios for full-output snapshot verification.
- [x] Stage E: Documentation, roadmap update, and final validation.

## Surprises & discoveries

- `rstest-bdd` passed quoted literal arguments (for example `"Info:"`) through
  to the new alignment step verbatim, so the step needed to trim surrounding
  quotes before comparing normalized stderr lines.
- The `leta` language-server background `cargo check` held the shared target
  directory lock during targeted test runs. Killing that background check was
  necessary before the repo-local cargo test slices could proceed.

## Decision log

- Decision: snapshot the `AccessibleReporter` output rather than the
  `IndicatifReporter` output for alignment regression testing. Rationale:
  `AccessibleReporter` is generic over `Write + Send` and already supports a
  `Vec<u8>` writer for test-time output capture (introduced in PR #272).
  `IndicatifReporter` writes to a real `MultiProgress` with terminal drawing,
  making deterministic capture impractical. The formatting helpers shared
  between both reporters (`stage_label`, `stage_summary`,
  `task_progress_update`, `format_completion_line`) can be snapshotted
  directly. End-to-end `IndicatifReporter` behaviour is already covered by BDD
  scenarios that capture stderr from the compiled binary. Date/Author:
  2026-03-23 / Codex.

- Decision: store snapshot files under `src/snapshots/status/` for status
  reporter snapshots and `src/snapshots/output_prefs/` for prefix snapshots,
  following the existing pattern used by `src/snapshots/diagnostic_json/`.
  Rationale: co-locating snapshots with their source modules keeps the
  relationship obvious and follows the established convention in
  `src/diagnostic_json_tests.rs`. Date/Author: 2026-03-23 / Codex.

- Decision: normalize Fluent bidi isolate markers before snapshotting rather
  than using insta redactions. Rationale: the markers are invisible formatting
  artefacts, not meaningful content. Stripping them produces snapshots that
  match what a human sees in the terminal. The `normalize_fluent_isolates`
  helper is already used throughout the test suite. Date/Author: 2026-03-23 /
  Codex.

- Decision: use `rstest` parameterized cases to generate snapshot tests for
  both Unicode and ASCII themes from a single test function, with distinct
  snapshot names per theme. Rationale: this avoids duplicating test logic while
  producing separate, reviewable snapshot files for each theme. The pattern is
  established in `src/output_prefs_tests.rs`. Date/Author: 2026-03-23 / Codex.

- Decision: normalize quoted literal arguments inside the new
  `progress_output` BDD alignment step by trimming surrounding `"` characters
  before comparison. Rationale: rstest-bdd passed the quoted fragments from the
  feature file to the step function verbatim, and the assertion should match
  the human-visible content rather than the Gherkin quoting syntax.
  Date/Author: 2026-03-27 / Codex.

## Outcomes & retrospective

- Added snapshot coverage for:
  - `AccessibleReporter` stage/completion output in Unicode and ASCII themes.
  - `AccessibleReporter` task-progress output in Unicode and ASCII themes.
  - `render_summary_lines()` timing summaries in Unicode and ASCII themes.
  - `OutputPrefs` semantic prefixes and spacing accessors in Unicode and ASCII
    themes.
- Added two `progress_output.feature` scenarios plus a reusable alignment step
  asserting that matching stderr lines begin with the expected theme prefix.
- Updated the user guide, design document, and roadmap to record the new
  regression guard coverage.
- Focused validation passed for:
  - `cargo test --lib status::tests`
  - `cargo test --lib status::timing::tests`
  - `cargo test --lib output_prefs::tests`
  - `cargo test --test bdd_tests progress_output`
- Final repository-wide validation (`make check-fmt`, `make lint`,
  `make test`, `make fmt`, `make markdownlint`, `make nixie`) completed
  successfully after implementation.

## Context and orientation

The following modules and files are relevant to this plan. A reader
implementing the plan needs to understand each one.

**Source modules (production code — read-only for this plan):**

- `src/status.rs` (400 lines): defines `StatusReporter` trait,
  `AccessibleReporter<W>`, `SilentReporter`, `IndicatifReporter`, and shared
  formatting helpers `stage_label()`, `stage_summary()`,
  `task_progress_update()`, `format_completion_line()`. The
  `AccessibleReporter` accepts a generic `Write + Send` sink and can be
  constructed with `with_writer(prefs, Vec::new())` for test-time output
  capture.
- `src/status_timing.rs` (211 lines): defines `VerboseTimingReporter`, the
  internal `TimingState`, `CompletedStage`, and the public
  `render_summary_lines(prefs, entries)` function that produces the multi-line
  timing summary. The `VerboseTimingReporter::with_clock()` constructor accepts
  a `Box<dyn Fn() -> Duration>` for deterministic time control.
- `src/output_prefs.rs` (284 lines): defines `OutputPrefs` with
  `error_prefix()`, `warning_prefix()`, `success_prefix()`, `info_prefix()`,
  `timing_prefix()`, `task_indent()`, and `timing_indent()`. The
  `resolve_from_theme_with()` function accepts a custom environment lookup for
  test isolation.
- `src/theme.rs` (295 lines): defines `ThemePreference` (Auto, Unicode,
  Ascii), `DesignTokens`, `SymbolTokens`, `SpacingTokens`, `ColourTokens`,
  `ResolvedTheme`, and `resolve_theme()`. The constant token sets
  `UNICODE_SYMBOLS`, `ASCII_SYMBOLS`, and `SPACING` define the glyph and
  spacing values.

**Existing test modules (will be extended):**

- `src/status_tests.rs` (~245 lines): unit tests for status formatting
  helpers and `AccessibleReporter` output capture. Uses
  `test_support::fluent::normalize_fluent_isolates` and an `en_us_localizer`
  rstest fixture.
- `src/status_timing_tests.rs` (~268 lines): unit tests for `TimingState`,
  `render_summary_lines()`, and `VerboseTimingReporter` with `FakeClock`.
- `src/output_prefs_tests.rs` (~193 lines): parameterized tests for
  `resolve_with()`, `resolve_from_theme_with()`, prefix rendering, and spacing
  accessors.

**BDD test infrastructure:**

- `tests/features/progress_output.feature` (170 lines): 17 scenarios
  covering standard/accessible mode, theme prefixes, localization, verbose
  timing, stream separation. Step definitions in
  `tests/bdd/steps/progress_output.rs`.
- `tests/bdd/fixtures/mod.rs`: `TestWorld` struct with `temp_dir`,
  `ninja_env_guard`, `output_prefs`, `rendered_prefix`, and simulated
  environment slots.
- `tests/bdd/helpers/assertions.rs`: shared assertion helpers including
  Fluent normalization.
- `tests/bdd_tests.rs`: BDD test harness entry point (must be `touch`ed
  when `.feature` files change).

**Existing snapshot infrastructure:**

- `Cargo.toml` dev-dependencies: `insta = { version = "1", features =
  ["yaml"] }`.
- `src/diagnostic_json_tests.rs`: existing `insta` snapshot usage pattern.
  Uses `Settings::new()` with
  `set_snapshot_path(concat!(env!( "CARGO_MANIFEST_DIR"), "/src/snapshots/diagnostic_json"))`.
- `src/snapshots/diagnostic_json/`: contains one `.snap` file for the
  manifest parse error diagnostic.
- `tests/snapshots/ninja/`: contains two `.snap` files for Ninja generation.
- `docs/snapshot-testing-in-netsuke-using-insta.md`: project guide for
  snapshot testing with `insta`.

**Test support crate:**

- `test_support/src/fluent.rs`: `normalize_fluent_isolates(text: &str) ->
  String` — strips `U+2068` and `U+2069` bidi isolate markers.
- `test_support/src/localizer.rs`: `localizer_test_lock()` — returns a mutex
  guard for exclusive localizer access during tests.

**Key terms:**

- Snapshot test: a test that captures the complete output of a function or
  reporter as a string, saves it to a `.snap` file on disk, and fails if the
  output changes in a subsequent run. The `.snap` file is committed to version
  control.
- Design token: a semantic value (symbol glyph, spacing string, colour
  identifier) resolved from the active theme.
- Fluent bidi isolate markers: invisible Unicode characters (`U+2068` first
  strong isolate, `U+2069` pop directional isolate) that Fluent inserts around
  interpolated values. These must be stripped before comparison.

## Interfaces and dependencies

No new public interfaces are introduced. The snapshot tests consume existing
internal APIs:

1. `AccessibleReporter::with_writer(prefs: OutputPrefs, writer: Vec<u8>)` —
   captures output for snapshot comparison.
2. `render_summary_lines(prefs: OutputPrefs, entries: &[CompletedStage])` —
   produces timing summary lines for snapshot comparison.
3. `OutputPrefs::error_prefix()`, `warning_prefix()`, `success_prefix()`,
   `info_prefix()`, `timing_prefix()`, `task_indent()`, `timing_indent()` —
   produce individual prefix and spacing strings.
4. `resolve_from_theme_with(theme, no_emoji, mode, read_env)` — resolves
   `OutputPrefs` with test-time environment injection.
5. `insta::assert_snapshot!(name, content)` — pins content to a `.snap` file.
6. `insta::Settings::new().set_snapshot_path(path)` — directs snapshot
   storage to a specific directory.

Snapshot storage layout after completion:

```plaintext
src/
  snapshots/
    status/
      netsuke__status__tests__accessible_unicode_stage_and_completion.snap
      netsuke__status__tests__accessible_ascii_stage_and_completion.snap
      netsuke__status__tests__accessible_unicode_task_progress.snap
      netsuke__status__tests__accessible_ascii_task_progress.snap
    status_timing/
      netsuke__status__timing__tests__timing_summary_unicode.snap
      netsuke__status__timing__tests__timing_summary_ascii.snap
    output_prefs/
      netsuke__output_prefs__tests__all_prefixes_unicode.snap
      netsuke__output_prefs__tests__all_prefixes_ascii.snap
```

The exact snapshot names will follow `insta`'s automatic naming convention
(`<crate>__<module_path>__<test_name>`) but can be overridden with explicit
first arguments to `assert_snapshot!` for clarity.

## Plan of work

### Stage A: Snapshot tests for AccessibleReporter output

Add snapshot tests to `src/status_tests.rs` that capture the full rendered
output of `AccessibleReporter` for both Unicode and ASCII themes. The tests
will drive the reporter through a realistic sequence of stage announcements,
task progress updates, and a completion message, then snapshot the accumulated
`Vec<u8>` output.

Create an `insta::Settings` helper that directs snapshots to
`src/snapshots/status/`. Add two new test functions:

1. A parameterized `rstest` test that exercises stage announcement and
   completion. For each theme (Unicode, Ascii), it:
   - Creates `OutputPrefs` via `resolve_from_theme_with(Some(theme), None,
     OutputMode::Standard, |_| None)`.
   - Constructs `AccessibleReporter::with_writer(prefs, Vec::new())`.
   - Calls `report_stage()` for stages 1 through 6 with representative
     descriptions.
   - Calls `report_complete()` with `STATUS_TOOL_MANIFEST`.
   - Extracts the writer contents, normalizes Fluent isolates, and passes the
     result to `assert_snapshot!` with a theme-specific name.

2. A parameterized `rstest` test that exercises task progress rendering. For
   each theme, it:
   - Creates the reporter as above.
   - Calls `report_task_progress(1, 2, "cc -c src/a.c")` and
     `report_task_progress(2, 2, "cc -c src/b.c")`.
   - Snapshots the accumulated output.

These tests will confirm that indentation, prefix symbols, and line structure
are identical between themes except for the glyph set.

If `src/status_tests.rs` approaches the 400-line limit after adding these
tests, extract the snapshot tests into a dedicated
`src/status_snapshot_tests.rs` file and include it via
`#[path = "status_snapshot_tests.rs"] #[cfg(test)] mod snapshot_tests;` at the
bottom of `src/status.rs`.

Validation gate for Stage A:

- `make check-fmt` passes.
- `make lint` passes.
- `cargo test status::tests` passes, including the new snapshot tests.
- Snapshot files exist under `src/snapshots/status/` and are committed.
- The Unicode and ASCII snapshots differ only in glyph characters, not in
  spacing, indentation, or line count.

### Stage B: Snapshot tests for timing summary rendering

Add snapshot tests to `src/status_timing_tests.rs` that capture the complete
output of `render_summary_lines()` for both themes. The tests will construct a
`TimingState` with deterministic durations (using the existing `FakeClock`
pattern or direct `Duration` values), call `render_summary_lines()`, join the
result, normalize Fluent isolates, and snapshot.

Create an `insta::Settings` helper that directs snapshots to
`src/snapshots/status_timing/`.

Add a parameterized test that, for each theme:

- Creates `OutputPrefs` for Unicode or ASCII.
- Builds a `TimingState` with three stages (reading manifest, parsing YAML,
  expanding templates) and deterministic durations (12ms, 4ms, 7ms).
- Calls `render_summary_lines(prefs, state.completed_stages())`.
- Joins the lines with newlines, normalizes, and snapshots.

This captures the timing header (with timing prefix), indented stage lines
(with timing indent), and the total line, all in one snapshot.

If `src/status_timing_tests.rs` approaches 400 lines, extract snapshot tests
into `src/status_timing_snapshot_tests.rs`.

Validation gate for Stage B:

- `make check-fmt` passes.
- `make lint` passes.
- `cargo test status::timing::tests` passes, including the new snapshot tests.
- Unicode and ASCII timing snapshots differ only in the timing prefix symbol.

### Stage C: Snapshot tests for prefix and spacing accessors

Add snapshot tests to `src/output_prefs_tests.rs` that capture all five
semantic prefixes and both spacing accessors as a single multi-line snapshot
per theme. This provides a compact regression guard for the entire token
surface.

Create an `insta::Settings` helper that directs snapshots to
`src/snapshots/output_prefs/`.

Add a parameterized test that, for each theme:

- Creates `OutputPrefs` for Unicode or ASCII via `resolve_from_theme_with`.
- Builds a multi-line string containing:

  ```plaintext
  error_prefix:   <value>
  warning_prefix: <value>
  success_prefix: <value>
  info_prefix:    <value>
  timing_prefix:  <value>
  task_indent:    "<value>"
  timing_indent:  "<value>"
  ```

- Snapshots the result.

This captures prefix alignment and ensures that token changes surface as a
single, readable diff.

Validation gate for Stage C:

- `make check-fmt` passes.
- `make lint` passes.
- `cargo test output_prefs::tests` passes, including the new snapshot tests.
- Each snapshot file is human-readable and shows the complete token surface.

### Stage D: BDD scenarios for full-output snapshot verification

Add new BDD scenarios to `tests/features/progress_output.feature` that verify
the full stderr output of a successful `manifest -` invocation for each theme.
These scenarios go beyond the existing substring-contains assertions and check
that the rendered output structure matches expectations.

Add two new scenarios:

1. "ASCII theme progress output is stable":
   - Given a minimal Netsuke workspace
   - When netsuke is run with arguments
     `"--theme ascii --accessible true --progress true manifest -"`
   - Then the command should succeed
   - And stderr should contain "+ Success:"
   - And stderr should contain "i Info:"
   - And stderr should contain "Stage 1/6"
   - And stderr should contain "Stage 6/6"
   - And stderr lines containing "Info:" should all start with "i Info:"
   - And stderr lines containing "Success:" should all start with "+ Success:"

2. "Unicode theme progress output is stable":
   - Given a minimal Netsuke workspace
   - When netsuke is run with arguments
     `"--theme unicode --accessible true --progress true manifest -"`
   - Then the command should succeed
   - And stderr should contain "✔ Success:"
   - And stderr should contain "ℹ Info:"
   - And stderr should contain "Stage 1/6"
   - And stderr should contain "Stage 6/6"
   - And stderr lines containing "Info:" should all start with "ℹ Info:"
   - And stderr lines containing "Success:" should all start with "✔ Success:"

These scenarios require new step definitions for prefix alignment assertions.
Add a new step definition in `tests/bdd/steps/progress_output.rs`:

<!-- markdownlint-disable MD013 -->

- `#[then("stderr lines containing {pattern} should all start with {prefix}")]`

<!-- markdownlint-enable MD013 -->

This step iterates over stderr lines, finds lines containing the pattern, and
asserts each starts with the expected prefix. This guards against alignment
drift where a prefix symbol changes width but the surrounding whitespace does
not adjust.

If the step definition file approaches 400 lines, extract the assertion step
into `tests/bdd/helpers/assertions.rs`.

Validation gate for Stage D:

- `touch tests/bdd_tests.rs && make test` passes.
- The new BDD scenarios pass in isolation
  (`cargo test --test bdd_tests progress_output`).
- Existing BDD scenarios remain green.

### Stage E: Documentation, roadmap update, and final validation

Update documentation and mark the roadmap item complete.

Required documentation changes:

- `docs/users-guide.md`: add a sentence to the existing "Theme and
  accessibility preferences" section noting that output alignment is
  regression-tested via snapshots for both themes.
- `docs/netsuke-design.md`: add a brief note to the testing strategy section
  (if one exists) or the CLI output section recording that progress and status
  output is snapshot-tested for alignment stability.
- `docs/roadmap.md`: mark 3.12.2 sub-items done and the parent item done.

Record any design decisions taken during implementation in this ExecPlan's
Decision Log section.

Final validation must include the project gates requested in the task, plus
documentation quality assurance:

- `make check-fmt`
- `make lint`
- `make test`
- `make fmt`
- `PATH="/root/.bun/bin:$PATH" make markdownlint`
- `make nixie`

## Concrete steps

All commands below run from the repository root:

```sh
cd /home/user/project
```

1. Verify the current baseline passes all gates before making changes.

   ```sh
   set -o pipefail && make check-fmt 2>&1 | tee /tmp/netsuke-3-12-2-baseline-fmt.log
   set -o pipefail && make lint 2>&1 | tee /tmp/netsuke-3-12-2-baseline-lint.log
   set -o pipefail && make test 2>&1 | tee /tmp/netsuke-3-12-2-baseline-test.log
   ```

   Expected: all three exit 0.

2. Implement Stage A: add snapshot tests for AccessibleReporter.

   Edit `src/status_tests.rs` (or create `src/status_snapshot_tests.rs` if
   needed). Add the `insta` import and snapshot settings helper. Add
   parameterized tests for Unicode and ASCII themes.

   ```sh
   cargo test --lib status::tests -- --nocapture 2>&1 | tee /tmp/netsuke-3-12-2-stage-a.log
   ```

   Expected: new snapshot tests create `.snap` files and pass.

3. Implement Stage B: add snapshot tests for timing summary rendering.

   Edit `src/status_timing_tests.rs` (or create a sibling file). Add
   parameterized timing summary snapshot tests.

   ```sh
   cargo test --lib status::timing::tests -- --nocapture 2>&1 | tee /tmp/netsuke-3-12-2-stage-b.log
   ```

4. Implement Stage C: add snapshot tests for prefix and spacing accessors.

   Edit `src/output_prefs_tests.rs`. Add parameterized prefix/spacing snapshot
   tests.

   ```sh
   cargo test --lib output_prefs::tests -- --nocapture 2>&1 | tee /tmp/netsuke-3-12-2-stage-c.log
   ```

5. Implement Stage D: add BDD scenarios and step definitions.

   Edit `tests/features/progress_output.feature` and
   `tests/bdd/steps/progress_output.rs`. Add alignment assertion steps.

   ```sh
   touch tests/bdd_tests.rs
   cargo test --test bdd_tests progress_output -- --nocapture 2>&1 | tee /tmp/netsuke-3-12-2-stage-d.log
   ```

6. Implement Stage E: update documentation and roadmap.

   Edit `docs/users-guide.md`, `docs/netsuke-design.md`, and `docs/roadmap.md`.

7. Run the full validation gates with logged output.

   <!-- markdownlint-disable MD029 -->

   1. `set -o pipefail && make check-fmt 2>&1 | tee /tmp/netsuke-3-12-2-check-fmt.log`
   2. `set -o pipefail && make lint 2>&1 | tee /tmp/netsuke-3-12-2-lint.log`
   3. `set -o pipefail && make test 2>&1 | tee /tmp/netsuke-3-12-2-test.log`
   4. `set -o pipefail && make fmt 2>&1 | tee /tmp/netsuke-3-12-2-fmt.log`
   5. `PATH="/root/.bun/bin:$PATH" make markdownlint 2>&1 | tee /tmp/ml.log`
   6. `make nixie 2>&1 | tee /tmp/netsuke-3-12-2-nixie.log`

   <!-- markdownlint-enable MD029 -->

   Expected final signal:

   ```plaintext
   make check-fmt    # exits 0
   make lint         # exits 0
   make test         # exits 0
   make markdownlint # exits 0
   make nixie        # exits 0
   ```

8. Inspect scope before finalizing.

   ```sh
   git status --short
   git diff --stat
   ```

   Only the intended test, snapshot, and documentation files for 3.12.2 should
   remain modified.

## Validation and acceptance

Acceptance is behavioural, not structural.

The milestone is done when all of the following are true:

- Running `cargo test --lib status::tests` passes and includes snapshot tests
  that pin `AccessibleReporter` output for both Unicode and ASCII themes.
- Running `cargo test --lib status::timing::tests` passes and includes
  snapshot tests that pin verbose timing summary output for both themes.
- Running `cargo test --lib output_prefs::tests` passes and includes snapshot
  tests that pin all five semantic prefixes and both spacing tokens for both
  themes.
- Running `cargo test --test bdd_tests progress_output` passes and includes
  BDD scenarios that verify prefix alignment consistency for both themes.
- The Unicode and ASCII snapshots for each reporter differ only in glyph
  characters, not in spacing, indentation, or line count.
- `make check-fmt`, `make lint`, and `make test` all pass.

Quality criteria:

- Tests: new `insta` snapshot tests and `rstest-bdd` BDD scenarios cover
  Unicode and ASCII themes, happy paths (stage/task/completion rendering), and
  alignment regression (prefix consistency across output lines).
- Lint/typecheck: `make check-fmt` and `make lint` pass without new warnings.
- Documentation: `docs/users-guide.md` notes snapshot regression coverage
  for theme output. `docs/roadmap.md` marks 3.12.2 complete.

Quality method:

- `make check-fmt` verifies formatting.
- `make lint` verifies Clippy compliance.
- `make test` runs the full workspace test suite including new snapshots and
  BDD scenarios.
- `PATH="/root/.bun/bin:$PATH" make markdownlint` verifies documentation
  formatting.
- `make nixie` validates Mermaid diagrams.

## Idempotence and recovery

The planned edits are safe to repeat.

- Snapshot tests are deterministic: given the same locale, theme, and duration
  inputs, they produce byte-identical output.
- If `insta` creates `.snap.new` files instead of updating `.snap` files, run
  `cargo insta accept --all` to promote them (or delete `.snap.new` and re-run
  to regenerate).
- If BDD feature edits are not picked up, `touch tests/bdd_tests.rs` before
  `make test`.
- If a snapshot test fails unexpectedly, compare the `.snap` diff to
  determine whether the change is intentional (update the snapshot) or a
  regression (fix the code).
- If `make fmt` rewrites unrelated Markdown files, inspect `git diff` and
  restore only formatter-introduced changes outside the 3.12.2 scope.

## Artifacts and notes

Useful evidence to keep while implementing:

- The first snapshot run output showing the created `.snap` files.
- A side-by-side comparison of the Unicode and ASCII snapshots for
  `AccessibleReporter` stage output, confirming identical structure.
- The passing BDD scenario output for the new alignment assertions.
- The final `git diff --stat` confirming the change stayed within scope.

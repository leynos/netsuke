# 3.10.1. Guarantee status message and subprocess output ordering

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

## Purpose / big picture

Netsuke currently writes both its own status messages (pipeline stage updates,
task progress, completion summaries) and Ninja subprocess output to the
terminal. To support reliable scripting, piping, and log capture, the
implementation must guarantee clear separation between these two streams:

- **Status messages** (progress bars, stage announcements, timing summaries)
  must stream exclusively to **stderr**.
- **Subprocess output** (Ninja's build commands) must stream to **stdout** with
  preserved ordering.

After this change, users can:

1. Redirect `stdout` to capture build artefacts (e.g., `ninja -t graph` DOT
   graph language output) without status noise.
2. Redirect `stderr` to capture progress diagnostics without build output.
3. Verify stream separation with end-to-end tests that redirect each stream
   independently.

## Constraints

Hard invariants that must hold throughout implementation:

- **No functional regressions**: All existing Behaviour-Driven Development (BDD)
  scenarios in `tests/features/progress_output.feature` and
  `tests/features/accessible_output.feature` must pass.
- **Existing API surface**: The `StatusReporter` trait and its implementations
  must maintain their existing public signatures.
- **Localization**: All user-facing messages must continue to use the Fluent
  localization system.
- **OrthoConfig compatibility**: Any new CLI flags or configuration options
  must integrate with the existing `OrthoConfig`-derived configuration.
- **Thread safety**: The current thread-based stderr forwarding architecture
  must remain intact to avoid blocking on status writes.
- **AGENTS.md compliance**: All changes must pass `make check-fmt`, `make lint`,
  and `make test`.

## Tolerances (exception triggers)

Thresholds that trigger escalation when breached:

- **Scope**: If implementation requires changes to more than 15 files or 500
  lines of code (net), stop and escalate.
- **Interface**: If the `StatusReporter` trait's public method signatures must
  change (beyond adding new methods), stop and escalate.
- **Dependencies**: If a new external dependency is required, stop and escalate.
- **Iterations**: If tests still fail after 3 implementation attempts, stop and
  escalate.
- **Ambiguity**: If multiple valid interpretations exist for how status messages
  should interleave with subprocess output, stop and present options with
  trade-offs.

## Risks

Known uncertainties that might affect the plan:

- Risk: Indicatif progress bars may have implicit stdout writes.
  Severity: medium
  Likelihood: low
  Mitigation: Verify `ProgressDrawTarget::stderr_with_hz()` is consistently used
  throughout `IndicatifReporter`. The codebase already uses stderr targets.

- Risk: Timing-sensitive interleaving between status updates and subprocess
  output may be difficult to test deterministically.
  Severity: medium
  Likelihood: medium
  Mitigation: Use BDD scenarios with controlled fake Ninja executables that emit
  predictable output patterns. Avoid timing-based assertions.

- Risk: The existing subprocess streaming architecture splits stdout/stderr
  handling across threads, which could introduce race conditions if status
  callbacks write to stdout.
  Severity: medium
  Likelihood: low
  Mitigation: Audit all paths where `StatusReporter` methods write output;
  confirm they exclusively use stderr.

## Progress

- [x] Stage A: Audit and understand current output architecture
  - [x] Verify all `StatusReporter` implementations write to stderr
  - [x] Document any stdout usage in status reporting paths
  - [x] Map the subprocess output forwarding flow
- [x] Stage B: Add configuration support via `OrthoConfig`
  - [x] Determined no new configuration needed (existing implementation correct)
- [x] Stage C: Implement stream separation guarantees
  - [x] Verified status messages exclusively use stderr
  - [x] Verified subprocess stdout is preserved without status interleaving
  - [x] Existing threading model provides necessary ordering guarantees
- [x] Stage D: Write comprehensive tests
  - [x] Add BDD scenarios for stdout-only capture
  - [x] Add BDD scenarios for stderr-only capture
  - [x] Add BDD scenarios verifying stream exclusivity
- [x] Stage E: Documentation and validation
  - [x] Update `docs/users-guide.md` with stream behaviour documentation
  - [x] Run full test suite
  - [x] Mark roadmap item as complete

## Surprises & discoveries

- The existing implementation already correctly separates streams. All
  `StatusReporter` implementations write exclusively to stderr, and subprocess
  stdout is forwarded through a separate path.
- No code changes were required; the work was primarily verification and adding
  end-to-end tests to guard the existing behaviour.
- Stage B (configuration) was deemed unnecessary since the existing architecture
  already provides the required guarantees.

## Decision log

- **2026-02-28**: Confirmed existing implementation is correct after Stage A
  audit. No configuration changes needed; skipping Stage B and Stage C code
  changes.
- **2026-02-28**: Added three BDD scenarios to verify stream separation:
  - `Subprocess stdout is separate from status messages`
  - `Status messages do not contaminate stdout in standard mode`
  - `Build artifacts can be captured via stdout redirection`
- **2026-02-28**: Added "Output streams" section to users-guide.md documenting
  the stream behaviour and common redirection patterns.

## Outcomes & retrospective

**Outcome**: SUCCESS

The existing implementation already correctly separates status messages (stderr)
from subprocess output (stdout). This plan was primarily a verification and
documentation exercise rather than an implementation task.

**What went well**:

- The codebase audit was straightforward due to consistent use of
  `io::stderr()` throughout `StatusReporter` implementations.
- Existing BDD infrastructure made adding new scenarios trivial.
- All quality gates passed on the first attempt.

**Lessons learned**:

- Always audit the existing implementation before assuming changes are needed.
  The roadmap item implied implementation work, but the architecture was already
  correct.
- End-to-end tests that verify stream separation provide valuable regression
  protection even when no code changes are required.

**Artefacts produced**:

- `tests/features/progress_output.feature`: Three new BDD scenarios
- `tests/bdd/steps/progress_output.rs`: One new fixture function
- `docs/users-guide.md`: New "Output streams" section
- `docs/roadmap.md`: Marked 3.10.1 as complete

## Context and orientation

### Repository structure

Netsuke is a build system compiler that transforms YAML+Jinja manifests into
Ninja build graphs. The command-line interface (CLI) is implemented in Rust
using `clap` for argument parsing, with `OrthoConfig` providing layered
configuration support.

### Key files and modules

The output architecture spans several modules:

1. **Status reporting trait and implementations** (`src/status.rs`):
   - `StatusReporter` trait: Core interface for progress updates.
   - `AccessibleReporter`: Text-only reporter for screen readers, writes to
     stderr via `writeln!(io::stderr(), ...)`.
   - `SilentReporter`: No-op implementation.
   - `IndicatifReporter`: Multi-progress bar reporter using
     `indicatif::MultiProgress` with `ProgressDrawTarget::stderr_with_hz(12)`.

2. **Timing wrapper** (`src/status_timing.rs`):
   - `VerboseTimingReporter`: Decorator that wraps any reporter to add timing
     metrics, writes timing summary to stderr.

3. **Pipeline stages** (`src/status_pipeline.rs`):
   - Defines the six pipeline stages and their descriptions.
   - `report_pipeline_stage()` helper for stage transitions.

4. **Runner module** (`src/runner/mod.rs`):
   - `make_reporter()`: Factory function for selecting reporter.
   - `handle_build()` and `handle_ninja_tool()`: Orchestrate build execution.
   - `on_task_progress_callback()`: Bridges Ninja status parsing to reporter.

5. **Process execution** (`src/runner/process/mod.rs`):
   - `spawn_and_stream_output()`: Core subprocess I/O handling.
   - Spawns stderr forwarding on a separate thread.
   - Drains stdout on main thread to preserve status callback ordering.
   - `run_ninja_with_status()`: Invokes Ninja with progress parsing.

6. **Streaming utilities** (`src/runner/process/streaming.rs`):
   - `forward_child_output()`: Basic byte-for-byte forwarding.
   - `forward_child_output_with_ninja_status()`: Parses Ninja status lines and
     invokes observer callback.
   - `NinjaStatusParsingReader`: Reader wrapper that extracts status updates.

### Current stream routing

Based on code analysis, the following table summarizes stream routing for
reporters and subprocesses:

| Component | Output destination | Notes |
| --------- | ------------------ | ----- |
| `AccessibleReporter::report_stage` | stderr | `writeln!(io::stderr(), ...)` |
| `AccessibleReporter::report_complete` | stderr | `writeln!(io::stderr(), ...)` |
| `AccessibleReporter::report_task_progress` | stderr | `writeln!(io::stderr(), ...)` |
| `IndicatifReporter` (progress bars) | stderr | `ProgressDrawTarget::stderr_with_hz(12)` |
| `IndicatifReporter` (hidden mode fallback) | stderr | `writeln!(io::stderr(), ...)` |
| `IndicatifReporter::report_complete` | stderr | `writeln!(io::stderr(), ...)` |
| `VerboseTimingReporter` timing summary | stderr | Via wrapped reporter |
| Ninja subprocess stdout | stdout | Via `spawn_and_stream_output()` |
| Ninja subprocess stderr | stderr | Via background thread forwarding |

### Existing test coverage

The `tests/features/progress_output.feature` file contains BDD scenarios that:

- Verify task updates appear on stderr
- Verify stage summaries appear on stderr
- Test accessible mode output
- Test `--progress false` suppression
- Verify verbose timing summaries

### What needs to change

Based on the audit above, the current implementation appears to already route
status messages to stderr and preserve subprocess stdout. The main work is:

1. **Verification**: Confirm no edge cases exist where status leaks to stdout.
2. **End-to-end tests**: Add BDD scenarios that explicitly verify stream
   separation by redirecting stdout and stderr independently.
3. **Documentation**: Update the user's guide to document the stream behaviour.

## Plan of work

### Stage A: Audit current implementation (verification)

The code audit above suggests the implementation may already be correct. Stage A
confirms this with targeted verification:

1. Review `src/status.rs` to confirm all `writeln!` calls use `io::stderr()`.
2. Review `src/status_timing.rs` to confirm timing summary writes to stderr.
3. Review `spawn_and_stream_output()` in `src/runner/process/mod.rs` to confirm
   stdout forwarding does not invoke status callbacks that could write to
   stdout.
4. Review `src/runner/process/streaming.rs` to confirm the observer callback
   pattern preserves separation.

If any stdout usage is found in status paths, document it in `Decision Log` and
proceed to fix it in Stage C.

### Stage B: Configuration design (completed)

**Outcome**: Stage A confirmed the existing implementation is correct. No new
configuration was required. This stage completed as a verification checkpoint.

### Stage C: Implementation (completed)

**Outcome**: Stage A revealed no issues. No code changes were required. This
stage completed as a verification checkpoint confirming the existing threading
model provides the necessary ordering guarantees.

### Stage D: End-to-end tests (completed)

**Outcome**: Three BDD scenarios were added to `tests/features/progress_output.feature`
to verify stream separation:

1. **Subprocess stdout is separate from status messages**: Verifies that stdout
   markers from the fake Ninja appear only in stdout, while stderr markers appear
   only in stderr. Uses stable machine markers (`NINJA_STDOUT_MARKER`,
   `NINJA_STDERR_MARKER`) to avoid coupling to localized UI strings.

2. **Status messages do not contaminate stdout in standard mode**: Verifies stream
   routing in non-accessible mode using the same stable markers.

3. **Build artifacts can be captured via stdout redirection**: Verifies that
   `netsuke manifest -` output goes to stdout without status contamination.

Supporting infrastructure added:

- `FakeNinjaConfig` struct in `tests/bdd/steps/progress_output.rs` for configurable
  fixture generation with optional stderr markers.
- `install_fake_ninja_with_config()` function for flexible fixture setup.
- Updated `fake_ninja_emits_stdout_output` fixture to emit both stdout and stderr
  markers for comprehensive stream routing verification.

### Stage E: Documentation (completed)

**Outcome**: The `docs/users-guide.md` file was updated with an "Output streams"
section documenting the stream separation behaviour:

- Explains that status messages go to stderr and subprocess output goes to stdout.
- Provides example redirection commands for common use cases.
- Documents the `--progress false` flag for suppressing status output entirely.

### Validation gates

At each stage transition:

1. `make check-fmt` must pass.
2. `make lint` must pass.
3. `make test` must pass.

For documentation changes, also run:

1. `make fmt` (formats Markdown files).
2. `make markdownlint`.
3. `make nixie` (validates Mermaid diagrams).

## Concrete steps

### Stage A verification commands

```bash
# Verify stderr usage in status.rs
grep -n "io::stdout\|stdout()" src/status.rs
# Expected: No matches (all output should be stderr)

grep -n "io::stderr\|stderr()" src/status.rs
# Expected: Multiple matches in AccessibleReporter and IndicatifReporter impls

# Verify stderr in status_timing.rs
grep -n "io::stdout\|stdout()" src/status_timing.rs
# Expected: No matches

# Verify subprocess stdout forwarding
grep -n "io::stdout" src/runner/process/mod.rs
# Expected: One stdout forwarding site in spawn_and_stream_output()

# Run existing tests
make test
```

### Stage D test implementation (completed)

The following artefacts were created:

- `tests/features/progress_output.feature`: Added three stream separation
  scenarios using stable machine markers.
- `tests/bdd/steps/progress_output.rs`: Added `FakeNinjaConfig` struct and
  `install_fake_ninja_with_config()` function; updated
  `fake_ninja_emits_stdout_output` fixture to emit both stdout and stderr
  markers.

Verification command run:

```bash
cargo test --test rstest_bdd -- progress_output
```

### Stage E documentation update (completed)

The following artefact was updated:

- `docs/users-guide.md`: Added "Output streams" section documenting stream
  separation behaviour with example redirection commands.

Validation commands run:

```bash
make fmt
make markdownlint
make nixie
```

## Validation and acceptance

Quality criteria:

- **Tests**: All tests pass including new stream separation scenarios.
- **Lint/typecheck**: `make check-fmt` and `make lint` pass.
- **Documentation**: User's guide documents stream behaviour; `make fmt`,
  `make markdownlint`, and `make nixie` pass.

Quality method:

- Run `make check-fmt && make lint && make test` after each stage.
- For documentation changes, also run `make fmt`, `make markdownlint`, and
  `make nixie`.
- Manual verification: Run `netsuke graph 2>/dev/null` and confirm only DOT
  output appears.

Observable behaviour after completion:

1. Running `netsuke graph > output.dot 2> progress.log` captures build graph to
   file without status messages contaminating the DOT content; status messages
   go to the separate progress log.
2. Running `netsuke build 2> log.txt` captures progress to log file while build
   output appears on terminal.
3. All existing BDD scenarios continue to pass.
4. New stream separation scenarios pass.

## Idempotence and recovery

All stages are repeatable without side effects:

- Code changes are version controlled and can be reverted.
- Tests are deterministic and isolated.
- Documentation changes are additive.

If a stage fails:

1. Identify failing tests with `make test 2>&1 | tee test.log`.
2. Review test output for specific failures.
3. If within tolerances, fix and retry.
4. If tolerances exceeded, escalate with documented context.

## Artifacts and notes

Key code locations:

- Status reporter trait: `src/status.rs` (`StatusReporter` trait definition)
- Accessible reporter stderr writes: `src/status.rs` (`AccessibleReporter` impl)
- Indicatif stderr target: `src/status.rs` (`IndicatifReporter::new()`)
- Subprocess stdout forwarding: `src/runner/process/mod.rs`
  (`spawn_and_stream_output()`)
- Existing progress tests: `tests/features/progress_output.feature`

Example test transcript (expected after implementation):

```plaintext
$ netsuke --progress true graph > /tmp/graph.dot
Stage 1/6: Manifest ingestion
Stage 2/6: Initial YAML parsing
...
Graph complete.

$ head -1 /tmp/graph.dot
digraph G {
```

## Interfaces and dependencies

### Existing interfaces (no changes expected)

In `src/status.rs`:

```rust
pub trait StatusReporter {
    fn report_stage(&self, current: StageNumber, total: StageNumber, description: &str);
    fn report_task_progress(&self, _current: u32, _total: u32, _description: &str) {}
    fn report_complete(&self, tool_key: LocalizationKey);
}
```

### Test fixtures (archival reference)

The following fixture was implemented in `tests/bdd/steps/progress_output.rs`:

- `FakeNinjaConfig` struct with `stdout_lines` and `stderr_marker` fields.
- `install_fake_ninja_with_config()` function for flexible fixture setup.
- `fake_ninja_emits_stdout_output` fixture emitting both `NINJA_STDOUT_MARKER`
  lines and `NINJA_STDERR_MARKER` for comprehensive stream routing verification.

### BDD step definitions (archival reference)

The following step definitions were verified to exist in
`tests/bdd/steps/manifest_command.rs`:

- `stdout_should_contain` / `stderr_should_contain`
- `stdout_should_not_contain` / `stderr_should_not_contain`
- `stdout_should_contain_in_order` (added for ordering assertions)

## Contingency (archived)

The following contingency guidance was prepared for Stages B and C but was not
required because the existing implementation already met all requirements.

### Stage B contingency: Configuration design

The roadmap item mentions using `ortho_config` for ergonomic configuration. An
evaluation is needed to determine whether any new configuration is required:

- If the current implementation already correctly separates streams, no new
  configuration may be required.
- If users need to override stream destinations (e.g., for testing or custom
  tooling), design a minimal configuration schema.

**Decision point**: Determine whether configuration is needed based on Stage A
findings.

### Stage C contingency: Implementation

If Stage A reveals issues:

1. Fix any status output paths that incorrectly use stdout.
2. Add any necessary synchronization to prevent interleaving issues.
3. Ensure the observer callback in `spawn_and_stream_output()` does not write
   to stdout.

# 3.10.1. Guarantee status message and subprocess output ordering

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

## Purpose / big picture

Netsuke currently writes both its own status messages (pipeline stage updates,
task progress, completion summaries) and Ninja subprocess output to the
terminal. To support reliable scripting, piping, and log capture, we must
guarantee clear separation between these two streams:

- **Status messages** (progress bars, stage announcements, timing summaries)
  must stream exclusively to **stderr**.
- **Subprocess output** (Ninja's build commands) must stream to **stdout** with
  preserved ordering.

After this change, users can:

1. Redirect `stdout` to capture build artifacts (e.g., `ninja -t graph` DOT
   output) without status noise.
2. Redirect `stderr` to capture progress diagnostics without build output.
3. Verify stream separation with end-to-end tests that redirect each stream
   independently.

## Constraints

Hard invariants that must hold throughout implementation:

- **No functional regressions**: All existing BDD scenarios in
  `tests/features/progress_output.feature` and
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
  - [x] Update `docs/users-guide.md` with stream behavior documentation
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
Ninja build graphs. The CLI is implemented in Rust using `clap` for argument
parsing, with `OrthoConfig` providing layered configuration support.

### Key files and modules

The output architecture spans several modules:

1. **Status reporting trait and implementations** (`src/status.rs`):
   - `StatusReporter` trait (lines 99-110): Core interface for progress updates.
   - `AccessibleReporter` (lines 118-147): Text-only reporter for screen
     readers, writes to stderr via `writeln!(io::stderr(), ...)`.
   - `SilentReporter` (lines 150-155): No-op implementation.
   - `IndicatifReporter` (lines 171-380): Multi-progress bar reporter using
     `indicatif::MultiProgress` with `ProgressDrawTarget::stderr_with_hz(12)`.

2. **Timing wrapper** (`src/status_timing.rs`):
   - `VerboseTimingReporter`: Decorator that wraps any reporter to add timing
     metrics, writes timing summary to stderr.

3. **Pipeline stages** (`src/status_pipeline.rs`):
   - Defines the six pipeline stages and their descriptions.
   - `report_pipeline_stage()` helper for stage transitions.

4. **Runner module** (`src/runner/mod.rs`):
   - `make_reporter()` (lines 106-125): Factory function for selecting reporter.
   - `handle_build()` and `handle_ninja_tool()`: Orchestrate build execution.
   - `on_task_progress_callback()`: Bridges Ninja status parsing to reporter.

5. **Process execution** (`src/runner/process/mod.rs`):
   - `spawn_and_stream_output()` (lines 276-316): Core subprocess I/O handling.
   - Spawns stderr forwarding on a separate thread (lines 289-294).
   - Drains stdout on main thread to preserve status callback ordering.
   - `run_ninja_with_status()`: Invokes Ninja with progress parsing.

6. **Streaming utilities** (`src/runner/process/streaming.rs`):
   - `forward_child_output()`: Basic byte-for-byte forwarding.
   - `forward_child_output_with_ninja_status()`: Parses Ninja status lines and
     invokes observer callback.
   - `NinjaStatusParsingReader`: Reader wrapper that extracts status updates.

### Current stream routing

Based on code analysis:

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
3. **Documentation**: Update the user's guide to document the stream behavior.

## Plan of work

### Stage A: Audit current implementation (verification)

The code audit above suggests the implementation may already be correct. Stage A
confirms this with targeted verification:

1. Review `src/status.rs` to confirm all `writeln!` calls use `io::stderr()`.
2. Review `src/status_timing.rs` to confirm timing summary writes to stderr.
3. Review `src/runner/process/mod.rs` line 300-310 to confirm stdout forwarding
   path does not invoke status callbacks that could write to stdout.
4. Review `src/runner/process/streaming.rs` to confirm the observer callback
   pattern preserves separation.

If any stdout usage is found in status paths, document it in `Decision Log` and
proceed to fix it in Stage C.

### Stage B: Configuration design

The roadmap item mentions using `ortho_config` for ergonomic configuration. We
should evaluate whether any new configuration is actually needed:

- If the current implementation already correctly separates streams, no new
  configuration may be required.
- If users need to override stream destinations (e.g., for testing or custom
  tooling), design a minimal configuration schema.

**Decision point**: Determine whether configuration is needed based on Stage A
findings.

### Stage C: Implementation (if needed)

If Stage A reveals issues:

1. Fix any status output paths that incorrectly use stdout.
2. Add any necessary synchronization to prevent interleaving issues.
3. Ensure the observer callback in `spawn_and_stream_output()` does not write
   to stdout.

### Stage D: End-to-end tests

Add BDD scenarios to `tests/features/progress_output.feature`:

```gherkin
Scenario: Subprocess stdout is separate from status messages
  Given a minimal Netsuke workspace
  And a fake ninja executable that emits output to stdout
  When netsuke is run with arguments "--accessible true --progress true build"
  Then the command should succeed
  And stdout should contain "fake ninja output"
  And stdout should not contain "Stage 1/6"
  And stderr should contain "Stage 1/6"
  And stderr should not contain "fake ninja output"

Scenario: Build artifacts can be captured via stdout redirection
  Given a minimal Netsuke workspace
  When netsuke is run with arguments "--progress true graph"
  Then the command should succeed
  And stdout should contain "digraph"
  And stdout should not contain "Stage"
  And stderr should contain "Stage 1/6"
```

Implement the step definitions in `tests/bdd/steps/progress_output.rs`:

1. Create a fake Ninja executable fixture that emits known output to stdout.
2. Add assertions for stdout content verification.
3. Add assertions that verify stream exclusivity.

### Stage E: Documentation

Update `docs/users-guide.md` to add a section on output streams:

```markdown
## Output streams

Netsuke separates its output into two streams for scriptability:

- **stderr**: Status messages, progress indicators, and diagnostics
- **stdout**: Subprocess output (e.g., `ninja -t graph` produces DOT on stdout)

This separation allows reliable piping and redirection:

```bash
# Capture build graph without status noise
netsuke graph > build.dot

# Capture progress log without build output
netsuke build 2> progress.log

# Suppress status messages entirely
netsuke --progress false build
```

### Validation gates

At each stage transition:

1. `make check-fmt` must pass.
2. `make lint` must pass.
3. `make test` must pass.

## Concrete steps

### Stage A verification commands

```bash
# Verify stderr usage in status.rs
grep -n "io::stdout\|stdout()" src/status.rs
# Expected: No matches (all output should be stderr)

grep -n "io::stderr\|stderr()" src/status.rs
# Expected: Multiple matches at lines 133, 140, 145, 247, 336, 378

# Verify stderr in status_timing.rs
grep -n "io::stdout\|stdout()" src/status_timing.rs
# Expected: No matches

# Verify subprocess stdout forwarding
grep -n "io::stdout" src/runner/process/mod.rs
# Expected: Line 300 (stdout lock for forwarding subprocess output)

# Run existing tests
make test
```

### Stage D test implementation

After Stage A confirms the architecture is correct:

```bash
# Add new feature file scenarios
# Edit tests/features/progress_output.feature

# Add step definitions
# Edit tests/bdd/steps/progress_output.rs

# Run BDD tests
cargo test --test rstest_bdd -- progress_output
```

### Stage E documentation update

```bash
# Update users guide
# Edit docs/users-guide.md

# Verify markdown formatting
make markdownlint
```

## Validation and acceptance

Quality criteria:

- **Tests**: All tests pass including new stream separation scenarios.
- **Lint/typecheck**: `make check-fmt` and `make lint` pass.
- **Documentation**: Users guide documents stream behavior.

Quality method:

- Run `make check-fmt && make lint && make test` after each stage.
- Manual verification: Run `netsuke graph 2>/dev/null` and confirm only DOT
  output appears.

Observable behavior after completion:

1. Running `netsuke graph > output.dot 2>&1` captures build graph to file
   without status messages in the DOT content.
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

- Status reporter trait: `src/status.rs:99-110`
- Accessible reporter stderr writes: `src/status.rs:133,140,145`
- Indicatif stderr target: `src/status.rs:179`
- Subprocess stdout forwarding: `src/runner/process/mod.rs:299-310`
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

### Test fixtures to add

In `tests/bdd/steps/progress_output.rs`:

```rust
/// Fixture for a fake Ninja executable that emits known stdout output.
#[fixture]
fn fake_ninja_with_stdout_output() -> /* TempDir or similar */ {
    // Create script that echoes known content to stdout
}
```

### BDD step definitions to add

```rust
#[then("stdout should contain {text}")]
fn stdout_should_contain(cli_output: &CliOutput, text: &str) {
    assert!(cli_output.stdout.contains(text));
}

#[then("stdout should not contain {text}")]
fn stdout_should_not_contain(cli_output: &CliOutput, text: &str) {
    assert!(!cli_output.stdout.contains(text));
}
```

Note: These step definitions may already exist in the codebase; verify before
adding duplicates.

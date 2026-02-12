# Add accessible output mode (roadmap 3.8.1)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

No `PLANS.md` file exists in this repository.

## Purpose / big picture

Netsuke currently produces no user-visible progress output during its five-stage
build pipeline (network policy through build execution). Future work (roadmap
3.9) will add animated spinners and progress bars via `indicatif`. Before that
can happen, the tool needs an accessible output mode that guarantees
screen-reader-friendly, static text output for users who cannot consume animated
terminal UI.

After this change:

- Users on `TERM=dumb` terminals, users with `NO_COLOR` set, and users who
  explicitly set `accessible = true` (via config file, `NETSUKE_ACCESSIBLE`
  environment variable, or `--accessible` CLI flag) receive static, labelled
  status lines instead of any future animated output.
- Every pipeline stage emits a textual status line to stderr (e.g.,
  "Stage 1/5: Configuring network policy") when accessible mode is active.
- The accessible mode detection is wired through the existing OrthoConfig
  layered configuration system.
- The output mode abstraction provides the foundation that future spinner work
  (3.9) will plug into, so that non-accessible terminals get animated progress
  while accessible terminals always get static text.

Observable success: running `TERM=dumb netsuke build` (or `NO_COLOR=1` or
`--accessible`) against a valid manifest produces static "Stage N/5: ..." lines
on stderr before the Ninja output. Running `make check-fmt && make lint &&
make test` passes with new unit and BDD tests covering the detection logic and
status output.

## Constraints

- No file may exceed 400 lines.
- Comments and documentation must use en-GB-oxendict spelling.
- Module-level `//!` doc comments are required on every new module.
- No `unsafe` code. No `expect()` in production code; return `Result` and
  propagate with `?`.
- Use `rstest` fixtures for unit tests and `rstest-bdd` v0.5.0 for
  behavioural tests.
- Existing public API signatures in `src/cli/mod.rs` must remain backward
  compatible (new fields with defaults are acceptable).
- The `Cli::default()` implementation must remain consistent with the new
  field (`Cli::default()` sets `accessible` to `None`, enabling auto-detection
  via `src/cli/mod.rs`).
- Fluent message keys must be added to both `locales/en-US/messages.ftl` and
  `locales/es-ES/messages.ftl`.
- `docs/users-guide.md` must document the new flag and auto-detection
  behaviour.
- `docs/roadmap.md` entry 3.8.1 must be marked done on completion.
- `AGENTS.md` quality gates apply: `make check-fmt`, `make lint`, and
  `make test` must pass before completion.

## Tolerances (exception triggers)

- Scope: if the implementation requires changes to more than 18 files or more
  than 800 net new lines, stop and escalate.
- Dependencies: no new external crate dependencies. The implementation must use
  only `std::env` for environment variable detection.
- Interfaces: if a public API signature must change (as opposed to adding a new
  field with a default), stop and escalate.
- Tests: if `make test` still fails after three investigation cycles, stop and
  escalate.
- Ambiguity: if the interaction between `NO_COLOR`, `TERM=dumb`, and explicit
  config produces conflicting signals, escalate with the options and
  trade-offs.

## Risks

- Risk: Adding a new field to `Cli` may break existing config files that use
  `deny_unknown_fields` or strict parsing.
  Severity: low. Likelihood: low. Mitigation: OrthoConfig merge is additive;
  unknown fields in config files are ignored by default. Verify with existing
  config-merge tests.

- Risk: The `cli_overrides_from_matches` function strips non-CLI-sourced fields
  and must be updated for the new `accessible` field, or it will be silently
  dropped during merge.
  Severity: high. Likelihood: high. Mitigation: Add `"accessible"` to the
  `value_source` check array in `cli_overrides_from_matches`. Covered by a
  config-merge BDD scenario.

- Risk: Emitting status lines to stderr during the pipeline may interleave with
  tracing output when `--verbose` is active.
  Severity: low. Likelihood: medium. Mitigation: Status lines use
  `writeln!(io::stderr(), ...)` which holds the stderr lock atomically per
  line, matching tracing behaviour. In verbose mode, both streams are
  informational, so interleaving is acceptable.

## Progress

- [x] Write ExecPlan to `docs/execplans/3-8-1-add-accessible-output-mode.md`.
- [x] Stage A: Define `OutputMode` enum and detection logic in new module
      `src/output_mode.rs`.
- [x] Stage A: Add unit tests for `OutputMode` detection (rstest parameterized).
- [x] Stage B: Add `accessible` field to `Cli` struct with OrthoConfig wiring.
- [x] Stage B: Update `cli_overrides_from_matches` to include `accessible`.
- [x] Stage B: Update `Cli::default()` to include `accessible: None`.
- [x] Stage B: Add Fluent message keys and translations.
- [x] Stage C: Create `src/status.rs` module with `StatusReporter` trait and
      `AccessibleReporter` implementation.
- [x] Stage C: Wire status reporting into `runner::run` pipeline.
- [x] Stage D: Write BDD feature file and step definitions for accessible
      output mode.
- [x] Stage D: Update `docs/users-guide.md` with accessible mode documentation.
- N/A Stage D: Update `docs/netsuke-cli-design-document.md` with design
      decisions. (Already documented in design document lines 224-243.)
- [x] Stage D: Mark roadmap entry 3.8.1 as done.
- [x] Stage D: Run `make check-fmt && make lint && make test` and verify all
      pass.
- [x] Stage D: Run `make fmt && make markdownlint && make nixie` for
      documentation QA.

## Surprises & discoveries

- The project has `clippy::print_stderr` denied globally, so `eprintln!`
  cannot be used. The `AccessibleReporter` uses
  `drop(writeln!(io::stderr(), ...))` following the pattern from `main.rs`.
- BDD step "the environment variable X is set to Y" already exists in
  `manifest/mod.rs` and sets the real process environment. To avoid
  test interference, the accessible output BDD scenarios use
  "the simulated TERM/NO_COLOR is" steps that store values in `TestWorld`
  and pass them to `resolve_with` via a closure.
- `Cli::default()` for `accessible` uses `None` (not `false`) since the
  field is `Option<bool>` to support tri-state auto-detection.

## Decision log

- Decision: Model the output mode as a two-variant enum (`Accessible` vs
  `Standard`) rather than a boolean.
  Rationale: Extensibility for future modes (e.g., `Json`, `Quiet`) without
  changing call sites. The enum also makes match exhaustiveness checks
  enforce handling all modes.
  Date/Author: 2026-02-09 (plan phase).

- Decision: Use `writeln!(io::stderr(), ...)` for status lines rather than
  `tracing::info!`. (Originally planned as `eprintln!`, changed to `writeln!`
  due to the global `clippy::print_stderr` denial.)
  Rationale: Status lines are user-facing progress output, not diagnostic logs.
  They should appear at the default verbosity level (tracing is gated to
  `ERROR` unless `--verbose` is set). Writing directly to stderr ensures they
  always appear regardless of tracing configuration.
  Date/Author: 2026-02-09 (plan phase; updated during implementation).

- Decision: Auto-detection checks `NO_COLOR` (any value), `TERM=dumb`, and
  the explicit `accessible` config field, with explicit config taking
  precedence.
  Rationale: `NO_COLOR` is a de facto standard[^1].
  `TERM=dumb` is standard for dumb terminals and screen readers. Explicit
  config (`--accessible`, `NETSUKE_ACCESSIBLE`, or config file) overrides
  auto-detection in both directions (user can force accessible on or off).
  Date/Author: 2026-02-09 (plan phase).

- Decision: The `accessible` field is `Option<bool>` in `Cli` (tri-state:
  `None` = auto-detect, `Some(true)` = force on, `Some(false)` = force off).
  Rationale: This allows the auto-detection logic to serve as a sensible
  default while still honouring explicit user intent in either direction.
  Date/Author: 2026-02-09 (plan phase).

## Outcomes & retrospective

Implementation complete. All quality gates pass (`make check-fmt`,
`make lint`, `make test`).

**What went well:**

- The `resolve_with` dependency-injection pattern for environment variable
  lookup made both unit tests and BDD tests clean and deterministic.
- The `OutputMode` enum and `StatusReporter` trait provide a clean
  abstraction that future spinner work (roadmap 3.9) can plug into.
- The plan's staged approach (A: detection, B: CLI, C: status, D: testing)
  worked well with checkpoint validation between stages.

**What was surprising:**

- `clippy::print_stderr` is denied globally, requiring `writeln!(io::stderr())`
  instead of `eprintln!`. The `drop()` wrapper follows the `main.rs` pattern.
- The BDD `Given the environment variable` step already existed in
  `manifest/mod.rs` and sets real process env vars. Using "simulated"
  env vars in the TestWorld avoided test interference.

**Metrics:**

- 4 new files created, 10 files modified.
- 12 unit tests (output_mode), 7 BDD scenarios (accessible_output).
- ~300 net new lines of code (well within the 800-line tolerance).

## Context and orientation

### Repository structure (relevant files)

- `src/lib.rs` — crate root; declares public modules.
- `src/main.rs` — entry point; parses CLI, merges config, calls
  `runner::run`.
- `src/cli/mod.rs` — `Cli` struct (clap + OrthoConfig), parsing, config
  merge. The `cli_overrides_from_matches` function (line 267) strips fields
  not explicitly provided on the command line.
- `src/runner/mod.rs` — `run()` dispatches to `handle_build`,
  `handle_clean`, `handle_graph`; `generate_ninja()` runs the first four
  pipeline stages.
- `src/runner/process/mod.rs` — Ninja subprocess management and I/O
  streaming.
- `src/localization/keys.rs` — Fluent message key constants via
  `define_keys!` macro.
- `locales/en-US/messages.ftl` — English Fluent translations.
- `locales/es-ES/messages.ftl` — Spanish Fluent translations.
- `tests/bdd_tests.rs` — BDD entry point;
  `scenarios!("tests/features", fixtures = [world: TestWorld])`.
- `tests/bdd/fixtures/mod.rs` — `TestWorld` struct for BDD state.
- `tests/bdd/steps/mod.rs` — step definition module declarations.
- `tests/bdd/types.rs` — newtype wrappers for BDD step parameters.
- `docs/users-guide.md` — end-user documentation.
- `docs/roadmap.md` — implementation roadmap with checkboxes.

### Key patterns

- **OrthoConfig** on `Cli`: fields get `#[arg(...)]` for clap,
  `#[ortho_config(default = ...)]` for defaults. Boolean flags with defaults
  use `#[ortho_config(default = false)]`. The `NETSUKE_` prefix provides
  automatic environment variable support.
- **Config merge**: `cli_overrides_from_matches` removes fields not explicitly
  set on the CLI so they don't override file/env layers. New boolean fields
  must be added to the `value_source` check array.
- **BDD steps**: use `#[given]`, `#[when]`, `#[then]` macros from
  `rstest_bdd_macros`. Steps receive `&TestWorld` and return `Result<()>`.
  New step modules are registered in `tests/bdd/steps/mod.rs`.
- **Localization**: keys defined in `src/localization/keys.rs` via
  `define_keys!`. Messages retrieved via
  `localization::message(keys::KEY_NAME)`.

### Terms

- **Accessible output mode**: a mode where all terminal output is static text
  with explicit labels, suitable for screen readers and dumb terminals.
- **OrthoConfig**: the layered configuration library used by Netsuke
  (defaults < config file < environment < CLI).
- **Fluent**: the localization framework; `.ftl` files contain message
  definitions with argument interpolation.
- **Pipeline stages**: the five sequential phases Netsuke executes: network
  policy configuration, manifest loading, dependency graph construction,
  Ninja file generation, and build execution.

## Plan of work

### Stage A: Output mode detection (new module, unit tests)

Create `src/output_mode.rs` defining the `OutputMode` enum and detection logic.

The module exports:

```rust
/// Whether terminal output should use accessible (static text) or
/// standard (potentially animated) formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// Static text output with explicit labels. Suitable for screen
    /// readers, dumb terminals, and CI environments.
    Accessible,
    /// Standard terminal output. May include animated progress indicators
    /// when future features are added.
    Standard,
}

/// Resolve the output mode from explicit configuration and environment.
///
/// Precedence:
/// 1. Explicit configuration (`accessible` field): `Some(true)` forces
///    `Accessible`, `Some(false)` forces `Standard`.
/// 2. `NO_COLOR` environment variable (any value): `Accessible`.
/// 3. `TERM=dumb`: `Accessible`.
/// 4. Default: `Standard`.
pub fn resolve(explicit: Option<bool>) -> OutputMode { ... }
```

The `resolve` function accepts the tri-state `Option<bool>` from CLI config.
It calls `std::env::var` for `NO_COLOR` and `TERM` internally but is also
provided with a testable variant `resolve_with` that accepts a closure for
environment variable lookup (matching the pattern used in
`src/runner/process/mod.rs` for `resolve_ninja_program_utf8_with`).

```rust
/// Testable variant that accepts an environment lookup function.
pub fn resolve_with<F>(explicit: Option<bool>, read_env: F) -> OutputMode
where
    F: Fn(&str) -> Option<String>,
{ ... }
```

Unit tests in the same file (gated by `#[cfg(test)]`) use `rstest`
parameterized cases:

```rust
#[rstest]
#[case(Some(true), None, None, OutputMode::Accessible)]
#[case(Some(false), Some("1"), Some("dumb"), OutputMode::Standard)]
#[case(None, Some("1"), None, OutputMode::Accessible)]
#[case(None, None, Some("dumb"), OutputMode::Accessible)]
#[case(None, None, Some("xterm-256color"), OutputMode::Standard)]
#[case(None, None, None, OutputMode::Standard)]
#[case(None, Some(""), None, OutputMode::Accessible)]
fn resolve_output_mode(
    #[case] explicit: Option<bool>,
    #[case] no_color: Option<&str>,
    #[case] term: Option<&str>,
    #[case] expected: OutputMode,
) { ... }
```

Register the new module in `src/lib.rs`:

```rust
pub mod output_mode;
```

### Stage B: CLI integration (config field, merge, localization)

**`src/cli/mod.rs`** — add `accessible` field to `Cli`:

```rust
/// Force accessible output mode on or off.
///
/// When set, overrides automatic detection from `NO_COLOR` and
/// `TERM=dumb`. When omitted, Netsuke auto-detects.
#[arg(long)]
pub accessible: Option<bool>,
```

Add `"accessible"` to the `value_source` check array in
`cli_overrides_from_matches` (alongside `"file"`, `"verbose"`, etc.) so it
is stripped from merge overrides when not explicitly set on the command line.

Update `Cli::default()` to include `accessible: None`.

**`src/localization/keys.rs`** — add keys:

```rust
CLI_FLAG_ACCESSIBLE_HELP => "cli.flag.accessible.help",
STATUS_STAGE_LABEL => "status.stage.label",
STATUS_STAGE_MANIFEST_LOAD => "status.stage.manifest_load",
STATUS_STAGE_NETWORK_POLICY => "status.stage.network_policy",
STATUS_STAGE_BUILD_GRAPH => "status.stage.build_graph",
STATUS_STAGE_GENERATE_NINJA => "status.stage.generate_ninja",
STATUS_STAGE_EXECUTE => "status.stage.execute",
STATUS_COMPLETE => "status.complete",
```

**`locales/en-US/messages.ftl`** — add:

```properties
cli.flag.accessible.help = Force accessible output mode on or off.
status.stage.label = Stage { $current }/{ $total }: { $description }
status.stage.manifest_load = Loading manifest
status.stage.network_policy = Configuring network policy
status.stage.build_graph = Building dependency graph
status.stage.generate_ninja = Generating Ninja file
status.stage.execute = Executing build
status.complete = Build complete.
```

**`locales/es-ES/messages.ftl`** — add corresponding Spanish translations.

**`src/cli_l10n.rs`** — add `accessible` to the localized help map so
`localize_command` applies the Fluent message to `--accessible`'s help text.

### Stage C: Status reporting (new module, pipeline wiring)

Create `src/status.rs` with a `StatusReporter` trait and concrete
implementations:

```rust
/// Report pipeline progress to the user.
pub trait StatusReporter {
    /// Emit a status line for the given pipeline stage.
    fn report_stage(&self, current: u32, total: u32, description: &str);
    /// Emit a completion message.
    fn report_complete(&self);
}

/// Accessible reporter: writes static labelled lines to stderr.
pub struct AccessibleReporter;

/// Silent reporter: emits nothing (used in standard mode until
/// future spinner work adds an animated reporter).
pub struct SilentReporter;
```

The `AccessibleReporter` uses `localization::message` with the
`STATUS_STAGE_LABEL` key to produce localized status lines, writing them via
`drop(writeln!(io::stderr(), ...))` (not `eprintln!`, which is denied by the
global `clippy::print_stderr` lint).

**`src/runner/mod.rs`** — modify `generate_ninja` to accept a
`&dyn StatusReporter` and call `report_pipeline_stage` at each stage:

1. Before `cli.network_policy()` — stage 1 "Configuring network policy"
2. Before `manifest::from_path_with_policy` — stage 2 "Loading manifest"
3. Before `BuildGraph::from_manifest` — stage 3 "Building dependency graph"
4. Before `ninja_gen::generate` — stage 4 "Generating Ninja file"
5. (`handle_build` / `handle_ninja_tool`) Before `run_ninja` /
   `run_ninja_tool` — stage 5 "Executing {tool}"
6. (`handle_build` / `handle_ninja_tool`) After successful completion —
   "{tool} complete."

The `run` function resolves the `OutputMode` using
`output_mode::resolve(cli.accessible)` and creates the appropriate reporter
(`AccessibleReporter` or `SilentReporter`).

### Stage D: Testing, documentation, cleanup

**BDD feature file**: `tests/features/accessible_output.feature`

```gherkin
Feature: Accessible output mode

  Scenario: Accessible mode is auto-detected from TERM=dumb
    Given the environment variable "TERM" is set to "dumb"
    When the output mode is resolved with no explicit setting
    Then the output mode is accessible

  Scenario: Accessible mode is auto-detected from NO_COLOR
    Given the environment variable "NO_COLOR" is set to "1"
    When the output mode is resolved with no explicit setting
    Then the output mode is accessible

  Scenario: Explicit accessible flag overrides TERM
    Given the environment variable "TERM" is set to "xterm-256color"
    When the output mode is resolved with accessible set to true
    Then the output mode is accessible

  Scenario: Explicit non-accessible overrides NO_COLOR
    Given the environment variable "NO_COLOR" is set to "1"
    When the output mode is resolved with accessible set to false
    Then the output mode is standard

  Scenario: Default output mode is standard
    When the output mode is resolved with no explicit setting
    Then the output mode is standard

  Scenario: CLI parses accessible flag
    When the CLI is parsed with "--accessible true"
    Then parsing succeeds
    And accessible mode is enabled

  Scenario: CLI parses accessible false
    When the CLI is parsed with "--accessible false"
    Then parsing succeeds
    And accessible mode is disabled
```

**BDD step definitions**: `tests/bdd/steps/accessible_output.rs`

New step module registered in `tests/bdd/steps/mod.rs`. The `TestWorld`
struct needs new fields:

```rust
/// Resolved output mode for accessible output scenarios.
pub output_mode: Slot<String>,
```

Steps use the `resolve_with` function to test detection without mutating the
real environment.

**`tests/bdd/steps/mod.rs`** — add `mod accessible_output;`.

**`tests/bdd/fixtures/mod.rs`** — add `output_mode: Slot<String>` field to
`TestWorld`.

**`docs/users-guide.md`** — add a new subsection under the CLI/Configuration
section:

```markdown
### Accessible output mode

Netsuke supports an accessible output mode that replaces animated progress
indicators with static, labelled status lines suitable for screen readers
and dumb terminals.

Accessible mode is auto-enabled when:

- `TERM` is set to `dumb`
- `NO_COLOR` is set (any value)

Accessible mode can be forced on or off:

- CLI flag: `--accessible true` or `--accessible false`
- Environment variable: `NETSUKE_ACCESSIBLE=true`
- Configuration file: `accessible = true`

When accessible mode is active, each pipeline stage produces a labelled
status line on stderr:

    Stage 1/5: Configuring network policy
    Stage 2/5: Loading manifest
    Stage 3/5: Building dependency graph
    Stage 4/5: Generating Ninja file
    Stage 5/5: Executing Build
    Build complete.
```

**`docs/roadmap.md`** — change `- [ ] 3.8.1.` to `- [x] 3.8.1.` and the
sub-items likewise.

## Concrete steps

All commands are run from the repository root `/home/user/project`.

1. Write this ExecPlan to
   `docs/execplans/3-8-1-add-accessible-output-mode.md`.

2. Create `src/output_mode.rs` with the `OutputMode` enum, `resolve`,
   `resolve_with`, and unit tests. Register in `src/lib.rs`.

3. Run `make check-fmt && make lint && make test` — expect existing tests to
   pass and new unit tests in `output_mode` to pass.

4. Add `accessible: Option<bool>` to `Cli` in `src/cli/mod.rs`. Update
   `Default`, `cli_overrides_from_matches`, and help localization.

5. Add Fluent keys to `src/localization/keys.rs` and translations to both
   `.ftl` files.

6. Run `make check-fmt && make lint && make test` — expect pass.

7. Create `src/status.rs` with `StatusReporter` trait, `AccessibleReporter`,
   and `SilentReporter`. Register in `src/lib.rs`.

8. Wire status reporting into `src/runner/mod.rs` (`generate_ninja` and
   command handlers).

9. Run `make check-fmt && make lint && make test` — expect pass.

10. Create `tests/features/accessible_output.feature`, step definitions in
    `tests/bdd/steps/accessible_output.rs`, register in steps `mod.rs`,
    update `TestWorld` fixtures.

11. Run `make check-fmt && make lint && make test` — expect all tests pass
    including new BDD scenarios.

12. Update `docs/users-guide.md` with accessible mode documentation.

13. Mark `docs/roadmap.md` entry 3.8.1 as done.

14. Run `make fmt && make markdownlint && make nixie` for documentation QA.

15. Final `make check-fmt && make lint && make test` via tee to confirm.

Expected final test output (appended to existing):

```plaintext
test output_mode::tests::resolve_output_mode::case_1 ... ok
test output_mode::tests::resolve_output_mode::case_2 ... ok
...
test bdd_tests::accessible_output::... ... ok
```

## Validation and acceptance

Quality criteria:

- Tests: `make test` passes. New tests:
  - `output_mode::tests::resolve_output_mode` (7+ parameterized cases)
  - BDD scenarios in `accessible_output.feature` (7 scenarios)
- Lint: `make lint` passes (clippy, rustdoc).
- Format: `make check-fmt` passes.
- Documentation: `make fmt`, `make markdownlint`, and `make nixie` pass.

Quality method:

```shell
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt.log
make lint 2>&1 | tee /tmp/lint.log
make test 2>&1 | tee /tmp/test.log
make fmt 2>&1 | tee /tmp/fmt.log
make markdownlint 2>&1 | tee /tmp/markdownlint.log
make nixie 2>&1 | tee /tmp/nixie.log
```

Manual verification:

```shell
# With a valid Netsukefile in the current directory:
TERM=dumb cargo run -- build 2>&1 | grep "Stage"
# Should show "Stage 1/5: Configuring network policy" etc. on stderr

NO_COLOR=1 cargo run -- build 2>&1 | grep "Stage"
# Same output

cargo run -- --accessible true build 2>&1 | grep "Stage"
# Same output

cargo run -- --accessible false build 2>&1 | grep "Stage"
# No "Stage" lines (standard mode, currently silent)
```

## Idempotence and recovery

All steps are additive (new files and new fields with defaults). No existing
behaviour is changed; standard mode remains silent until future spinner work
adds animated output.

If a step fails partway through, fix the issue and re-run the validation
command. No rollback is needed beyond `git checkout` of the affected files.

## Artifacts and notes

Key files created:

- `src/output_mode.rs` — `OutputMode` enum and detection logic
- `src/status.rs` — `StatusReporter` trait and implementations
- `tests/features/accessible_output.feature` — BDD scenarios
- `tests/bdd/steps/accessible_output.rs` — BDD step definitions

Key files modified:

- `src/lib.rs` — register new modules
- `src/cli/mod.rs` — add `accessible` field, update merge
- `src/runner/mod.rs` — wire status reporting into pipeline
- `src/localization/keys.rs` — add Fluent key constants
- `locales/en-US/messages.ftl` — English translations
- `locales/es-ES/messages.ftl` — Spanish translations
- `tests/bdd/fixtures/mod.rs` — add `output_mode` field to `TestWorld`
- `tests/bdd/steps/mod.rs` — register new step module
- `docs/users-guide.md` — document accessible mode
- `docs/roadmap.md` — mark 3.8.1 done

## Interfaces and dependencies

No new external dependencies. All detection uses `std::env`.

In `src/output_mode.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Accessible,
    Standard,
}

pub fn resolve(explicit: Option<bool>) -> OutputMode
pub fn resolve_with<F>(explicit: Option<bool>, read_env: F) -> OutputMode
where
    F: Fn(&str) -> Option<String>
```

In `src/status.rs`:

```rust
pub trait StatusReporter {
    fn report_stage(&self, current: u32, total: u32, description: &str);
    fn report_complete(&self);
}

pub struct AccessibleReporter;
pub struct SilentReporter;
```

In `src/cli/mod.rs`, the `Cli` struct gains:

```rust
pub accessible: Option<bool>,
```

[^1]: <https://no-color.org/>

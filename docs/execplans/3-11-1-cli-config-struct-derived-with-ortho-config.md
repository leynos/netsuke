# 3.11.1. Introduce `CliConfig` struct derived with OrthoConfig v0.8.0

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: IMPLEMENTED

## Purpose / big picture

Netsuke already derives `OrthoConfig` on the `Cli` struct in `src/cli/mod.rs`,
but the schema is incomplete: several user-facing preferences that the roadmap
specifies—colour policy, spinner mode, output format, and default build
targets—are not yet representable through the configuration file or environment
variable layers. The current `ortho_config` dependency is pinned at `0.7.0`
while the roadmap calls for `0.8.0`. Moreover, the `Cli` struct conflates
argument parsing with configuration shape; the roadmap envisions a dedicated
`CliConfig` that separates the configuration schema from the `clap` parsing
surface.

After this work is complete:

1. A new `CliConfig` struct (derived with `OrthoConfig` v0.8.0) captures
   every user-configurable preference: verbosity, colour policy, locale,
   spinner mode, output format, default build targets, and theme.
2. All seven fields are configurable via CLI flags, `NETSUKE_`-prefixed
   environment variables, and TOML/YAML/JSON5 configuration files, with the
   standard OrthoConfig precedence (defaults < file < env < CLI).
3. The existing `Cli` struct delegates to `CliConfig` for preference
   resolution, keeping backward compatibility for all current flags and
   environment variables.
4. `make check-fmt`, `make lint`, and `make test` pass, with new `rstest`
   unit tests and `rstest-bdd` v0.5.0 behavioural scenarios covering happy
   paths, unhappy paths, and edge cases.
5. `docs/users-guide.md` documents the new configuration fields.
6. `docs/netsuke-design.md` records the architectural decision.
7. Roadmap item 3.11.1 is marked done.

Observable success means a user can write a `.netsuke.toml` file such as:

```toml
verbose = true
colour_policy = "always"
spinner_mode = "disabled"
output_format = "human"
theme = "ascii"
default_targets = ["all", "test"]
```

and have those values honoured when running `netsuke` without any CLI flags,
overridden by `NETSUKE_COLOUR_POLICY=auto`, and further overridden by
`--colour-policy never` on the command line.

## Constraints

- Preserve all existing CLI flags, environment variables, and configuration
  file keys. The change must be backward-compatible: existing `.netsuke.toml`
  files, `NETSUKE_*` environment variables, and CLI invocations must continue
  to work without modification.
- Keep the CLI localizable. Every new flag or validation error must be wired
  through `src/cli_l10n.rs`, `src/localization/keys.rs`, and both Fluent
  bundles (`locales/en-US/messages.ftl` and `locales/es-ES/messages.ftl`).
- Use `ortho_config` v0.8.0 for the configuration surface. Do not introduce
  ad-hoc parsers or a separate precedence ladder.
- No single source file may exceed 400 lines (per `AGENTS.md`).
- Do not add `unsafe` code or suppress lints without narrow, documented
  justification.
- Do not modify the Ninja synthesis, IR generation, or manifest expansion
  pipeline. This work is scoped to CLI and configuration.
- Any new shared helper in `src/cli/mod.rs`, `src/cli_l10n.rs`, or related
  modules must be anchored in `build.rs` using the established `const _`
  pattern, or `make lint` will fail with dead-code warnings.
- Mark roadmap item 3.11.1 done in `docs/roadmap.md` only after full
  validation.

## Tolerances (exception triggers)

- Scope: if the implementation requires more than 25 files changed or more
  than 1200 net new lines, stop and escalate.
- Dependencies: if upgrading `ortho_config` from 0.7.0 to 0.8.0 introduces
  breaking API changes beyond what is documented in the user's guide, stop and
  escalate.
- Interfaces: if the change requires removing any existing public function or
  changing an existing public API signature in a way that breaks downstream
  callers, stop and escalate.
- Iterations: if `make test` or `make lint` still fail after three focused
  fix-and-rerun cycles within a single stage, stop and document the blocking
  failures.
- Ambiguity: if the mapping between a new configuration concept (e.g.,
  "spinner mode") and its runtime effect is unclear, stop and present options
  with trade-offs before implementing.

## Risks

- Risk: upgrading `ortho_config` from 0.7.0 to 0.8.0 may introduce API
  changes or new derive macro behaviour. Severity: medium Likelihood: medium
  Mitigation: read the ortho-config user's guide (already present at
  `docs/ortho-config-users-guide.md`) before coding. Pin the upgrade early
  (Stage A) and run `make lint` + `make test` to surface breakage before
  proceeding.

- Risk: introducing `CliConfig` as a separate struct may break the existing
  `merge_with_config` and `resolve_merged_diag_json` functions that assume
  `Cli` is the OrthoConfig merge root. Severity: high Likelihood: high
  Mitigation: use an incremental approach. First add the new fields to the
  existing `Cli` struct, then extract `CliConfig` as a flattened inner struct.
  `Cli` continues to be the `clap::Parser` and OrthoConfig merge root; it just
  delegates configuration fields to `CliConfig`.

- Risk: adding six new CLI flags may cause short-flag collisions in clap's
  auto-derivation. Severity: low Likelihood: medium Mitigation: use explicit
  `cli_short` on new fields where collisions arise, or omit short flags for
  less-frequent options. Test parsing immediately after adding each field.

- Risk: environment variable mutations in BDD tests can deadlock or become
  flaky if `EnvLock` is not held for the entire scenario lifetime. Severity:
  high Likelihood: medium Mitigation: use the wrappers in `test_support::env`
  instead of raw `std::env::set_var` calls. `test_support::env::set_var()`
  acquires `EnvLock` internally for one-off updates, `VarGuard` gives RAII-safe
  scoped restoration, and `remove_var()` mirrors the same pattern for unsets.
  When a scenario needs batched cleanup, collect the original values and
  restore them with `restore_many()` from `TestWorld::drop`, or keep
  `VarGuard`s alive for the scenario lifetime. For BDD scenarios specifically,
  `tests/bdd/helpers/env_mutation::mutate_env_var()` provides a convenient
  wrapper that acquires the scenario `EnvLock` internally and tracks variables
  for automatic cleanup via `TestWorld::drop`. Replace any examples that
  reference `std::env::set_var` directly with `test_support::env::set_var` or
  `mutate_env_var`, and show `VarGuard` for scoped restores so `EnvLock` is
  always respected.

- Risk: `build.rs` symbol anchoring may be missed for new shared helpers,
  causing `make lint` failures. Severity: medium Likelihood: high Mitigation:
  add `const _` anchors in `build.rs` for every new shared function before
  running `make lint`.

## Progress

- [x] (completed) Stage A: Upgrade `ortho_config` to v0.8.0 and verify
      existing tests pass.
- [x] (completed) Stage B: Define new configuration types (`ColourPolicy`,
      `SpinnerMode`, `OutputFormat`) and add them to `Cli`.
- [x] (completed) Stage C: Introduce `CliConfig`, wire runtime resolution, and
      update `config_merge.rs`.
- [x] (completed) Stage D: Add localization keys and Fluent translations.
- [x] (completed) Stage E: Add unit tests with `rstest`.
- [x] (completed) Stage F: Add BDD scenarios with `rstest-bdd` v0.5.0.
- [x] (completed) Stage G: Update documentation, design record, roadmap, and
      run final validation.

## Surprises & discoveries

- `clap` + `OrthoConfig` + `serde(flatten)` on a nested `CliConfig` field did
  not compose cleanly in this codebase: clap attempted to parse the entire
  nested struct as a value instead of flattening it. The implementation kept
  `Cli` as the merge root and parser surface, then exposed a shared `CliConfig`
  view via `Cli::config()` and `From<&Cli> for CliConfig`.
- The new typed aliases (`spinner_mode`, `output_format`) needed early
  resolution before full merge completion so startup JSON diagnostics could
  still honour `output_format = "json"` from files and environment variables.
- `default_targets` runtime behaviour was easiest to validate in
  `tests/runner_tests.rs` with a purpose-built fake Ninja script that records
  argv, rather than by snapshotting generated Ninja manifests.

## Decision log

- Decision: add new fields to the existing `Cli` struct first, then extract
  `CliConfig` as a `#[serde(flatten)]`-ed inner struct, rather than introducing
  `CliConfig` as a standalone top-level OrthoConfig root. Rationale: the
  existing `merge_with_config`, `resolve_merged_diag_json`, and
  `cli_overrides_from_matches` functions all assume `Cli` is the OrthoConfig
  merge root. A `#[serde(flatten)]` extraction preserves serialization
  compatibility while giving the codebase a clean `CliConfig` type that
  downstream features can reference. Date/Author: 2026-03-23 / ExecPlan.

- Decision: model `colour_policy` as a three-valued enum (`Auto`, `Always`,
  `Never`) matching the `NO_COLOR` spec and common Rust CLI precedent. `Auto`
  detects from `NO_COLOR` and terminal capability. `Always` forces colours even
  on pipes. `Never` suppresses colours unconditionally. Rationale: this is the
  standard pattern across `cargo`, `ripgrep`, `bat`, and the `NO_COLOR`
  specification. It gives users predictable control without inventing a new
  vocabulary. Date/Author: 2026-03-23 / ExecPlan.

- Decision: model `spinner_mode` as a two-valued enum (`Enabled`,
  `Disabled`) rather than a freeform string or boolean. This gives a type-safe
  configuration surface while leaving room to add presets (e.g., `Dots`,
  `Braille`) in a future milestone. Rationale: the current
  `progress: Option<bool>` already controls spinner visibility. `SpinnerMode`
  formalizes this into the configuration schema while keeping the legacy
  `progress` flag as an alias. Date/Author: 2026-03-23 / ExecPlan.

- Decision: model `output_format` as an enum (`Human`, `Json`) that maps to
  the existing `diag_json` boolean. The field name `output_format` is more
  discoverable in configuration files than `diag_json`, and the enum is
  forward-compatible with formats like `tap` or `junit`. Rationale: `diag_json`
  is a boolean that only makes sense on the CLI. In a config file,
  `output_format = "json"` is clearer. The merge layer resolves both
  representations. Date/Author: 2026-03-23 / ExecPlan.

- Decision: `default_targets` is a `Vec<String>` with `append` merge
  strategy, paralleling `fetch_allow_scheme`. Rationale: users should be able
  to declare default build targets in a config file that accumulate with
  CLI-specified targets, matching the existing vector-append merge semantics.
  Date/Author: 2026-03-23 / ExecPlan.

- Decision: retain `Cli` as the concrete clap/OrthoConfig merge root and
  expose `CliConfig` as a typed extracted view instead of a flattened runtime
  field. Rationale: the attempted flattened-field extraction conflicted with
  clap derive behaviour in this repository, while the extracted-view approach
  still centralizes typed preference logic and preserves all existing CLI/file/
  environment names. Date/Author: 2026-03-27 / Codex.

## Outcomes & retrospective

- Upgraded to `ortho_config 0.8.0`.
- Added typed configuration enums and flags for `colour_policy`,
  `spinner_mode`, `output_format`, and repeatable `default_targets`.
- Added runtime alias resolution so `spinner_mode` supersedes `progress`,
  `output_format` supersedes `diag_json`, and `colour_policy` participates in
  `NO_COLOR`-driven mode/theme behaviour.
- Added unit, integration, and BDD coverage for parsing, merge precedence,
  default-target dispatch, and localized validation failures.

## Context and orientation

The following files and modules are relevant to this task. A novice should read
them in the order presented to build understanding.

**CLI definition and parsing:**

- `src/cli/mod.rs` (240 lines): defines the `Cli` struct with
  `#[derive(Parser, Serialize, Deserialize, OrthoConfig)]`, its subcommands
  (`Commands` enum), and the `parse_with_localizer_from` function that creates
  a localized clap command and parses arguments.
- `src/cli/config_merge.rs` (178 lines): implements `merge_with_config`
  (the 4-layer OrthoConfig merge), `resolve_merged_diag_json`, and
  `cli_overrides_from_matches` (which strips non-CLI-supplied fields from the
  merge).
- `src/cli/parsing.rs`: custom value parsers for jobs, locale, theme, and
  host patterns.
- `src/cli/validation.rs`: wires custom parsers into the clap command via
  `configure_validation_parsers`.

**Localization:**

- `src/cli_l10n.rs` (224 lines): maps clap argument IDs and subcommand
  names to Fluent message keys, localizes command/argument help text, and
  provides raw-argument extractors (`locale_hint_from_args`,
  `diag_json_hint_from_args`).
- `src/localization/keys.rs`: string constants for every Fluent message key
  used by the CLI.
- `locales/en-US/messages.ftl`, `locales/es-ES/messages.ftl`: Fluent
  bundles containing all user-facing CLI text.

**Output and theme:**

- `src/output_mode.rs` (158 lines): resolves `Accessible` vs `Standard`
  output mode from `accessible`, `NO_COLOR`, and `TERM`.
- `src/output_prefs.rs`: resolved output formatting preferences, including
  semantic prefixes and spacing, backed by the theme system.
- `src/theme.rs`: defines `ThemePreference` (Auto, Unicode, Ascii),
  `DesignTokens`, and `resolve_theme`.

**Build script:**

- `build.rs`: compiles shared CLI modules via `#[path = ...]` and pins
  shared symbols with `const _` anchors. Any new public function in
  `src/cli/mod.rs` or `src/cli_l10n.rs` must be anchored here.

**Tests:**

- `tests/cli_tests/merge.rs`: integration tests for OrthoConfig layer
  precedence using `MergeComposer` and live `merge_with_config`.
- `tests/cli_tests/parsing.rs`: CLI argument parsing tests.
- `tests/cli_tests/policy.rs`: theme parsing and merge tests.
- `tests/bdd/` directory: BDD step definitions using `rstest-bdd` v0.5.0.
- `tests/features/*.feature`: Gherkin feature files for behavioural tests.

**Documentation:**

- `docs/ortho-config-users-guide.md`: the OrthoConfig v0.8.0 user's guide,
  documenting derive attributes, merge semantics, `ConfigDiscovery`,
  `MergeComposer`, and localization helpers.
- `docs/netsuke-design.md §8.2`: the original CLI structure specification.
- `docs/users-guide.md`: user-facing documentation.
- `docs/rstest-bdd-users-guide.md`: BDD testing framework guide.

**Key terms:**

- OrthoConfig: a Rust library that unifies CLI, environment, and config file
  values into a single typed struct via a derive macro.
- Merge layer: a tier in the configuration precedence stack (defaults <
  file < env < CLI).
- `MergeComposer`: OrthoConfig helper that assembles layers for unit tests
  without invoking the CLI parser.
- Fluent: the localization framework used by Netsuke for all user-facing
  strings.
- `EnvLock`: a test-support mutex that serializes environment variable
  mutations across parallel tests.

## Interfaces and dependencies

### New types

In `src/cli/config.rs` (new file), define:

```rust
/// User-facing colour output policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColourPolicy {
    /// Detect colour support from terminal capability and `NO_COLOR`.
    #[default]
    Auto,
    /// Always emit ANSI colour codes, even when piped.
    Always,
    /// Never emit ANSI colour codes.
    Never,
}

/// Progress spinner display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpinnerMode {
    /// Show animated spinners when the terminal supports them.
    #[default]
    Enabled,
    /// Suppress animated spinners unconditionally.
    Disabled,
}

/// Diagnostic output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// Human-readable diagnostic output with colour and context.
    #[default]
    Human,
    /// Machine-readable JSON diagnostic output on stderr.
    Json,
}
```

Each enum must implement `FromStr` (for clap parsing) and `Display` (for
serialization and error messages), following the `ThemePreference` pattern in
`src/theme.rs`.

### Extended Cli struct

In `src/cli/mod.rs`, add the following fields to `Cli`:

```rust
/// Colour output policy (auto, always, never).
#[arg(long, value_name = "POLICY")]
pub colour_policy: Option<ColourPolicy>,

/// Spinner display mode (enabled, disabled).
#[arg(long, value_name = "MODE")]
pub spinner_mode: Option<SpinnerMode>,

/// Diagnostic output format (human, json).
#[arg(long, value_name = "FORMAT")]
pub output_format: Option<OutputFormat>,

/// Default build targets used when none are specified on the CLI.
#[arg(long = "default-target", value_name = "TARGET")]
#[ortho_config(merge_strategy = "append")]
pub default_targets: Vec<String>,
```

### Updated merge plumbing

In `src/cli/config_merge.rs`, add the new fields to the
`cli_overrides_from_matches` field list so that file/env values are not
overridden by absent CLI defaults:

```rust
for field in [
    "file", "verbose", "fetch_default_deny", "fetch_allow_scheme",
    "fetch_allow_host", "fetch_block_host", "accessible", "progress",
    "no_emoji", "theme", "diag_json",
    "colour_policy", "spinner_mode", "output_format", "default_targets",
] { ... }
```

### Localization keys

In `src/localization/keys.rs`, add:

```rust
pub const CLI_FLAG_COLOUR_POLICY_HELP: &str = "cli.flag.colour_policy.help";
pub const CLI_FLAG_SPINNER_MODE_HELP: &str = "cli.flag.spinner_mode.help";
pub const CLI_FLAG_OUTPUT_FORMAT_HELP: &str = "cli.flag.output_format.help";
pub const CLI_FLAG_DEFAULT_TARGETS_HELP: &str =
    "cli.flag.default_targets.help";
pub const CLI_COLOUR_POLICY_INVALID: &str =
    "cli.validation.colour_policy.invalid";
pub const CLI_SPINNER_MODE_INVALID: &str =
    "cli.validation.spinner_mode.invalid";
pub const CLI_OUTPUT_FORMAT_INVALID: &str =
    "cli.validation.output_format.invalid";
```

### External dependency

Upgrade `ortho_config` from `"0.7.0"` to `"0.8.0"` in both the `[dependencies]`
and `[build-dependencies]` sections of `Cargo.toml`.

## Plan of work

### Stage A: Upgrade `ortho_config` to v0.8.0 and stabilize

Before adding any new fields, upgrade the dependency and ensure the existing
test suite passes. This isolates any breakage caused by the library upgrade
from changes introduced by the new fields.

Edit `Cargo.toml` to change both `ortho_config` entries from `"0.7.0"` to
`"0.8.0"`. Run `cargo update -p ortho_config`. Then run:

```sh
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt.log
make lint 2>&1 | tee /tmp/lint.log
make test 2>&1 | tee /tmp/test.log
```

If any of these fail due to API changes in `ortho_config` 0.8.0, fix the call
sites and document the migration in `Decision Log`.

Validation gate for Stage A: `make check-fmt`, `make lint`, and `make test` all
pass with the new dependency version. No new features are added yet.

### Stage B: Define new configuration types and add fields to `Cli`

Create `src/cli/config.rs` containing the `ColourPolicy`, `SpinnerMode`, and
`OutputFormat` enums. Each must derive `Debug`, `Clone`, `Copy`, `PartialEq`,
`Eq`, `Default`, `Serialize`, `Deserialize`, and implement `FromStr` and
`Display`. Follow the `ThemePreference` pattern in `src/theme.rs` for
implementation.

Add `pub mod config;` to `src/cli/mod.rs` and import the new types.

Add four new fields to `Cli`: `colour_policy: Option<ColourPolicy>`,
`spinner_mode: Option<SpinnerMode>`, `output_format: Option<OutputFormat>`, and
`default_targets: Vec<String>`. Use
`#[ortho_config(merge_strategy = "append")]` on `default_targets`.

Update `Cli::default()` to initialize the new fields (`None` for the `Option`
types, empty `Vec` for `default_targets`).

Update `src/cli/config_merge.rs::cli_overrides_from_matches` to include the
four new field names in the value-source check loop.

Add custom validation parsers for the three new enum fields in
`src/cli/validation.rs` and `src/cli/parsing.rs`, following the existing
`parse_theme` pattern.

Update `src/cli_l10n.rs::flag_help_key` to map the new argument IDs to their
localization keys.

Add `const _` symbol anchors in `build.rs` for any new shared functions.

Validation gate for Stage B: `make check-fmt`, `make lint`, and `make test`
pass. Running `netsuke --help` shows the four new flags with localized help
text. `netsuke --colour-policy invalid` fails with a localized validation error.

### Stage C: Wire runtime behaviour for new fields

Connect the new configuration fields to the runtime:

- `colour_policy`: integrate with the colour/NO_COLOR logic. When
  `ColourPolicy::Never` is active, set `NO_COLOR`-equivalent behaviour
  internally. When `ColourPolicy::Always`, bypass `NO_COLOR` detection. `Auto`
  preserves the current behaviour. Update `output_mode::resolve` or add a new
  colour resolution function.
- `spinner_mode`: map to the existing `progress` behaviour. When
  `SpinnerMode::Disabled`, treat as `progress = Some(false)`. When
  `SpinnerMode::Enabled`, treat as `progress = Some(true)`. Resolve together
  with the legacy `progress` field, with `spinner_mode` taking precedence when
  both are specified.
- `output_format`: map `OutputFormat::Json` to `diag_json = true`.
  Resolve together with the legacy `diag_json` field, with `output_format`
  taking precedence when both are specified.
- `default_targets`: in `src/main.rs` or `src/runner/mod.rs`, when the
  `build` subcommand has an empty `targets` list, fall back to
  `cli.default_targets` before falling back to the manifest's `defaults`
  section.

Update `src/main.rs` to use the new resolution functions.

Validation gate for Stage C: `make check-fmt`, `make lint`, and `make test`
pass. Setting `colour_policy = "never"` in a config file suppresses colour.
Setting `default_targets = ["all"]` in a config file builds `all` when no
targets are specified on the CLI.

### Stage D: Add localization keys and Fluent translations

Add the new Fluent keys and their translations to both
`locales/en-US/messages.ftl` and `locales/es-ES/messages.ftl`. The keys should
follow the existing naming pattern:

```fluent
# English (en-US)
cli.flag.colour_policy.help = Colour output policy (auto, always, never).
cli.flag.spinner_mode.help = Progress spinner display mode (enabled, disabled).
cli.flag.output_format.help = Diagnostic output format (human, json).
cli.flag.default_targets.help = Default build targets when none are specified.
cli.validation.colour_policy.invalid =
    Invalid colour policy '{ $value }'. Valid options: auto, always, never.
cli.validation.spinner_mode.invalid =
    Invalid spinner mode '{ $value }'. Valid options: enabled, disabled.
cli.validation.output_format.invalid =
    Invalid output format '{ $value }'. Valid options: human, json.
```

Add corresponding key constants to `src/localization/keys.rs`.

Validation gate for Stage D: `make check-fmt` and `make lint` pass. The Fluent
compile-time audit (if present) detects no missing keys.

### Stage E: Add unit tests with `rstest`

Add or extend unit tests:

1. In `src/cli/config.rs` or a sibling test file, add `rstest` parametrized
   tests for `FromStr` and `Display` round-tripping of each new enum. Include
   invalid values to test error paths.

2. In `tests/cli_tests/merge.rs`, extend the existing
   `build_precedence_and_append_composer` and its assertion function to cover
   the new fields. Test that:
   - defaults < file < env < CLI precedence holds for `colour_policy`,
     `spinner_mode`, and `output_format`.
   - `default_targets` appends across layers.
   - `output_format = "json"` and `diag_json = true` produce consistent
     behaviour.
   - `spinner_mode = "disabled"` and `progress = false` produce consistent
     behaviour.

3. In `tests/cli_tests/parsing.rs`, add parsing tests for the four new
   flags, including both valid and invalid values.

4. Add tests for the resolution logic that combines `colour_policy` with
   `NO_COLOR`, `spinner_mode` with `progress`, and `output_format` with
   `diag_json`.

Validation gate for Stage E: `make test` passes with all new unit tests. Each
new test fails before its implementation code and passes after.

### Stage F: Add BDD scenarios with `rstest-bdd` v0.5.0

Create `tests/features/cli_config.feature` with scenarios:

1. Happy path: colour policy set in config file is honoured.
2. Happy path: spinner mode set via environment variable is honoured.
3. Happy path: output format set via CLI flag is honoured.
4. Happy path: default targets set in config file are used when no CLI
   targets specified.
5. Precedence: CLI colour policy overrides environment and config.
6. Precedence: environment spinner mode overrides config.
7. Precedence: CLI output format overrides environment and config.
8. Precedence: CLI targets extend config default targets.
9. Unhappy path: invalid colour policy value fails with localized error.
10. Unhappy path: invalid spinner mode value fails with localized error.
11. Unhappy path: invalid output format value fails with localized error.
12. Edge case: `output_format = "json"` and `diag_json = true` coexist
    without conflict.
13. Edge case: `spinner_mode = "disabled"` and `progress = false` coexist
    without conflict.

Add corresponding step definitions in `tests/bdd/steps/cli_config.rs` or extend
existing step files as appropriate. Use the `EnvLock` pattern for scenarios
that mutate environment variables.

Validation gate for Stage F: `make test` passes with all new BDD scenarios.
`touch tests/bdd_tests.rs` before the final run if feature text changed.

### Stage G: Documentation, design record, roadmap, and final validation

Update the following documents:

1. `docs/users-guide.md`: add a "Configuration fields" section documenting
   `colour_policy`, `spinner_mode`, `output_format`, and `default_targets`,
   including examples of CLI, environment, and config file usage. Update the
   existing configuration layering description to mention the new fields.

2. `docs/netsuke-design.md §8.2`: update the CLI struct listing to include
   the new fields. Add a short design note about the `CliConfig` extraction
   pattern.

3. `docs/roadmap.md`: mark 3.11.1 and its sub-items as done (`[x]`).

Run the full validation suite:

```sh
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt.log
make lint 2>&1 | tee /tmp/lint.log
make test 2>&1 | tee /tmp/test.log
make fmt 2>&1 | tee /tmp/fmt.log
PATH="/root/.bun/bin:$PATH" make markdownlint 2>&1 | tee /tmp/mdlint.log
make nixie 2>&1 | tee /tmp/nixie.log
```

Validation gate for Stage G: all six commands above pass. The diff is scoped to
the intended implementation and does not touch unrelated modules.

## Concrete steps

All commands below run from the repository root (`/home/user/project`).

### Stage A commands

```sh
# 1. Upgrade ortho_config in Cargo.toml (both deps and build-deps).
# Edit Cargo.toml: change ortho_config = "0.7.0" → "0.8.0"

# 2. Update the lock file.
cargo update -p ortho_config

# 3. Verify the upgrade.
set -o pipefail
make check-fmt 2>&1 | tee /tmp/stage-a-fmt.log
make lint 2>&1 | tee /tmp/stage-a-lint.log
make test 2>&1 | tee /tmp/stage-a-test.log
```

### Stage B commands

```sh
# After implementing the new types and fields:
set -o pipefail
make check-fmt 2>&1 | tee /tmp/stage-b-fmt.log
make lint 2>&1 | tee /tmp/stage-b-lint.log
make test 2>&1 | tee /tmp/stage-b-test.log

# Smoke test:
cargo run -- --help
cargo run -- --colour-policy invalid
```

### Stage C commands

```sh
# After wiring runtime behaviour:
set -o pipefail
make check-fmt 2>&1 | tee /tmp/stage-c-fmt.log
make lint 2>&1 | tee /tmp/stage-c-lint.log
make test 2>&1 | tee /tmp/stage-c-test.log
```

### Stage D–F commands

```sh
# After tests and translations:
set -o pipefail
make check-fmt 2>&1 | tee /tmp/stage-ef-fmt.log
make lint 2>&1 | tee /tmp/stage-ef-lint.log
touch tests/bdd_tests.rs
make test 2>&1 | tee /tmp/stage-ef-test.log
```

### Stage G commands

```sh
set -o pipefail
make check-fmt 2>&1 | tee /tmp/stage-g-fmt.log
make lint 2>&1 | tee /tmp/stage-g-lint.log
make test 2>&1 | tee /tmp/stage-g-test.log
make fmt 2>&1 | tee /tmp/stage-g-fmtfix.log
PATH="/root/.bun/bin:$PATH" make markdownlint 2>&1 | tee /tmp/stage-g-mdlint.log
make nixie 2>&1 | tee /tmp/stage-g-nixie.log
```

## Validation and acceptance

Quality criteria:

- Tests: `make test` passes. New unit tests cover `FromStr`/`Display`
  round-tripping for all three enums, merge precedence for all four new fields,
  resolution logic for `colour_policy`/`spinner_mode`/`output_format` aliases,
  and `default_targets` append semantics. BDD scenarios cover happy, unhappy,
  and precedence paths for each new field.
- Lint: `make lint` passes with zero warnings.
- Formatting: `make check-fmt` passes.
- Documentation: `docs/users-guide.md` describes all new configuration fields.
  `docs/netsuke-design.md` records the design decision.
- Markdown: `make markdownlint` passes.
- Diagrams: `make nixie` passes.

Quality method:

1. Run `make check-fmt && make lint && make test`.
2. Manually verify `netsuke --help` shows the four new flags.
3. Create a temporary `.netsuke.toml` with:

   ```toml
   colour_policy = "never"
   spinner_mode = "disabled"
   output_format = "human"
   default_targets = ["hello"]
   ```

4. Run `NETSUKE_CONFIG_PATH=/tmp/test.toml netsuke` and verify the
   configured defaults take effect (no colour, no spinner, human output, and
   `hello` target is attempted).
5. Verify `NETSUKE_COLOUR_POLICY=always netsuke` overrides the file value.
6. Verify `netsuke --colour-policy never` overrides both.

## Idempotence and recovery

Every stage's validation commands can be re-run safely. The implementation does
not modify any persistent state outside the repository working tree.

If a stage fails halfway:

- Revert uncommitted changes with `git checkout -- .` and retry.
- If the failure is in `build.rs` anchoring, add the missing `const _`
  anchor and re-run `make lint`.
- If BDD tests are not picked up after feature-file edits, run
  `touch tests/bdd_tests.rs` before `make test`.

## Artifacts and notes

Expected new files:

- `src/cli/config.rs`: `ColourPolicy`, `SpinnerMode`, `OutputFormat` enums.
- `tests/features/cli_config.feature`: BDD scenarios for new config fields.
- `tests/bdd/steps/cli_config.rs`: BDD step definitions (if not merged into
  existing step files).

Expected modified files:

- `Cargo.toml`: `ortho_config` version bump.
- `Cargo.lock`: updated dependency graph.
- `src/cli/mod.rs`: new fields, `config` module import.
- `src/cli/config_merge.rs`: updated field list.
- `src/cli/parsing.rs`: new parsers.
- `src/cli/validation.rs`: new parser wiring.
- `src/cli_l10n.rs`: new flag help key mappings.
- `src/localization/keys.rs`: new key constants.
- `locales/en-US/messages.ftl`: new Fluent messages.
- `locales/es-ES/messages.ftl`: new Spanish translations.
- `build.rs`: new symbol anchors.
- `src/main.rs`: colour/spinner/format resolution.
- `src/runner/mod.rs` or `src/main.rs`: default-targets fallback.
- `tests/cli_tests/merge.rs`: extended merge tests.
- `tests/cli_tests/parsing.rs`: new parsing tests.
- `tests/bdd/steps/` or `tests/bdd/mod.rs`: step registration.
- `docs/users-guide.md`: new configuration documentation.
- `docs/netsuke-design.md`: design record update.
- `docs/roadmap.md`: 3.11.1 marked done.

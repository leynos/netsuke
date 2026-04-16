# 3.11.3. Expose `--config <path>` and `NETSUKE_CONFIG`

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

After this work, a Netsuke user can point the tool at an arbitrary
configuration file in two new, visible ways:

1. A CLI flag: `netsuke --config /path/to/config.toml build`
2. An environment variable: `NETSUKE_CONFIG=/path/to/config.toml netsuke build`

Both surfaces bypass automatic discovery and load the specified file directly.
The existing `NETSUKE_CONFIG_PATH` environment variable continues to work as a
silent alias for backward compatibility, but `NETSUKE_CONFIG` becomes the
documented, user-facing name.

The repository also ships an annotated sample configuration file at
`docs/sample-netsuke.toml` that documents every supported key, so users have a
starting point without reading source code.

Observable success means all of the following hold simultaneously:

- `netsuke --config /tmp/custom.toml build` loads the custom file instead of
  the discovered one.
- `NETSUKE_CONFIG=/tmp/custom.toml netsuke build` does the same.
- The legacy `NETSUKE_CONFIG_PATH` still works when `NETSUKE_CONFIG` is not
  set.
- When both are set, `NETSUKE_CONFIG` takes precedence over
  `NETSUKE_CONFIG_PATH`.
- `netsuke --help` shows the `--config` flag with localised help text.
- `docs/sample-netsuke.toml` is a valid, parsable config file with comments
  explaining every key.
- `make check-fmt`, `make lint`, and `make test` all pass.
- The roadmap entry 3.11.3 is checked off.

## Constraints

- Keep `Cli` as the concrete clap and OrthoConfig merge root. Do not
  restructure the derive hierarchy.
- Preserve backward compatibility with `NETSUKE_CONFIG_PATH`. Removing it
  would break existing CI pipelines and user workflows.
- Preserve the standard precedence ladder: defaults < config files <
  environment variables < CLI flags. The `--config` flag is a file-selection
  mechanism, not a value-level override: it selects which file to load, but the
  file's values still sit below environment and CLI in the precedence chain.
- Keep all source files below the 400-line limit per `AGENTS.md`.
- Keep all new user-facing strings localizable via Fluent. Update both
  `en-US` and `es-ES` bundles.
- Add `build.rs` symbol anchors for any new public helpers.
- Do not use OrthoConfig's built-in `discovery(...)` attribute on the `Cli`
  struct. Netsuke manages its own discovery through `config_discovery()` in
  `src/cli/config_merge.rs` because OrthoConfig's `compose_layers()` returns
  only the first found file, and Netsuke's two-pass approach is needed for
  correct project-over-user precedence. The new `--config` flag must integrate
  with this custom discovery path, not replace it.
- The `--config` flag must use a long-form argument only. The short `-c` is
  not assigned because `-C` (uppercase) is already taken by `--directory` and
  the visual similarity would cause confusion.
- Mark roadmap item 3.11.3 done only after all validation gates pass.

## Tolerances (exception triggers)

- Scope: if implementation requires more than 16 files changed or more than
  700 net new lines, stop and escalate before proceeding.
- Interface: if the change requires altering the signature of
  `merge_with_config` or `resolve_merged_diag_json` in a way that breaks
  existing callers, stop and escalate.
- Dependencies: if a new external crate dependency is required, stop and
  escalate.
- Iterations: if `make lint` or `make test` still fail after three focused
  fix-and-rerun cycles within a single stage, stop, document the blocker, and
  escalate.
- Ambiguity: if the interaction between `--config`, `NETSUKE_CONFIG`, and
  `NETSUKE_CONFIG_PATH` creates an unresolvable precedence conflict, stop and
  present options with trade-offs.

## Risks

- Risk: adding a `--config` field to the `Cli` struct may interact with
  OrthoConfig's hidden `--config-path` flag, creating a clap conflict or
  ambiguity. Severity: medium. Likelihood: medium. Mitigation: OrthoConfig's
  hidden `--config-path` is only injected when the struct uses
  `discovery(...)`, which Netsuke does not. Netsuke's `Cli` already manages its
  own discovery externally, so adding a plain `config: Option<PathBuf>` field
  should not collide. Verify during Stage A by running `netsuke --help` and
  confirming no duplicate flags.

- Risk: the `cli_overrides_from_matches` function strips fields not
  explicitly set on the command line. A new `config` field must be handled
  correctly there — it should be stripped from value-level overrides because it
  is a meta-field (selects which file to load), not a config preference.
  Severity: high. Likelihood: high. Mitigation: add `"config"` to the exclusion
  list in `cli_overrides_from_matches` during Stage B.

- Risk: the `config` field will be serialized by `sanitize_value` and included
  in the merge pipeline if not handled carefully. Because it is a file
  selector, not a preference, including it in the merged `Cli` output would be
  confusing and could cause issues if the merged struct is re-serialized.
  Severity: medium. Likelihood: high. Mitigation: mark the field with
  `#[serde(skip)]` so it does not participate in OrthoConfig serialization.
  Keep it as a clap-only, parse-time field.

- Risk: backward compatibility between `NETSUKE_CONFIG` and
  `NETSUKE_CONFIG_PATH` could create confusion if both are set to different
  files. Severity: low. Likelihood: low. Mitigation: define a clear precedence:
  `--config` (CLI) > `NETSUKE_CONFIG` (env) > `NETSUKE_CONFIG_PATH` (env,
  legacy). Document this in the user guide.

- Risk: the new `--config` flag could be passed alongside `-C` /
  `--directory`, creating ambiguity about which directory anchors the config
  path. Severity: low. Likelihood: medium. Mitigation: `--config` accepts an
  absolute or relative path resolved against the process working directory (not
  `-C`). This matches the semantics of `NETSUKE_CONFIG_PATH` and avoids
  surprising interactions. Document this clearly.

- Risk: new Fluent keys need both `en-US` and `es-ES` translations or the
  build-time audit will fail. Severity: high. Likelihood: high. Mitigation: add
  keys to both bundles in Stage C. Use a reasonable Spanish translation (or a
  close English fallback with a `TODO(l10n)` comment if unsure) and verify with
  `make lint`.

## Progress

- [ ] Stage A: add `--config` CLI field and wire it into discovery.
- [ ] Stage B: support `NETSUKE_CONFIG` environment variable alongside legacy
      `NETSUKE_CONFIG_PATH`.
- [ ] Stage C: add Fluent localization keys and `build.rs` anchoring.
- [ ] Stage D: add `rstest` integration tests for `--config` and
      `NETSUKE_CONFIG`.
- [ ] Stage E: add `rstest-bdd` behavioural tests.
- [ ] Stage F: ship annotated sample config and update documentation.
- [ ] Stage G: validation, roadmap update, and evidence capture.

## Surprises & discoveries

(None yet — this section will be populated during implementation.)

## Decision log

- Decision: use a plain `config: Option<PathBuf>` field on `Cli` with
  `#[serde(skip)]` rather than OrthoConfig's
  `discovery(config_cli_long = "config", config_cli_visible = true)` attribute.
  Rationale: Netsuke's two-pass discovery in `config_merge.rs` is required for
  correct project-over-user file precedence. OrthoConfig's built-in
  `compose_layers()` returns only the first found file and cannot express this.
  Introducing the `discovery(...)` attribute would replace Netsuke's custom
  pipeline, and the interaction between a `discovery()`-generated flag and the
  existing manual `config_discovery()` builder is untested and risky. A plain
  clap field avoids the coupling. Date/Author: 2026-04-16 / planning agent.

- Decision: keep `NETSUKE_CONFIG_PATH` as a silent backward-compatible alias.
  Rationale: CI pipelines may already use it. Removing it would be a breaking
  change for no user benefit. The new `NETSUKE_CONFIG` env var takes precedence
  when both are set. Date/Author: 2026-04-16 / planning agent.

- Decision: `--config` uses long-form only (no `-c` short flag).
  Rationale: `-C` (uppercase) is already assigned to `--directory`. A lowercase
  `-c` for a different flag would cause visual confusion, particularly in
  documentation and error messages. Long-form `--config` is unambiguous.
  Date/Author: 2026-04-16 / planning agent.

- Decision: `--config` path is resolved against the process working
  directory, not the `-C` directory. Rationale: this matches the existing
  `NETSUKE_CONFIG_PATH` semantics and the user's shell expectations. The `-C`
  flag re-anchors project-scope discovery and manifest lookup, but the config
  file path is specified before any directory change is applied. Date/Author:
  2026-04-16 / planning agent.

## Outcomes & retrospective

(To be completed after implementation.)

## Context and orientation

Read these files in order before changing code.

1. `src/cli/mod.rs` — the `Cli` struct (lines 40–142). This is the clap
   parser and OrthoConfig merge root. It defines the current `CONFIG_ENV_VAR`
   constant (`"NETSUKE_CONFIG_PATH"`) and the `ENV_PREFIX` constant
   (`"NETSUKE_"`). The new `config` field will be added here.

2. `src/cli/config_merge.rs` — the merge pipeline. The key functions are:

   - `config_discovery(directory)` (line 38): builds a `ConfigDiscovery`
     using `CONFIG_ENV_VAR`. This function must be updated to also accept an
     explicit config path from `--config`.
   - `push_file_layers(composer, errors, directory)` (line 176): two-pass
     file discovery. When an explicit config path is provided, this function
     should load that file directly and skip all discovery.
   - `collect_diag_file_layers(directory)` (line 111): mirrors
     `push_file_layers` for early diag-JSON resolution. Must also honour
     `--config`.
   - `merge_with_config(cli, matches)` (line 264): the top-level merge
     entry point. The `cli.config` field is read here to decide whether to
     use explicit loading or discovery.

3. `src/cli/config.rs` — the `CliConfig` typed view. The `config` field does
   NOT belong here because it is a file selector, not a runtime preference.

4. `src/cli_l10n.rs` — localization helpers. `flag_help_key()` (line 122)
   maps argument IDs to Fluent keys. A new mapping for `"config"` must be added.

5. `src/localization/keys.rs` — Fluent key constants. A new key
   `CLI_FLAG_CONFIG_HELP` must be added.

6. `locales/en-US/messages.ftl` and `locales/es-ES/messages.ftl` — Fluent
   bundles. New messages for the `--config` flag help text.

7. `build.rs` — symbol anchoring. If any new public helper is exposed from
   `src/cli/mod.rs` or `src/cli/config_merge.rs`, a `const _` anchor must be
   added.

8. `tests/cli_tests/config_discovery.rs` — existing integration tests for
   config discovery. New tests for `--config` and `NETSUKE_CONFIG` will be
   added here or in a neighbouring file.

9. `tests/features/configuration_discovery.feature` and
   `tests/bdd/steps/configuration_discovery.rs` — existing BDD coverage for
   config discovery. New scenarios will extend this feature file.

10. `docs/users-guide.md` (lines 543–630) — user-facing configuration
    documentation. Must be updated to describe `--config` and
    `NETSUKE_CONFIG`.

11. `docs/netsuke-design.md` (lines 2030–2111) — design decisions section
    8.4. Must be updated to record the new config override surface.

12. `docs/roadmap.md` (lines 296–298) — roadmap item 3.11.3. Must be marked
    done after all gates pass.

## Plan of work

### Stage A. Add `--config` CLI field and wire it into discovery

Add a new `config: Option<PathBuf>` field to the `Cli` struct in
`src/cli/mod.rs`. This field:

- accepts a file path via `--config <PATH>`;
- is marked `#[serde(skip)]` so it does not participate in OrthoConfig
  serialization or merging (it is a meta-field, not a preference);
- is excluded from the override detection in `cli_overrides_from_matches`
  by virtue of `#[serde(skip)]` (serde-skipped fields do not appear in the
  serialized JSON, so they cannot leak into the merge pipeline);
- has localised help text via the existing localization infrastructure.

Update the default impl for `Cli` to set `config: None`.

Then, update the merge pipeline in `src/cli/config_merge.rs`:

1. Add a new constant: `const CONFIG_ENV_VAR_NEW: &str = "NETSUKE_CONFIG";`
   (or rename the semantics — see below).

2. Create a helper `fn resolve_config_path(cli: &Cli) -> Option<PathBuf>`
   that implements the precedence: `cli.config` (from `--config`) >
   `NETSUKE_CONFIG` env var > `NETSUKE_CONFIG_PATH` env var > `None` (use
   discovery). This helper reads environment variables directly
   (`std::env::var_os`) because the env-var layer in the merge pipeline is for
   preference values, not for file selection.

3. Update `config_discovery()` to accept an `Option<&Path>` for the explicit
   config path. When `Some`, skip `ConfigDiscovery` entirely and load the file
   directly via `load_config_file_as_chain`.

4. Update `push_file_layers()` to call `resolve_config_path` and, when a
   path is returned, load that single file instead of running two-pass
   discovery. When the explicit path does not exist or fails to parse,
   propagate the error rather than falling back to discovery — the user
   explicitly requested this file.

5. Update `collect_diag_file_layers()` with the same explicit-path logic
   so `resolve_merged_diag_json` honours `--config` and `NETSUKE_CONFIG`.

6. Update `merge_with_config()` — the existing code passes
   `cli.directory.as_deref()` into `push_file_layers`. Now also pass the
   resolved config path. The function signature of `push_file_layers` will gain
   an `explicit_config: Option<&Path>` parameter.

Acceptance for Stage A:

- `cargo check` succeeds.
- `netsuke --help` shows the `--config` flag (with English help text as a
  placeholder until Stage C).
- `netsuke --config /nonexistent.toml build` reports an error about the
  missing file rather than falling back to discovery.
- `netsuke --config <valid-config> build` loads the specified file.

### Stage B. Support `NETSUKE_CONFIG` environment variable

Update `resolve_config_path()` to check `NETSUKE_CONFIG` before
`NETSUKE_CONFIG_PATH`. The full precedence for file selection is:

```plaintext
--config <PATH>   (CLI flag, highest)
NETSUKE_CONFIG    (env var, new user-facing name)
NETSUKE_CONFIG_PATH  (env var, legacy silent alias)
automatic discovery  (lowest, two-pass project > user)
```

Update the `has_explicit_config` checks in `push_file_layers` and
`collect_diag_file_layers` to also check `NETSUKE_CONFIG`. Currently these
check `CONFIG_ENV_VAR` (`NETSUKE_CONFIG_PATH`); they must now check both env
vars and the CLI field.

Acceptance for Stage B:

- `NETSUKE_CONFIG=/tmp/custom.toml netsuke build` uses the custom file.
- `NETSUKE_CONFIG_PATH=/tmp/legacy.toml netsuke build` still works.
- When both are set to different files, `NETSUKE_CONFIG` wins.
- `cargo check` succeeds.

### Stage C. Fluent localization keys and `build.rs` anchoring

1. Add a new key constant in `src/localization/keys.rs`:

   ```rust
   CLI_FLAG_CONFIG_HELP => "cli.flag.config.help",
   ```

2. Add the Fluent message to `locales/en-US/messages.ftl`:

   ```ftl
   cli.flag.config.help = Path to a configuration file, bypassing automatic discovery.
   ```

3. Add the Fluent message to `locales/es-ES/messages.ftl`:

   ```ftl
   cli.flag.config.help = Ruta a un archivo de configuración, omitiendo la detección automática.
   ```

4. Update `flag_help_key()` in `src/cli_l10n.rs` to map `"config"` to
   `keys::CLI_FLAG_CONFIG_HELP`.

5. If `resolve_config_path` or any other new helper is made `pub` and
   used from `build.rs`-compiled modules, add a `const _` symbol anchor in
   `build.rs::assert_symbols_linked()`. If the helpers remain `pub(super)` or
   private to `config_merge.rs`, no anchor is needed.

Acceptance for Stage C:

- `make lint` passes (including `cargo doc` and the build-time Fluent
  audit).
- `netsuke --help` shows the `--config` flag with localised English text.

### Stage D. `rstest` integration tests

Add integration tests in `tests/cli_tests/config_discovery.rs` (or a
neighbouring file if the 400-line limit is reached). Tests should follow the
existing pattern: acquire `EnvLock`, create temp directories, write config
files, parse with `parse_with_localizer_from`, merge with `merge_with_config`,
and assert on the merged `Cli` struct.

Test cases:

1. `config_flag_loads_specified_file` — write a custom config file with a
   distinctive theme, pass `--config <path>`, assert theme matches.

2. `config_flag_skips_project_discovery` — place a project `.netsuke.toml`
   in the current directory with theme `ascii`, pass `--config` pointing to a
   file with theme `unicode`, assert `unicode` wins.

3. `config_flag_with_nonexistent_file_produces_error` — pass `--config`
   pointing to a path that does not exist, assert the merge returns an error.

4. `netsuke_config_env_loads_specified_file` — set `NETSUKE_CONFIG` to a
   custom file, parse without `--config`, assert the custom file is loaded.

5. `netsuke_config_env_takes_precedence_over_legacy` — set
   `NETSUKE_CONFIG` to file A and `NETSUKE_CONFIG_PATH` to file B (with
   different themes), assert file A wins.

6. `config_flag_takes_precedence_over_netsuke_config_env` — set
   `NETSUKE_CONFIG` to file A, pass `--config` pointing to file B, assert file
   B wins.

7. `config_flag_values_still_overridden_by_env_and_cli_preferences` —
   custom config sets theme `ascii`, env sets `NETSUKE_THEME=unicode`, assert
   theme is `unicode` (the file is loaded, but preference-level env vars still
   override preference values from the file).

Use `rstest` parameterization where appropriate. Use `EnvVarGuard` for all
environment mutations. Use `CwdGuard` if changing working directory.

Acceptance for Stage D:

- `cargo test --test cli_tests -- config` runs the new tests and they pass.
- Tests are deterministic and do not interfere with parallel execution.

### Stage E. `rstest-bdd` behavioural tests

Extend `tests/features/configuration_discovery.feature` with new scenarios that
prove user-observable behaviour:

```gherkin
Scenario: Explicit config file overrides project discovery
  Given a temporary workspace
  And a project config file ".netsuke.toml" with theme "ascii"
  And a custom config file "custom.toml" with theme "unicode"
  When the CLI is parsed with "--config custom.toml"
  Then parsing succeeds
  And the theme preference is "unicode"

Scenario: NETSUKE_CONFIG environment variable selects config file
  Given a temporary workspace
  And a project config file ".netsuke.toml" with theme "ascii"
  And a custom config file "override.toml" with theme "unicode"
  And the environment variable "NETSUKE_CONFIG" points to "override.toml"
  When the CLI is parsed with no additional arguments
  Then parsing succeeds
  And the theme preference is "unicode"

Scenario: NETSUKE_CONFIG takes precedence over NETSUKE_CONFIG_PATH
  Given a temporary workspace
  And a custom config file "new.toml" with theme "unicode"
  And a custom config file "legacy.toml" with theme "ascii"
  And the environment variable "NETSUKE_CONFIG" points to "new.toml"
  And the environment variable "NETSUKE_CONFIG_PATH" points to "legacy.toml"
  When the CLI is parsed with no additional arguments
  Then parsing succeeds
  And the theme preference is "unicode"

Scenario: CLI config flag takes precedence over NETSUKE_CONFIG
  Given a temporary workspace
  And a custom config file "cli.toml" with theme "unicode"
  And a custom config file "env.toml" with theme "ascii"
  And the environment variable "NETSUKE_CONFIG" points to "env.toml"
  When the CLI is parsed with "--config cli.toml"
  Then parsing succeeds
  And the theme preference is "unicode"
```

Add or extend step definitions in `tests/bdd/steps/configuration_discovery.rs`
as needed. The existing `write_config_file` helper and
`custom_config_with_theme` step should work for most scenarios. The
`When the CLI is parsed with "--config custom.toml"` step needs to resolve
`custom.toml` relative to the temp workspace directory before passing to the
parser — update the existing When step to handle `--config` arguments that
reference filenames in the workspace.

Acceptance for Stage E:

- `cargo test --test bdd_tests configuration_discovery` runs all scenarios
  (old and new) and they pass.
- Scenarios read as user stories, not as unit tests in Gherkin clothing.

### Stage F. Annotated sample config and documentation updates

1. Create `docs/sample-netsuke.toml` — an annotated sample configuration
   file that documents every supported key. Use TOML comments (`#`) to explain
   each field, its default value, and valid options. The file must be parsable
   by Netsuke (all values should be valid defaults or commented out). Structure
   it by section: general, build behaviour, output preferences, network policy.

   Example structure:

   ```toml
   # Netsuke sample configuration
   #
   # Place this file at .netsuke.toml in your project root, or point
   # to it with --config or NETSUKE_CONFIG.

   # Enable verbose diagnostic logging and timing summaries.
   # verbose = false

   # Locale for CLI messages (e.g., "en-US", "es-ES").
   # locale = "en-US"

   # CLI theme preset: "auto", "unicode", or "ascii".
   # theme = "auto"

   # ... (all other fields)
   ```

2. Update `docs/users-guide.md`:

   - In the "Configuration and Localization" section (around line 543), add
     documentation for `--config <PATH>` and `NETSUKE_CONFIG`.
   - Document the full config override precedence:
     `--config` > `NETSUKE_CONFIG` > `NETSUKE_CONFIG_PATH` > automatic
     discovery.
   - Mention the sample config file and where to find it.
   - Ensure the existing `NETSUKE_CONFIG_PATH` documentation is preserved
     but de-emphasised as a legacy alias.

3. Update `docs/netsuke-design.md` section 8.4:

   - Record the config override surface design decision.
   - Document the interaction between `--config`, `NETSUKE_CONFIG`, and
     `NETSUKE_CONFIG_PATH`.

4. Run `make fmt` and `make markdownlint` after documentation changes.

Acceptance for Stage F:

- `netsuke --config docs/sample-netsuke.toml build` does not error on
  config parsing (the sample file should be valid or entirely commented out).
- `make markdownlint` passes.
- `make nixie` passes (if any Mermaid diagrams were added or changed).
- A user can learn how to use `--config` and `NETSUKE_CONFIG` by reading
  only the user guide.

### Stage G. Validation, roadmap update, and evidence capture

1. Run all validation gates using `tee` and `set -o pipefail`:

   ```sh
   set -o pipefail && make fmt 2>&1 | tee /tmp/3-11-3-make-fmt.log
   set -o pipefail && make check-fmt 2>&1 | tee /tmp/3-11-3-check-fmt.log
   set -o pipefail && make lint 2>&1 | tee /tmp/3-11-3-make-lint.log
   set -o pipefail && make test 2>&1 | tee /tmp/3-11-3-make-test.log
   set -o pipefail && make markdownlint 2>&1 | tee /tmp/3-11-3-markdownlint.log
   set -o pipefail && make nixie 2>&1 | tee /tmp/3-11-3-make-nixie.log
   ```

2. Review log files for truncated output, not just exit codes.

3. Mark roadmap item 3.11.3 done in `docs/roadmap.md`.

4. Update the `Progress`, `Outcomes & Retrospective`, and
   `Surprises & Discoveries` sections of this ExecPlan.

Acceptance for Stage G:

- All six validation commands exit with status 0.
- Roadmap item 3.11.3 is checked off.
- This ExecPlan status is updated to COMPLETE.

## Interfaces and dependencies

### New field on `Cli` (`src/cli/mod.rs`)

```rust
/// Path to a configuration file, bypassing automatic discovery.
///
/// When specified, Netsuke loads this file instead of searching for
/// `.netsuke.toml` in project and user scopes. The file path is resolved
/// against the process working directory.
#[arg(long, value_name = "PATH")]
#[serde(skip)]
#[ortho_config(skip_cli)]
pub config: Option<PathBuf>,
```

The `#[serde(skip)]` annotation prevents the field from being serialized into
the JSON value that feeds the merge pipeline. The `#[ortho_config (skip_cli)]`
annotation (matching the existing `command` field pattern) prevents OrthoConfig
from trying to merge this field across layers.

### Config path resolution helper (`src/cli/config_merge.rs`)

```rust
/// Resolve the effective explicit config file path.
///
/// Precedence: `--config` CLI flag > `NETSUKE_CONFIG` env var >
/// `NETSUKE_CONFIG_PATH` env var > `None` (use automatic discovery).
fn resolve_config_path(cli: &Cli) -> Option<PathBuf> {
    if let Some(ref path) = cli.config {
        return Some(path.clone());
    }
    if let Some(val) = std::env::var_os("NETSUKE_CONFIG") {
        if !val.is_empty() {
            return Some(PathBuf::from(val));
        }
    }
    if let Some(val) = std::env::var_os(CONFIG_ENV_VAR) {
        if !val.is_empty() {
            return Some(PathBuf::from(val));
        }
    }
    None
}
```

### Updated `push_file_layers` signature

```rust
fn push_file_layers(
    composer: &mut MergeComposer,
    errors: &mut Vec<Arc<ortho_config::OrthoError>>,
    directory: Option<&Path>,
    explicit_config: Option<&Path>,
)
```

When `explicit_config` is `Some`, the function loads that single file via
`load_config_file_as_chain` and pushes the resulting layers. It does not run
`config_discovery()` or the second-pass project-scope loader. When the file
does not exist or fails to parse, the error is pushed to `errors`.

### Updated `collect_diag_file_layers` signature

```rust
fn collect_diag_file_layers(
    directory: Option<&Path>,
    explicit_config: Option<&Path>,
) -> Vec<MergeLayer<'static>>
```

Mirrors the `push_file_layers` change for early diag-JSON resolution.

### Fluent key (`src/localization/keys.rs`)

```rust
CLI_FLAG_CONFIG_HELP => "cli.flag.config.help",
```

### Localization mapping (`src/cli_l10n.rs`, in `flag_help_key`)

```rust
"config" => Some(keys::CLI_FLAG_CONFIG_HELP),
```

## Validation and acceptance

Quality criteria (what "done" means):

- Tests: `make test` passes the full workspace suite, including at least 7
  new integration tests and 4 new BDD scenarios.
- Lint: `make lint` passes with zero warnings.
- Format: `make check-fmt` passes.
- Markdown: `make markdownlint` passes.
- Mermaid: `make nixie` passes.
- Sample config: `docs/sample-netsuke.toml` parses without errors.
- Docs: the user guide documents `--config`, `NETSUKE_CONFIG`, the full
  precedence chain, and the sample config file.

Quality method (how we check):

```sh
set -o pipefail && make check-fmt 2>&1 | tee /tmp/3-11-3-check-fmt.log
set -o pipefail && make lint 2>&1 | tee /tmp/3-11-3-make-lint.log
set -o pipefail && make test 2>&1 | tee /tmp/3-11-3-make-test.log
set -o pipefail && make markdownlint 2>&1 | tee /tmp/3-11-3-markdownlint.log
set -o pipefail && make nixie 2>&1 | tee /tmp/3-11-3-make-nixie.log
```

## Idempotence and recovery

All stages are safe to re-run. Config file writes are idempotent (write the
same content). Test execution is stateless. Environment variable mutations are
protected by `EnvLock` and `EnvVarGuard` RAII guards.

If a stage fails partway through, fix the issue and re-run from the start of
that stage. No rollback is needed because no destructive operations are
performed.

## Artifacts and notes

### File change summary (expected)

New files:

- `docs/sample-netsuke.toml` — annotated sample configuration file.

Modified files:

- `src/cli/mod.rs` — add `config: Option<PathBuf>` field; update `Default`
  impl.
- `src/cli/config_merge.rs` — add `resolve_config_path`; update
  `push_file_layers`, `collect_diag_file_layers`, and `merge_with_config` to
  honour explicit config paths.
- `src/cli_l10n.rs` — add `"config"` mapping in `flag_help_key`.
- `src/localization/keys.rs` — add `CLI_FLAG_CONFIG_HELP` key.
- `locales/en-US/messages.ftl` — add `cli.flag.config.help` message.
- `locales/es-ES/messages.ftl` — add `cli.flag.config.help` message.
- `tests/cli_tests/config_discovery.rs` (or new neighbour) — add 7+
  integration tests.
- `tests/features/configuration_discovery.feature` — add 4 BDD scenarios.
- `tests/bdd/steps/configuration_discovery.rs` — extend step definitions if
  needed.
- `docs/users-guide.md` — document `--config` and `NETSUKE_CONFIG`.
- `docs/netsuke-design.md` — record design decision in section 8.4.
- `docs/roadmap.md` — mark 3.11.3 done.
- `build.rs` — add symbol anchor if new public helpers are exposed.
- `src/cli/config_merge_tests.rs` — add unit tests for `resolve_config_path`
  and updated `push_file_layers`.

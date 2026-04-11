# Developer guide

This guide describes the day-to-day engineering workflow for Netsuke, with a
focus on writing and maintaining tests. It is the source of truth for how the
test suite is expected to be used by contributors.

## Quality gates

Run these commands before finalizing any change:

- `make check-fmt`
- `make lint`
- `make test`

When command output is long, preserve exit codes and logs:

```bash
set -o pipefail
make test 2>&1 | tee /tmp/netsuke-make-test.log
```

For documentation changes, also run:

- `make fmt`
- `make markdownlint`
- `make nixie`

## Test suite map

Netsuke uses a mixed strategy:

- Unit and integration tests live under `tests/` as ordinary Rust test files.
- Behavioural tests use Gherkin feature files in `tests/features/` and
  `tests/features_unix/`.
- Behavioural step definitions and fixtures live in `tests/bdd/`.
- Behavioural test discovery is defined in `tests/bdd_tests.rs`.

## Behavioural testing strategy

Behavioural tests run through `cargo test` using `rstest-bdd`, not a bespoke
runner. The `scenarios!` macro in `tests/bdd_tests.rs` discovers feature files
and binds a shared fixture entry point (`world: TestWorld`) to each generated
scenario test.

### State and isolation policy

- Scenario isolation is the default: scenario state must be recreated per test.
- Shared process-wide state is avoided unless infrastructure cost requires
  controlled reuse.
- Use `Slot<T>` for optional or replaceable scenario values.
- Use typed wrappers in `tests/bdd/types.rs` for step parameters to avoid
  ambiguous string-heavy signatures.

### Step authoring policy

- Keep `Given` steps for context and setup.
- Keep `When` steps for one observable action.
- Keep `Then` steps for user-visible outcomes, not internal implementation
  details.
- Prefer explicit, domain-focused helper functions over large step bodies.
- Keep step modules cohesive by domain (`cli`, `manifest`, `ir`, `stdlib`,
  `process`, `locale_resolution`).

### Compile-time safety

`rstest-bdd-macros` is configured with `strict-compile-time-validation`, so
missing or ambiguous step bindings should be treated as compile-time failures.

## rstest-bdd v0.5.0 usage

The migration plan and implementation record are tracked in
`docs/execplans/rstest-bdd-v0-5-0-behavioural-suite-migration.md`.

Current usage in this repository is:

- `rstest-bdd` and `rstest-bdd-macros` pinned to `0.5.0`.
- Step parameters favour typed wrappers from `tests/bdd/types.rs`; wrappers
  implement `FromStr` so step signatures can use domain types directly.
- Prefer inferred step patterns for simple, no-argument steps when this
  reduces duplication and keeps feature wording clear.
- Use `rstest_bdd::async_step::sync_to_async` for manual sync-to-async wrappers
  and the concise wrapper aliases (`StepCtx`, `StepTextRef`, `StepDoc`,
  `StepTable`) where required.
- Introduce async step definitions only where asynchronous behaviour is natural
  and improves coverage.
- Keep async execution on Tokio current-thread runtime for behavioural tests.
- Restrict `#[once]` fixtures to expensive, effectively read-only
  infrastructure.

These points are strategy rules, not optional style guidance.

## How to add or update behavioural tests

1. Add or update the feature text in `tests/features/` or
   `tests/features_unix/`.
2. Implement or update matching steps under `tests/bdd/steps/`.
3. Reuse existing fixtures/helpers before adding new world state.
4. Add typed parameter wrappers in `tests/bdd/types.rs` when step arguments
   represent distinct domain concepts.
5. Run `cargo test --test bdd_tests` and then the full quality gates.

## Test isolation utilities

Environment variable mutations and working-directory changes are process-global
side effects that can cause data races when tests run in parallel. The
`test_support` crate and test fixtures provide RAII-based utilities to
serialize and safely restore these mutations.

### `EnvLock`

`test_support::env_lock::EnvLock` is a global mutex that serializes all
process-global mutations (environment variables, current working directory)
across concurrent test threads. Acquire it at the start of any test that
mutates the environment:

```rust
use test_support::env_lock::EnvLock;

let _env_lock = EnvLock::acquire();
```

The lock is released when the guard is dropped. In BDD scenarios,
`TestWorld::ensure_env_lock()` acquires it once per scenario and holds it for
the scenario lifetime.

### `EnvVarGuard`

`test_support::EnvVarGuard` is a lightweight RAII guard for setting or removing
a single environment variable and restoring it on drop:

```rust
use test_support::EnvVarGuard;

let _guard = EnvVarGuard::set("HOME", temp.path().as_os_str());
let _guard = EnvVarGuard::remove("NETSUKE_CONFIG_PATH");
```

For BDD steps that need to track mutations through `TestWorld`, use
`mutate_env_var` from `tests/bdd/helpers/env_mutation.rs` instead.

### `CwdGuard`

Tests that call `std::env::set_current_dir` must restore the original working
directory after the test. `CwdGuard` (defined locally in
`tests/cli_tests/config_discovery.rs`) captures the current directory on
construction and restores it on drop:

```rust
struct CwdGuard(std::path::PathBuf);

impl CwdGuard {
    fn acquire() -> anyhow::Result<Self> {
        Ok(Self(std::env::current_dir()?))
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        drop(std::env::set_current_dir(&self.0));
    }
}
```

Acquire `CwdGuard` *after* `EnvLock` so the drop order (CWD restored first,
lock released second) mirrors the acquire order.

### `restore_many` and `restore_many_locked`

`test_support::env::restore_many` restores a batch of environment variables
from a `HashMap<String, Option<OsString>>` snapshot. It acquires `EnvLock`
internally, so callers do not need to hold the lock:

```rust
use std::collections::HashMap;
use std::ffi::OsStr;
use test_support::env::{restore_many, set_var};

let mut snapshot = HashMap::new();
snapshot.insert("HELLO".into(), set_var("HELLO", OsStr::new("world")));
restore_many(snapshot);
// "HELLO" is now restored to its prior value (or removed if it was unset).
```

`restore_many_locked` is the `unsafe` variant for callers that already hold
`EnvLock` — typically `Drop` implementations. The caller **must** hold the lock
for the duration of the call:

```rust
// SAFETY: EnvLock is held via self.env_lock.
unsafe { test_support::env::restore_many_locked(vars) };
```

Prefer `restore_many` in normal test code. Use `restore_many_locked` only
inside `Drop` or other contexts where `EnvLock` is already acquired.

### `mutate_env_var` (BDD scenarios)

`mutate_env_var` in `tests/bdd/helpers/env_mutation.rs` is the canonical way to
set or remove an environment variable within a BDD scenario. It acquires the
scenario-scoped `EnvLock`, performs the mutation, and registers the key for
automatic restoration when the scenario ends:

```rust
use crate::bdd::helpers::env_mutation::mutate_env_var;
use crate::bdd::types::EnvVarKey;

// Set a variable
mutate_env_var(world, EnvVarKey::from("NETSUKE_THEME"), Some("ascii"))?;

// Remove a variable
mutate_env_var(world, EnvVarKey::from("NETSUKE_CONFIG_PATH"), None)?;
```

Do **not** call `std::env::set_var` directly in BDD steps — use
`mutate_env_var` so that cleanup is tracked through `TestWorld`.

### Ordering rules

1. Acquire `EnvLock` first.
2. Acquire `CwdGuard` second.
3. Create `EnvVarGuard`s for all variables that need sandboxing.
4. Perform the test.
5. Guards drop in reverse declaration order — CWD and environment
   variables are restored while the lock is still held, preventing races.

## `TestWorld` field groups

`TestWorld` (`tests/bdd/fixtures/mod.rs`) is the shared fixture for all BDD
scenarios. Its fields are organized by domain:

### Scenario state groups

State fields organized by concern to facilitate scenario authoring and maintenance.

| Group              | Fields                                                                                                                                                                                                                                   | Purpose                                                                  |
| :----------------- | :--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :----------------------------------------------------------------------- |
| CLI state          | `cli`, `cli_error`                                                                                                                                                                                                                       | Parsed CLI configuration and parse error capture.                        |
| Manifest state     | `manifest`, `manifest_error`                                                                                                                                                                                                             | Parsed manifest and error capture.                                       |
| IR state           | `build_graph`, `removed_action_id`, `generation_error`                                                                                                                                                                                   | Build graph, negative-test identifiers, generation errors.               |
| Ninja state        | `ninja_content`, `ninja_error`                                                                                                                                                                                                           | Generated Ninja file content and errors.                                 |
| Process state      | `run_status`, `run_error`, `command_stdout`, `command_stderr`, `temp_dir`, `workspace_path`, `path_guard`, `ninja_env_guard`                                                                                                             | Process execution results, temporary directories, and path/ninja guards. |
| Stdlib state       | `stdlib_root`, `stdlib_output`, `stdlib_error`, `stdlib_state`, `stdlib_command`, `stdlib_policy`, `stdlib_path_override`, `stdlib_fetch_max_bytes`, `stdlib_command_max_output_bytes`, `stdlib_command_stream_max_bytes`, `stdlib_text` | Stdlib rendering, network policy, and size constraints.                  |
| Localization state | `localization_lock`, `localization_guard`, `locale_config`, `locale_env`, `locale_cli_override`, `locale_system`, `resolved_locale`, `locale_message`                                                                                    | Scenario-level localizer overrides and resolution state.                 |
| HTTP server state  | `http_server`, `stdlib_url`                                                                                                                                                                                                              | Test HTTP server fixture for fetch scenarios.                            |
| Output state       | `output_mode`, `simulated_no_color`, `simulated_term`, `output_prefs`, `simulated_no_emoji`, `rendered_prefix`                                                                                                                           | Accessibility and output preference resolution.                          |
| Environment state  | `env_vars`, `env_lock`, `original_cwd`                                                                                                                                                                                                   | Environment variable snapshots, scenario-scoped lock, and CWD capture.   |

### Key `TestWorld` methods

- `track_env_var(key, previous)` — record a variable for restoration at
  scenario end.
- `ensure_env_lock()` — acquire the scenario-scoped `EnvLock` on first
  call; subsequent calls are no-ops. Also captures the current working
  directory for later restoration.
- `restore_environment_locked()` (unsafe, private) — called from `Drop` to
  restore all tracked variables while the lock is still held.

## Configuration merge architecture

Configuration merging lives in `src/cli/config_merge.rs`. The module keeps
config-layer plumbing separate from the public CLI surface in `cli::mod`.

### Two-pass file discovery

OrthoConfig's `ConfigDiscovery::compose_layers()` returns only the **first**
matching config file it finds. Because user-scope locations (XDG (X Desktop
Group), HOME) are checked before the project root, a user config can shadow a
project config.

To enforce **project scope > user scope** precedence, `merge_with_config` uses
a two-pass approach:

1. **First pass** — run `config_discovery()` to find whatever file exists
   first (typically user-scope).
2. **Second pass** — if the first pass did not find the project-scope file
   and `NETSUKE_CONFIG_PATH` is not set, load `.netsuke.toml` from the project
   root directly via `load_config_file_as_chain` and push its layers last.

Because `MergeComposer` uses last-wins semantics, pushing the project layers
after user layers gives them higher precedence.

The same logic is mirrored in `collect_diag_file_layers` for early `diag_json`
resolution (before full merging).

### Layer precedence

The final merge order is:

1. **Defaults** — `Cli::default()` serialized as a base layer.
2. **File layers** — discovered config files in the two-pass order above.
3. **Environment** — `NETSUKE_*` environment variables via the Figment Env
   provider.
4. **CLI flags** — values explicitly passed on the command line.

### Private helpers

### Configuration merge helper functions

Private helper functions for config discovery and diagnostic-JSON resolution.

| Function                     | Purpose                                                              |
| :--------------------------- | :------------------------------------------------------------------- |
| `config_discovery`           | Build single-pass `ConfigDiscovery` with optional directory anchor.  |
| `project_scope_file_str`     | Resolve the expected project `.netsuke.toml` path as a string.       |
| `project_scope_layers`       | Load project-scope config directly, bypassing discovery.             |
| `push_file_layers`           | Push all file layers onto a `MergeComposer` in precedence order.     |
| `collect_diag_file_layers`   | Mirror of `push_file_layers` for early `diag_json` resolution.       |
| `is_empty_value`             | Return `true` for an empty JSON object (no CLI overrides).           |
| `diag_json_from_layer`       | Extract `diag_json` from a config layer, preferring `output_format`. |
| `diag_json_from_matches`     | Resolve final `diag_json` from CLI matches with fallback.            |
| `cli_overrides_from_matches` | Extract CLI-supplied fields, stripping defaults and non-CLI sources. |
| `env_provider`               | Return the `NETSUKE_` prefixed Figment environment provider.         |

## Documentation upkeep

When test strategy or behavioural test usage changes, update this file in the
same change-set, so the documented approach remains aligned with the codebase.

# Introduce `CliConfig` as the layered CLI configuration schema

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETED

No `PLANS.md` file exists in this repository.

## Purpose / big picture

Roadmap item `3.11.1` asks for a dedicated `CliConfig` struct derived with
`OrthoConfig` so Netsuke has one explicit, typed schema for layered CLI
configuration. This plan now targets `ortho_config` `0.8.0` and uses the
repository copy of
[`docs/ortho-config-users-guide.md`](/home/user/project/docs/ortho-config-users-guide.md),
 which has been replaced with the upstream `v0.8.0` guide. Today the repository
already has partial layered configuration, but it is centered on
[`src/cli/mod.rs`](/home/user/project/src/cli/mod.rs), where `Cli` currently
serves three roles at once:

1. Clap parser.
2. OrthoConfig merge target.
3. Runtime command model passed into the runner.

That coupling makes the code difficult to extend. It also leaves roadmap fields
such as colour policy, spinner mode, output format, default targets, and theme
either implicit or missing.

After this change, a novice should be able to point at one config schema and
see how Netsuke resolves:

- verbosity
- locale
- colour policy
- spinner mode
- progress and accessible output behaviour
- output format
- default build targets
- theme selection

Observable success means:

1. `CliConfig` exists and is the authoritative OrthoConfig-derived schema.
2. Clap parsing still works, but parsing and layered configuration are no
   longer the same type.
3. Configuration files, environment variables, and CLI flags resolve through
   the same typed fields with documented precedence.
4. Unit tests (`rstest`) and behavioural tests (`rstest-bdd` v0.5.0) cover
   happy paths, unhappy paths, and precedence edge cases.
5. [`docs/users-guide.md`](/home/user/project/docs/users-guide.md),
   [`docs/netsuke-design.md`](/home/user/project/docs/netsuke-design.md), and
   [`docs/roadmap.md`](/home/user/project/docs/roadmap.md) reflect the final
   behaviour.

## Constraints

- Keep existing top-level commands stable: `build`, `clean`, `graph`, and
  `manifest` must continue to parse and dispatch.
- Upgrade every in-repo `ortho_config` dependency to `0.8.0`, and add or
  upgrade `ortho_config_macros` to `0.8.0` if the final implementation uses
  derive-generated selected-subcommand merging.
- Keep the toolchain at Rust `1.88` or newer. The current repository already
  uses Rust `1.89.0`, so this is a compatibility floor rather than a required
  toolchain migration.
- Do not regress localized Clap help or localized runtime diagnostics.
- Use `ortho_config` as the primary merge mechanism rather than adding another
  bespoke loader.
- When derive-generated code touches `figment`, `uncased`, or `xdg`, prefer the
  `ortho_config::...` re-exports described by the `0.8.0` guide unless the
  application source genuinely needs those crates directly.
- Preserve the current configuration discovery behaviour for this milestone.
  Do not expose the visible `--config` / `NETSUKE_CONFIG` interface here; that
  belongs to roadmap item `3.11.3`.
- Preserve existing user-facing flags where feasible. If a new canonical field
  supersedes an old one, keep a compatibility path unless the user explicitly
  approves a breaking change.
- If `cli_default_as_absent` is used on any new or refactored field, use typed
  Clap defaults (`default_value_t` / `default_values_t`) rather than
  string-based `default_value`, and pass `ArgMatches` into merge flows so
  explicit CLI overrides remain distinguishable from inferred defaults.
- No source file may exceed 400 lines. This is a hard constraint, not a style
  preference.
- All new or changed public types and modules require Rustdoc and module-level
  documentation.
- Behaviour must be validated with both `rstest` unit tests and
  `rstest-bdd` v0.5.0 behavioural tests.
- The implementation must end with successful `make check-fmt`, `make lint`,
  and `make test` runs captured via `tee` and `set -o pipefail`.
- Documentation updates are part of the feature, not follow-up work.

## Tolerances (exception triggers)

- Scope: if implementation requires more than 18 files or more than 900 net
  new lines, stop and escalate.
- Interfaces: if the runner or subcommand API must change in a way that breaks
  existing tests or user-facing CLI syntax, stop and escalate.
- Dependencies: if implementation requires a new runtime dependency beyond
  `ortho_config` facilities already in use, stop and escalate.
- Compatibility: if preserving `--no-emoji` compatibility while introducing a
  canonical theme field becomes impossible without ambiguous precedence, stop
  and escalate.
- Investigation: if configuration merging still behaves unexpectedly after
  three red/green cycles, stop and document the blocking case before proceeding.

## Risks

- Risk: [`src/cli/mod.rs`](/home/user/project/src/cli/mod.rs) is already 398
  lines, so any additive work there will violate the file-size limit. Severity:
  high. Likelihood: high. Mitigation: split parser definitions, config schema,
  and merge helpers into separate modules before adding new fields.

- Risk: the repository already has partial OrthoConfig support, manual config
  discovery, and merge tests. A careless refactor could duplicate logic rather
  than simplifying it. Severity: medium. Likelihood: high. Mitigation: treat
  this as a consolidation task and delete superseded merge code as part of the
  same change.

- Risk: the repository currently depends on `ortho_config 0.7.0`, while this
  plan now targets `0.8.0`. Severity: medium. Likelihood: high. Mitigation:
  treat the crate upgrade as part of the same branch and apply the documented
  migration rules up front instead of retrofitting them after the schema
  refactor lands.

- Risk: roadmap item `3.11.1` reaches into future roadmap items by naming
  output format and theme before `3.10.3` and `3.12.*` are complete. Severity:
  medium. Likelihood: high. Mitigation: introduce typed config fields now, but
  limit behaviour to what the current product can honour and fail clearly on
  unsupported values.

- Risk: build default targets are subcommand-specific, while most other config
  is global. Severity: medium. Likelihood: medium. Mitigation: use
  OrthoConfig's subcommand configuration support for `build` instead of forcing
  targets into the global namespace.

## Progress

- [x] (2026-03-07) Reviewed roadmap `3.11.1`, the OrthoConfig guide, current
  CLI/config code, and nearby execplans.
- [x] (2026-03-07) Drafted this ExecPlan.
- [x] (2026-03-09) Continued implementation from the existing branch state.
- [x] (2026-03-09) Stage A: split parser, config schema, and merge
  responsibilities.
- [x] (2026-03-09) Stage B: introduce typed config groups and compatibility
  mapping.
- [x] (2026-03-09) Stage C: wire global and subcommand merges through
  `CliConfig`.
- [x] (2026-03-09) Stage D: add unit and behavioural coverage.
- [x] (2026-03-09) Stage E: update user/design docs, mark roadmap item done,
  and run all quality gates.

## Surprises & Discoveries

- The codebase already derives `OrthoConfig` on `Cli`; roadmap `3.11.1` is
  therefore a refactor-and-expansion task, not a greenfield introduction.
- The repository is already on Rust `1.89.0`, which satisfies the
  `ortho_config 0.8.0` minimum of Rust `1.88`. The crate version is the real
  migration step; the compiler floor is not.
- The current repository already documents configuration discovery in
  [`docs/users-guide.md`](/home/user/project/docs/users-guide.md), and already
  has merge tests in
  [`tests/cli_tests/merge.rs`](/home/user/project/tests/cli_tests/merge.rs).
  The implementation must preserve these guarantees while changing the type
  layout.
- `rstest-bdd` feature-file edits may require touching
  [`tests/bdd_tests.rs`](/home/user/project/tests/bdd_tests.rs) to force Cargo
  to rebuild generated scenarios.
- The new configuration-preferences BDD coverage initially flaked only in the
  full suite because `NETSUKE_CONFIG_PATH` is process-global. Holding
  [`EnvLock`](test_support/src/env_lock.rs) for the whole scenario fixed the
  race without weakening the coverage.

## Decision Log

- Decision: introduce a dedicated `CliConfig` type and treat the existing CLI
  parser as a separate concern. Rationale: parsing tokens and merging layered
  configuration are related but not identical jobs, and combining them has
  already pushed the current module to the file-size limit. Date/Author:
  2026-03-07 (Codex, plan draft)

- Decision: use OrthoConfig subcommand configuration for build defaults rather
  than a global `default_targets` field. Rationale: target lists only make
  sense for `build`; placing them under `cmds.build` matches the OrthoConfig
  user's guide and avoids leaking command-specific semantics into global
  config. Date/Author: 2026-03-07 (Codex, plan draft)

- Decision: keep current hidden config-path override behaviour in this
  milestone and defer the visible `--config` / `NETSUKE_CONFIG` surface to
  roadmap item `3.11.3`. Rationale: the roadmap splits schema introduction from
  config-path UX, and this plan should not silently complete later work.
  Date/Author: 2026-03-07 (Codex, plan draft)

- Decision: make `theme` the canonical presentation setting and treat
  `no_emoji` as a compatibility alias that resolves to the ASCII theme.
  Rationale: roadmap `3.12.*` already talks about themes, while the current
  implementation only exposes `no_emoji`; a compatibility bridge avoids
  breaking existing users. Date/Author: 2026-03-07 (Codex, plan draft)

- Decision: add typed enums for colour policy, spinner mode, output format,
  and theme even where runtime behaviour is still limited. Rationale: the
  schema should become explicit now so future milestones extend behaviour
  without reworking config names a second time. Unsupported combinations must
  fail with actionable diagnostics. Date/Author: 2026-03-07 (Codex, plan draft)

- Decision: treat `ortho_config 0.8.0` as the baseline for this work and align
  the implementation with its migration rules. Rationale: the local guide now
  reflects `v0.8.0`, and the implementation plan should not be written against
  `0.7.x` semantics. The repository does not currently alias the dependency
  name, so `#[ortho_config(crate = "...")]` is not required unless that changes
  during implementation. Date/Author: 2026-03-08 (Codex, plan revision)

- Decision: keep `Cli` as the parser/runtime command carrier while moving all
  layered schema responsibilities into `CliConfig`. Rationale: this achieved
  the roadmap separation with a smaller, safer surface-area change while
  preserving runner tests and command-dispatch call sites. Date/Author:
  2026-03-09 (Codex, implementation)

- Decision: validate `output_format = "json"` as unsupported for now instead of
  silently accepting it. Rationale: roadmap item `3.10.3` is still open, so
  accepting JSON output configuration without delivering the behaviour would be
  misleading. Date/Author: 2026-03-09 (Codex, implementation)

These decisions must be recorded in
[`docs/netsuke-design.md`](/home/user/project/docs/netsuke-design.md) during
implementation if they remain unchanged after coding begins.

## Outcomes & Retrospective

Completed on 2026-03-09.

Implemented results:

- Added [`CliConfig`](/home/user/project/src/cli/config.rs) as the
  authoritative OrthoConfig-derived schema and split the CLI module into
  parser, config, and merge submodules.
- Upgraded `ortho_config` to `0.8.0`.
- Kept `Cli` as the parser/runtime command carrier while rooting configuration
  merge in `CliConfig`.
- Added typed config fields for `colour_policy`, `spinner_mode`,
  `output_format`, and `theme`.
- Canonicalized `no_emoji = true` to the ASCII theme while rejecting
  contradictory combinations.
- Wired `[cmds.build] targets` and `emit` defaults into the runtime build
  command when the user does not supply explicit CLI values.
- Added unit coverage in
  [`tests/cli_tests/merge.rs`](/home/user/project/tests/cli_tests/merge.rs)
  plus behavioural coverage in
  [`tests/features/configuration_preferences.feature`](/home/user/project/tests/features/configuration_preferences.feature).
- Updated the user guide, design document, and roadmap entry for `3.11.1`.

Quality-gate evidence:

- `make check-fmt`
- `make lint`
- `make test`
- `PATH="/root/.bun/bin:$PATH" make markdownlint`
- `make nixie`

Lessons learned:

- Keeping the parser/runtime type stable while introducing a separate merge
  schema is a pragmatic migration path when downstream code already consumes
  the parser type pervasively.
- BDD coverage that touches process-wide environment variables must hold
  `EnvLock` for the full scenario, not only for individual mutations.

## Context and orientation

The current implementation is spread across the following files:

- [`src/cli/mod.rs`](/home/user/project/src/cli/mod.rs): current `Cli` type,
  Clap parser, OrthoConfig derive, validation parsers, and merge logic.
- [`Cargo.toml`](/home/user/project/Cargo.toml): currently pins
  `ortho_config = "0.7.0"` and `rust-version = "1.89.0"`.
- [`rust-toolchain.toml`](/home/user/project/rust-toolchain.toml): currently
  pins toolchain `1.89.0`, which already satisfies the `0.8.0` minimum.
- [`src/main.rs`](/home/user/project/src/main.rs): startup parse/merge flow and
  runtime localization bootstrap.
- [`src/output_mode.rs`](/home/user/project/src/output_mode.rs): accessible
  versus standard output mode resolution.
- [`src/output_prefs.rs`](/home/user/project/src/output_prefs.rs): emoji-aware
  semantic prefixes and current `no_emoji` handling.
- [`src/runner/mod.rs`](/home/user/project/src/runner/mod.rs): uses merged CLI
  state to choose output mode, progress behaviour, and build targets.
- [`tests/cli_tests/merge.rs`](/home/user/project/tests/cli_tests/merge.rs):
  current merge precedence coverage.
- [`tests/cli_tests/parsing.rs`](/home/user/project/tests/cli_tests/parsing.rs)
  and
  [`tests/features/cli.feature`](/home/user/project/tests/features/cli.feature):
   current parse-only coverage.

Two important facts shape this plan:

1. The repo already has a layered configuration story.
2. The missing piece is a stable, explicit schema that separates merged config
   from raw CLI parsing and adds the roadmap fields that do not yet exist.

## Target architecture

The implementation should converge on three layers of types.

1. A Clap-facing parser model, still responsible for token parsing and
   user-facing command syntax.
2. A layered configuration model rooted at `CliConfig`, derived with
   `OrthoConfig`, `Serialize`, and `Deserialize`, using `0.8.0` semantics.
3. A runtime model passed into the runner after configuration and subcommand
   selection have been resolved.

The preferred shape is:

- `src/cli/mod.rs`: parser entry points and minimal top-level glue.
- `src/cli/config.rs`: `CliConfig` plus nested typed config groups.
- `src/cli/merge.rs` or similar: conversion from parsed CLI overrides plus
  OrthoConfig layer composition into the merged runtime shape.
- `BuildArgs` (or a renamed build-config type) derived with `OrthoConfig` so
  `cmds.build` configuration can supply default targets and optional emit-path
  defaults.

The config schema should cover at least these concepts:

- `verbose`
- `locale`
- `colour_policy`
- `spinner_mode`
- `output_format`
- `theme`
- `progress`
- `accessible`
- current fetch-policy settings
- build default targets through subcommand configuration

For this milestone, a valid example config should look like this:

```toml
verbose = true
locale = "es-ES"
colour_policy = "auto"
spinner_mode = "auto"
output_format = "human"
theme = "ascii"
progress = true
accessible = false

[cmds.build]
targets = ["all"]
```

The final user guide must explain the actual accepted values and any
compatibility aliases such as `no_emoji = true`.

## Plan of work

### Stage A: split responsibilities before adding new fields

Start by reducing the blast radius in
[`src/cli/mod.rs`](/home/user/project/src/cli/mod.rs). Move the merge logic and
the future `CliConfig` definition out of that file first. The goal is to make
later edits mechanical instead of risky. This stage also establishes the
`ortho_config 0.8.0` baseline before higher-level schema refactors pile on.

Concrete work in this stage:

1. Extract the current OrthoConfig-driven merge helpers into a dedicated module.
2. Introduce a parser-only root type if needed (`Cli`, `CliArgs`, or similar),
   while keeping command syntax unchanged.
3. Upgrade `ortho_config` to `0.8.0` and confirm the repository still builds
   against Rust `1.89.0` without toolchain changes.
4. Keep existing parsing tests green before introducing new schema fields.

Acceptance for Stage A:

- CLI parsing tests still pass unchanged.
- No file exceeds 400 lines.
- No behaviour changes are visible yet.

### Stage B: introduce `CliConfig` and typed config groups

Create `CliConfig` as the authoritative layered configuration schema. Use typed
enums or newtypes where values are semantic rather than free-form text.

Concrete work in this stage:

1. Define `CliConfig` and any nested groups needed to keep modules readable.
2. Add typed enums for:
   - `ColourPolicy`
   - `SpinnerMode`
   - `OutputFormat`
   - `Theme`
3. Decide and implement compatibility handling for the current `no_emoji`
   surface.
4. Use OrthoConfig discovery attributes on `CliConfig` to preserve the current
   `NETSUKE_CONFIG_PATH` and hidden config-path behaviour.
5. Keep global fields optional where layered precedence requires absence to be
   meaningful.
6. Where Clap defaults are required, use typed defaults so
   `cli_default_as_absent` remains valid under `0.8.0`.

Acceptance for Stage B:

- `CliConfig` can be merged from defaults, file, env, and CLI layers in unit
  tests without invoking the full parser.
- Invalid enum values fail with actionable errors.
- Existing config-discovery paths still work.

### Stage C: merge global config plus selected subcommand defaults

This stage makes `CliConfig` drive runtime behaviour instead of the current
all-in-one `Cli` type.

Concrete work in this stage:

1. Replace `Cli::merge_with_config` with a merge path rooted in `CliConfig`.
2. Merge `build` subcommand defaults using OrthoConfig's subcommand support so
   `[cmds.build] targets = [...]` becomes the default target list when the user
   does not pass targets explicitly.
3. Convert the merged config plus parsed command into the runtime shape
   consumed by [`src/runner/mod.rs`](/home/user/project/src/runner/mod.rs).
4. Update output-mode and output-preference resolution to consume the new typed
   fields rather than a loose collection of booleans.
5. Keep startup locale resolution intact so localized Clap help still works
   before full merge.
6. If selected-subcommand merge derives are introduced, add
   `ortho_config_macros 0.8.0` and pass `ArgMatches` where required by
   `cli_default_as_absent`.

Acceptance for Stage C:

- Running `netsuke` with no explicit targets can pick up configured build
  targets from `cmds.build`.
- CLI overrides still beat environment and file values.
- Existing commands continue to dispatch correctly.

### Stage D: add test coverage for happy and unhappy paths

Unit tests should prove the schema and merge logic. Behavioural tests should
prove the user-visible outcomes.

Unit coverage to add with `rstest`:

- defaults < file < env < CLI precedence for representative global fields
- `theme` / `no_emoji` compatibility mapping
- enum parsing failures for `colour_policy`, `spinner_mode`, and
  `output_format`
- build-target defaulting through `cmds.build`
- unsupported but syntactically valid combinations fail clearly when required
- any `cli_default_as_absent` fields continue to prefer file/env values when
  the user did not explicitly supply the CLI flag

Behavioural coverage to add with `rstest-bdd` v0.5.0:

- config file supplies default build targets and `netsuke` uses them
- CLI `--locale` or `--verbose` overrides file and env values
- invalid config values produce user-facing diagnostics
- compatibility alias (`no_emoji`) still produces ASCII-themed output

Prefer a dedicated feature file such as
[`tests/features/configuration_preferences.feature`](/home/user/project/tests/features/configuration_preferences.feature)
 plus matching step definitions, rather than overloading the existing
CLI-parsing feature with merge semantics.

### Stage E: document the final contract and close the roadmap item

After behaviour is working and tested, update the docs as part of the same
change.

Documentation work:

1. Update [`docs/users-guide.md`](/home/user/project/docs/users-guide.md) with
   the new config schema, precedence rules, accepted values, and example TOML.
2. Update [`docs/netsuke-design.md`](/home/user/project/docs/netsuke-design.md)
   to record:
   - separation of parser and config schema
   - subcommand config for default build targets
   - canonical theme handling and `no_emoji` compatibility
   - which output-format values are supported in this milestone
3. Mark roadmap item `3.11.1` as done in
   [`docs/roadmap.md`](/home/user/project/docs/roadmap.md).

Acceptance for Stage E:

- User docs match the shipped behaviour.
- Design docs capture the decisions from this plan that survived
  implementation.
- Only roadmap item `3.11.1` is marked done unless later work is intentionally
  completed and validated too.

## Concrete steps

Run all commands from `/home/user/project`.

Before editing feature files, remember the existing `rstest-bdd` gotcha:

```sh
touch tests/bdd_tests.rs
```

Use `tee` and `set -o pipefail` for every long-running gate:

```sh
set -o pipefail
make check-fmt 2>&1 | tee /tmp/netsuke-check-fmt.log
```

```sh
set -o pipefail
make lint 2>&1 | tee /tmp/netsuke-lint.log
```

```sh
set -o pipefail
make test 2>&1 | tee /tmp/netsuke-test.log
```

Because docs will change, also run:

```sh
set -o pipefail
PATH="/root/.bun/bin:$PATH" make markdownlint 2>&1 | tee /tmp/netsuke-markdownlint.log
```

```sh
set -o pipefail
make nixie 2>&1 | tee /tmp/netsuke-nixie.log
```

```sh
set -o pipefail
make fmt 2>&1 | tee /tmp/netsuke-fmt.log
```

After `make fmt`, inspect `git status --short` and remove incidental edits in
unrelated files before finalizing the change.

The local OrthoConfig reference for this task is now the `v0.8.0` guide at
[`docs/ortho-config-users-guide.md`](/home/user/project/docs/ortho-config-users-guide.md).
 Do not rely on older `0.7.x` examples when the two guides disagree.

## Validation and acceptance

The feature is complete only when all of the following are true:

1. `CliConfig` is the authoritative OrthoConfig-derived schema.
2. Parser-only code and merge-only code are separated enough to keep files
   under the 400-line limit.
3. `build` default targets can be supplied through configuration without
   breaking explicit CLI targets.
4. Typed config values are documented and validated with clear unhappy-path
   errors.
5. Unit tests and behavioural tests cover both precedence and failure cases.
6. `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
   `make nixie` succeed.
7. [`docs/users-guide.md`](/home/user/project/docs/users-guide.md),
   [`docs/netsuke-design.md`](/home/user/project/docs/netsuke-design.md), and
   [`docs/roadmap.md`](/home/user/project/docs/roadmap.md) are updated.

## Idempotence and recovery

This work should be implemented in small, reversible steps. If a stage fails,
return the tree to the last green checkpoint, update this ExecPlan's
`Decision Log` and `Progress`, and retry with a narrower diff rather than
stacking speculative fixes. If formatting tools touch unrelated Markdown files,
restore only the incidental edits created during the current turn before
proceeding.

# ADR 004: Explicit config selection outside OrthoConfig

## Status

Accepted.

Accepted: 2026-05-31. Netsuke will resolve explicit configuration file
selection in `src/cli/discovery.rs` rather than delegate this behaviour to
OrthoConfig's built-in discovery attributes.

## Date

2026-05-31.

## Context and problem statement

Netsuke needs an explicit configuration selector for operators who want one
known configuration file to control a run. The public selector order is
`--config` > `NETSUKE_CONFIG` > automatic discovery.

The existing merge pipeline is deliberately two-pass. It first resolves early
diagnostic JSON preferences from file layers, so startup errors can be emitted
in the requested format. It then runs the full OrthoConfig-backed merge for the
final `Cli` value. Automatic discovery also has Netsuke-specific precedence
requirements: project configuration must outrank user configuration, and a
missed project `.netsuke.toml` requires a direct second-pass project load.

OrthoConfig can discover configuration files, but its built-in discovery
attribute does not own Netsuke's `--config` spelling, early diagnostic merge,
or project-over-user second pass. Putting explicit selection into OrthoConfig
would either expose Netsuke-specific policy through a generic library API or
force Netsuke to work around library-owned behaviour in the CLI adapter.

## Decision drivers

- Keep Netsuke's command-line contract in the CLI adapter that owns the
  command-line spelling.
- Preserve the existing two-pass merge pipeline for early diagnostic JSON
  resolution and final configuration merging.
- Keep `OrthoConfig` responsible for generic layer composition, not
  Netsuke-specific selector precedence.
- Keep `NETSUKE_CONFIG` as the only environment selector.
- Make explicit selection fail closed: an invalid selected file must not fall
  through to automatic discovery.

## Options considered

### Option A: use OrthoConfig's built-in discovery attribute

This would let OrthoConfig own the config-path selector and merge discovered
files as part of its normal derived merge behaviour.

It was rejected because Netsuke needs the public spelling `--config`, the
`NETSUKE_CONFIG` environment selector, and the two-pass diagnostic path.
OrthoConfig's generic discovery machinery cannot express those Netsuke-specific
semantics without broadening its API around one consumer's policy.

### Option B: add Netsuke-specific explicit selection to OrthoConfig

This would extend OrthoConfig, so Netsuke could delegate the selector order and
legacy alias handling to the library.

It was rejected because the policy is part of Netsuke's CLI contract rather
than OrthoConfig's domain. Baking `NETSUKE_CONFIG` or Netsuke's project-scope
fallback into OrthoConfig would invert the dependency: the generic merge
library would know too much about one adapter.

### Option C: resolve explicit selection in `discovery.rs`

This keeps explicit path selection beside Netsuke's CLI merge code. Private
helpers resolve the selector, load file layers for the diagnostic pass, and
push the same layers into the full merge composer.

It is accepted because it keeps the boundary clear. OrthoConfig remains the
layer-composition engine, while Netsuke's CLI adapter owns how user input,
environment aliases, diagnostics, and automatic discovery are combined.

## Decision outcome

Netsuke resolves explicit configuration paths in `src/cli/discovery.rs`.

- `explicit_config_path` applies `--config` > `NETSUKE_CONFIG`, ignoring empty
  environment values.
- `env_config_path(var_name)` reads one environment variable with
  `std::env::var_os`, so precedence tests use current-process values.
- `push_file_layers` drains successful layer loads into the merge composer, or
  records the load error for final diagnostics.
- Automatic discovery remains the fallback only when no explicit selector is
  present.

## Consequences

- The CLI adapter has a small amount of Netsuke-specific orchestration logic,
  but the rules are visible and testable where the public contract is defined.
- OrthoConfig does not gain Netsuke-specific configuration selector semantics.
- Explicit selected files fail closed. A missing or invalid file reports the
  selected-file error instead of silently inheriting a discovered file.
- Future changes to selector precedence must update `discovery.rs`, the
  developer guide, the design document, and this ADR together.

## Related documents

- [`docs/developers-guide.md`](developers-guide.md)
- [`docs/execplans/3-11-3-expose-config-path-and-netsuke-config.md`][execplan]
- [`docs/netsuke-design.md`](netsuke-design.md)

[execplan]: execplans/3-11-3-expose-config-path-and-netsuke-config.md

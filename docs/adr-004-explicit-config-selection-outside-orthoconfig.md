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

The merge pipeline is a cached one-pass model. A single discovery pass through
the injected environment provider produces a `DiscoveryOutcome` whose
side-effect-free diagnostics are replayed once after tracing is configured; the
full OrthoConfig-backed merge then reuses the already-discovered
`DiscoveredLayers` for the final `Cli` value, with no second environment or
filesystem pass. Automatic discovery also has Netsuke-specific precedence
requirements: project configuration must outrank user configuration, and a
missed project `.netsuke.toml` is appended to the layer stack when present,
because OrthoConfig's own scan can miss it.

OrthoConfig can discover configuration files, but its built-in discovery
attribute does not own Netsuke's `--config` spelling, early diagnostic merge,
or project-over-user fallback. Putting explicit selection into OrthoConfig
would either expose Netsuke-specific policy through a generic library API or
force Netsuke to work around library-owned behaviour in the CLI adapter.

## Decision drivers

- Keep Netsuke's command-line contract in the CLI adapter that owns the
  command-line spelling.
- Preserve the cached one-pass merge pipeline: early diagnostic JSON
  resolution and final configuration merging share one discovery pass.
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
`NETSUKE_CONFIG` environment selector, and its cached one-pass diagnostic path.
OrthoConfig's generic discovery machinery cannot express those Netsuke-specific
semantics without broadening its API around one consumer's policy.

### Option B: add Netsuke-specific explicit selection to OrthoConfig

This would extend OrthoConfig, so Netsuke could delegate its selector policy to
the library.

It was rejected because the policy is part of Netsuke's CLI contract rather
than OrthoConfig's domain. The Netsuke CLI adapter owns selector precedence, the
`--config` spelling, and `NETSUKE_CONFIG` handling. Baking those details or
Netsuke's project-scope fallback into OrthoConfig would invert the dependency:
the generic merge library would know too much about one adapter.

### Option C: resolve explicit selection in `discovery.rs`

This keeps explicit path selection beside Netsuke's CLI merge code. Private
helpers resolve the selector, load file layers for the diagnostic pass, and
push the same layers into the full merge composer.

It is accepted because it keeps the boundary clear. OrthoConfig remains the
layer-composition engine, while Netsuke's CLI adapter owns how user input,
environment selection, diagnostics, and automatic discovery are combined.

## Decision outcome

Netsuke resolves explicit configuration paths in `src/cli/discovery.rs`.

- `resolve_config_selector` applies `--config` > `NETSUKE_CONFIG`, ignoring
  empty environment values.
- `env_config_path(env, var_name)` reads through Netsuke's injected
  `EnvProvider` port. Production supplies `StdEnvProvider`; tests use a
  map-backed provider without mutating process-global state.
- Discovery is one pass through the injected environment provider that
  produces a `DiscoveryOutcome`. Its `emit_diagnostics` replays the retained,
  side-effect-free diagnostics once after tracing is configured, and its
  `into_layers` hands the same `DiscoveredLayers` to the full merge, so early
  diagnostic JSON resolution and final merging share one pass.
- The project-scope fallback is preserved: when OrthoConfig's own scan misses
  a project `.netsuke.toml`, `project_scope_file` detects it and
  `project_scope_layers` appends it to the layer stack.
- Automatic discovery remains the fallback only when no explicit selector is
  present.

## Consequences

- The CLI adapter has a small amount of Netsuke-specific orchestration logic,
  but the rules are visible and testable where the public contract is defined.
- OrthoConfig does not gain Netsuke-specific configuration selector semantics.
- Ambient and injected composition drive discovery through the same
  `discovery_env_source` adapter, which projects the `ConfigEnvProvider` port
  into a closed `MapEnv` containing only documented discovery keys. This keeps
  automatic discovery hermetic in tests while retaining platform home fallback
  for users.
- Explicit selected files fail closed. A missing or invalid file reports the
  selected-file error instead of silently inheriting a discovered file.
- Future changes to selector precedence must update `discovery.rs`, the
  developer guide, the design document, and this ADR together.

## Addendum — 2026-08-30

The original decision above remains unchanged: explicit configuration selection
belongs to the Netsuke CLI adapter rather than OrthoConfig. The current
selector contract is explicit about precedence and directory handling:
`--config` takes precedence over `NETSUKE_CONFIG`, and either explicit selector
bypasses automatic discovery. Relative selectors retain process-working-
directory semantics independently of `-C/--directory`; absolute selectors
remain unchanged. The `-C/--directory` value anchors automatic project
discovery and manifest lookup only. The production implementation is
`selector::resolve_config_selector` in `src/cli/discovery_selector.rs`, with
`src/cli/discovery.rs` owning the layer-loading boundary.

## Related documents

- [`docs/developers-guide.md`](developers-guide.md)
- [`docs/execplans/adopt-ortho-config-v0-9-0.md`](execplans/adopt-ortho-config-v0-9-0.md)
- [`docs/execplans/3-11-3-expose-config-path-and-netsuke-config.md`][execplan]
- [`docs/netsuke-design.md`](netsuke-design.md)

[execplan]: execplans/3-11-3-expose-config-path-and-netsuke-config.md

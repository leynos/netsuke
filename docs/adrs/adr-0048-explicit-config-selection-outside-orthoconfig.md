# Architectural decision record (ADR) 0048: Explicit config selection outside OrthoConfig

## Status

Accepted.

Accepted: 2026-05-31. Netsuke will resolve explicit configuration file
selection in `src/cli/discovery/mod.rs` rather than delegate this behaviour to
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

Netsuke resolves explicit configuration paths in `src/cli/discovery/mod.rs`.

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
`selector::resolve_config_selector` in `src/cli/discovery/selector.rs`, with
`src/cli/discovery/mod.rs` owning the layer-loading boundary.

## Addendum — 2026-11-12

Automatic discovery now also owns user-before-system ordering. OrthoConfig
0.10.0 assembles automatic candidates as `XDG_CONFIG_HOME`, `XDG_CONFIG_DIRS`,
`APPDATA`/`LOCALAPPDATA`, then the home fallbacks, and stops at the first
candidate that loads. A valid system file therefore suppressed
`$HOME/.config/netsuke/config.toml` or `$HOME/.netsuke.toml` whenever
`XDG_CONFIG_HOME` did not name that user file directly, which let a system file
replace the operator's fetch and environment policy.

Netsuke now runs its own user-scope pass before calling `compose_layers()`.
`paths::user_scope_candidates` derives the user subset from the injected
environment in preference order, and `layers` loads the first candidate that
exists, returning a load error rather than continuing to a lower-priority file.
`compose_layers()` runs only when no user candidate exists, so system scope and
the project roots remain reachable exactly as before. Authority assignment is
deliberately unchanged: a user file that is also the project configuration
stays project-scoped and keeps its existing quarantine, because reading the
user scope first changes which candidates are considered, not how a loaded file
is trusted. This is a Netsuke CLI-adapter decision for the same reason as the
original one: the ordering is Netsuke policy, not a generic library contract.

## Addendum — 2026-11-16

The user-scope pass also settles what counts as an existing user file and what
counts as a usable base value, because OrthoConfig 0.10.0 answers both
questions differently from the policy in the previous addendum.

A candidate that exists but does not resolve is now an error rather than an
absence. The dependency probes a candidate through a capability on its parent
directory and maps `NotFound` to `Ok(None)`, which is the same answer it gives
for a file that is not installed. A dangling `$HOME/.netsuke.toml` would
therefore fall through to system scope, reinstating the policy the user meant
to override. `paths::candidate_entry_metadata` decides presence for a candidate
that did not load, using `cap_std` to stat the entry without following it at
its final component: a genuinely absent entry is still skipped, and a link
whose target exists still loads that target.

A base value that is empty or not absolute is now ignored by both passes. The
predicate is `Path::is_absolute`, not `has_root`, because a Windows value such
as `\config` is rooted but names no drive and would resolve on whichever drive
the process runs from. The dependency filters only empty values, so its own
scan receives `paths::SanitisedUserBases`, an environment view in which a
rejected user base value reads as unset; otherwise a relative value would
return as a working-directory relative candidate in the fallback scan.
`XDG_CONFIG_DIRS` stays outside that view, because system-scope handling
remains the dependency's contract.

Both changes are Netsuke CLI-adapter policy for the same reason as the ordering
above. The rejection is reported as `OrthoError::File`, so the existing
discovery series counts the pass under `outcome="error"` and the discovery span
records `error_category="file"`.

## Amendment — 2026-11-18

The 2026-11-16 design kept the adoption decision and the trust decision apart:
a candidate that was also the project configuration was adopted and then
reported an untrusted root. Adoption returns before the dependency scan runs,
so in that layout the system layers never loaded, and a system `env_block_var`
or `fetch_default_deny` restriction stopped applying. That allocates authority
in the wrong direction: the project configuration is not a user file, so
reading the user scope first must not displace system policy for it.

[`candidate_is_trusted`] now decides adoption as well as authority. A candidate
that is the project configuration is refused, and the refusal is decisive: the
dependency scan runs rather than a lower-priority candidate being tried, so a
project file cannot promote another user file over the system layers. The scan
composes the system file and appends the project chain — the composition this
ADR already requires for a project file reached from an operator root — and the
single occurrence keeps the project quarantine described above. Adopted chains
therefore always report a trusted root.

The users' guide and the migration guide state the resulting behaviour, and
`tests/cli_tests/config_discovery_env_provenance/discovery.rs` pins it end to
end by requiring a system `env_block_var` entry to keep blocking while the
project value still applies.

[`candidate_is_trusted`]: ../../src/cli/discovery/layers/user_candidate/mod.rs

## Amendment — 2026-11-19

The decisions above are unchanged; their helpers now live in
single-responsibility modules. `paths/sanitised.rs` owns `SanitisedUserBases`,
`is_usable_base`, and the user base key list, so the environment view and the
predicate it shares with the candidate derivation cannot drift apart.
`layers/user_candidate/probe.rs` owns `CandidateProbe`, `FsCandidateProbe`, and
`candidate_entry_metadata`, so the presence probe sits beside the trait that
makes its failure branch testable rather than in the path module. The
2026-11-16 addendum's references to `paths::candidate_entry_metadata` and
`paths::SanitisedUserBases` name those helpers before this split.

## Amendment — 2026-11-19: the fallback scan skips a refused candidate

The 2026-11-18 amendment made the refusal decisive and relied on the dependency
scan to compose the system file with the project chain. That holds only while
the scan reaches a system candidate first. `ortho_config` 0.10.0 lists the
`XDG_CONFIG_HOME` base candidates before `XDG_CONFIG_DIRS`, so when
`XDG_CONFIG_HOME` names the project directory the scan returns the project file
the user pass just refused, and the system layers never load. The refusal was
decisive at the user-pass boundary only.

[`first_automatic_chain`] now takes the user candidate list the pass derived,
and its [`is_refused_user_candidate`] predicate skips a candidate that is both
a definite identity match for the project configuration file and one of those
derived locations. A project file reached only through a project root still
wins the walk: the dependency lists project roots last, so it cannot outrank a
system candidate, and returning it keeps the project-only composition path. An
ambiguous key never matches, because it withholds every candidate's authority
without identifying any of them, so treating it as a match would skip the
system candidates too. The skipped chain is not lost: the project pass loads it
with the project quarantine already required above and appends it after the
system layers.

`tests/cli_tests/config_discovery_env_provenance/user_project_overlap.rs` pins
the layout end to end, requiring a system `env_block_var` entry to keep
blocking while the project value still applies.

[`first_automatic_chain`]: ../../src/cli/discovery/layers/mod.rs
[`is_refused_user_candidate`]: ../../src/cli/discovery/layers/mod.rs

## Related documents

- [`docs/developers-guide.md`](../developers-guide.md)
- [`docs/execplans/adopt-ortho-config-v0-9-0.md`](../execplans/adopt-ortho-config-v0-9-0.md)
- [`docs/execplans/3-11-3-expose-config-path-and-netsuke-config.md`][execplan]
- [`docs/netsuke-design.md`](../netsuke-design.md)

[execplan]: ../execplans/3-11-3-expose-config-path-and-netsuke-config.md

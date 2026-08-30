# ADR 016: Public CLI metadata source of truth

## Status

Accepted.

## Date

2026-08-30.

## Context and problem statement

OrthoConfig v0.9.0 exposes documentation metadata for `CliConfig`'s layered
configuration fields. Netsuke's parser also exposes public CLI surface that is
not a configuration field: the `--config` selector and command tree. A
release-help generator that consumes only `CliConfig` therefore omits public,
localized CLI documentation.

The missing selector must not become a layered configuration source. ADR 004
assigns selector precedence, explicit-file loading, and fail-closed behaviour to
`src/cli/discovery.rs`; putting that policy into OrthoConfig would reverse the
established ownership boundary.

## Decision

`ReleaseHelpCli` is the sole composition site for Netsuke's release-help
metadata. It starts with `CliConfig::get_doc_metadata()`, then reads
`Cli::command()` to add parser-only `--config` metadata and documented
subcommands. It projects existing CLI Fluent keys onto published configuration
fields and omits the structural `cmds` container. The synthetic selector uses
the Fluent `cli.flag.config.help` key and has no environment or file source.

`ReleaseHelpCli` supplies metadata only. It does not resolve a selector, load a
file, establish precedence, or decide failure behaviour. `discovery.rs` retains
the public `--config` and `NETSUKE_CONFIG` policy defined by ADR 004.

## Rationale

- **One public metadata root.** `Cargo.toml` names `ReleaseHelpCli` as the
  `ortho_config` `root_type`, so the generator sees the complete public CLI.
- **Ownership remains local.** `CliConfig` remains the source of truth for
  layered fields, while Clap remains the source of truth for parser-only
  selectors and subcommands. `ReleaseHelpCli` only joins the existing Fluent
  keys to the published field metadata.
- **Localization remains explicit.** Parser-only fields carry declared Fluent
  keys, rather than relying on inferred message-name conventions.
- **Policy stays separate from description.** Describing `--config` in help
  does not transfer its precedence or fail-closed loading semantics.

## Consequences

Generated Unix manuals and Windows PowerShell help include the selector and
documented subcommands. Contributors add future parser-only release-help
surface in `ReleaseHelpCli`, declare its Fluent key through `define_keys!`, and
cover the composition with unit, snapshot, and artefact tests.

`CliConfig` must not gain parser-only fields, and a second parser metadata
model must not be introduced. Changes to selector policy still update ADR 004,
`discovery.rs`, and the configuration documentation rather than this adapter.
The structural `cmds` container is absent from release help because it is not a
standalone public configuration setting.

## Alternatives considered

- **Put `--config` in `CliConfig`.** Rejected because the selector is not a
  layered configuration field and would falsely imply environment or file
  sources.
- **Move selector handling into OrthoConfig.** Rejected because it would give a
  generic library Netsuke-specific precedence and fail-closed loading policy.
- **Maintain a second parser documentation model.** Rejected because two CLI
  metadata sources would drift from the Clap command tree.
- **Infer help keys from flag names.** Rejected because explicit Fluent keys
  are auditable and `cargo-orthohelp` resolves field help from metadata.

## Implementation references

- Release-help composition:
  [`src/cli/release_help.rs`](../src/cli/release_help.rs)
- Layered configuration metadata:
  [`src/cli/config.rs`](../src/cli/config.rs)
- Selector precedence and fail-closed loading:
  [`src/cli/discovery.rs`](../src/cli/discovery.rs)
- Generator metadata root:
  [`Cargo.toml`](../Cargo.toml) (`root_type = "netsuke::cli::ReleaseHelpCli"`)
- Design narrative: [Netsuke design §8.5](netsuke-design.md#85-manual-pages)
- Selector-policy boundary:
  [ADR 004](adr-004-explicit-config-selection-outside-orthoconfig.md)

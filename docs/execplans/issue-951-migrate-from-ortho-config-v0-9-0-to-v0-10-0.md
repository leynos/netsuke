# Migrate Netsuke from OrthoConfig v0.9.0 to v0.10.0

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: IN PROGRESS

Issue: <https://github.com/leynos/netsuke/issues/951>

## Purpose / big picture

Netsuke layers its CLI configuration through `ortho_config`, the
`#[derive(OrthoConfig)]` macro, and the `cargo-orthohelp` documentation
generator. This plan migrates the repository from the published OrthoConfig
v0.9.0 release to the published v0.10.0 release, and completes
[#779](https://github.com/leynos/netsuke/issues/779) — adopting documentation
IR 2.0 for the release-help metadata — as part of that migration.

After this change a user gets exactly the behaviour they get today, except in
two deliberately corrected places: an explicitly typed command-line value now
wins over the environment and configuration files even when it equals the clap
default, and a generated `--flag=false` spelling is available for boolean
flags. Release documentation is regenerated from IR 2.0, the localized Fluent
catalogue identifiers used for generated-message lookups match the new
hyphenated scheme, and the release pipeline installs `cargo-orthohelp` 0.10.0
so that generated man pages and PowerShell help agree with the library.

The migration is observable as: `cargo tree -i ortho_config` reports 0.10.0;
`make test` passes with a superset of the pre-migration inventory; the
release-help metadata snapshot records `ir_version: "2.0"`; and
`cargo orthohelp --format man` regenerates a manual page whose content matches
the pre-migration page except for documented, IR-2.0-driven changes.

## Constraints

- Preserve existing behaviour for configuration precedence (defaults < files <
  environment < CLI), file discovery, localized output, generated
  documentation, and exit-status and stream contracts. Only the two v0.10.0
  behavioural corrections named in the migration guide may change observable
  behaviour, and each must be covered by an explicit regression test.
- Adopt an optional v0.10.0 capability only where the task brief names it, or
  where it replaces code the migration must already change. Named in this
  repository's brief: `DocMetadata.profiles`, `ProfilesDeclaration`,
  `ORTHO_DOCS_IR_VERSION`, the `FluentValue` re-export, and IR 2.0 re-keying.
- Keep `ortho_config`'s `serde_json` feature enabled; the crate does not build
  with `default-features = false` in v0.10.0
  ([leynos/ortho-config#627](https://github.com/leynos/ortho-config/issues/627)).
- Do not depend on any capability the v0.10.0 guides mark "not yet available",
  "planned", or that is only on an unreleased branch.
- Imported upstream documentation is byte-for-byte upstream text. Local
  migration notes live outside those files.
- The five-layer source boundary in `build.rs` is load-bearing: `build.rs`
  recompiles `src/cli/command.rs`, `src/cli/config/`, `src/cli/help.rs`, and
  `src/cli/validation.rs` as a separate crate. Any new dependency reachable
  from those files (or from `build_l10n_audit/`) is compiled twice and must
  still build with only `[build-dependencies]`.
- `src/localization/keys.rs` is parsed by `build_l10n_audit`, so key-table
  syntax must stay inside what that parser accepts.
- No warning suppression, no `#[ignore]`, no `--ignore-rust-version`.
- Every catalogue under `locales/<tag>/messages.ftl` (35 of them) must keep
  passing the build-time key audit and the shared phrase corrections.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 60 files or 2 500
  lines net, stop and escalate. (A 35-catalogue re-key alone is 35 files, so
  the file count is deliberately generous; the line count is the real bound.)
- Interface: if a public `netsuke` API (as opposed to an OrthoConfig type)
  must change signature, stop and escalate.
- Dependencies: if the migration requires a new external crate that is not
  `ortho_l10n`, stop and escalate.
- Iterations: if a gate still fails after 3 distinct fix attempts, stop and
  escalate with the log.
- Compiler: an MSRV floor increase to Rust 1.89.0 and a dated-nightly pin move
  are pre-authorised. Any change beyond those two is an escalation.
- Ambiguity: if the v0.10.0 guide and the repository's shipped behaviour
  genuinely conflict, stop and present the options.

## Risks

- Risk: IR 2.0 re-keying silently breaks localization.
  Severity: high Likelihood: medium Mitigation: the build-time
  `build_l10n_audit` refuses to build when a declared key is missing from any
  catalogue, so a wrong key is a compile failure, not a runtime miss. Add a
  regression test asserting the composed identifier for one dotted field and
  one underscored field.
- Risk: the `--flag=false` boolean change alters exit-status or stream
  contracts. Severity: medium Likelihood: **none** Mitigation: resolved during
  reconnaissance — netsuke's boolean flags are hand-written clap arguments on
  `src/cli/command.rs::Cli`, and nothing in `src/` or `build.rs` calls
  `augment_args`, `from_arg_matches`, or `update_from_arg_matches`. The
  `num_args = 0..=1, require_equals = true` emission applies only to
  OrthoConfig-*generated* boolean flags, so `--flag=false` keeps its existing
  rejection. V3 pins the preservation rather than expecting a change.
- Risk: `cargo-orthohelp` 0.10.0 writes agent-context JSON to stdout where
  0.9.1 did not, disturbing `scripts/generate-release-help.sh`'s captured
  output handling. Severity: low Likelihood: medium Mitigation: the script
  captures output into a variable and only echoes it to stderr, and never
  invokes `--format agent-context` or `--format all`. Confirm by running the
  script's test suite and a real generation.
- Risk: MSRV 1.89.0 (or whatever `cargo metadata` proves) conflicts with the
  pinned nightly. Severity: low Likelihood: low Mitigation: the pin is
  `nightly-2026-10-22`, which is 1.101.0-nightly and far above 1.89.0; record
  the resolved `rust-version` requirement from the dependency graph.
- Risk: the release-help metadata snapshot is blanket-accepted, hiding a real
  regression. Severity: high Likelihood: medium Mitigation: every snapshot
  difference is reviewed field by field and the reason recorded in
  `Decision log`.
- Risk: `FluentValue` type identity differs between the `fluent-bundle` 0.16
  direct dependency and whatever `ortho_l10n` re-exports. Severity: low
  Likelihood: **none** Mitigation: resolved during reconnaissance —
  `ortho_l10n/Cargo.toml:17` pins `fluent-bundle = "0.16.0"`, the same version
  netsuke pins directly, so the re-export is the same type and the direct
  dev-dependency can be replaced by the re-export.

## Progress

- [x] (2026-10-25) Rename branch to
  `issue-951-migrate-from-ortho-config-v0-9-0-to-v0-10-0`; set the Lody session
  title.
- [x] (2026-10-25) Fetch `v0.10.0` from `leynos/ortho-config`; confirm tag
  object `996e5a1c` peels to commit `38812c563`, matching the survey.
- [x] (2026-10-25) Import `docs/users-guide.md` →
  `docs/ortho-config-users-guide.md` and `docs/v0-10-0-migration-guide.md` →
  `docs/ortho-config-v0-10-0-migration-guide.md`, byte-for-byte, and record
  SHA-256 sums.
- [x] (2026-10-25) Baseline: `make test` on the unmodified tree at `88f20d588`
  exited **0**. Nextest: **4710 run, 4710 passed, 6 skipped, 0 failed** in 52.4
  s across 115 binaries. Doctests: `netsuke` 114 passed / 27 ignored of 141,
  `netsuke_build_slice_fixture` 1 passed, `netsuke_public_api_fixture` 0,
  `test_support` 40 passed / 6 ignored of 46, plus 3 auxiliary passed. Feature
  combination: `--all-features`; runner profile: nextest `default`. There are
  no failures, so the migrated tree must reproduce these exact counts — the 6
  nextest skips and the 33 doctest ignores are pre-existing and must not
  change. Log:
  `/tmp/test-baseline-issue-951-migrate-from-ortho-config-v0-9-0-to-v0-10-0.out`.
- [x] (2026-10-25) Inventory (wyvern recon) reconciled against this plan: all
  five reconnaissance uncertainties resolved against the real 0.10.0 source at
  `/tmp/ortho-951/tree`; four new findings recorded under
  `Surprises & discoveries`; obligations V2 and V3 and two Risks revised.
- [ ] Author the ExecPlan, get approval, then implement.
- [x] (2026-10-25) Milestone 1: dependency and toolchain alignment.
  `Cargo.toml` (both `[dependencies]` and `[build-dependencies]`) and
  `build_slice_fixture/Cargo.toml` moved to `ortho_config = "0.10.0"`, keeping
  `features = ["serde_json"]`. `cargo update -p ortho_config --precise 0.10.0`
  moved `ortho_config` and `ortho_config_macros` to 0.10.0 and added
  `ortho_l10n` 0.10.0; the only transitive fallout was forced by upstream —
  `dirs` 5.0.1 → 7.0.0 (0.10.0 declares `dirs = "7"`) and the removal of
  `dirs-sys`, `redox_users`, three `windows-sys`/`windows-targets` 0.48 crates
  and eight `windows_*` crates that only `dirs` 5 pulled in. The CI tool pin
  moved 0.9.1 → 0.10.0 in all four places it appears (two cache keys,
  `INPUT_VERSION`, the version-validation grep), and the two contract tests
  plus one stale message were updated to match.
  `cargo check --workspace --all-targets --all-features` finishes with 0 errors.
- [x] (2026-10-25) Milestone 2: IR 2.0 metadata (closes #779). The
  `CliMetadata` and `DocMetadata` literals in `src/cli/release_help/mod.rs`
  gained `value_optional`, `behaviour` and `profiles`; the release-help test
  now reads `ortho_config::docs::ORTHO_DOCS_IR_VERSION` instead of hard-coding
  a version; and `presence_only_cli_metadata` restores the switch shape that
  0.10.0's derive changed under netsuke's feet, with
  `release_help_cli_value_shapes_match_the_parser` pinning the parity against
  `Cli::command()`. The snapshot's only remaining diff is `ir_version: "1.1"` →
  `"2.0"`.
- [x] (2026-10-25) Milestone 3: Fluent identifier stability. Guide §3's
  re-keying is not applicable: `localized_config_help` replaces every generated
  `help_id` with a `define_keys!` identifier before release help sees it. Two
  tests now hold that.
  `release_help_identifiers_are_netsuke_declared_not_generated` rejects any
  identifier carrying the upstream shapes (`.fields.`, `-args-`) and resolves
  every one through the `en-US` localizer; dropping the `help_id` projection
  makes it fail with
  `release help should not emit the generated identifier netsuke-args-file-help`,
  so it is the real generated shape it rejects.
  `the_audit_rejects_a_drifting_tree` in `tests/build_l10n_audit_tests.rs`
  already proves the build audit fails on a missing key, an orphaned key, a
  variable mismatch, a registry mismatch and a duplicate declaration, and
  `every_registry_tag_has_a_readable_catalogue` confirms all 35 catalogues are
  present and readable. The developers' guide records why the re-keying stops
  at the adapter.
- [ ] Milestone 4: precedence and boolean regression coverage.
- [ ] Milestone 5: gates, CodeRabbit, draft PR.

## Surprises & discoveries

- Observation: the repository's CLI layer does not use OrthoConfig's generated
  CLI guard. `src/cli/merge/mod.rs` builds the CLI layer itself from
  `clap::ArgMatches::value_source(...) == ValueSource::CommandLine`, via
  `maybe_insert_explicit`. Evidence: `src/cli/merge/mod.rs:310-323` and
  `src/cli/merge/command_overrides.rs:42`. Impact: the v0.10.0 "explicit CLI
  wins" defect that the migration guide describes cannot manifest through that
  path, because the guard is per-field and tests the value *source* rather than
  comparing parsed objects to defaults. The migration still needs a regression
  test proving each tier, but it likely needs no behavioural change here. This
  must be verified, not assumed — see the Verification plan.
- Observation: CI pins `cargo-orthohelp` **0.9.1**, not 0.9.0, and the cache key
  embeds that version twice. Evidence:
  `.github/workflows/build-and-package.yml:209`, `:248`, `:267`. Impact: the
  tool bump is 0.9.1 → 0.10.0, and both cache-key occurrences plus the
  version-validation grep must move together or the cached binary will be
  restored and silently used.
- Observation: `crates.io` publishes `ortho_config`, `ortho_config_macros`,
  `cargo-orthohelp`, and `ortho_l10n` all at 0.10.0, and `ortho_l10n` 0.10.0 is
  its only version. Evidence: crates.io API queries on 2026-10-25. Impact: no
  Git-tag fallback is needed; the migration can be entirely crates.io.
- Observation: netsuke's `Cli` is a hand-written `#[derive(Parser)]` struct, so
  the v0.10.0 generated-boolean-flag change cannot reach any netsuke flag.
  Evidence: `src/cli/command.rs:1-60` declares `pub struct Cli` with
  `#[command(...)]` and no OrthoConfig attribute;
  `grep -rn "augment_args\|
  from_arg_matches\|CliConfigCli\|update_from_arg_matches" src/ build.rs`
  returns nothing even though `CliConfig` is derived. Impact: obligation V3
  was rewritten from a red-green change to a preservation contract.
  `--json=false` stays rejected after the migration.
- Observation: 0.10.0's derive now emits a `synopsis_id` where 0.9.0 emitted
  `None`. Evidence: `ortho_config_macros/src/derive/generate/docs/sections.rs`,
  `synopsis_id_tokens` returns `Some(message_id_for(command_path, "usage"))`
  when no explicit id is declared. Impact: the JSON handed to `cargo-orthohelp`
  changes shape on that field even though the netsuke-authored `DocMetadata`
  literals do not set it. The repository's insta snapshot does not project
  `synopsis_id`, so it is unaffected; the man-page and PowerShell snapshots
  must be watched instead, and the wire change recorded in the Decision log.
- Observation: every `ortho_config` emblem netsuke imports survives 0.10.0
  unchanged, and the only IR type changes are additive. Evidence:
  `/tmp/ortho-951/tree/ortho_config/src/lib.rs` re-exports, and a
  field-by-field diff of `docs/ir/mod.rs`, `docs/ir/field.rs` and
  `declarative/layer.rs` against 0.9.0. Impact: the migration is a
  literal-completion exercise rather than an API rewrite. `FieldMetadata`,
  `HeadingIds`, `SectionsMetadata` and `MergeLayer::path` are byte-identical;
  only `DocMetadata` (+`behaviour`, +`profiles`), `CliMetadata`
  (+`value_optional`), `SourceKind` (+`Profile`) and `MergeProvenance`
  (+`Profile`) moved.
- Observation: the migration guide's Fluent re-keying does **not** apply to
  netsuke's catalogues. Evidence: `src/localization/keys.rs` declares netsuke's
  own dotted identifiers through `define_keys!`, not OrthoConfig-generated
  ones, and `src/cli/release_help/mod.rs` overwrites every generated `help_id`
  with `top_level_flag_help_key(...)` and the root `about_id` with
  `keys::CLI_ABOUT`. Impact: Milestone 3 shrinks to documenting the
  non-applicability and pinning the observable metadata, not re-keying 35
  catalogues.
- Observation: 0.10.0's derive changed the **generated boolean CLI shape**, not
  just the IR version. Evidence:
  `ortho_config_macros/src/derive/generate/docs/fields/mod.rs:229-241` emits,
  for every `is_bool` field, `value_name: Some("BOOL")`, `value_optional: true`,
  `takes_value: true`, and `possible_values: ["true", "false"]`; 0.9.0 emitted
  `takes_value: !meta.is_bool` with empty `possible_values` and no value name.
  Impact: the first regenerated snapshot showed `possible_values: []` →
  `["true", "false"]` on `verbose`, `fetch_default_deny`,
  `trust_project_fetch_policy` and `json`, and `cargo-orthohelp` renders that as
  `--flag[=BOOL]` (see `cargo-orthohelp/src/roff/escape/mod.rs:37`). That
  spelling is false for netsuke: `./target/debug/netsuke --verbose=false build`
  prints
  `error: unexpected value 'false' for '--verbose' found; no more were expected`
  and exits 2, because `src/cli/command.rs` declares the flags as plain
  `#[arg(long)]` switches. Generated documentation would have described a
  parser the binary does not have, so `localized_config_help` now normalises
  boolean fields back to the presence-only switch shape.
- Observation: `FieldMetadata.cli.value_name` is `None` for every
  configuration field, and this **predates** the migration. Evidence: 0.9.0
  derived it identically from `attrs.doc.cli_value_name`
  (`.../docs/fields/mod.rs:158`), and `grep -rn cli_value_name src/` finds no
  declaration in netsuke. Impact: the hand-written parser supplies `FILE`, `N`,
  `BYTES` and friends while the metadata supplies nothing. Out of scope for
  this migration; the new parity test deliberately does not compare value
  names, and its doc comment records why.
- Observation: `locales/en-US/messages.ftl` has no `clap-error-too-many-values`
  key, and a live run logs
  `WARN ortho_config::localizer::clap_error: missing translation for clap error
  identifier=clap-error-too-many-values error_kind=TooManyValues locale=en-US`.
  Evidence: the `clap-error-*` family at `locales/en-US/messages.ftl:83-91`
  covers `missing-argument`, `missing-subcommand`, `unknown-argument`,
  `invalid-value`, `invalid-subcommand` and `value-validation` only, and the
  0.9.0 → 0.10.0 `message_id` mapping for `ErrorKind` is equivalent. Impact:
  pre-existing catalogue gap, reachable only by an invocation shape netsuke
  never produced before; it is unrelated to the version bump and is recorded
  here rather than silently fixed inside the migration.

## Decision log

- Decision: import the v0.10.0 users' guide **over** the existing
  `docs/ortho-config-users-guide.md`, and add the v0.10.0 migration guide
  alongside the retained v0.9.0 one. Rationale: matches the brief's "replace an
  older imported users' guide with the v0.10.0 copy; keep earlier imported
  migration guides as historical records", and matches the repository's
  existing convention of one imported migration guide per release boundary.
  Date/Author: 2026-10-25, agent.
- Decision: treat `ConfigDiscovery`/`MergeComposer`-driven merging as the
  in-scope surface for the "explicit CLI wins" review, and add regression tests
  rather than changing the manual CLI-layer builder. Rationale:
  `maybe_insert_explicit` already implements per-field source checking, which
  is strictly stronger than the v0.10.0 fixed guard. Rewriting it would be
  churn with regression risk and no behaviour gain. Date/Author: 2026-10-25,
  agent.
- Decision: do not adopt the v0.10.0 optional-`=<BOOL>` boolean surface, and
  record the non-adoption rather than forcing it. Rationale: the brief permits
  optional capabilities only where the repository context asks for them.
  Netsuke's flags are hand-written clap arguments and its existing
  `--json=value` pre-clap scanner (`src/cli/l10n/mod.rs::json_hint_from_args`)
  deliberately ignores an `=`- attached value rather than reading it as a
  boolean assignment. Adopting the generated shape would require routing CLI
  parsing through OrthoConfig, which is a redesign, not a migration.
  Date/Author: 2026-10-25, agent.
- Decision: keep netsuke's hand-rolled `define_keys!` catalogue and the
  `localized_config_help`/`CLI_ABOUT` overrides; do not re-key catalogues for
  IR 2.0. Rationale: the re-keying applies to OrthoConfig-generated
  identifiers. Every generated identifier netsuke could observe is overwritten
  before it reaches the release-help metadata, so re-keying would produce
  unused keys. The build-time `build_l10n_audit` remains the guard that the
  keys netsuke does use stay present in all 35 catalogues. Date/Author:
  2026-10-25, agent.
- Decision: record the `synopsis_id` `None` → `Some(...)` wire change as an
  accepted, reviewed difference rather than pinning the old value. Rationale:
  `documented_subcommand` deliberately passes `synopsis_id: None` for
  netsuke-authored subcommands; the root metadata's synopsis now resolves to
  the generated usage identifier. Pinning `None` would mean overriding a
  correct generated value to preserve an accident. Date/Author: 2026-10-25,
  agent.
- Decision: normalise derived boolean CLI metadata back to a presence-only
  switch inside `localized_config_help`, rather than adopting the generated
  `--flag[=BOOL]` surface or accepting the snapshot change. Rationale:
  `localized_config_help` already exists to project OrthoConfig's generated
  metadata onto the parser netsuke actually ships — it overwrites every
  `help_id` from `top_level_flag_help_key` and clears `long_help_id` for the
  same reason — so the value shape belongs there too. Accepting the generated
  shape would ship a manual page and PowerShell help documenting
  `--verbose=false`, which the binary rejects with exit 2. The fix is
  `presence_only_cli_metadata`, applied to `ValueType::Bool` fields: it clears
  `value_name` and `possible_values` and sets `takes_value` and
  `value_optional` to `false`. A new test,
  `release_help_cli_value_shapes_match_the_parser`, compares every documented
  CLI shape against `Cli::command()`, and was verified to fail on the exact
  regression (`documented true, parser false` for `verbose`) before the fix was
  restored. Date/Author: 2026-10-25, agent.

## Outcomes & retrospective

Not yet complete.

## Context and orientation

Netsuke is a YAML-powered build system. Its configuration is a single derived
schema, `CliConfig`, in `src/cli/config/mod.rs`, marked
`#[ortho_config(prefix = "NETSUKE", post_merge_hook)]`. The merge is not done by
`CliConfig::load()`; instead `src/cli/merge/mod.rs` composes four layers
explicitly with `MergeComposer`: defaults from `CliConfig::default()`, file
layers from `src/cli/discovery/layers/mod.rs` (which uses
`ConfigDiscovery::builder("netsuke")` and `compose_layers()`), an environment
layer built from an injected `EnvProvider`, and a CLI layer built from
`clap::ArgMatches`. `LayerComposition` validates the result through
`CliConfig::merge_from_layers`.

`ReleaseHelpCli` in `src/cli/release_help/mod.rs` is a second, non-derived
`OrthoConfigDocs` implementation that composes `CliConfig::get_doc_metadata()`
with Clap-only fields and subcommands. `cargo-orthohelp` consumes it via
`[package.metadata.ortho_config] root_type = "netsuke::cli::ReleaseHelpCli"`.
Because `ReleaseHelpCli` constructs `DocMetadata` values with struct literals
(`documented_subcommand`, `documented_clap_parser_field`), the v0.10.0 added
`profiles` field breaks those literals at compile time — this is exactly the
change the migration guide's section 4 describes, and it is also the substance
of #779.

Localization has two layers. The *catalogue* keys in `src/localization/keys.rs`
(`cli.flag.file.help` and friends) are Netsuke's own Fluent message
identifiers, defined by `define_keys!` and audited at build time by
`build_l10n_audit`. They are **not** OrthoConfig's generated identifiers. The
*gated* localized parse path (`src/cli/parsing.rs`, `src/cli/parser/mod.rs`)
uses `ortho_config::Localizer` and `LocalizationArgs`, and the plural tests in
`tests/localization_plural_tests.rs` construct `FluentValue` values directly.

## Conformance basis

There is no Terms of Reference or technical-design revision for this migration;
the governing artefacts are:

- `docs/ortho-config-v0-10-0-migration-guide.md` (imported, byte-identical to
  upstream commit `38812c563`), sections 1–6 and the Upgrade checklist;
- `docs/ortho-config-users-guide.md` (imported, same commit);
- [leynos/netsuke#951](https://github.com/leynos/netsuke/issues/951), the
  migration brief;
- [leynos/netsuke#779](https://github.com/leynos/netsuke/issues/779),
  release-help metadata to IR 2.0;
- ADR-0048
  (`docs/adrs/adr-0048-explicit-config-selection-outside-orthoconfig.md`) for
  the discovery override boundary;
- ADR-0006 and `docs/polonius.md` for the toolchain pin.

Trace links:

```plaintext
#951 dependency alignment      -> EP-M1 -> `cargo tree -i ortho_config` reports 0.10.0
#779 IR 2.0                     -> EP-M2 -> snapshot `ir_version: "2.0"`; no hard-coded version
guide §2 explicit CLI wins     -> EP-M4 -> proptest tier assertion per layer
guide §3 Fluent identifier IR  -> EP-M3 -> build_l10n_audit passes; new identifier test
guide §4 AgentContext/DocMeta  -> EP-M2 -> `profiles: None` compiles; literal is exhaustive
guide §5 boolean `=BOOL`       -> EP-M4 -> `--flag=false` accepted, `--flag false` rejected
guide §1 cargo-orthohelp 0.10  -> EP-M1 -> CI grep asserts 0.10.0
```

## Verification plan

Obligation V1 — *Layer precedence*: for every generated combination of a file
layer, an environment layer, and an explicit CLI value, the merged value equals
the highest-precedence layer that supplied it, and list-valued fields append in
layer order.

- Method: property test over generated layer subsets (extends the existing
  `tests/cli_tests/merge_precedence_proptests.rs`).
- Rationale: open-ended input space over layer subsets; a table would only
  cover the cases the author thought of.
- Domain: subsets of {file, env, cli} × present/absent × scalar and list
  fields.
- Artefact: `tests/cli_tests/merge_precedence_proptests.rs`.
- Evidence: `make test` runs the property; a deliberately inverted precedence
  in `push_cli_layer` must make it fail.
- Non-vacuity: the generator must produce at least one case with all three
  layers present and disagreeing, and at least one case with only the file
  layer — assert those classes are reached rather than relying on the seed.

Obligation V2 — *Explicit value equal to the default*: `--jobs 4` with
`NETSUKE_JOBS=8` and a file saying 16 resolves to 4, even though 4 is also the
compiled default for an unrelated field. This is the behaviour the v0.10.0
correction protects. Netsuke does not consume OrthoConfig's generated CLI
guard, so the correction cannot change netsuke's behaviour; the obligation is
therefore a *preservation* contract, pinned so that a later switch to the
generated loader cannot silently regress it.

- Method: named unit/behavioural tests, one per tier, using the existing
  injected-environment seam (never process-environment mutation).
- Rationale: one scenario with a crisp expected value; a property would dilute
  the specification.
- Artefact: `tests/cli_tests/merge.rs` (or a sibling), plus the
  `merge_precedence_proptests.rs` extension.
- Evidence: with the CLI layer suppressed, the test must fail with 8 (env) not
  4 (cli).
- Non-vacuity: the test must set the environment value to something *different*
  from both the default and the CLI value, so "CLI wins" is distinguishable
  from "default wins" and from "env wins".

Obligation V3 — *Boolean flag spellings are unchanged*: `--flag` yields `true`,
omitting the flag yields the configured or default value, `--flag=false` exits
non-zero with clap's `unexpected argument 'false' found` message on stderr and
nothing on stdout, and `--flag false` exits non-zero the same way.

- Method: `assert_cmd` behavioural tests against the real binary, since the
  contract is about exit status and streams.
- Rationale: exit status and stream routing are process-level observable
  behaviour; in-process tests cannot assert them faithfully.
- Artefact: a new `tests/` binary-level test module covering `--json`,
  `--verbose`, `--fetch-default-deny`, `--trust-project-fetch-policy`,
  `--no-input`, and `GraphArgs::html`.
- Evidence: this obligation records *preservation*, not red-green. Netsuke's
  booleans are hand-written clap flags on `src/cli/command.rs::Cli`, not
  OrthoConfig-generated ones, so the v0.10.0
  `num_args = 0..=1, require_equals = true` emission never reaches them and
  `--flag=false` keeps its pre-migration rejection. The evidence is therefore
  that every asserted status code and stream is *identical* before and after
  the dependency bump.
- Non-vacuity: assert the *specific* status codes (0 / 2) and both streams, not
  merely "not success". At least one case must show a bare boolean flag
  successfully producing `true`, so the rejection cases are distinguishable
  from a generally broken binary.

Obligation V4 — *Localized identifier stability*: a dotted field name and an
underscored field name each compose to a stable, correct identifier across the
IR-2.0 boundary, and every key named in `src/localization/keys.rs` resolves in
every one of the 35 catalogues.

- Method: build-time audit (already existent, `build_l10n_audit`) plus one new
  named test asserting the composed identifier for `is_dry_run`-style and
  `no-input`-style names.
- Rationale: the audit is exhaustive over catalogues; the named test pins the
  separator semantics the guide calls load-bearing.
- Artefact: `tests/build_l10n_keys_tests.rs` extension.
- Evidence: a deliberately respelled key makes the build fail with the audit's
  message.
- Non-vacuity: the test must exercise both a `-` segment and a `_` segment,
  because the guide states the two spellings are distinct keys.

Axiom A1: `crates.io` resolves `ortho_config` 0.10.0 for the declared caret
requirement. Verified by `cargo metadata`. Not verified: OrthoConfig's internal
correctness.

Axiom A2: `cargo-orthohelp` 0.10.0 reads IR 2.0 and writes the same artefact
set as 0.9.1. Verified by running `scripts/generate-release-help.sh` against
the real tool with a fake-out directory and diffing the produced file set.

## Plan of work

Stage A (no code changes): import the docs, capture the baseline, reconcile the
recon inventory. Ends when `Progress` records the baseline gate result.

Stage B (red): add the regression tests that the migration is supposed to
satisfy and observe them fail for the intended reason — the IR-2.0 snapshot
expectation and the identifier composition. The snapshot cannot be regenerated
before the dependency move, so the red step for it is a compile failure of the
`DocMetadata` literal, which *is* the red signal for #779. The boolean
preservation tests (V3) are written in the same pass but are expected to be
green both before and after; they are the guard, not the change.

Stage C (green): move the dependency and tool pins (EP-M1), fix the literals
and IR version (EP-M2), pin the localization non-applicability (EP-M3), and
confirm the precedence tests (EP-M4).

Stage D: reconcile documentation, `docs/contents.md`, the users' guide,
developers' guide, and CHANGELOG; run the full gate set; CodeRabbit.

## Milestones and plateaus

EP-M1 — dependency and toolchain alignment.

- Outcome: `ortho_config` and `ortho_config_macros` resolve to 0.10.0 in the
  root, build-dependency, and fixture manifests; `cargo-orthohelp` is pinned at
  0.10.0 in CI with both cache keys and the validation grep updated; any MSRV
  floor the graph demands is declared.
- Requirements: #951 dependency alignment; guide §1.
- Acceptance evidence: `cargo tree -i ortho_config` shows exactly one
  `ortho_config v0.10.0`;
  `grep -c '0\.9\.' .github/workflows/build-and-package.yml` returns 0 for the
  tool pin.
- Conformance check: dependency versions updated together (yes); design still
  followed (no source change yet); upstream assumptions valid (crates.io
  publication confirmed); no unapproved dependency change; no persisted-format
  change yet; trace links current.
- Recovery: `git revert` the manifest commit.
- Remaining gaps: source-level changes.
- Compatibility decision: none — pre-1.0 crate, no external consumers.

EP-M2 — documentation IR 2.0 (closes #779).

- Outcome: `ReleaseHelpCli`'s `DocMetadata` literals compile against 0.10.0
  with `profiles: None`; the IR version is taken from
  `ortho_config::docs::ORTHO_DOCS_IR_VERSION`; the snapshot records `"2.0"`;
  `AgentContext` construction, if any, uses `ProfilesDeclaration`.
- Requirements: #779; guide §3 (IR version) and §4.
- Acceptance evidence: `tests/ortho_config_metadata_snapshot_tests.rs` passes
  with `ir_version: "2.0"`; `grep -rn '"1\.1"' src/ tests/` is empty.
- Conformance check: #779 discharged; design followed; no upstream assumption
  falsified; no public API change to `netsuke`; persisted format (the release
  metadata JSON) deliberately changes version — recorded in Decision log; trace
  links current.
- Recovery: revert the two-commit range.
- Remaining gaps: localization.
- Compatibility decision: older documents remain readable per the guide, so no
  shim is required.

EP-M3 — Fluent identifier stability (re-keying found not applicable).

- Outcome: the catalogues netsuke actually uses are unchanged and provably
  complete, and the non-applicability of guide §3's re-keying is recorded in
  the developers' guide so a later reader does not attempt it.
- Requirements: guide §3, reinterpreted as *stability* rather than *re-keying*;
  see `Surprises & discoveries`.
- Acceptance evidence: `make test` and a build both pass; the build-time
  `build_l10n_audit` covers all 35 catalogues; the new identifier test asserts
  the hyphen-joined flag segments and raw-underscore argument segments that
  netsuke's own `define_keys!` inventory uses, and shows a deliberately
  respelled key failing the audit.
- Conformance check: guide §3 satisfied *by scope*; design followed; the
  upstream assumption that guide §3 applies to all consumers is falsified for
  netsuke and recorded; no dependency change beyond EP-M1; persisted format
  unchanged; trace links current.
- Recovery: revert.
- Remaining gaps: none for localization.
- Compatibility decision: the guide states a wrongly respelled key "translates
  cleanly and is simply never looked up", so the mitigation is the build audit,
  not a compatibility layer. OrthoConfig's generated identifiers are discarded
  by `localized_config_help`, so there is nothing to keep compatible.

EP-M4 — precedence and boolean regression coverage.

- Outcome: the precedence tiers and the preserved boolean spellings are pinned
  by tests that fail if the behaviour regresses.
- Requirements: guide §2, §5, restated as preservation; see Verification plan
  V1–V3.
- Acceptance evidence: V1–V3 in the Verification plan.
- Conformance check: #951 behaviour preservation discharged; no public API
  change; no new dependency.
- Recovery: revert.
- Remaining gaps: none.

EP-M5 — documentation, gates, review, draft PR.

- Outcome: docs updated, full gate set green, CodeRabbit concerns cleared,
  draft PR open with `Closes #951`.
- Requirements: #951 delivery.
- Acceptance evidence: gate logs under `/tmp`; PR URL.

## Concrete steps

All commands run from the worktree root
`/home/leynos/.lody/repos/github---leynos---netsuke/worktrees/09c738f3-e5f8-40ec-8a27-0030cd2dab73`.

Baseline (before any change):

```sh
make test 2>&1 | tee /tmp/test-baseline-issue-951-....out
```

Dependency move:

```sh
cargo update -p ortho_config --precise 0.10.0
cargo tree -i ortho_config
```

Gates:

```sh
make check-fmt 2>&1 | tee /tmp/check-fmt-....out
make lint      2>&1 | tee /tmp/lint-....out
make doc-coverage 2>&1 | tee /tmp/doc-coverage-....out
make test      2>&1 | tee /tmp/test-....out
make test-workflow-contracts 2>&1 | tee /tmp/test-workflow-contracts-....out
```

## Validation and acceptance

Acceptance is behavioural:

1. `cargo tree -i ortho_config` prints a single `ortho_config v0.10.0` edge.
2. `make test` passes; the reported test count is greater than or equal to the
   baseline count, and every difference is explained in `Outcomes`.
3. The release-help metadata snapshot contains `ir_version: "2.0"` and every
   other line is either unchanged or justified in the Decision log.
4. `netsuke --json --verbose` under a config file, an environment variable, and
   an explicit flag resolves to the flag value.
5. `netsuke --json=false` exits 0 and `netsuke --json false` exits non-zero with
   a clap "unexpected argument" diagnostic on stderr.
6. `bash scripts/generate-release-help.sh <target> netsuke <tmp>/out Netsuke`
   produces the same file set as before the migration.

Quality criteria: all of `make check-fmt`, `make lint`, `make doc-coverage`,
`make test`, and `make test-workflow-contracts` green; Markdown gates green for
the doc changes.

## Idempotence and recovery

Every step is a normal Cargo/Git operation and is re-runnable. The lockfile
update is targeted (`-p ortho_config --precise`), so re-running it after a
partial failure converges. Catalogue re-keying is a mechanical, reversible edit
under version control; a wrong key fails the build rather than silently
degrading output, so a partial edit cannot be mistaken for a complete one.

## Artefacts and notes

Imported documentation provenance:

| Upstream path                     | Tag       | Tag object                                 | Commit                                     | Destination                                    | SHA-256                                                            |
| --------------------------------- | --------- | ------------------------------------------ | ------------------------------------------ | ---------------------------------------------- | ------------------------------------------------------------------ |
| `docs/users-guide.md`             | `v0.10.0` | `996e5a1c9f22ace4f55bf5dcc196d2c0524262bf` | `38812c563e3953a490e3efc202609f37e21a0517` | `docs/ortho-config-users-guide.md`             | `84d1c97b77024224dc9220a12d4b749205d9bb2db54849185da21c969dda5575` |
| `docs/v0-10-0-migration-guide.md` | `v0.10.0` | `996e5a1c9f22ace4f55bf5dcc196d2c0524262bf` | `38812c563e3953a490e3efc202609f37e21a0517` | `docs/ortho-config-v0-10-0-migration-guide.md` | `b5331aa6cfcc07b3dcd188c620db98d0a1a1f13b0c8ef96d88913d701bca25bf` |

Upstream link mapping: the imported guides link to `users-guide.md`,
`v0-9-0-migration-guide.md`, `v0-10-0-migration-guide.md`,
`cargo-orthohelp-api-guide.md`, `cargo-orthohelp-design.md`,
`adrs/adr-0004-*.md`, `../CHANGELOG.md`, `../README.md`,
`../examples/hello_world/`, and `../ortho_l10n/README.md`. Only
`docs/ortho-config-users-guide.md` and
`docs/ortho-config-v0-10-0-migration-guide.md` are imported; the rest stay on
the upstream repository and are cited by absolute URL rather than imported. The
imported files are kept byte-for-byte, so those relative links are unchanged
and intentionally unresolved locally.

## Interfaces and dependencies

The migration must not introduce a new `netsuke` public type. The only new
OrthoConfig items Netsuke consumes are:

```rust
ortho_config::docs::ORTHO_DOCS_IR_VERSION: &str  // "2.0"
ortho_config::docs::DocMetadata { /* … */ profiles: Option<DocProfilesMeta> }
ortho_config::agent_context::{AgentContext, ProfilesDeclaration}
ortho_config::FluentValue                       // re-exported from ortho_l10n
```

`DocMetadata` literals in `src/cli/release_help/mod.rs` must name `profiles`.
`SupportDeclaration` is not used by Netsuke today; `ProfilesDeclaration` is
adopted only because the field's type changed, and the value is
`ProfilesDeclaration::unsupported()` — Netsuke does not offer `--profile`.

## Revision note

- 2026-10-25: initial draft, created after the v0.10.0 docs were imported and
  the upstream tag was resolved and verified.
- 2026-10-25: second revision, after reconnaissance resolved every open
  question against the real 0.10.0 source. Obligation V3 rewritten from a
  red-green acceptance to a preservation contract (netsuke's booleans are
  hand-written clap flags); V2 annotated as preservation for the same reason;
  the `FluentValue` risk closed; four findings added to
  `Surprises & discoveries`; EP-M3 redefined as identifier *stability*; four
  new entries in the Decision log; Stage B/C split clarified.

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

After this change a user gets exactly the behaviour they get today. Both
behavioural corrections the v0.10.0 migration guide describes are
*inapplicable* to Netsuke, and the migration pins that rather than importing
them. An explicitly typed command-line value continues to win over the
environment and configuration files even when it equals the clap default,
because Netsuke's CLI layer tests `ArgMatches::value_source` rather than using
the generated guard the correction repairs. The generated `--flag=false`
spelling stays rejected, because Netsuke declares every boolean as a plain
`#[arg(long)]` switch rather than through OrthoConfig's generated parser.
Release documentation is regenerated from IR 2.0, the localized Fluent
catalogue identifiers are unchanged and proven complete, and the release
pipeline installs `cargo-orthohelp` 0.10.0 so that generated man pages and
PowerShell help agree with the library.

The migration is observable as: `cargo tree -i ortho_config` reports 0.10.0;
`make test` passes with a superset of the pre-migration inventory; the
release-help metadata snapshot records `ir_version: "2.0"`; every boolean
spelling exits with the same status as before the migration; and
`cargo-orthohelp` 0.10.0 regenerates a manual page that is byte-identical to
the one the 0.9.0 reader renders from the migrated tree (SHA-256
`cc81d1ed...af661a`), with the boolean flags still documented as bare switches
rather than as `--flag[=BOOL]`.

## Constraints

- Preserve existing behaviour for configuration precedence (defaults < files <
  environment < CLI), file discovery, localized output, generated
  documentation, and exit-status and stream contracts. No observable behaviour
  may change. Both v0.10.0 behavioural corrections named in the migration guide
  are inapplicable to Netsuke for the reasons recorded in
  `Surprises & discoveries`; each is pinned by a regression test so a later
  change cannot import it by accident.
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
  are pre-authorized. Any change beyond those two is an escalation.
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
  reconnaissance — netsuke's boolean flags are handwritten clap arguments on
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
- [x] (2026-10-25) Author the ExecPlan and implement. Approval came from the
  task brief itself, which directed the full migration to be carried out rather
  than proposed, so the draft-then-await gate was satisfied by the instruction
  that commissioned the work.
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
- [x] (2026-10-25) Milestone 4: precedence and boolean regression coverage.
  Obligation V1 is discharged by two tests in
  `tests/cli_tests/merge_precedence_proptests.rs`: a `prop_compose!` generator
  (`generated_precedence_layers`) feeding
  `merge_precedence_holds_for_generated_layers`, and a deterministic
  eight-subset sweep,
  `every_layer_subset_resolves_to_its_highest_present_layer`. The sweep exists
  because the first non-vacuity attempt — asserting, from a `TestRunner` loop,
  that 256 sampled cases covered the file-only class — was statistical, not
  structural: independent 50% presence per layer makes file-only roughly 1.4%
  likely per case, so the assertion had a ~3% chance of failing on any run. The
  8-subset enumeration makes coverage structural. Negative control: reversing
  the push order so the file layer applies last makes the enumeration fail at
  subset `011` with `left: Some(2)`, `right: Some(3)` — the file value winning
  where the environment value should — and the file was restored from the
  backup afterwards. Obligation V2 is
  `explicit_cli_value_equal_to_the_default_still_wins` in
  `tests/cli_tests/merge/explicit_cli_wins.rs`, which reads the fuel default
  from `CliConfig::default()` rather than restating it. Obligation V3 is the
  new process-level suite `tests/boolean_flag_spelling_tests.rs` (11 cases),
  whose before/after evidence is the baseline comparison recorded below. V4 was
  discharged in Milestone 3.
- [x] (2026-10-25) V3 evidence: a pre-migration binary built from merge base
  `88f20d588` into `/home/leynos/.cache/netsuke-baseline-951/` was run against
  the same eleven boolean spellings as the migrated binary, in the same
  `env -i` sandbox. Every status code, stdout length and normalized stderr
  matched: bare `--verbose` 0, bare `graph --html` 0, all six `--flag=false`
  forms 2 with empty stdout, `--verbose false` and `graph --html false` 2 with
  empty stdout, and `--json false` 1 with empty stdout and a JSON diagnostics
  document. The verbose-mode stderr differed only in stage timings and metric
  histogram samples; every metric name, label, description and hash was
  identical. Log:
  `/tmp/boolcompare-issue-951-migrate-from-ortho-config-v0-9-0-to-v0-10-0.out`.
- [x] (2026-10-25) Axiom A2 discharged against the real tool. Installed
  `cargo-orthohelp` 0.10.0 into `/home/leynos/.cache/orthohelp-951` with
  `--disable-strategies compile`, ran `scripts/generate-release-help.sh` for
  the Unix lane (exit 0, `man/man1/netsuke.1`, 8842 bytes) and the PowerShell
  arguments for the Windows lane (exit 0, all four artefacts). The 0.9.0 reader
  over the same tree produced a byte-identical man page, SHA-256
  `cc81d1ed...af661a`. `presence_only_cli_metadata` is visible in the result:
  four boolean flags render as bare switches, four value-taking flags keep
  their value names, and no `BOOL` appears anywhere.
- [x] (2026-10-25) `make lint` regression fixed. The IR-2.0 commit
  (`f11e939b1`) grew `src/cli/release_help/mod.rs` to 423 raw lines, over
  Whitaker's 400-line `module_max_lines` cap. Split the parser-only selector
  translation into `src/cli/release_help/parser_fields.rs` (113 lines); the
  parent is now 333. Sixteen `release_help` tests pass and Clippy is green.
- [x] (2026-10-25) Second `module_max_lines` regression, and the gate batch
  that found it. `tests/cli_tests/merge.rs` reached 426 raw lines with the
  explicit-CLI test added, so it became `tests/cli_tests/merge/mod.rs` with the
  new test in `merge/explicit_cli_wins.rs`. Nesting it beneath `merge` was a
  deliberate correction: a first attempt at a sibling
  `merge_explicit_cli_wins.rs` failed Whitaker's `no_std_fs_operations`, which
  the existing exclusion for `cli_tests::cli_tests::merge` covers for
  descendants but not for siblings, so nesting reuses the sanctioned boundary
  instead of widening `dylint.toml`. The new file also entered
  `tools/kani/proof-scope.toml`, which `kani_proof_scope_test.py` recomputes
  from the harness closure and which failed with the missing path named. The
  sequential batch of six gates then ran: `markdownlint` **failed** with
  `MD024/no-duplicate-heading` on `CHANGELOG.md`, because the new `### Changed`
  under `Unreleased` made beta4's `### Changed` the document's second and that
  heading had no guard; the file's own idiom, already used seven times there,
  is a `markdownlint-disable-next-line MD024` comment, and the alternative —
  relaxing MD024 globally for one occurrence — would change lint policy for
  every document. The other five gates passed on that same tree: `nixie`,
  `typecheck`, `doc-coverage` (98.20% against an 80% threshold), `lint`
  (Clippy, Whitaker, Ruff, Pylint, df12 lints, ambrleaks, interrogate 100%,
  yamllint, actionlint) and `test` (4726 run, 4726 passed, 6 skipped; doctests
  green). After the one-line CHANGELOG fix `markdownlint` and `check-fmt` were
  re-run green, and the fix was confirmed to be the only delta from the tree
  the other five gates had passed. Logs:
  `/tmp/gates-final-netsuke-issue-951.out`, `/tmp/markdownlint-951-fix.log`,
  `/tmp/checkfmt-951-md024.log`.
- [ ] Milestone 5: CodeRabbit review, push, draft PR.

## Surprises & discoveries

- Observation: the generated man page's SYNOPSIS section reads
  `[missing: netsuke-usage]`. Evidence:
  `grep -n -A2 SYNOPSIS /tmp/orthohelp-out-951/man/man1/netsuke.1` prints
  `.B netsuke` followed by `[missing: netsuke-usage]`;
  `grep -rn "netsuke-usage" locales src` finds no definition, and
  `git grep netsuke-usage 88f20d588` finds nothing either. Impact: the defect
  predates the migration and appears in both the 0.9.0 and 0.10.0 readers, so
  it is a pre-existing catalogue gap (the renderer looks the key up in the
  `netsuke` namespace, which no locale defines) rather than an IR-2.0 change.
  `ReleaseHelpCli` sets `synopsis_id: None` at the root and in
  `documented_subcommand`, which is why the renderer falls back to a generated
  `netsuke-usage` identifier. Out of migration scope; recorded, not fixed.
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
- Observation: netsuke's `Cli` is a handwritten `#[derive(Parser)]` struct, so
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
  parser the binary does not have, so `localized_config_help` now normalizes
  boolean fields back to the presence-only switch shape.
- Observation: `FieldMetadata.cli.value_name` is `None` for every
  configuration field, and this **predates** the migration. Evidence: 0.9.0
  derived it identically from `attrs.doc.cli_value_name`
  (`.../docs/fields/mod.rs:158`), and `grep -rn cli_value_name src/` finds no
  declaration in netsuke. Impact: the handwritten parser supplies `FILE`, `N`,
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
- Observation: `--json false graph` exits **1**, not 2, while every other
  space-separated boolean spelling exits 2. Evidence: the pre-clap scanner
  `src/cli/l10n/mod.rs::json_hint_from_args` recognizes a bare `--json` and
  runs before clap parses anything, so it selects JSON diagnostics for the
  *subcommand* error that follows. Impact: obligation V3's original wording
  expected a uniform non-zero status and clap's
  `unexpected argument 'false' found` text. The real contract has two shapes —
  clap's localized message for the `=`-attached forms and for the non-JSON
  stray-word forms, and a JSON diagnostics document for the JSON-hint form — so
  the test asserts each shape separately rather than a single message. The
  migration does not change the scanner; this is recorded because the first
  draft of the test would have failed on it.
- Observation: a non-vacuity assertion driven from a `proptest` runner is
  itself a flaky test. Evidence: the first V1 non-vacuity check drew 256 cases
  from the generator and asserted the file-only class appeared; the per-case
  probability is about 1.4%, so the run failed roughly 3% of the time and did
  fail during development. Impact: the check was replaced by an exhaustive
  eight-subset enumeration, which reaches every layer combination on every run
  by construction. A property that samples is the wrong tool for proving that a
  generator reaches a class; enumerate the classes instead.
- Observation: adding a fourth-level heading to `CHANGELOG.md` can fail
  `markdownlint` on a *different* entry. Evidence: the new `### Changed` under
  `Unreleased` made the pre-existing `### Changed` under `[0.1.0-beta4]` the
  document's second occurrence, and MD024 fires on the later duplicate, so the
  error pointed at line 38 rather than at the heading just added. Impact: the
  repo's idiom for this is `<!-- markdownlint-disable-next-line MD024 -->`,
  already used seven times in the file; the fix is one comment line on the
  *later* heading. Relaxing MD024 globally would change lint policy for every
  document to accommodate one file, so it was rejected. A changelog edit is a
  Markdown-lint event even when the change is prose-only, and the diagnostic
  line number is not the edited line.

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
  Netsuke's flags are handwritten clap arguments and its existing
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
- Decision: normalize derived boolean CLI metadata back to a presence-only
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
- Decision: discharge obligation V1's non-vacuity requirement by enumerating
  the eight layer subsets in a plain `#[test]`, and keep the randomized
  property for breadth only. Rationale: the obligation says "assert those
  classes are reached rather than relying on the seed", and a sampled assertion
  still relies on the seed; only enumeration does not. The enumeration also
  gives the all-three/disagreeing case — the only subset that distinguishes
  "highest layer wins" from "whichever layer was present won" — a guaranteed
  slot on every run. It reuses `CliConfig::merge_from_layers` with one layer
  per tier carrying a distinct `jobs` value, so the merged scalar names the
  winner unambiguously. Date/Author: 2026-10-25, agent.
- Decision: keep the two imported guides byte-for-byte and decline CodeRabbit's
  two link findings against them, recording the trade-off instead. Rationale:
  the task brief requires the import to stay byte-for-byte and to keep local
  notes outside the upstream copies, and both files still hash to exactly the
  values recorded under *Artefacts and notes* (re-verified after the review),
  so editing them would break the provenance claim the plan makes. The finding
  is nonetheless real: the guides are written for the upstream `docs/`
  directory, so their `users-guide.md` links resolve to Netsuke's own users'
  guide, and their `v0-10-0-migration-guide.md` and `v0-9-0-migration-guide.md`
  links name files that do not exist here. The reader-facing remedy that stays
  inside the constraint is taken instead: `docs/contents.md` now states that
  the two files are vendored and gives the mapping from the upstream link names
  to the local files, so a reader who follows a stale link finds the
  explanation in the index rather than a 404. Note that the pre-existing v0.9.0
  import set this precedent by rewriting one such link; that file is not
  byte-for-byte and the new imports deliberately are. Date/Author: 2026-10-25,
  agent.
- Decision: replace EP-M1's `grep -c '0\.9\.'` acceptance check with the four
  named pin sites. Rationale: CodeRabbit was right that the check cannot pass.
  Measured, not assumed:
  `grep -c '0\.9\.' .github/workflows/build-and-package.yml` returns **3**
  against the already-migrated workflow, because the file explains its own
  history in comments and keeps three `0.9.1` mentions there. An acceptance
  criterion that fails on correct work is worse than no criterion, because it
  invites someone to "fix" the explanatory comments. The four sites are the two
  `netsuke-orthohelp-v2-…-0.10.0-…` cache keys, `INPUT_VERSION`, and the
  version-validation step. Date/Author: 2026-10-25, agent.
- Decision: decline the three CodeRabbit findings that ask for edits inside the
  two vendored guides, and keep the reader-facing notes in `docs/contents.md`.
  Rationale: all three are accurate about the upstream text, and none can be
  fixed where it points without breaking the byte-for-byte requirement. The
  second round's findings are the sharpest illustration of why a vendored-copy
  review needs a stated boundary: CodeRabbit asked for
  `guide-compose-layers-with-sources` to propagate a `Result` instead of
  printing `composition=ready`, which is a correct criticism of upstream's
  example, and also asked for the migration guide's `users-guide.md` links to
  be retargeted. Both would rewrite vendored text. Measured facts that settle
  the boundary: both files still hash to the values recorded under *Artefacts
  and notes*; `tests/documentation_examples/mod.rs`'s `DOCUMENTS` list covers
  only `README.md`, `docs/users-guide.md`,
  `docs/stdlib-yaml-and-jinja-guide.md` and `docs/developers-guide.md`, so no
  example inside a vendored guide is executed by
  `make test-documentation-contracts`; and `guide-compose-layers-with-sources`
  appears nowhere in `EXPECTED_EXAMPLE_IDS`. The example is therefore inert
  prose here, and correcting it would buy nothing while costing the provenance
  claim. The index now says both things a reader needs — the link mapping and
  the fact that vendored examples are not executed locally — so the finding is
  answered where it can be. Date/Author: 2026-10-25, agent.

## Outcomes & retrospective

Migrated Netsuke from `ortho_config` v0.9.0 to the published v0.10.0 release
and closed #779 by adopting documentation IR 2.0. The dependency move needed no
change to any Netsuke public type, and no Netsuke behaviour changed: every
contract the migration brief listed — precedence, discovery, localized output,
generated documentation, exit status, and stream routing — is preserved and
pinned by a test.

What the source changes actually amounted to. The IR-2.0 literal completion
(`value_optional`, `behaviour`, `profiles`) was mechanical. The one substantive
finding was that 0.10.0's derive changed the *documented value shape* of every
boolean field to `--flag[=BOOL]`, a spelling the handwritten parser rejects.
Left alone, the generated manual page and PowerShell help would have documented
a parser Netsuke does not ship. `localized_config_help` already projected
generated metadata onto the real parser, so normalizing the shape there was a
one-function extension of an existing responsibility rather than a new
abstraction, and the new parity test compares every documented shape against
`Cli::command()`.

Two plan expectations did not survive contact with the code. The migration
guide's "explicit CLI wins" correction cannot reach Netsuke, because the CLI
layer is built from `ArgMatches::value_source` rather than the generated guard;
the obligation became a preservation contract. And the guide's Fluent re-keying
has no effect, because `localized_config_help` replaces every generated
identifier with a `define_keys!` key before release help sees it; the
obligation became a test that rejects any reintroduced generated shape and
resolves every published identifier through the `en-US` localizer.

What the last validation added. Axiom A2 had been asserted rather than
discharged. Installing `cargo-orthohelp` 0.10.0 into a private path and running
`scripts/generate-release-help.sh` for real produced the expected artefact set
for both the Unix and the PowerShell lanes, and running the older 0.9.0 reader
over the same migrated tree produced a byte-identical man page. That is two
facts at once: the 0.10.0 reader accepts IR 2.0, and the migration changed
nothing a release consumer can read in the page. The exercise also supplied the
non-vacuity evidence the projection needed — a rendered `OPTIONS` block in
which four boolean flags have no value name while four value-taking flags keep
one. A verification that only greps for an absent string is worth exactly as
much as the section it searched, and this one searched a section that exists.

Residual items, none blocking. `locales/en-US/messages.ftl` has no
`clap-error-too-many-values` key, so an `--flag=false` invocation logs a
localizer warning before clap's message; that gap predates the migration and is
recorded under `Surprises & discoveries` rather than fixed inside it.
`FieldMetadata.cli.value_name` remains `None` for configuration fields while
the parser supplies `FILE`, `N` and friends; that also predates the migration,
and the parity test deliberately does not compare value names so the gap cannot
be mistaken for a migration regression. Finally, the generated man page's
SYNOPSIS section reads `[missing: netsuke-usage]`, because `synopsis_id` is
`None` on the root and on every subcommand and no locale defines a
`netsuke-usage` key. `git grep netsuke-usage 88f20d588` finds nothing, so the
defect predates this work; it is a catalogue gap of the same kind as the two
above and is left for a separate change rather than folded into a dependency
migration.

Lessons. First, a sampled assertion is the wrong instrument for proving that a
generator reaches a class: the first V1 non-vacuity check would have failed
about 3% of runs, and replacing it with an eight-subset enumeration made
coverage structural and let the negative control (inverting the push order)
fail on the exact subset that distinguishes "highest layer wins" from
"whichever layer was present won". Second, a plan is a hypothesis and its prose
goes stale the moment reconnaissance resolves a risk: three claims in "Purpose
/ big picture" and two acceptance criteria described behaviour the migration
does not produce, and reconciling them was part of the work rather than a
formality. Third, a process-level preservation claim needs the *old* binary as
its evidence. Running the eleven boolean spellings against a build of the
pre-migration merge base, and diffing normalized stderr, is what turned "these
exit codes look right" into a measured before/after. Fourth, an axiom that says
"verified by running X" is not verified until X has actually run: this plan
carried that sentence for hours while no invocation of the real tool existed,
and the cost of closing the gap was one `cargo binstall` and two script runs.
Run the command the plan names before writing the sentence that cites it.

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
guide §5 boolean `=BOOL`       -> EP-M4 -> preserved: `--flag=false` still exits 2, `--flag false` still exits 2
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
- Artefact: `tests/cli_tests/merge/explicit_cli_wins.rs`, plus the
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
  booleans are handwritten clap flags on `src/cli/command.rs::Cli`, not
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
set as 0.9.1. Discharged on 2026-10-25 by running the real tool on the migrated
tree:

- Unix. Installed 0.10.0 into a private root with
  `cargo binstall --no-confirm --disable-strategies compile --install-path
  /home/leynos/.cache/orthohelp-951 cargo-orthohelp@0.10.0`,
  put that root first on `PATH`, and ran
  `scripts/generate-release-help.sh x86_64-unknown-linux-gnu netsuke
  /tmp/orthohelp-out-951 Netsuke`.
  Exit 0; the script's own `require_file` checks passed and it reported
  `man/man1/netsuke.1` at 8842 bytes.
- Windows. Ran the same binary directly with the script's PowerShell arguments
  (`--format ps --locale en-US --ps-module-name Netsuke --ps-split-subcommands
  true --ensure-en-us true`).
  Exit 0; all four artefacts the script requires were produced: `Netsuke.psm1`,
  `Netsuke.psd1`, `en-US/Netsuke-help.xml`, and `en-US/about_Netsuke.help.txt`.
- File set unchanged. The Unix run produced exactly `man/man1/netsuke.1`, the
  set the 0.9.1 lane produced.
- Content unchanged where it matters. Running the locally installed 0.9.0
  binary (`cargo orthohelp --format man ...`, which is how that release spells
  the invocation) over the same migrated tree produced a man page with the
  identical SHA-256,
  `cc81d1ed935f417b171721740f6b7a970ef2268e502bc77d0028c89ed1af661a`. The tree
  emitting IR 2.0 therefore renders identically through both tool versions,
  which is the strongest available statement short of checking out the
  pre-migration tree, and it simultaneously demonstrates that the 0.10.0 reader
  accepts the migrated metadata.
- Non-vacuity: the option block does render, so "no `BOOL` in the page" is a
  real observation rather than an empty section. `--verbose`, `--json`,
  `--fetch-default-deny` and `--trust-project-fetch-policy` appear as bare
  switches (`\fB\-\-verbose\fR, \fB\-v\fR`), while `--file`, `--jobs`,
  `--directory` and `--config` still carry value names (`\fIUTF8PATHBUF\fR`,
  `\fIINT\fR`, `\fIDIR\fR`, `\fIFILE\fR`). That is exactly the split
  `presence_only_cli_metadata` exists to produce, and it is the contrast that
  makes the absent `=BOOL` suffix meaningful. The PowerShell MAML contains no
  `BOOL` either.
- Residual gap, pre-existing and unchanged: the SYNOPSIS section renders
  `[missing: netsuke-usage]` because `synopsis_id` is `None` for the root and
  for every subcommand, and `netsuke-usage` is absent from every locale
  catalogue. `git grep netsuke-usage 88f20d588` finds nothing, so this predates
  the migration and is recorded in `Outcomes & retrospective` rather than fixed
  here.

The axiom is the tool's documented interface, not its internal correctness:
that 0.10.0 emits a *correct* man page is not claimed, only that it accepts the
IR 2.0 metadata, produces the same file set, and renders the same bytes for
this tree as the previous reader did.

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
  `ortho_config v0.10.0`; the four tool pin sites in
  `.github/workflows/build-and-package.yml` all read `0.10.0` — the two
  `netsuke-orthohelp-v2-…-0.10.0-…` cache keys, `INPUT_VERSION: '0.10.0'`, and
  the version-validation step. Do not test this with
  `grep -c '0\.9\.' .github/workflows/build-and-package.yml`: the file keeps
  three deliberate `0.9.1` mentions in its own explanatory comments, so that
  grep returns 3 whether or not every pin has moved, and it would have reported
  a failure on a correctly migrated workflow. Name the four sites instead.
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
4. `cargo nextest run -E 'test(explicit_cli_value_equal_to_the_default_still_wins)'`
   passes, and `cargo nextest run -E 'binary(cli_tests)'` passes, so an explicit
   `--manifest-fuel` equal to the compiled default beats a file layer saying 7
   and an environment layer saying 9, and every layer subset resolves to its
   highest present layer.
5. `cargo nextest run -E 'binary(boolean_flag_spelling_tests)'` passes all
   eleven cases, which pin the measured matrix: a bare switch exits 0 and takes
   effect; each of six `--flag=false` forms exits 2 with empty stdout and clap's
   `unexpected value 'false'`; `--verbose false` and `graph --html false` exit
   2 with empty stdout and `Unknown subcommand` / `Unknown argument`; and
   `--json false graph` exits 1 with empty stdout and a JSON diagnostics
   document. Every status code and stream was measured identical against a
   binary built from the pre-migration merge base.
6. `bash scripts/generate-release-help.sh <target> netsuke <tmp>/out Netsuke`
   produces the same file set as before the migration. Measured on 2026-10-25
   with `cargo-orthohelp` 0.10.0: the Unix run exits 0 and writes exactly
   `man/man1/netsuke.1`; the PowerShell artefact set is the same four files;
   and the man page is byte-identical to the one the 0.9.0 reader renders from
   the same tree. Axiom A2 records the hashes and the non-vacuity check that
   the boolean flags render as bare switches while value-taking flags keep
   their value names.

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
  handwritten clap flags); V2 annotated as preservation for the same reason; the
  `FluentValue` risk closed; four findings added to `Surprises & discoveries`;
  EP-M3 redefined as identifier *stability*; four new entries in the Decision
  log; Stage B/C split clarified.
- 2026-10-25: third revision, after the nine-gate batch returned seven green
  and two red. Replaced the "deliberately corrected places" framing in
  `Purpose / big picture` with the measured result: both v0.10.0 behavioural
  corrections are inapplicable to Netsuke, and the migration is a preservation
  exercise pinned by tests. Rewrote Constraints bullet 1 and acceptance items 4
  and 5 the same way, quoting the measured exit-status matrix rather than the
  guide's prediction. Wrote `Outcomes & retrospective` and recorded the three
  process lessons, including the sampled non-vacuity test that would have
  failed about 3% of runs. Recorded the two residual catalogue gaps as out of
  migration scope. Fixed the `shadow_reuse` Clippy error and the
  `-ize`/`handwritten` spellings that made the two red gates fail.
- 2026-10-25: fourth revision. Split
  `src/cli/release_help/parser_fields.rs` out of the parent module to clear
  Whitaker's 400-line cap, which the IR-2.0 commit had breached at 423 lines.
  Discharged Axiom A2 by running `cargo-orthohelp` 0.10.0 for real on both
  lanes, replacing the plan's previously unearned "Verified by running"
  sentence with the measured file sets, the shared SHA-256, and the non-vacuity
  contrast between boolean and value-taking flags. Recorded the pre-existing
  `[missing: netsuke-usage]` SYNOPSIS defect as a fourth residual catalogue gap
  and added the matching lesson: an axiom citing a command is not verified
  until that command has run. Ticked the stale authoring checkbox and added the
  `### Changed` changelog entry.

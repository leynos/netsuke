# Add optional target descriptions and `netsuke help targets`

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

## Purpose / big picture

Netsuke currently supports descriptions only on reusable rules. Those
descriptions feed Ninja progress output, but targets and actions have no
discovery metadata. This plan adds an optional `description` field to targets
(and, by inheritance, actions) and exposes the rendered target and action
catalogue through a new `netsuke help targets` subcommand. The command loads,
expands, renders, and validates the selected manifest without invoking Ninja,
then prints the available targets and actions with their descriptions.

The discovery query uses a restricted, side-effect-free Jinja surface. It
allowlists only the lexical path filters `basename`, `dirname`, `with_suffix`,
and `relative_to`, the collection filters `uniq`, `flatten`, and `group_by`,
and the clock-independent `timedelta` function. It rejects `env()` and `glob()`,
file tests, filesystem metadata filters such as `size` and `linecount`, `hash`,
`digest`, `contents`, `realpath`, and `expanduser`, executable discovery through
`which` and `command_available`, network and command helpers (`fetch`, `shell`,
and `grep`), and the clock-dependent `now()` function. This keeps manifest
inspection from disclosing host state, reading file contents, fetching data,
executing commands, or writing caches. Normal build manifest rendering retains
the full standard library; the restriction applies only to query rendering.

A user can verify the change by writing a manifest with an action and a target
that carry `description`, then running `netsuke help targets` and observing the
two catalogue sections with aligned name/description columns and a localized
default marker such as `[★ default]` on manifest defaults.

## Constraints

- Traditional AST/render/expansion open-source repo layering must be respected:
  `src/ast.rs`, `src/manifest/render.rs`, `src/manifest/expand.rs`.
- `description` must be optional on every target and action; duplicate or
  unknown fields must remain validation errors.
- A target description is discovery metadata and must not silently replace a
  referenced rule description used for Ninja progress.
- Existing manifests must remain valid and retain their current execution
  output.
- The help command performs no recipes and creates no build outputs.
- The build-time `build_l10n_audit` must pass: every key declared in
  `src/localization/keys.rs` must exist in every `locales/*/messages.ftl` with
  matching interpolation variables.
- No file may exceed 400 lines (AGENTS.md), and every module must begin with a
  `//!` comment.
- en-GB-oxendict spelling and grammar in comments and docs.
- Polonius borrow-checker rules must be respected; never rewrite tagged sites.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than ~40 files or a
  substantial new dependency, stop and escalate.
- Interface: if a public API signature must change beyond the planned
  `Target::description` field and the new `Commands::Help` variant, stop and
  escalate.
- Dependencies: if a new external dependency is required, stop and escalate.
- Iterations: if a gate still fails after 3 attempts without a fix, stop and
  escalate.
- Ambiguity: if a design choice materially affects the outcome, stop and
  present options.

## Risks

- Risk: clap's implicit `help` subcommand collides with the new `Commands::Help`
  variant. Severity: high Likelihood: high Mitigation: call
  `.disable_help_subcommand(true)` in the command-building path; verify
  `netsuke help` still matches `--help` via `tests/novice_flow_smoke_tests.rs`.
- Risk: the l10n audit rejects the build when only some locales receive the new
  keys. Severity: high Likelihood: high Mitigation: add all six new keys to
  every one of the 35 `locales/*/messages.ftl` files in the same commit as
  `keys.rs`.
- Risk: snapshot tests for CLI help (`help_en_us`, `help_es_es`) change because
  the `help` subcommand now carries a custom about line. Severity: medium
  Likelihood: high Mitigation: regenerate and accept the snapshots as part of
  Phase 2/3.
- Risk: the `main` entry point and config merge treat `Commands::Help` like a
  build command. Severity: medium Likelihood: low Mitigation: `resolve_command`
  already clones unknown variants; verify with the full test suite.

## Progress

- [x] (2026-08-09) Reconnaissance: read AST, render, expand, CLI parser,
      cli_l10n, runner dispatch/graph, status pipeline, l10n keys/audit,
      result_json, output_prefs, BDD infrastructure.
- [x] (2026-08-09) Phase 1: `Target::description` through AST, render, and
      expansion; parser/actions/render/expand tests pass.
- [x] (2026-08-09) Phase 2: `Commands::Help`/`HelpTopic`, `help.rs` handler,
      text/JSON renderers, l10n keys in all 35 locales, dispatch wiring.
- [x] (2026-08-09) Phase 3: help_tests snapshots (text/accessible/es-ES/JSON),
      runner_help_targets_tests, BDD CLI+full-process scenarios, regenerated
      help_en_us/help_es_es snapshots.
- [x] (2026-08-09) Phase 4: users-guide updated (schema field, distinction
      from rule descriptions, subcommand list, worked example + tested-example
      and its test); man page and PowerShell help pick up `help` automatically.
- [x] (2026-08-09) All gates green: check-fmt, lint (rustdoc/clippy/Whitaker),
      nextest (1936), doctests, markdownlint, spelling, nixie. Committed as
      four atomic commits.
- [x] (2026-08-09) CodeRabbit `--agent` review: 0 findings.
- [x] (2026-08-09) Branch renamed to
      `issue-551-add-target-descriptions-and-netsuke-help-targets`, pushed,
      PR opened: <https://github.com/leynos/netsuke/pull/555>.
- [x] (2026-08-12, `d524941`) Documented the restricted, side-effect-free Jinja
      surface for `netsuke help targets` in the migration, users', developers',
      and CLI design guides.
- [x] (2026-08-12, `e5edb0d`) Routed target help through a restricted manifest
      query path, escaped terminal control characters in text output, and added
      end-to-end, IR, and property coverage for the query and catalogue
      invariants.
- [x] (2026-08-12, `e9efae6`) Clarified that target and action descriptions
      remain discovery metadata and do not replace rule descriptions in Ninja
      progress; added `cli.help.targets.about` to all 35 shipped locales.
- [x] (2026-08-12, `625e93f`) Used a dedicated localized synopsis for the nested
      `targets` help topic and aligned the localized help assertions with it.
- [x] (2026-08-14) Documented the complete query-mode allowlist, its excluded
      host-observing helpers, and the full standard library retained by normal
      manifest rendering.

## Surprises & discoveries

- Observation: clap's implicit `help` pseudo-subcommand already appears in the
  CLI help snapshots as
  `help  Print this message or the help of the given subcommand(s)`, so the
  snapshot change is contained to the description line. Evidence:
  `src/snapshots/cli/netsuke__cli__parser__tests__help_en_us.snap`. Impact:
  Phase 2 must regenerate these snapshots.
- Observation: the l10n audit compares interpolation variables against the
  English source, so the new keys must introduce no `$` variables to keep all
  locale translations simple. Evidence: `build_l10n_audit/compare.rs`. Impact:
  keep all six new keys free of Fluent variables.
- Observation: `test_support::localizer::locale_localizer` does not affect the
  library's own global `LOCALIZER` static inside unit-test binaries (the crate
  is compiled twice). Unit tests must set the localizer directly via
  `crate::localization::set_localizer_for_tests`. Evidence: the localized
  snapshot stayed English until the unit test installed the localizer through
  the library's own API. Impact: unit snapshot tests use the library-local
  localizer installer.
- Observation [type:docstyle]: The
  `cli_localization::tracing_tests::a_resolved_locale_reports_requested_and_effective_tags`
  test is a PRE-EXISTING flake on the base commit (reproduced with `git stash`
  on 487f77e, ~2/3 failure rate). Root cause: `tracing` caches callsite interest
  from the first subscriber to register it; the `Dispatch::none()` default in
  the test binary returns `Interest::never()`, poisoning the callsite for the
  process when a no-op thread touches it first. A global TRACE-hinted
  subscriber was tried but did not fully fix it and added risk, so the change
  was reverted. Impact: gates may intermittently fail on this test; re-run the
  suite when it hits (it passes in isolation and with `--test-threads=1`).
  Fixing the infrastructure properly is a separate concern from issue #551.
- Observation: the post-`314f12b` query path uses a dedicated localized
  synopsis for the nested `targets` help topic rather than the catalogue's
  section heading. Evidence: `625e93f`. Impact: keep the
  `cli.help.targets.about` key separate from `actions_heading` and
  `targets_heading`.

## Decision log

- Decision: follow the issue's supplied coding plan exactly, phase by phase.
  Rationale: the plan has already been reviewed and accepted as requirements.
  Date/Author: 2026-08-09 / Claude.
- Decision: create the execplan under `docs/execplans/fef13161.md` (derived
  from the current branch name as instructed). Rationale: AGENTS.md names the
  plan file from the current branch. Date/Author: 2026-08-09 / Claude.

## Outcomes & retrospective

The issue's acceptance criteria are met: the AST, rendered manifest, and
catalogue carry target/action descriptions; parser, validation, render, and
expansion coverage exists; `netsuke help targets` is snapshot-tested in text,
accessible, localized, and JSON modes; alternate manifest selection is tested;
the users guide documents the schema field and the subcommand; and the man
page plus PowerShell help pick up the new command surface automatically
through clap derivation (no shell completions exist to update).

The post-`314f12b` follow-up additionally isolates discovery rendering from
impure template helpers, keeps terminal text safe, preserves rule descriptions
as the source of Ninja progress text, and supplies the nested help synopsis in
all 35 shipped locales. These outcomes are recorded from commits `d524941`,
`e5edb0d`, `e9efae6`, and `625e93f`; the current history does not record a new
full-gate run after those commits, so no additional gate result is claimed
here.

Lessons learned:

- The `tracing` callsite interest cache makes capture-based tracing tests
  flaky under parallel execution; this is a pre-existing issue on the base
  commit and was left untouched (see Surprises & Discoveries).
- `test_support` localizer helpers target a separate crate instance in
  unit-test binaries; unit tests must install the library's own localizer.
- `make fmt` runs `mdformat-all`, which reformats files outside the
  `check-fmt` gate; those changes were reverted to keep the PR focused.

## Context and orientation

This repository is a Rust CLI (`netsuke`) that parses YAML+Jinja manifests and
generates Ninja build files. Key files and modules for this task:

- `src/ast.rs` — `NetsukeManifest`, `Target`, `Rule`, `Recipe`. `Target` has
  `deny_unknown_fields`; actions are `Vec<Target>` deserialized by
  `deserialize_actions`, which forces `phony = true`.
- `src/manifest/mod.rs` — `from_str_named` pipeline: YAML parse, vars
  registration, `expand_foreach`, serde deserialize, `render_manifest`.
- `src/manifest/render.rs` — `render_manifest`, `render_rule`, `render_target`;
  `render_str_with` renders Jinja in a string against a context.
- `src/manifest/expand.rs` — `expand_foreach`; `foreach`/`when` clone the whole
  entry map, so a `description` key flows through unmodified.
- `src/cli/parser.rs` — `Cli`, `Commands` enum, `parse_with_localizer_from`.
- `src/cli_l10n.rs` — `localize_command`, `Subcommand` enum, key helpers.
- `src/runner/dispatch.rs` — `execute` matches `Commands` variants.
- `src/runner/graph.rs` — pattern for an in-process handler using
  `load_manifest_with_stage_reporting` and `BuildGraph::from_manifest`.
- `src/status.rs` / `src/status_pipeline.rs` — `PipelineStage`,
  `StatusReporter`,
  `report_pipeline_stage`.
- `src/localization/keys.rs` — Fluent key registry (`define_keys!`). The
  build-time audit requires every key in every locale.
- `src/result_json.rs` / `src/json_envelope.rs` — versioned JSON document
  envelope.
- `src/output_prefs.rs` / `src/theme.rs` — theme/accessibility resolution.
- `tests/novice_flow_smoke_tests.rs` — `netsuke help` must match `--help`.
- `tests/runner_graph_tests.rs` — model for the new integration test module.
- `tests/features/cli.feature` + `tests/bdd/steps/cli.rs` — BDD CLI parsing.
- `tests/bdd/steps/manifest_command.rs` + helpers — full-process subcommand
  BDD pattern.

## Plan of work

- Phase 1: add `pub description: Option<String>` to `Target` in `src/ast.rs`
  with `#[serde(default)]` and a Rustdoc comment mirroring `Rule::description`;
  render it in `render_target` through `render_str_with` exactly like
  `render_rule`; leave `expand.rs` untouched because `foreach`/`when` clone the
  entry map. Extend `tests/ast_tests/parsing.rs` (present, absent,
  duplicate/unknown rejection), `tests/ast_tests/actions.rs` (action carries
  description, stays phony), add a render test proving Jinja resolution against
  `vars`, and add `src/manifest/expand_test_cases/` cases proving a description
  survives `foreach` and is dropped by `when` filtering.
- Phase 2: add `Commands::Help(HelpArgs)` with `topic: Option<HelpTopic>` and
  `HelpTopic::Targets`; call `.disable_help_subcommand(true)` in
  `localize_command`/`parse_with_localizer_from`; add a dispatch arm routing
  `HelpTopic::Targets` to a new `help::handle_help_targets`; for the no-topic
  case rebuild the localized command and render long help; accept existing
  subcommand names as topics. Create `src/runner/help.rs` following the
  `graph.rs` pattern. Add a deterministic listing model and text/JSON
  renderers. Add l10n keys and all locale translations.
- Phase 3: snapshot tests under `src/runner/help_tests.rs`, integration tests
  in `tests/runner_help_targets_tests.rs`, BDD scenarios in
  `tests/features/cli.feature` + `tests/bdd/steps/cli.rs`, and a full-process
  BDD scenario. Regenerate `help_en_us`/`help_es_es` snapshots.
- Phase 4: document the field and the subcommand in `docs/users-guide.md`;
  confirm man page and PowerShell help pick up the command automatically.

## Validation and acceptance

- Run `make check-fmt`, `make lint`, and `make test` (via `scrutineer`) before
  each commit.
- `cargo nextest run` focused tests: `tests/ast_tests`, `expand_tests`,
  `runner_help*`, `novice_flow_smoke_tests`, `man_page_contract_tests`,
  `release_help_script_tests`.
- `netsuke help targets` on a fixture with actions, targets, defaults, and a
  missing description prints both sections with a localized marker such as
  `[★ default]` (or `[* default]` in accessible output) and an empty
  description column for the missing case.
- `netsuke --json help targets` emits a JSON envelope with
  `command: "help-targets"`.
- `netsuke help` and `netsuke --help` both succeed and print the long help.

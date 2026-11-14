# 31.2.1 Move rule summary, rationale, and remediation into the localization catalogues

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT

The plan awaits user approval. Decisions `D1` to `D5` in the `Decision log`
change scope or a build-time contract and are proposed for approval with the
plan rather than settled by it.

Branch: `31-2-1-move-linter-messages-into-l10n-catalogues`

Roadmap item: `31.2.1` in `docs/roadmap.md`, under step 31.2 ("Localize lint
output") of phase 31 ("Manifest linting").

## Purpose / big picture

Netsuke is a build tool that compiles YAML and Jinja manifests into Ninja build
files. Its `netsuke check` subcommand, compiled in when the Cargo feature
`lint` is enabled, runs a manifest linter: 24 rules, each with a stable
kebab-case name such as `manual-ninja-escape`. A rule reports *findings*. Each
finding carries a per-finding *message* ("action `build` escapes a dollar for
Ninja, which Netsuke now does itself") and the rule's *remediation* ("Write the
shell variable normally, for example `$PATH` rather than `$$PATH`."), which the
human output prints as `help:` and the JSON output carries as `help`. Each rule
also owns a one-line *summary* and a *rationale*, printed by
`netsuke check --explain`.

Today all of that prose is English, hard-coded in Rust. The rest of the
command-line interface (CLI) is localized into 35 locales through Fluent
catalogues (`locales/<tag>/messages.ftl`), selected with `--locale <tag>`,
`NETSUKE_LOCALE`, a configuration file, or the system locale. A German user
running `netsuke --locale de check` therefore reads German framing text ("Lint
results — …") around English findings.

After this change:

```plaintext
$ netsuke --locale de check
  ⚠ netsuke::lint::manual_ninja_escape
  │ Aktion `build` maskiert ein Dollarzeichen für Ninja, was Netsuke inzwischen selbst übernimmt
  …
  help: Schreiben Sie die Shell-Variable normal, zum Beispiel `$PATH` statt `$$PATH`.
```

(The German wording is illustrative; the delivered text follows the
localization glossary and style guide.) The finding's message, its remediation,
and the rule's summary and rationale in `--explain` text output come from the
catalogue for the selected locale. Every machine-matched value — the diagnostic
`code`, the `severity`, the rule name used as the span label, the documentation
`url`, the spans, the finding order, and the whole `--explain --json` catalogue
— is byte-identical across locales. A locale whose catalogue has not yet
translated a rule falls back to the English source catalogue, and if even that
lacks the text, to the English text compiled into the rule registry, so a
diagnostic is never empty and never shows a raw message key.

Deliberately changed behaviour: interpolated values in finding messages (for
example the target name `build` above) are now wrapped in Unicode bidirectional
isolation marks (U+2068 FIRST STRONG ISOLATE and U+2069 POP DIRECTIONAL
ISOLATE), as every other localized CLI message with arguments already is. This
applies in every locale, including `en-US`, and in both human and JSON output.
See `D4`.

This work closes the provisional decision recorded in
`docs/adrs/adr-0056-manifest-linting-under-netsuke-check.md` ("Rule text is not
localized yet") and the matching paragraph in section 12 of
`docs/netsuke-linter-design.md`. It does not localize the `--explain` framing
labels (roadmap task 31.2.2), does not rebind the rule reference contract test
to the catalogue (31.2.3), and translates only one pilot locale (31.2.4 owns
the rest; see `D3`).

The user has waived this task's roadmap prerequisite (31.1.3, "Apply the
dispositions") so the linter is not published untranslated. Consequence: the
prose localized here may still change under step 31.1, and each such change
must update the English source catalogue in the same commit (enforced by
`INV-source-parity`).

## Constraints

- Rule identifiers never become localizable. A rule's `name`, its `category`
  spelling, its `stage` spelling, its severity spelling, its diagnostic `code`
  (`netsuke::lint::<snake_name>`), and its documentation `url` must be computed
  without consulting a localizer. (Source: roadmap step 31.2;
  `docs/translators-guide.md` section on untranslated identifiers; ADR-0056.)
- `netsuke check --explain --json` stays in the source locale and its bytes
  must not change in this task. (ADR-0056; roadmap 31.2.2.)
- No JSON field is added, renamed, or removed in the check result document,
  the threshold diagnostic document, or the `--explain` catalogue document.
  Only the *values* of `message` and `help` in finding entries become
  localized. The schema freeze is roadmap task 31.4.1.
- The lint core (`src/lint/**`) stays framework-free: it must not name
  `ortho_config`, `fluent*`, `miette`, or `crate::localization`. Localization
  happens at the runner boundary (`src/runner/check/**`). (Roadmap 31.5.1;
  `docs/netsuke-linter-design.md` section 3.2.)
- Finding order, and therefore which findings `--limit` keeps, must not depend
  on the locale.
- The registry's English text (`RuleMeta::summary`, `rationale`,
  `remediation`) is retained as the last-resort fallback. (Roadmap 31.2.1.)
- Fluent keys and their catalogue entries stay ungated by the `lint` feature,
  because the build-time audit reads them regardless of features
  (`docs/developers-guide.md`, "Fluent keys stay ungated").
- No in-process environment mutation in tests and no new process-wide test
  locks (`AGENTS.md`, Testing). New tests inject a `&dyn Localizer` rather than
  installing the global one; the existing `localizer_test_lock` in
  `src/runner/check/tests.rs` is not extended to new tests.
- Files stay under 400 lines (`AGENTS.md`; Whitaker module cap).
  `src/localization/keys.rs` is at 351 lines and must not grow; new keys go in
  a new table file (precedent: ADR-0069, "Split the localization key tables").
- Polonius posture: the pinned nightly enables Polonius alpha; do not add
  defensive clones or double lookups (`AGENTS.md`, Borrow checker).
- No new dependencies. `ortho_config` 0.10.0 (re-exporting `ortho_l10n`
  0.10.0 and `fluent-bundle` 0.16) already provides `Localizer`,
  `LocalizationArgs`, `FluentValue`, and `FluentLocalizer`; `rstest`,
  `rstest-bdd`, `insta`, `proptest`, `googletest`, `pretty_assertions`, and
  `mockall` are already development dependencies (`Cargo.toml` lines 180-199).
- Completed ExecPlans under `docs/execplans/` are historical and must not be
  edited.
- Implementation begins only after the user approves this plan.

## Tolerances (exception triggers)

Comparison base: the merge base with `origin/main` at implementation start
(reconnaissance base `5c707ef33`). Count with:

```sh
git diff --numstat "$(git merge-base origin/main HEAD)" -- . \
  ':(exclude)docs/execplans/31-2-1-move-linter-messages-into-l10n-catalogues.md'
```

Categories (whole-file rule):

- Code: `src/**`, `tests/**`, `build.rs`, `build_l10n_audit/**`,
  `test_support/**`, and insta snapshot files.
- Configuration: `locales/**/*.ftl`, `Cargo.toml`, `.config/**`,
  `.github/**`.
- Documentation: `docs/**` (excluding this ExecPlan), `README*.md`,
  `CHANGELOG.md`.

Limits, measured as net lines (additions minus deletions) per category and
enforced independently:

- Files: more than 55 non-ExecPlan files changed → stop and escalate.
- Code: more than 2,000 net lines → stop and escalate. Estimate 1,600: finding
  text type and adapter rendering (~300), 24 rule call-site conversions and
  label helpers (~200), lint key table (~130), audit optional tier (~60), tests
  including parity, coverage, cross-locale sweep, proptest, audit cases, and
  BDD steps (~900).
- Configuration: more than 700 net lines → stop and escalate. Estimate 520:
  the `en-US` catalogue (~260 lines for 72 prose keys, ~30 message forms, nine
  advice phrases, and subject phrases) and the pilot `de` catalogue (~260).
- Documentation: more than 500 net lines → stop and escalate. Estimate 380:
  new ADR (~150), linter design (~60), translators' guide (~60), developers'
  guide (~50), users' guide (~40), changelog and roadmap (~10), one reworded
  rationale in `docs/netsuke-linter-rules.md`.
- Interface: any change to the JSON field set, to `RuleMeta`'s identifier
  fields, or to a public item outside `crate::lint::finding`,
  `crate::runner::check`, `crate::localization`, and `build_l10n_audit` → stop
  and escalate.
- Dependencies: any new crate → stop and escalate.
- Architecture: any need for `src/lint/**` to see a localizer, or any change
  to the audit beyond the optional tier and normalization check in `EP-M1` →
  stop and escalate.
- Iterations: a focused test still failing after three fix attempts, or a
  full gate failing twice for the same cause → stop and escalate.
- Time: a milestone exceeding six hours of wall-clock work, excluding gate
  queueing → stop and escalate.
- Ambiguity: a rule message whose English cannot be expressed as one Fluent
  pattern with the finding's arguments without changing its English wording →
  record, then escalate if more than three rules are affected.

Budget evidence: the comparable localization plans set looser or similar
budgets and recorded no increases.
`docs/execplans/3-7-1-externalize-user-facing-strings.md` allowed 30 files or
1,500 net lines (no revision raised it); `3-7-2-locale-resolution.md` and
`3-7-3-translator-tooling-and-documentation.md` each allowed 10 to 12 files and
600 lines with no recorded increase. Neither plan converted per-call-site
message construction across a rule set, which is why this plan's code budget is
larger. No ExecPlan has previously touched `src/lint`, so there is no
lint-specific budget history.

## Risks

- Risk: Fluent's isolation marks change the bytes of `en-US` finding messages,
  breaking string-matching tests and any consumer grepping `message`. Severity:
  medium. Likelihood: high (certain for interpolated messages). Mitigation:
  `D4` accepts the change, consistent with existing localized messages; tests
  normalize with `test_support::fluent::normalize_fluent_isolates`; users'
  guide and changelog state it; `message` is prose, not a contract field.
- Risk: the registry English and the `en-US` catalogue drift, recreating the
  drift ADR-0056 warned about. Severity: high. Likelihood: medium. Mitigation:
  `INV-source-parity` tests bind them byte-for-byte for prose and by rendered
  equality for messages.
- Risk: two keys normalize to the same Fluent identifier (`ortho_l10n` rewrites
  `.` to `-` at load), so a locale lacking key A silently renders key B's text.
  Severity: high. Likelihood: low. Mitigation: `L-normalized-injective`,
  checked by a dedicated test over every declared key.
- Risk: nested noun phrases ("target `x`") cannot inflect for case or gender in
  languages such as German, producing awkward translations. Severity: low.
  Likelihood: medium. Mitigation: the pilot `de` translation exercises it
  early; translators may phrase around the subject (for example with a colon);
  the translators' guide documents it. Escalate only if a rule's message cannot
  be translated acceptably.
- Risk: the optional catalogue tier (`D2`) hides translation gaps.
  Severity: medium. Likelihood: medium. Mitigation: the tier is confined to
  `lint.*` keys declared in one file; 31.2.4 owns completing the translations
  and may tighten the tier; the translators' guide states the policy.
- Risk: the default-features CI lane (`make test-default-features`) runs BDD
  scenarios without `lint`. Severity: medium. Likelihood: high if scenarios
  live in `tests/features`. Mitigation: place them in `tests/features_lint/`
  behind `#[cfg(feature = "lint")]` (`D12`).
- Risk: changing the rule messages' construction alters an English message by
  accident. Severity: medium. Likelihood: medium. Mitigation: the existing
  per-rule message assertions (`crate::lint_messages!`) and the result-document
  snapshot pin the English; the snapshot may change only by isolation marks.

## Progress

- [x] (2026-11-15 00:40Z) Branch renamed; Wyvern reconnaissance of lint prose
  flow, localization infrastructure, and docs/tests completed.
- [x] (2026-11-15 01:30Z) Initial draft written.
- [ ] Community-of-experts design review applied.
- [ ] Plan approved by the user.
- [ ] `EP-M1` key table, audit optional tier, normalization check, and
  `en-US` prose entries.
- [ ] `EP-M2` rule prose localized at the runner boundary, with the pilot `de`
  prose.
- [ ] `EP-M3` finding messages localizable end to end, with the pilot `de`
  messages.
- [ ] `EP-M4` cross-locale contract sweep, BDD feature, and property test.
- [ ] `EP-M5` documentation, ADR, changelog, and roadmap closure.

## Surprises & discoveries

- Observation: the per-finding `message` is not one of the three registry
  prose fields; it is English built with `format!` inside each rule. Evidence:
  `Finding.message: String` at `src/lint/finding/mod.rs:30`; about 28 `format!`
  sites under `src/lint/rules/**/mod.rs` (for example
  `src/lint/rules/migration/mod.rs:64-68`). Impact: meeting the roadmap's
  success criterion ("reports a finding's message … in that locale") requires
  localizable message construction across all rules (`D1`).
- Observation: the build audit rejects any declared key missing from any of
  the 35 catalogues, so "a catalogue gap" cannot exist today, contradicting
  ADR-0056's stated migration shape ("translate incrementally"). Evidence:
  `build_l10n_audit/compare.rs:159-172`; ADR-0056 "Known risks". Impact: `D2`.
- Observation: `ortho_l10n` rewrites dots in catalogue identifiers to hyphens
  when loading (`normalize_resource_ids`), and lookup tries the dotted id then
  the normalized one. Evidence: `ortho_l10n-0.10.0/src/fluent.rs:99-114` and
  `src/lib.rs:236-276`. Impact: key uniqueness must hold after normalization
  (`L-normalized-injective`); a duplicate normalized id within one catalogue
  makes the whole catalogue fail registration.
- Observation: Fluent inserts isolation marks around every placeable.
  Evidence: existing tests normalize them (`tests/localization_tests.rs:100`).
  Impact: `D4`.
- Observation: the roadmap's success criterion names `rule` and `category`
  fields, but finding entries carry neither; the rule name appears as the span
  `label`, and `category` appears only in `--explain --json`. Evidence:
  `src/snapshots/check/netsuke__runner__check__tests__result_document.snap`.
  Impact: `INV-ident` is stated over the fields that exist; no field is added
  (schema freeze is 31.4.1).
- Observation: the only British-specific spelling in user-facing rule prose
  is one "behaviour" (a second occurrence is a module comment). Evidence: grep
  over `src/lint/rules`. Impact: `D5`.
- Observation: `src/runner/check/tests.rs` installs the global localizer under
  a process-wide `localizer_test_lock`. Impact: new tests inject the localizer
  instead (`D6`).

## Decision log

- D1. Decision (proposed; needs approval): localize the per-finding message as
  well as the three prose fields. Each rule builds its message as a catalogue
  key under its own name (`lint.rule.<name>.message` or
  `lint.rule.<name>.message.<variant>`), Fluent arguments, and the English
  rendering it builds today. Rationale: the roadmap's success criterion
  requires the message in the selected locale; localizing only the remediation
  would leave the headline line English. Rejected alternative: prose-only
  scope, amending the success criterion and adding a new roadmap task for
  messages — smaller (about 40% of the code estimate) but ships a half-English
  diagnostic, which the user's stated reason for waiving 31.1.3 argues against.
  Date/Author: 2026-11-15, planning agent.
- D2. Decision (proposed; needs approval): give the build audit an *optional
  key tier*. Keys declared in `src/localization/lint_keys.rs` must be present
  in the `en-US` source catalogue, may be absent from other catalogues, must
  not be orphaned anywhere, and when present must use the same Fluent variables
  as `en-US`. Rationale: this is the migration shape ADR-0056 describes
  ("translate incrementally", "a catalogue gap degrades to the source locale")
  and honours its driver that per-rule work must stay small; it makes the
  fallback chain a real, testable runtime path. Rejected alternatives: (a) seed
  all 34 non-source catalogues with English copies — no audit change, but about
  9,000 lines of placeholder text that the localization style guide calls a
  scaffold rather than a translation, and that 31.2.4 must then replace; (b)
  translate every locale now — delivers the user's goal fastest but absorbs
  roadmap task 31.2.4 wholesale. Date/Author: 2026-11-15, planning agent.
- D3. Decision (proposed; needs approval): translate all lint keys into one
  pilot locale, German (`de`), in this task. Rationale: the success criterion
  needs a real locale whose output differs from English for end-to-end
  evidence; German's case and gender stress the subject-phrase design (`D8`)
  early. Other locales fall back to `en-US` until 31.2.4. Date/Author:
  2026-11-15, planning agent.
- D4. Decision (proposed; needs approval): accept Fluent's isolation marks
  around interpolated values in finding messages in every locale, including
  `en-US`. Rationale: they keep right-to-left locales (`ar`, `fa`, `he`)
  legible and match every other localized CLI message; disabling them is not
  configurable per message in `ortho_l10n`. Rejected: rendering `en-US` from
  the in-code English to keep bytes unchanged — a source-locale special case
  that would make `en-US` the only locale not read from its catalogue.
  Date/Author: 2026-11-15, planning agent.
- D5. Decision (proposed; needs approval): write rule source prose in
  spelling-neutral English, rewording the one user-facing use of "behaviour"
  (`src/lint/rules/redundancy/duplication/mod.rs:99`; the module comment in
  `src/lint/rules/migration/mod.rs:1` is a code comment and stays en-GB), so
  `en-US` and `en-GB` can share the text and `en-GB` needs no lint entries.
  Rationale: avoids two English catalogues diverging for one word; the
  en-GB-oxendict rule in `AGENTS.md` governs comments and docs, not user-facing
  `en-US` strings. Date/Author: 2026-11-15, planning agent.
- D6. Decision: inject the localizer into the check adapters as
  `&dyn Localizer` parameters; `src/runner/check/mod.rs` obtains it once per
  run from `crate::localization::localizer()`, which `src/main.rs`
  (`configure_runtime`) has already installed from the merged `ortho_config`
  configuration. Rationale: keeps adapters pure and testable without the global
  lock; the global remains the single composition point.
- D7. Decision: the lint core keeps the English rendering of every finding
  message; ordering and the last fallback use it. Rationale: ordering stays
  locale-independent by construction (`INV-order`), and a message can never
  degrade to a key id. The duplication of each template (Rust English and
  `en-US` Fluent) is bound by `INV-source-parity`.
- D8. Decision: noun phrases naming the offending item ("target `x`",
  "action 3") and the `bashism` rule's per-construct advice become nested
  *phrases*: a finding argument may be a phrase (its own key, verbatim
  arguments, and English), rendered first and passed as text. Phrases cannot
  contain phrases, enforced by the type, so rendering is non-recursive.
  Rationale: the member words are English vocabulary inside today's messages.
  Rejected: Fluent select expressions over a `$member` string in every message
  — duplicates the member vocabulary into ~20 messages.
- D9. Decision: no Verus or Kani in this task; see
  `Formal verification decision`. Date/Author: 2026-11-15, planning agent.
- D10. Decision: record `D1`, `D2`, `D4`, `D7`, and `D8` in a new ADR that
  amends ADR-0056, following the amendment convention in
  `docs/documentation-style-guide.md` (an `Amended by ADR-nnnn (YYYY-MM-DD)`
  banner in ADR-0056's Status section and an `## Amendments` section;
  ADR-0056's accepted decision text is preserved). Provisional number 0082 (PR
  #808 holds 0075 to 0081); re-derive by sweeping every remote branch before
  minting.
- D11. Decision: `--explain` *text* output shows the localized summary,
  rationale, and remediation; its labels stay English until 31.2.2.
  `--explain --json` keeps reading the registry. Rationale: the prose has
  moved; the framing has not.
- D12. Decision: BDD scenarios for this feature live in
  `tests/features_lint/check_localization.feature`, collected by a
  `#[cfg(feature = "lint")]` `scenarios!` invocation in `tests/bdd_tests.rs`,
  mirroring the existing `tests/features_unix` pattern.
- D13. Decision: the detached-finding prefix (`"{symbol}: {message}"` in
  `Finding::display_message`) becomes a shared key `lint.finding.detached` with
  arguments `location` and `message`, so locales can adjust punctuation (for
  example French spacing before a colon).

## Outcomes & retrospective

Not started. To be completed at each milestone and at closure, including the
reconciliation of `D1` to `D13` with ADR-0056, the new ADR, and the linter
design document.

## Context and orientation

Inspected base revision: `5c707ef339254a60d58f191c563bfe8597be9deb` (`main`,
"Align AGENTS.md with the Rust project template (#1135)"). Line numbers below
refer to that revision; relocate symbols with `rg -n '<symbol>' src tests`.

### The lint core

- `src/lint/rule.rs`: `RuleMeta` (lines 99-133) holds `name`, `category`,
  `stage`, `default_severity`, `summary`, `rationale`, `remediation`, all
  `&'static str` or enums; `RuleMeta::code()` derives
  `netsuke::lint::<name with - replaced by _>`. Its doc comment already names
  31.2.1. `FindingSink` (around lines 210-275) binds a rule to its findings;
  `at`, `detached`, and `at_or_detached` take `impl Into<String>` messages.
- `src/lint/finding/mod.rs`:
  `Finding { meta, severity, message: String, location: Location }`;
  `sort_key()` orders by location, rule name, then message; `display_message()`
  prefixes detached findings with their symbol.
- `src/lint/rules/**`: 24 `static … : RuleMeta` definitions across 11 files
  (suppression 3, determinism 2, caching 2, migration 2, hygiene 3,
  clarity/descriptions 2, clarity/recipe_shape 2, graph 2, portability 1,
  redundancy/declarations 3, redundancy/duplication 2). Messages interpolate
  target, action, and rule names, paths, tool names, and counts. Shared label
  helpers: `Item::label()` in `src/lint/rules/recipes.rs:68-73` (member word
  plus name or 1-based index), `label()` in
  `src/lint/rules/caching/mod.rs:261`, similar helpers in `graph/mod.rs:66` and
  `redundancy/duplication/mod.rs:185`. `portability/mod.rs:43-97` has nine
  `CONSTRUCTS` entries with English `advice`.
- `src/lint/registry/mod.rs`: `all()`, `meta_by_name()`, `catalogue()`.
  `src/lint/registry/tests.rs` enforces kebab-case names and non-empty prose.
- `src/lint/test_support.rs`: fixture helpers and `crate::lint_messages!`,
  which assert English `display_message()` text per rule.

### The runner boundary (`src/runner/check/`)

- `mod.rs` (196 lines): orchestrates a run; `finish` (around lines 130-148)
  builds
  `RunnerError::LintThresholdExceeded { findings: report.diagnostics() }`.
- `diagnostics/mod.rs`: `CheckReport` and `FindingDiagnostic`, the `miette`
  projection. `FindingDiagnostic::new` sets `message` from `display_message()`,
  `help` from `meta.remediation`, `code`, `url`, and the span `label` (rule
  name).
- `json.rs` (184 lines): `render_result` (finding entries via
  `crate::diagnostic_json::diagnostic_entry`), `render_catalogue` and
  `RuleEntry::new` for `--explain --json`.
- `explain.rs`: text `--explain`, reading `summary`, `rationale`, and
  `remediation` directly (around lines 79-112).
- `text.rs`: human rendering; summary and truncation lines already localized
  via `CHECK_SUMMARY_*`.
- `documentation/mod.rs`: `rule_documentation_url`.
- Snapshots:
  `src/snapshots/check/netsuke__runner__check__tests__{result_document,truncated_result_document,rule_catalogue}.snap`.

### Localization

- `src/localization/mod.rs`: `define_keys!` (lines 13-22) generates
  `pub const NAME: &str = "dotted.id"` and a table slice; `ALL_KEYS` unions the
  tables (lines 45-78); the global localizer (`set_localizer`, `localizer()`);
  `LocalizedMessage` (lines 172-270), whose `render_with` passes the key itself
  as the fallback.
- Key tables: `keys.rs` (351 lines, re-exports the others), `check_keys.rs`,
  `clap_error_keys.rs`, `manifest_keys.rs`.
- `src/cli/localization/mod.rs`: `LayeredLocalizer` consults the requested
  catalogue, then the `en-US` catalogue; `build_localizer(Option<&str>)`.
  `ortho_config::Localizer::message(id, args, fallback)` returns `lookup` or
  `fallback`; `FluentLocalizer::lookup` returns `None` on a missing message or
  a formatting error (after reporting it), so the layer falls through.
- `src/locale/catalogues.rs`: 35 embedded catalogues (`include_str!`);
  `en-US` is the source locale.
- Locale resolution: `--locale`, `NETSUKE_LOCALE`, configuration files (via
  `ortho_config`'s merged `Cli.locale`), then the system locale
  (`src/locale/resolution.rs`; `src/main.rs` `configure_runtime`). This task
  adds no flag or configuration key.
- `build.rs` calls `build_l10n_audit::audit_localization_keys()` on every
  build. `build_l10n_audit/mod.rs` lists `KEY_SOURCES`;
  `compare.rs::audit_catalogue` reports `missing`, `orphaned`, and
  `variable_mismatches` per locale; duplicates across tables fail.
- Tests: `tests/build_l10n_audit_tests.rs`, `build_l10n_audit_rules_tests.rs`,
  `build_l10n_keys_tests.rs`, `build_l10n_parser_tests.rs`,
  `tests/locale_catalogue_tests.rs` (parse-all, render sweep, not-copies,
  identifiers preserved), `tests/localization_tests.rs`.

### Tests and docs bound to rule prose

- `tests/lint_rule_reference_tests.rs` binds `docs/netsuke-linter-rules.md`
  to the registry: summary cell verbatim, rationale and remediation verbatim in
  each section. It stays bound to the registry in this task (31.2.3 rebinds it);
  `INV-source-parity` makes the registry and `en-US` equal, so the reference
  transitively matches the catalogue.
- `tests/check_command_tests/{explanation,output,policy}.rs` (end-to-end via
  `assert_cmd`, `--locale en-US`); `src/runner/check/tests.rs` and
  `diagnostics/tests.rs`; `tests/documentation_examples_tests/lint_examples.rs`
  (each reference section's reported and fixed manifests).
- `tests/check_command_absent_tests.rs` covers builds without `lint`.
- Docs: `docs/netsuke-linter-design.md` (sections 5.2, 9, 11, 12),
  `docs/netsuke-linter-rules.md`, `docs/users-guide.md` ("Lint a manifest with
  netsuke check", around lines 1640-1738; "Choose a language with --locale",
  around 1348-1373), `docs/translators-guide.md` (section 4 key domains),
  `docs/localization-styleguide.md`, `docs/localization-glossary.md`,
  `docs/developers-guide.md` (lint feature gating, around lines 1488-1510).

### Tooling and gates

`make check-fmt`, `make lint`, `make doc-coverage`, `make test` (nextest with
`--all-features`, then doctests), `make test-default-features`,
`make markdownlint` (includes spelling), `make nixie`, and, because
`build_l10n_audit/` changes, `make test-package-verify`. Verus (`make verus`)
and Kani (`make kani-*`) exist but are not used here (`D9`).

### Signposts

Documentation: `docs/netsuke-design.md`, `docs/netsuke-linter-design.md`,
`docs/rust-testing-with-rstest-fixtures.md`, `docs/rust-doctest-dry-guide.md`,
`docs/reliable-testing-in-rust-via-dependency-injection.md`,
`docs/ortho-config-users-guide.md`, `docs/rstest-bdd-users-guide.md`,
`docs/translators-guide.md`, `docs/localization-styleguide.md`,
`docs/localization-glossary.md`, `docs/documentation-style-guide.md`. Skills:
`rust-router` (then `rust-types-and-apis` for the finding text type and
`rust-unit-testing` for rstest tables), `proptest`, `hexagonal-architecture`
(port and adapter placement), `execplans`, `en-gb-oxendict` for documentation.

## Conformance basis

Upstream artefacts, inspected at `5c707ef33`:

- `docs/roadmap.md`, task 31.2.1 and step 31.2 preamble; tasks 31.2.2 to
  31.2.4 and 31.4.1 as scope boundaries.
- `docs/adrs/adr-0056-manifest-linting-under-netsuke-check.md` (Accepted,
  2026-08-30): "Rule text is not localized yet; rule identifiers never will
  be", the migration paragraph, and "Known risks and limitations".
- `docs/adrs/adr-0069-split-the-localization-key-tables.md`: key-table
  splitting precedent.
- `docs/netsuke-linter-design.md` sections 3.2, 5.2, 5.4, 9.1, 9.2, 12.
- `docs/translators-guide.md`, `docs/localization-styleguide.md`,
  `docs/developers-guide.md` ("Fluent keys stay ungated").
- No Terms of Reference document exists for this phase.

Requirements:

- `REQ-keys`: rule prose is keyed `lint.rule.<name>.summary`, `.rationale`,
  `.remediation`, following the rule's stable name.
- `REQ-fallback`: a missing or unformattable translation degrades to the
  `en-US` catalogue, then to the registry English; never empty, never a key.
- `REQ-localized`: `netsuke --locale <tag> check` reports a finding's message
  and remediation in that locale.
- `REQ-ident`: `code`, `severity`, rule-name `label`, `url`, spans, and the
  `--explain --json` document are byte-identical across locales.
- `REQ-explain-json`: `--explain --json` stays in the source locale.
- `REQ-core-pure`: the lint core has no localization dependency.

Gaps:

- `GAP-message`: finding messages are English `String`s built in rules.
- `GAP-audit`: the audit forbids catalogue gaps.
- `GAP-normalized`: no check that declared keys stay distinct after
  `ortho_l10n` normalization.
- `GAP-adr`: ADR-0056 states localization is deferred.

Trace:

```plaintext
roadmap 31.2.1 -> REQ-keys -> EP-M1 -> tests::lint_rule_keys_are_declared
roadmap 31.2.1 -> REQ-fallback -> EP-M2 -> runner::check::prose::tests::fallback_chain
roadmap 31.2.1 -> REQ-localized -> EP-M2, EP-M3 -> tests/features_lint/check_localization.feature
roadmap 31.2.1 -> REQ-ident -> EP-M4 -> runner::check::locale_sweep::identifiers_are_locale_invariant
ADR-0056 -> REQ-explain-json -> EP-M2 -> rule_catalogue snapshot unchanged
roadmap 31.5.1 -> REQ-core-pure -> EP-M3 -> tests/lint_core_boundary_tests.rs
GAP-audit -> D2 -> EP-M1 -> build_l10n_audit optional-tier cases
GAP-normalized -> L-normalized-injective -> EP-M1 -> declared_keys_stay_distinct_after_normalization
GAP-adr -> D10 -> EP-M5 -> new ADR + ADR-0056 amendment banner
```

## Verification plan

### Abstract model

Let `R` be the finite set of registered rules, each with a name `n(r)` (a
lowercase kebab-case string, unique in `R`) and English prose `E(r, f)` for `f`
in `{summary, rationale, remediation}`. Let `K` be the set of declared message
keys (all tables), and `N(k)` the `ortho_l10n` normalization replacing every
`.` with `-`. A catalogue `C_t` for locale tag `t` is a partial map from
normalized keys to Fluent patterns; `C_src` is the `en-US` catalogue. Rendering
`render(C, k, args)` is `Some(text)` when `C` has `N(k)` and formats without
error, otherwise `None`.

A finding `x` carries `rule(x)`, `location(x)`, and a text
`T(x) = (key, args, english)`, where each argument is verbatim text, a count,
or a phrase `(key', verbatim args', english')`.

Resolution, for a locale `t` and a fallback string `e`:

```plaintext
resolve(t, k, args, e) = render(C_t, k, args)
                         orElse render(C_src, k, args)
                         orElse e
```

Prose: `prose_t(r, f) = resolve(t, key(r, f), {}, E(r, f))` with
`key(r, f) = "lint.rule." ++ n(r) ++ "." ++ f`. Message: phrases are resolved
first, then `message_t(x) = resolve(t, key(T(x)), args', english(T(x)))`.

Output projection: `ident(x)` is the tuple (code, severity, label, url, spans)
computed from `rule(x)`, `severity(x)`, and `location(x)` alone; `prose(x)` is
(message, help). Output order is `sort(findings, sort_key)` with `sort_key`
reading only `location`, `n(rule(x))`, and `english(T(x))`.

### Invariants and lemmas

- `INV-ident` (`REQ-ident`): for every finding set and every pair of tags
  `t, u`, the sequence of `ident(x)` in the output under `t` equals that under
  `u`; the `--explain --json` bytes are equal.
- `INV-order`: the output order and the `--limit` truncation set are equal
  under every tag.
- `INV-fallback` (`REQ-fallback`): `resolve` returns the first defined tier;
  its result is non-empty whenever `e` is non-empty, and never equals the key.
- `INV-source-parity`: for every `r` and `f`, `render(C_src, key(r, f), {})`
  equals `E(r, f)`; for every finding `x` produced by the test corpus,
  `strip_isolates(render(C_src, key(T(x)), args)) = english(T(x))`, and the
  same for each phrase.
- `INV-key-binding` (`REQ-keys`): the declared lint key set equals
  `{key(r, f)}` over `R` and the prose fields, together with the message,
  phrase, advice, and framing keys used by the rules; every message key used by
  rule `r` starts with `"lint.rule." ++ n(r) ++ "."`.
- `INV-audit-tier`: the audit fails when `C_src` lacks any declared key, when
  any catalogue contains an undeclared key, or when a present key's variables
  differ from `C_src`; it succeeds when a non-source catalogue lacks only
  optional-tier keys.
- `INV-arg-verbatim`: every verbatim argument value appears unmodified in the
  rendered output (between isolation marks) for every tag.
- `L-normalized-injective`: `N` is injective on `K`. Proof sketch: the audit
  requires `C_src` to contain every key in `K` and nothing else; if
  `N(k1) = N(k2)` for `k1 ≠ k2`, loading `C_src` would register two messages
  with the same identifier, which `fluent-bundle`'s `add_resource` rejects,
  making `C_src` fail to load; `tests/locale_catalogue_tests.rs` loads `C_src`
  successfully; hence no such pair exists. The dedicated test states the
  property directly so a failure names the colliding keys rather than a
  registration error. Design rule recorded for future keys: every lint key has
  a head word (`summary`, `rationale`, `remediation`, `message`, `advice`)
  immediately after the rule name, and no later segment of a key repeats a head
  word; together with unique dot-free rule names this rules out collisions such
  as `lint.rule.a.message.summary` against `lint.rule.a-message.summary`.
- `L-ident-independent`: `ident(x)` is computed without a localizer. Shown
  structurally (the projection functions take no localizer argument for those
  fields) and tested by `INV-ident`.
- `L-order-independent`: `sort_key` reads no localized value. Shown
  structurally by `REQ-core-pure` (the core cannot reach a localizer) and
  tested by `INV-order`.

### Axioms (trusted, not verified here)

- `AXIOM-fluent-lookup`: `ortho_l10n::FluentLocalizer::lookup` returns
  `Some` only for a present message that formats without error, tries the
  dotted then the normalized identifier, and inserts isolation marks around
  placeables (`ortho_l10n` 0.10.0, `src/lib.rs:236-276`). Exercised through the
  real catalogues in `EP-M2` and `EP-M4`.
- `AXIOM-fluent-duplicates`: `fluent-bundle` 0.16 `add_resource` reports an
  error for a duplicate message identifier. Used by `L-normalized-injective`;
  exercised by a negative control in `EP-M1`.
- `AXIOM-layering`: `crate::cli_localization::build_localizer(Some(t))` layers
  `C_t` over `C_src` (`src/cli/localization/mod.rs`). Repository-owned;
  exercised directly.
- `AXIOM-locale-plumbing`: `configure_runtime` installs the localizer for the
  merged locale before `runner::run`. Existing behaviour, exercised end to end
  by the BDD feature.

### Methods per obligation

- Obligation: `INV-audit-tier`. Method: `rstest` table over
  `audit_catalogue` inputs (source locale vs other; required vs optional key;
  present, absent, orphaned, variable mismatch). Rationale: a finite partition.
  Artefact: `build_l10n_audit` unit tests and
  `tests/build_l10n_audit_rules_tests.rs`. Evidence: red before the tier exists
  (the "non-source catalogue lacks an optional key" case fails), green after.
  Non-vacuity: includes the case that must still fail (`en-US` lacking an
  optional key) and an orphan in `de`.
- Obligation: `L-normalized-injective`. Method: exhaustive test over
  `ALL_KEYS`. Artefact: `src/localization/mod.rs` tests,
  `declared_keys_stay_distinct_after_normalization`. Evidence: passes on the
  real set. Non-vacuity: a negative-control case feeds a two-key set
  (`lint.rule.a.message.summary`, `lint.rule.a-message.summary`) to the same
  helper and expects it to report the pair; a second control loads a
  two-message resource through `FluentLocalizerBuilder` and expects a
  registration error (`AXIOM-fluent-duplicates`).
- Obligation: `INV-key-binding`. Method: exhaustive test over the registry
  and the declared table, plus an ownership check over every finding produced
  by the corpus. Artefact: `src/runner/check/prose/tests.rs` (or the lint
  test-support module), `lint_rule_keys_are_declared`. Non-vacuity: removing
  one declared key, or renaming one rule in a scratch copy, must fail it.
- Obligation: `INV-source-parity` (prose). Method: exhaustive over `R` ×
  three fields, reading `C_src` through a `FluentLocalizer` built from the
  embedded `en-US` text only. Artefact: `en_us_prose_matches_the_registry`.
  Non-vacuity: altering one character of one `en-US` entry fails it (seeded
  fault, reverted).
- Obligation: `INV-source-parity` (messages and phrases). Method: corpus
  test. The corpus is every reported manifest in `docs/netsuke-linter-rules.md`
  (one per rule, already extracted by
  `tests/documentation_examples_tests/lint_examples.rs`) plus the per-rule
  fixtures behind `crate::lint_messages!`. For every finding, render its key
  and arguments through `C_src` alone, strip isolation marks, and compare with
  the stored English. Coverage assertion: every declared `lint.rule.*.message*`,
  `lint.subject.*`, and `lint.rule.bashism.advice.*` key is exercised at least
  once, so a form no fixture reaches fails the test rather than passing
  vacuously. Non-vacuity: change one Rust English template without the
  catalogue (seeded fault) and observe failure.
- Obligation: `INV-fallback`. Method: `rstest` table over a stub
  `Localizer` (hand-written, recording the ids it was asked for) layered as the
  real `LayeredLocalizer` does: tier states {present, absent, format-error} for
  the requested and source tiers (nine cases), checking the returned text and
  that the key id is never returned. Plus two real-catalogue cases: `ja` (no
  lint entries) returns the `en-US` text; a key absent from every catalogue
  returns the registry text. Artefact: `src/runner/check/prose/tests.rs`.
  Non-vacuity: a mutant that passes the key as the fallback (today's
  `LocalizedMessage::render_with` behaviour) fails the "absent everywhere" case.
- Obligation: `INV-ident` and `INV-order`. Method: in-process exhaustive
  sweep over all 35 supported tags × the corpus manifests, rendering the JSON
  result document through `json::render_result` with an injected
  `build_localizer(Some(tag))`, projecting each finding to (code, severity,
  url, labels, primary span, related) and comparing with `en-US`; also
  comparing finding order and the `--explain --json` bytes. Artefact:
  `src/runner/check/locale_sweep_tests.rs` (name provisional). Rationale:
  finite domain, enumerated completely; cheaper and deterministic compared with
  840 subprocesses. Non-vacuity: assert that under `de` at least one `message`
  and one `help` differ from `en-US` (so the sweep compares localized output,
  not English twice); seeded fault routing `code` through the localizer must
  fail.
- Obligation: `INV-arg-verbatim`. Method: `proptest` over generated identifier
  strings (classes: plain ASCII, Fluent syntax characters `{ } $ [ ] * .`,
  backticks and quotes, non-ASCII including right-to-left text, leading and
  trailing whitespace; non-empty, length up to 40), rendered through a message
  with a verbatim argument under `en-US` and `de`. Oracle: the generated value
  appears as a contiguous substring after isolation marks are stripped,
  independent of the catalogue text. Artefact:
  `src/runner/check/l10n/proptests.rs` (name provisional). Classification:
  `prop_assert!` with `proptest` case counts per class recorded once in
  `Artefacts and notes`. Non-vacuity: a mutant that escapes braces in values
  fails.
- Obligation: `REQ-core-pure`. Method: boundary test scanning `src/lint/**`
  for `ortho_config`, `fluent`, `miette`, and `crate::localization`, reusing
  the approach of `tests/ninja_gen_hashmap_boundary.rs`. Artefact:
  `tests/lint_core_boundary_tests.rs`. Non-vacuity: the test's matcher is
  unit-tested against a sample line that names `ortho_config`.
- Obligation: `REQ-localized`, `REQ-explain-json` end to end. Method: BDD
  scenarios through the real binary (below) and `assert_cmd` cases in
  `tests/check_command_tests/`. Oracle: expected German text read from
  `locales/de/messages.ftl` and rendered by a fresh `FluentLocalizer`
  (independent of the runner), with isolation marks stripped on both sides.

### Formal verification decision

No Verus proof and no Kani harness are planned (`D9`). The candidate properties
are `L-normalized-injective`, `INV-fallback`, and `INV-ident`. Each ranges over
a domain that is closed at compile time — 24 rules, a declared key table, 35
embedded catalogues, three fallback tiers — and is enumerated completely by the
tests above on every `make test`, which is a decision procedure for the shipped
artefact. A Verus proof of `L-normalized-injective` would cover rule names not
yet written, but any such name enters the same exhaustive test before it can
merge, so the proof would add no assurance the gate does not already give,
while leaving the real risk (the Fluent adapter and catalogue contents) outside
its model. Kani is unsuited to `String`-heavy code that calls into
`fluent-bundle`. The proof sketch above is still recorded so that the naming
rule it implies is documented in the design document (`EP-M5`). Escalate if
implementation reveals an obligation over an unbounded domain that the tests
cannot enumerate.

### Suggested BDD features

Proposed path: `tests/features_lint/check_localization.feature`, collected by
`#[cfg(feature = "lint")] scenarios!("tests/features_lint", fixtures = [world:
TestWorld]);`
in `tests/bdd_tests.rs`. Steps reuse the existing
`netsuke is run with arguments` and stream assertions; new steps live in
`tests/bdd/steps/check_localization.rs`. Each scenario maps to `REQ-*` and
milestones as noted.

```gherkin
Feature: Localized lint findings
  The prose of a lint finding follows the selected locale, while every value a
  machine matches stays the same in every locale.

  Background:
    Given a Netsuke workspace whose recipe doubles a dollar for Ninja

  # REQ-localized, EP-M3
  Scenario: A German run reports the finding in German
    When netsuke is run with arguments "--locale de --json check"
    Then the command should succeed
    And the first check finding message should be the "de" text of "lint.rule.manual-ninja-escape.message"
    And the first check finding help should be the "de" text of "lint.rule.manual-ninja-escape.remediation"

  # REQ-ident, EP-M4
  Scenario: Identifiers do not change with the locale
    When netsuke is run with arguments "--locale de --json check"
    And netsuke is run again with arguments "--locale en-US --json check"
    Then both check results should have identical codes, severities, labels, urls, and spans

  # REQ-fallback, EP-M2
  Scenario: A locale without lint translations falls back to the English source
    When netsuke is run with arguments "--locale ja --json check"
    Then the command should succeed
    And the first check finding help should be the "en-US" text of "lint.rule.manual-ninja-escape.remediation"

  # REQ-localized (human output), EP-M3
  Scenario: Human output shows the German remediation as help
    When netsuke is run with arguments "--locale de check"
    Then the command should succeed
    And stdout should contain the "de" text of "lint.rule.manual-ninja-escape.remediation"

  # REQ-explain-json, EP-M2
  Scenario: The machine-readable rule catalogue stays in the source locale
    When netsuke is run with arguments "--locale de --json check --explain"
    And netsuke is run again with arguments "--locale en-US --json check --explain"
    Then both outputs should be byte-identical
```

The exact step wording is adjusted to the existing step library during `EP-M4`;
the scenarios' inputs and observable outcomes are fixed here.

## Plan of work

### `EP-M1` key table, audit optional tier, and source entries

Assigned: `REQ-keys`, `GAP-audit`, `GAP-normalized`, `D2`, `D5`.

1. Red: add audit cases for the optional tier (see methods) and run
   `cargo test -p netsuke --test build_l10n_audit_rules_tests` (or the
   in-module tests, matching where `audit_catalogue` is tested today); the
   "non-source lacks optional key" case fails.
2. Green: in `build_l10n_audit/mod.rs` add
   `const OPTIONAL_KEY_SOURCES: [&str; 1] = ["src/localization/lint_keys.rs"];`
   and thread a second declared set into `compare.rs::audit_catalogue`, so
   `missing = required − present` for non-source tags and
   `(required ∪ optional) − present` for `en-US`; `orphaned` uses the union. Add
   `src/localization/lint_keys.rs` with a `define_keys!` table (initially the
   72 prose keys), add it to `ALL_KEYS` and to the `keys.rs` re-exports, and add
   `rerun-if-changed` for it in `build.rs`.
3. Add the 72 `en-US` entries under a `## Lint rules` comment block in
   `locales/en-US/messages.ftl`, copying the registry text byte-for-byte
   (Fluent multiline values for long rationales; escape a literal `{` as
   `{"{"}` where the prose contains braces such as `{{ outs }}`).
4. Reword "behaviour" in `src/lint/rules/redundancy/duplication/mod.rs` and
   the matching sentence in `docs/netsuke-linter-rules.md` (`D5`).
5. Add `declared_keys_stay_distinct_after_normalization` with its negative
   controls, `lint_rule_keys_are_declared`, and
   `en_us_prose_matches_the_registry` (the last two gated on `lint`).
6. Run the focused tests, then the milestone gates.

End state: catalogues and audit accept lint keys; nothing renders them yet.

### `EP-M2` rule prose localized at the runner boundary

Assigned: `REQ-fallback`, `REQ-explain-json`, `D3` (prose half), `D6`, `D11`.

1. Red: `src/runner/check/prose/tests.rs` with the `INV-fallback` table and a
   `de` case expecting German remediation; fails because the module does not
   exist (compile failure is the expected red here, recorded as such).
2. Green: add `src/runner/check/prose/mod.rs` with `RuleProse` and
   `rule_prose(meta, localizer)`; pass `&dyn Localizer` into
   `CheckReport::diagnostics`, `FindingDiagnostic::new` (`help`), and
   `explain::render` (text path only). `json::render_catalogue` is unchanged.
   `src/runner/check/mod.rs` obtains `localization::localizer()` once and
   passes it down. Update existing callers and tests to pass a localizer
   explicitly.
3. Add the 72 `de` prose entries following `docs/localization-glossary.md`
   and `docs/localization-styleguide.md`, keeping manifest identifiers
   (`order_only_deps`, `{{ outs }}`, `dependency_order`, rule names, flags)
   verbatim.
4. Confirm the `rule_catalogue` snapshot is unchanged; extend
   `tests/check_command_tests/explanation.rs` with a `--locale de` text case.

End state: remediation and `--explain` prose are localized; messages are still
English.

### `EP-M3` finding messages localizable end to end

Assigned: `GAP-message`, `REQ-localized`, `REQ-core-pure`, `D1`, `D3` (message
half), `D7`, `D8`, `D13`.

1. Red: add the corpus parity test (`INV-source-parity` for messages) and the
   boundary test; add a `de` end-to-end case expecting a German message. They
   fail (missing type, missing keys).
2. Green, core: add `src/lint/finding/text.rs` with `FindingText`,
   `FindingArg`, and `Phrase` (see `Interfaces`); change `Finding.message` to
   `Finding.text: FindingText`; `sort_key` and `display_message` read
   `text.english()`. Change `FindingSink::{at, detached, at_or_detached}` to
   take `FindingText`. Convert each rule's `format!` site to build a
   `FindingText` from its message key constant, its English (the same `format!`
   as today), and its arguments; convert label helpers to return a `Phrase`;
   convert the nine `bashism` advice strings to phrases.
3. Green, keys and catalogues: declare the message, subject, advice, and
   `lint.finding.detached` keys in `src/localization/lint_keys.rs` (split into
   a second table file if it approaches 400 lines); add `en-US` and `de`
   entries.
4. Green, adapter: add `src/runner/check/l10n.rs` (name provisional) with
   `render_finding_text(text, localizer) -> String`, resolving phrases first
   then the message with `resolve`; use it in `FindingDiagnostic::new` for
   `message`, wrapping detached findings with `lint.finding.detached`.
5. Update the `result_document` and `truncated_result_document` snapshots;
   review that the only difference is isolation marks. Update English string
   assertions that cross a placeable to normalize isolation marks.
6. Refactor: remove any now-unused `String` constructors; keep
   `crate::lint_messages!` asserting English.

End state: every finding message and remediation is localized; `de` complete.

### `EP-M4` cross-locale contract sweep, BDD feature, and property test

Assigned: `REQ-ident`, `INV-order`, `INV-arg-verbatim`, `D12`.

1. Red: add the 35-locale sweep with its non-vacuity assertion, the
   `proptest`, and the BDD feature with its `cfg`-gated `scenarios!`. Steps are
   new, so the red is "step not found" until step 2.
2. Green: implement the steps in `tests/bdd/steps/check_localization.rs`
   (register in `tests/bdd/steps/mod.rs`), using the existing workspace
   fixtures; write the manifest from the `manual-ninja-escape` reported example.
3. Run seeded faults listed in the methods, record transcripts, revert.

End state: all verification obligations discharged.

### `EP-M5` documentation and delivery closure

Assigned: `GAP-adr`, `D10`.

1. Sweep remote branches for the next free ADR number; write
   `docs/adrs/adr-<nnnn>-localize-lint-rule-prose-and-finding-messages.md`
   recording `D1`, `D2`, `D4`, `D7`, `D8`, and the key naming rule from
   `L-normalized-injective`; amend ADR-0056 (banner and `## Amendments`); index
   the new ADR in `docs/contents.md`.
2. `docs/netsuke-linter-design.md`: replace "Rule text is not localized yet"
   in section 12; describe `FindingText` and phrases in section 5.4, the prose
   keys in 5.2, localized output and isolation marks in 9.1 and 9.2, and the
   new tests in 11; reference the new ADR.
3. `docs/translators-guide.md`: add `lint.*` to the key-domain table; explain
   the optional tier, the fallback, subject phrases, and identifiers that stay
   verbatim (rule names, codes, manifest keys, `{{ outs }}`).
4. `docs/developers-guide.md`: "Adding or changing a lint rule's text" —
   declare keys, add `en-US` entries, keep English parity, spelling-neutral
   prose, the key naming rule; note the injected-localizer convention for check
   adapters.
5. `docs/users-guide.md`: in the `netsuke check` section, state that findings
   follow `--locale`, that identifiers and `--explain --json` do not, that
   untranslated rules fall back to English, and that interpolated values are
   wrapped in isolation marks. Prose only; if a fenced example is added, give
   it a `tested-example` marker and an `EXPECTED_EXAMPLE_IDS` entry.
6. `CHANGELOG.md` entry; update the doc comment on `RuleMeta` in
   `src/lint/rule.rs`; mark roadmap 31.2.1 done; set this plan to `COMPLETE`
   after reconciliation.

## Milestones and plateaus

- `EP-M1`: catalogues and audit accept the lint key tier; `en-US` carries the
  prose. Requirements: `REQ-keys`, `GAP-audit`, `GAP-normalized`. Evidence:
  audit tier cases, normalization test, prose parity test. Conformance: no
  output change; no public interface change outside `build_l10n_audit`; audit
  contract change matches `D2`. Recovery: revert the milestone commits; the
  tier is self-contained. Remaining: nothing rendered yet. Compatibility: none.
- `EP-M2`: prose localized; `de` prose present. Requirements:
  `REQ-fallback`, `REQ-explain-json`. Evidence: fallback table, `de`
  explanation case, unchanged `rule_catalogue` snapshot. Conformance: core
  untouched; adapters take an injected localizer. Recovery: revert. Remaining:
  messages English. Compatibility: none (internal adapters).
- `EP-M3`: messages localized. Requirements: `GAP-message`,
  `REQ-localized`, `REQ-core-pure`. Evidence: corpus parity with coverage,
  boundary test, updated snapshots differing only by isolation marks.
  Conformance: core still framework-free; JSON field set unchanged. Recovery:
  revert; `EP-M2` remains a coherent plateau. Compatibility: none
  (`FindingSink` and `Finding` are crate-internal pre-1.0 APIs updated with all
  callers).
- `EP-M4`: contracts proven across locales. Requirements: `REQ-ident`.
  Evidence: sweep, property test, BDD. Recovery: tests only.
- `EP-M5`: documentation reconciled; roadmap ticked. Evidence: Markdown gates
  and a search for stale claims:
  `rg -n "not localized yet|English-only" docs --glob '!docs/execplans/**'`.

## Concrete steps

Working directory: the repository root of this worktree. Every gate pipes
through `tee` with `set -o pipefail`, logging to
`/tmp/$ACTION-netsuke-31-2-1-move-linter-messages-into-l10n-catalogues.out`.

Focused commands (red, then green):

```plaintext
EP-M1  cargo nextest run --all-features -E 'test(/optional|normaliz|lint_rule_keys|en_us_prose/)'
EP-M2  cargo nextest run --all-features -E 'test(/prose|explanation/)'
EP-M3  cargo nextest run --all-features -E 'test(/parity|boundary|result_document|check_command/)'
EP-M4  cargo nextest run --all-features -E 'test(/locale_sweep|verbatim/)' && \
       cargo nextest run --all-features --test bdd_tests -E 'test(/localized_lint/)'
```

Milestone gates, sequentially, delegated to `scrutineer`:

```sh
set -o pipefail
make check-fmt            2>&1 | tee /tmp/check-fmt-netsuke-31-2-1-move-linter-messages-into-l10n-catalogues.out
make lint                 2>&1 | tee /tmp/lint-netsuke-31-2-1-move-linter-messages-into-l10n-catalogues.out
make doc-coverage         2>&1 | tee /tmp/doc-coverage-netsuke-31-2-1-move-linter-messages-into-l10n-catalogues.out
make test                 2>&1 | tee /tmp/test-netsuke-31-2-1-move-linter-messages-into-l10n-catalogues.out
make test-default-features 2>&1 | tee /tmp/test-default-features-netsuke-31-2-1-move-linter-messages-into-l10n-catalogues.out
make test-package-verify  2>&1 | tee /tmp/test-package-verify-netsuke-31-2-1-move-linter-messages-into-l10n-catalogues.out
make markdownlint         2>&1 | tee /tmp/markdownlint-netsuke-31-2-1-move-linter-messages-into-l10n-catalogues.out
make nixie                2>&1 | tee /tmp/nixie-netsuke-31-2-1-move-linter-messages-into-l10n-catalogues.out
```

`make test-package-verify` is required for `EP-M1` (audit change) and at
closure; the others after every milestone. Commit after each green milestone
with a message naming the milestone.

## Validation and acceptance

Acceptance, observable after `EP-M4`:

- `netsuke --locale de --json check` on the `manual-ninja-escape` reported
  manifest succeeds; the finding's `message` and `help` equal the `de`
  catalogue's rendering; `code` is `netsuke::lint::manual_ninja_escape`,
  `severity` is `warning`, the label is `manual-ninja-escape`, and the `url`
  ends `#manual-ninja-escape`, identical to the `en-US` run.
- `netsuke --locale ja --json check` reports the `en-US` remediation text.
- `netsuke --locale de --json check --explain` is byte-identical to the
  `en-US` run.
- `netsuke --locale de check --explain manual-ninja-escape` prints the German
  summary, rationale, and remediation under English labels.
- `make test` passes; the new tests fail before their milestone's change
  (recorded red evidence) and pass after.

Quality criteria: all gates in `Concrete steps` pass; every obligation in the
`Verification plan` is discharged with its non-vacuity evidence recorded in
`Artefacts and notes`; no `#[allow]` added; no file over 400 lines.

## Idempotence and recovery

All steps are re-runnable. Seeded faults are applied in the working tree only
after the preceding commit, then reverted with `git checkout -- <file>`;
confirm with `git status --short` that nothing remains. Proptest regression
files created by a seeded fault are deleted. If a milestone must be abandoned,
`git revert` its commits; each earlier milestone is a coherent plateau.

## Artefacts and notes

Reconnaissance evidence (at `5c707ef33`):

```plaintext
$ grep -c '' locales/en-US/messages.ftl
506
$ grep -n '^check.threshold_exceeded = ' locales/en-US/messages.ftl
493:check.threshold_exceeded = Lint findings reached the { $severity } threshold: …
$ grep -rhoiE '\b[a-z]*our[a-z]*\b' src/lint/rules | sort | uniq -c   (excerpt)
      2 behaviour
```

Implementation transcripts (red, green, seeded faults, proptest class counts)
are appended here per milestone.

## Interfaces and dependencies

No new dependencies. Signatures below are targets; names may be refined during
implementation if the `Decision log` records why.

In `src/lint/finding/text.rs` (framework-free core):

```rust
/// The localizable text of one finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingText {
    key: &'static str,
    args: Vec<(&'static str, FindingArg)>,
    english: String,
}

/// One named argument of a finding's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindingArg {
    /// An identifier or other value shown exactly as written.
    Verbatim(String),
    /// A count, so Fluent can select a plural form.
    Count(usize),
    /// A localizable noun phrase rendered before the message.
    Phrase(Phrase),
}

/// A localizable phrase whose own arguments are never phrases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phrase {
    key: &'static str,
    args: Vec<(&'static str, PlainArg)>,
    english: String,
}

/// A phrase argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlainArg {
    Verbatim(String),
    Count(usize),
}
```

with builder methods `FindingText::new(key, english)`, `.verbatim(name, value)`,
`.count(name, n)`, `.phrase(name, phrase)`, and accessors `key()`, `args()`,
`english()`. Keys are the `&'static str` constants from
`crate::localization::lint_keys` re-exported through `keys`, which are plain
string constants, so the core gains no localization dependency.

In `src/runner/check/prose/mod.rs`:

```rust
pub(super) struct RuleProse {
    pub(super) summary: String,
    pub(super) rationale: String,
    pub(super) remediation: String,
}

pub(super) fn rule_prose(meta: &RuleMeta, localizer: &dyn Localizer) -> RuleProse;
pub(super) fn prose_key(name: &str, field: ProseField) -> String;
```

In `src/runner/check/l10n.rs`:

```rust
pub(super) fn render_finding_text(text: &FindingText, localizer: &dyn Localizer) -> String;
```

Both call `Localizer::message(id, args, fallback)` with the registry or stored
English as `fallback`, relying on `LayeredLocalizer` for the `en-US` tier.

Changed signatures:
`CheckReport::diagnostics(&self, localizer: &dyn Localizer)`,
`json::render_result(report, localizer)`, `explain::render(…, localizer)`,
`FindingSink::{at, detached, at_or_detached}` taking `FindingText`. All callers
change together; no compatibility layer (pre-1.0, crate-internal).

In `build_l10n_audit`: `OPTIONAL_KEY_SOURCES` and an extra
`optional: &BTreeSet<String>` parameter on `audit_catalogue` (or a small
`DeclaredKeys { required, optional }` struct if the parameter list grows past
three).

Reuse policy: `FindingText` is the only way a rule reports text; adapters own
rendering; no other module renders lint keys. `prose_key` is the single place
the key grammar is spelt. The optional tier is reserved for lint keys; adding
another file to `OPTIONAL_KEY_SOURCES` requires an ADR amendment.

## Revision note

- 2026-11-15: initial draft from Wyvern reconnaissance of the lint prose
  flow, localization infrastructure, and docs/tests at `5c707ef33`. Awaiting
  community-of-experts review and user approval.

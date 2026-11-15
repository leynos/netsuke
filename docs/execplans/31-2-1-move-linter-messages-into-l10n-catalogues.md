# 31.2.1 Move rule summary, rationale, and remediation into the localization catalogues

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT

The community-of-experts design review has been applied. The plan awaits user
approval. Decisions `D1` to `D5`, `D7`, and `D14` change scope, a build-time
contract, or a contributor workflow, and are proposed for approval with the
plan rather than settled by it.

Branch: `31-2-1-move-linter-messages-into-l10n-catalogues`

Roadmap item: `31.2.1` in `docs/roadmap.md`, under step 31.2 ("Localize lint
output") of phase 31 ("Manifest linting").

## Purpose / big picture

Netsuke is a build tool that compiles YAML and Jinja manifests into Ninja build
files. Its `netsuke check` subcommand, compiled in when the Cargo feature
`lint` is enabled, runs a manifest linter: 24 rules, each with a stable
kebab-case name such as `manual-ninja-escape`. A rule reports *findings*. Each
finding carries a per-finding *message* and the rule's *remediation*, which
human output prints as `help:` and JSON output carries as `help`. Each rule
also owns a one-line *summary* and a *rationale*, printed by
`netsuke check --explain`.

Today all of that prose is English, hard-coded in Rust. The rest of the
command-line interface (CLI) is localized into 35 locales through Fluent
catalogues (`locales/<tag>/messages.ftl`), selected with `--locale <tag>`,
`NETSUKE_LOCALE`, a configuration file, or the system locale. A German user
running `netsuke --locale de check` reads German framing text ("Lint results —
…" in German) around English findings such as:

```plaintext
action `build` `command` escapes a dollar for Ninja, which Netsuke now does itself
help: Write the shell variable normally, for example `$PATH` rather than `$$PATH`.
```

After this change the same run prints, for example:

```plaintext
Aktion `build` `command` maskiert ein Dollarzeichen für Ninja, was Netsuke inzwischen selbst übernimmt
help: Schreiben Sie die Shell-Variable normal, zum Beispiel `$PATH` statt `$$PATH`.
```

(The German wording is illustrative; the delivered text follows
`docs/localization-glossary.md` and `docs/localization-styleguide.md`.) The
finding's message, its remediation, and the rule's summary and rationale in
`--explain` text output come from the catalogue for the selected locale. Every
machine-matched value — the diagnostic `code`, the `severity`, the rule name
used as the span label, the documentation `url`, the spans, the finding order,
the `--limit` truncation, and the whole `--explain --json` catalogue — is
byte-identical across locales. A locale whose catalogue has not translated a
rule falls back to the English source catalogue (`en-US`), and if even that
cannot render the text, to English compiled into the binary, so a diagnostic is
never empty and never shows a raw message key.

Deliberately changed behaviour: interpolated values in finding messages (the
target name `build` above) are wrapped in Unicode bidirectional isolation marks
(U+2068 FIRST STRONG ISOLATE and U+2069 POP DIRECTIONAL ISOLATE), as every
other localized CLI message with arguments already is. A noun phrase that is
itself interpolated (the item label "action `build`") is marked twice: once
around its own argument and once as an argument of the message. This applies in
every locale, including `en-US`, in human and JSON output. See `D4`.

This work closes the provisional decision recorded in
`docs/adrs/adr-0056-manifest-linting-under-netsuke-check.md` ("Rule text is not
localized yet") and the matching paragraph in section 12 of
`docs/netsuke-linter-design.md`. It does not localize the `--explain` framing
labels (roadmap task 31.2.2), does not rebind the rule reference contract test
to the catalogue (31.2.3), and translates only one pilot locale (31.2.4 owns
the rest; see `D3`).

The user waived this task's roadmap prerequisite (31.1.3, "Apply the
dispositions") so that the linter is not published untranslated. Two
consequences follow. First, the prose localized here may still change under
step 31.1, and each such change must update the `en-US` catalogue in the same
commit (enforced by `INV-source-parity`). Second, this task alone does not make
the linter's published text fully translated: after it, 33 locales still fall
back to English for lint text. The linter is not in release binaries until
roadmap task 31.4.4 removes the `lint` feature gate (ADR-0056), so the user's
goal is met only if 31.2.4 completes before 31.4.4. The translation coverage
ledger added here (`EP-M1`) makes that state visible in every pull request (PR).

## Constraints

- Rule identifiers never become localizable. A rule's `name`, its `category`
  spelling, its `stage` spelling, its severity spelling, its diagnostic `code`
  (`netsuke::lint::<snake_name>`), and its documentation `url` must be computed
  without consulting a localizer (roadmap step 31.2;
  `docs/translators-guide.md`; ADR-0056).
- `netsuke check --explain --json` stays in the source locale, and its bytes
  must not change in this task (ADR-0056; roadmap 31.2.2).
- No JSON field is added, renamed, or removed in the check result document,
  the threshold diagnostic document, or the `--explain` catalogue document.
  Only the *values* of `message` and `help` in finding entries become
  localized. The schema freeze is roadmap task 31.4.1.
- The lint core (`src/lint/**`) stays framework-free. It must not name
  `ortho_config`, `fluent`, `fluent_bundle`, `miette`, or any
  `crate::localization` path except the pure-data key table
  `crate::localization::lint_keys`, whose items are `&'static str` constants
  (roadmap 31.5.1; `docs/netsuke-linter-design.md` section 3.2). Localization
  happens at the runner boundary (`src/runner/check/**`).
- Finding order, and therefore which findings `--limit` keeps, must not
  depend on the locale.
- The registry's English text (`RuleMeta::summary`, `rationale`,
  `remediation`) is retained as the last-resort fallback (roadmap 31.2.1).
- Fluent keys and their catalogue entries stay ungated by the `lint` feature,
  because the build-time audit reads them regardless of features
  (`docs/developers-guide.md`, "Fluent keys stay ungated").
- No in-process environment mutation in tests and no new process-wide test
  locks (`AGENTS.md`, Testing). New tests inject a `&dyn Localizer`; the
  existing `localizer_test_lock` in `src/runner/check/tests.rs` is not used by
  new tests.
- Files stay under 400 lines (`AGENTS.md`; Whitaker module cap).
  `src/localization/keys.rs` is at 351 lines and must not grow; new keys go in
  a new table file (precedent: ADR-0069, "Split the localization key tables").
  The audit scanner reads only the first `define_keys!` block of each file
  (`build_l10n_audit/keys.rs:47`), so each table file holds one block.
- Catalogue entries for lint keys are single-line Fluent values (multiline
  values keep line breaks and would break byte parity with the single-line
  registry text), and literal braces are written as Fluent string literals
  (`{"{"}`, `{"}"}`, or a whole `{"{{ outs }}"}`), following the existing
  convention at `locales/en-US/messages.ftl:465`.
- Polonius posture: the pinned nightly enables Polonius alpha; do not add
  defensive clones or double lookups (`AGENTS.md`, Borrow checker).
- No new dependencies. `ortho_config` 0.10.0 (re-exporting `ortho_l10n`
  0.10.0 and `fluent-bundle` 0.16) already provides `Localizer`,
  `LocalizationArgs`, `FluentValue`, `FluentLocalizer`, and
  `FluentLocalizerBuilder::with_error_reporter`; `rstest`, `rstest-bdd`,
  `insta`, `proptest`, `googletest`, `pretty_assertions`, and `mockall` are
  already development dependencies (`Cargo.toml` lines 180-199).
- Completed ExecPlans under `docs/execplans/` are historical and must not be
  edited.
- Implementation begins only after the user approves this plan.

## Tolerances (exception triggers)

Comparison base: the merge base with `origin/main` at implementation start
(reconnaissance base `5c707ef33`). For the second PR (`D14`), the base is the
first PR's head. Count with:

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

Limits are net lines (additions minus deletions) per category, enforced
independently and per PR. Spare capacity in one category never offsets another.

PR A (`EP-M1`, `EP-M2`, and the documentation for those decisions):

- Files: more than 28 non-ExecPlan files → stop and escalate.
- Code: more than 900 net lines → stop and escalate. Estimate 700: audit tier,
  per-rule atomicity, and coverage ledger (~200 with tests), lint key table for
  prose (~90), prose resolver and injected localizer (~150), fallback, parity,
  normalization, sentinel-format, verbatim-span, and spelling tests (~260).
- Configuration: more than 450 net lines → stop and escalate. Estimate 330:
  72 single-line `en-US` prose entries plus translator comments (~160) and the
  same for `de` (~160).
- Documentation: more than 300 net lines → stop and escalate. Estimate 230:
  new ADR (~150), translators' guide tier and comments policy (~50),
  developers' guide (~30).

PR B (`EP-M3`, `EP-M4`, `EP-M5`):

- Files: more than 42 non-ExecPlan files → stop and escalate.
- Code: more than 1,400 net lines → stop and escalate. Estimate 1,100:
  template and finding-text types with interpolator (~200), 24 rules' message
  conversion and label phrases (~300), key table additions (~60), adapter
  rendering (~100), boundary test, cross-locale sweep, proptest, BDD steps, and
  snapshot updates (~440).
- Configuration: more than 350 net lines → stop and escalate. Estimate 250:
  about 30 message templates, 9 advice phrases, and subject phrases with
  comments, in `en-US` and `de`.
- Documentation: more than 300 net lines → stop and escalate. Estimate 220:
  linter design (~80), users' guide (~40), translators' guide (~40),
  developers' guide (~30), ADR amendment and changelog (~30).

Other triggers:

- Interface: any change to the JSON field set, to `RuleMeta`'s identifier
  fields, or to a public item outside `crate::lint::finding`,
  `crate::lint::rule`, `crate::runner::check`, `crate::localization`, and
  `build_l10n_audit` → stop and escalate.
- Dependencies: any new crate → stop and escalate.
- Architecture: any need for `src/lint/**` to see a localizer, or any audit
  change beyond the `lint.` optional tier, per-rule atomicity, and the coverage
  ledger → stop and escalate.
- Iterations: a focused test still failing after three fix attempts, or a
  full gate failing twice for the same cause → stop and escalate.
- Time: a milestone exceeding six hours of wall-clock work, excluding gate
  queueing → stop and escalate.
- Ambiguity: a rule message whose English cannot be expressed as one static
  template plus arguments without changing its English wording → record;
  escalate if more than three rules are affected.
- Translation feasibility: if the inflection probe in `EP-M3` shows a
  message family cannot be translated acceptably into a case-inflecting
  language with phrases as designed → stop and escalate with the contingency in
  `D8`.

Budget evidence: the comparable localization plans recorded no budget increases.
`docs/execplans/3-7-1-externalize-user-facing-strings.md` allowed 30 files or
1,500 net lines; `3-7-2-locale-resolution.md` and
`3-7-3-translator-tooling-and-documentation.md` each allowed 10 to 12 files and
600 lines. Neither converted per-call-site message construction across a rule
set, which is why this plan's combined code budget (2,300 across two PRs) is
larger. No ExecPlan has previously touched `src/lint`, so there is no
lint-specific budget history. The design review raised the estimate from 1,600
to 1,800 code lines by adding the sentinel-format test, coverage ledger,
verbatim-span test, and per-rule atomicity; the limits include that growth, not
as contingency for it.

## Risks

- Risk: Fluent's isolation marks change the bytes of `en-US` finding
  messages, breaking string-matching tests and any consumer grepping `message`.
  Severity: medium. Likelihood: certain for interpolated messages. Mitigation:
  `D4` accepts the change, consistent with existing localized messages; tests
  normalize with `test_support::fluent::normalize_fluent_isolates`; users'
  guide and changelog state it; `message` is prose, not a contract field.
- Risk: the registry English, the core message templates, and the `en-US`
  catalogue drift. Severity: high. Likelihood: medium. Mitigation:
  `INV-source-parity` is exhaustive over every declared key (prose, message
  templates, phrases) using sentinel arguments, independent of any corpus.
- Risk: a syntax error in a catalogue fails the whole locale's bundle; in
  `en-US` the CLI falls back to `NoOpLocalizer` and every message prints its key
  (`src/cli/localization/mod.rs:109-116`). Severity: high. Likelihood: medium,
  given ~250 new `en-US` lines. Mitigation:
  `every_catalogue_parses_without_the_english_fallback`
  (`tests/localization_tests.rs:75`) is named in `EP-M1` and `EP-M2`
  acceptance; single-line values and the brace convention in `Constraints`.
- Risk: a translator writes `$name` as literal text, or references a Fluent
  term that fails to resolve; the audit's variable scan matches `$word` anywhere
  (`build_l10n_audit/ftl.rs:48-61`) and cannot tell, and a runtime formatting
  error silently falls back to English (warnings are suppressed in `--json`
  mode, `src/main.rs:346-349`). Severity: medium. Likelihood: medium.
  Mitigation: `INV-format-clean` renders every present lint entry in every
  catalogue with sentinel arguments and an issue-collecting reporter.
- Risk: two keys normalize to the same Fluent identifier (`ortho_l10n`
  rewrites `.` to `-` at load), so a catalogue fails registration, or a locale
  lacking key A renders key B's text. Severity: high. Likelihood: low.
  Mitigation: `L-normalized-injective` and its dedicated test; naming rule in
  `D7`.
- Risk: nested noun phrases ("target `x`") cannot inflect for grammatical
  case in Polish, Russian, Czech, Ukrainian, Finnish, Hungarian, or Turkish,
  and the audit's equal-variables rule stops translators using `$name`
  directly. Severity: medium. Likelihood: medium. Mitigation: German pilot plus
  a throwaway inflection probe in `EP-M3`; contingency in `D8`.
- Risk: the optional tier (`D2`) hides untranslated locales indefinitely.
  Severity: medium. Likelihood: medium. Mitigation: per-rule atomicity (no
  half-translated rule), the coverage ledger snapshot, and an explicit tier
  lifetime decision assigned to 31.2.4 (`D2`).
- Risk: the default-features CI lane runs BDD scenarios without `lint`.
  Severity: medium. Likelihood: high if scenarios live in `tests/features`.
  Mitigation: `tests/features_lint/` behind `#[cfg(feature = "lint")]` (`D12`).
- Risk: converting rule messages changes an English message by accident.
  Severity: medium. Likelihood: medium. Mitigation: the existing per-rule
  assertions (`crate::lint_messages!`) and the result-document snapshot pin the
  English; the snapshot may change only by isolation marks.
- Risk: a broken translation emits one `tracing` warning per finding in human
  mode (up to the default `--limit` of 200, unbounded with `--limit 0`).
  Severity: low. Likelihood: low once `INV-format-clean` holds. Mitigation:
  accepted; changing the CLI-wide reporter is out of scope (`D15`).

## Progress

- [x] (2026-11-15 00:40Z) Branch renamed; Wyvern reconnaissance of the lint
  prose flow, localization infrastructure, and docs and tests completed.
- [x] (2026-11-15 01:30Z) Initial draft written and committed (`20cf652d7`).
- [x] (2026-11-15 02:40Z) Community-of-experts design review (structure and
  contracts; failure modes and cost; alternatives and viability) applied.
- [ ] Plan approved by the user.
- [ ] `EP-M1` key table, audit optional tier with per-rule atomicity and
  coverage ledger, and `en-US` prose entries.
- [ ] `EP-M2` rule prose localized at the runner boundary, with the pilot
  `de` prose. End of PR A.
- [ ] `EP-M3` finding messages localizable end to end, with the pilot `de`
  messages and the inflection probe.
- [ ] `EP-M4` cross-locale contract sweep, BDD feature, and property test.
- [ ] `EP-M5` documentation, ADR amendment, changelog, and roadmap closure.
  End of PR B.

## Surprises & discoveries

- Observation: the per-finding `message` is not one of the three registry
  prose fields; it is English built with `format!` inside each rule. Evidence:
  `Finding.message: String` at `src/lint/finding/mod.rs:30`; about 28 `format!`
  sites under `src/lint/rules/**/mod.rs` (for example
  `src/lint/rules/migration/mod.rs:64-68`). Impact: meeting the roadmap's
  success criterion requires localizable message construction (`D1`).
- Observation: detached findings carry an English location string ("rule
  `x`", "target `x`") in `Location::Symbol(String)`. Evidence:
  `src/lint/rules/redundancy/duplication/mod.rs:59,185`,
  `src/lint/rules/graph/mod.rs:66,250`, `src/lint/rules/hygiene/mod.rs:230`,
  `src/lint/rules/caching/mod.rs:139,261`. Impact: locations become phrases
  (`D8`).
- Observation: further English fragments are passed into messages as
  parameters: the caching `kind` ("phony target", "directory";
  `caching/mod.rs:70,111,140`), the descriptions `consequence`
  (`clarity/descriptions/mod.rs:82-101`), and the joined unused-suppression list
  (`suppression/mod.rs:137-145`). Impact: inventory and dispositions in
  `EP-M3` step 1.
- Observation: the build audit rejects any declared key missing from any of
  the 35 catalogues, so "a catalogue gap" cannot exist today, contradicting
  ADR-0056's stated migration shape ("translate incrementally"). Evidence:
  `build_l10n_audit/compare.rs:159-172`; ADR-0056 "Known risks". Impact: `D2`.
- Observation: `ortho_l10n` rewrites dots in catalogue identifiers to hyphens
  when loading (`normalize_resource_ids`), and lookup tries the dotted id then
  the normalized one. The audit compares raw dotted ids and silently merges
  duplicates (`build_l10n_audit/ftl.rs:117-121`). Evidence:
  `ortho_l10n-0.10.0/src/fluent.rs:99-114`, `src/lib.rs:236-276`. Impact:
  `L-normalized-injective`. No existing `en-US` ids collide after normalization
  (checked by the review).
- Observation: Fluent inserts isolation marks around every placeable, so a
  pattern such as `{ $location }: { $message }` would wrap both halves.
  Evidence: `fluent-bundle-0.16.0/src/resolver/pattern.rs:50-61`. Impact: `D13`
  composes the detached prefix in the adapter instead.
- Observation: the roadmap's success criterion names `rule` and `category`
  fields, but finding entries carry neither; the rule name appears as the span
  `label`, and `category` appears only in `--explain --json`. Evidence:
  `src/snapshots/check/netsuke__runner__check__tests__result_document.snap`.
  Impact: `INV-ident` is stated over the fields that exist; no field is added.
- Observation: user-facing rule prose contains two en-GB-only spellings:
  "behaviour" (`redundancy/duplication/mod.rs:99`) and "catalogue"
  (`clarity/descriptions/mod.rs:32`). Evidence: grep and design review. Impact:
  `D5`.
- Observation: the audit treats every `$word` in a message as a variable, so
  literal `$PATH`, `$in`, and `$out` in rule prose
  (`src/lint/rules/migration/mod.rs:41,94-104`) count as variables. Impact: a
  translation must keep them verbatim, which is the desired behaviour;
  documented for translators rather than changed.
- Observation: `serial-order-without-deps` reports "over 1 direct
  dependencies" for one dependency (`redundancy/declarations/mod.rs:85-100`).
  Impact: out of scope (prose belongs to step 31.1); the `en-US` template
  mirrors today's English without a plural select so parity holds; recorded for
  31.1.
- Observation: the translators' guide "Current limitation" passage
  (`docs/translators-guide.md:309-318`) says arguments are always strings, which
  `LocalizedMessage::with_count` has since made false. Impact: fixed in
  `EP-M5`.
- Observation: `src/runner/check/tests.rs` installs the global localizer
  under a process-wide `localizer_test_lock`, and `text.rs` summary lines and
  the threshold error resolve through the global at display time. Impact: `D6`.
- Observation: no existing test extracts each rule reference section's
  reported manifest; `tests/lint_rule_reference_tests.rs:208-220` only checks
  for a "Reported:" substring. Impact: the cross-locale sweep uses the in-crate
  per-rule fixtures (`lint_fixture!`, `src/lint/test_support.rs`) instead.

## Decision log

- D1. Decision (proposed; needs approval): localize the per-finding message
  as well as the three prose fields. Rationale: the roadmap's success criterion
  requires the message in the selected locale; localizing only the remediation
  would leave the headline English, which the user's reason for waiving 31.1.3
  argues against. Rejected: prose-only scope with a new roadmap task for
  messages (about 40% of the code, half-English output). Date/Author:
  2026-11-15, planning agent.
- D2. Decision (proposed; needs approval): give the build audit an optional
  key tier defined by the key prefix `lint.`. Such keys must be present in
  `en-US`, may be absent from other catalogues, must not be orphaned anywhere,
  must use the same variables as `en-US` when present, and are all-or-nothing
  per group: a catalogue carries every key of a rule (`lint.rule.<name>.*`) or
  none of them, and every shared phrase (`lint.subject.*`) or none. A
  translation coverage ledger (locale → translated rules over total) is pinned
  by an `insta` snapshot so every change in coverage appears in PR diffs. The
  tier's lifetime is decided by 31.2.4: keep it with a release-time coverage
  gate before 31.4.4, or remove it once every locale is complete. Rationale:
  matches ADR-0056's migration shape and its driver that per-rule work must
  stay small; makes the fallback chain a real, testable path; atomicity
  prevents a German message with English help. Rejected: (a) English copies in
  all 34 non-source catalogues (about 9,000 placeholder lines that the
  localization style guide calls a scaffold, which 31.2.4 must replace); (b)
  translating all locales now (absorbs 31.2.4). Date/Author: 2026-11-15,
  planning agent; revised after review.
- D3. Decision (proposed; needs approval): translate every lint key into one
  pilot locale, German (`de`), in this task. Rationale: end-to-end evidence
  needs a locale whose output differs from English; German case and gender
  exercise the phrase design early. Date/Author: 2026-11-15, planning agent.
- D4. Decision (proposed; needs approval): accept Fluent isolation marks
  around interpolated values in every locale, including `en-US`, and accept
  double marking of interpolated phrases. Rationale: keeps right-to-left locales
  (`ar`, `fa`, `he`) legible and matches every other localized CLI message;
  isolation is not configurable per message in `ortho_l10n`. Rejected: rendering
  `en-US` from compiled English to keep bytes unchanged — a source-locale
  special case. Date/Author: 2026-11-15, planning agent; widened after review.
- D5. Decision (proposed; needs approval): rule source prose is
  spelling-neutral English. Reword "behaviour"
  (`redundancy/duplication/mod.rs:99`) and "catalogue"
  (`clarity/descriptions/mod.rs:32`) and the matching sentences in
  `docs/netsuke-linter-rules.md`; add a test rejecting en-GB-only stems (`-our`,
  `catalogue`, `analyse`, `centre`, `licence`, `artefact`, `-ise` forms listed
  in the test) in registry prose and message templates. `en-GB` carries no lint
  entries and permanently falls back to `en-US`; 31.2.4's "untranslated copy"
  check excludes it. Rationale: avoids two English catalogues diverging;
  contributors write en-GB by habit, so a one-time reword would not hold.
  Module comments stay en-GB-oxendict. Date/Author: 2026-11-15, planning agent;
  extended after review.
- D6. Decision: inject the localizer into the check adapters as
  `&dyn Localizer` parameters, covering finding diagnostics, the JSON result,
  `--explain` text, and the `text.rs` summary and truncation lines (via
  `LocalizedMessage::render_with`). `src/runner/check/mod.rs` obtains it once
  per run from `crate::localization::localizer()` before the `--explain`
  branch. The threshold `RunnerError` keeps resolving through the global at
  display time, which in production is the same object; tests asserting human
  failure output use one locale. Rationale: pure, testable adapters without the
  global lock.
- D7. Decision (proposed; needs approval; revised after review): every
  message form is a static *template* declared in the lint core: a key constant
  from `crate::localization::lint_keys` and an English pattern using `{$name}`
  placeholders (for example
  ``{$subject} `{$kind}` escapes a dollar for Ninja``, abbreviated). A finding
  stores the template and its arguments; the core's interpolator renders the
  English on demand, used for ordering, for `lint_messages!`, and as the last
  fallback. Each rule lists its templates in a new non-identifier field
  `RuleMeta::messages`; shared phrases are listed in
  `crate::lint::finding::phrases::ALL`. Key naming rule: `lint.rule.<name>.`
  followed by a head word (`summary`, `rationale`, `remediation`, `message`,
  `advice`), then optional variant segments that contain no hyphens and are not
  head words; shared phrases use `lint.subject.<member>.<form>`. Rationale: one
  English template per key, adjacent to its key, with exhaustive parity against
  `en-US` and no corpus; arguments cannot drift from the English. Rejected
  alternative (strongest, raised by review): messages rendered only from the
  `en-US` catalogue, with no English in the core, ordering by (location, rule,
  key, arguments), and argument names declared beside each key for a static
  audit check. It removes the English template entirely but moves
  `lint_messages!` assertions to runner tests, degrades a runtime formatting
  failure to a generic line, and extends the audit further; offered to the user
  as an alternative at approval.
- D8. Decision: noun phrases naming the offending item ("target `x`",
  "action 3"), detached-finding locations, and the `bashism` rule's
  per-construct advice become phrases: a template plus plain (non-phrase)
  arguments, rendered first and passed to the message as text. Phrases cannot
  contain phrases, enforced by the type. `Location::Symbol` holds a phrase.
  English fragments passed as parameters today (caching `kind`, descriptions
  `consequence`) become message variants
  (`lint.rule.<name>.message.<variant>`). The unused-suppression list stays one
  verbatim argument (identifiers joined with ", ") plus a count; the English
  separator is an accepted limitation. Contingency if the inflection probe
  fails: additionally pass the phrase's `$kind` and `$name` to the message and
  allow lint translations to use a subset of the `en-US` variables, which is an
  audit change requiring approval.
- D9. Decision: no Verus or Kani in this task; see
  `Formal verification decision`.
- D10. Decision: record `D1`, `D2`, `D4`, `D5`, `D7`, and `D8` in a new ADR
  that amends ADR-0056 per `docs/documentation-style-guide.md` (an
  `Amended by ADR-nnnn (YYYY-MM-DD)` banner in ADR-0056's Status section and an
  `## Amendments` section; ADR-0056's accepted text is preserved). The ADR
  lands in PR A with every decision, because the decisions were designed
  together; PR B adds an implementation note if anything moved. Provisional
  number 0082 (PR #808 holds 0075 to 0081); re-derive by sweeping every remote
  branch before minting.
- D11. Decision: `--explain` text output shows the localized summary,
  rationale, and remediation; its labels stay English until 31.2.2.
  `--explain --json` keeps reading the registry.
- D12. Decision: BDD scenarios live in
  `tests/features_lint/check_localization.feature`, collected by a
  `#[cfg(feature = "lint")]` `scenarios!` invocation in `tests/bdd_tests.rs`
  (mirroring `tests/features_unix`), with the steps module gated the same way.
- D13. Decision (revised after review): the detached-finding prefix is
  composed in the adapter as the localized location phrase, `": "`, and the
  localized message, rather than through a Fluent wrapper key, which would wrap
  the whole message in isolation marks. The colon stays untranslated
  punctuation; revisit under 31.2.2 if a locale needs different spacing.
- D14. Decision (proposed; needs approval): deliver the implementation as
  two stacked PRs split at the `EP-M2` plateau. PR A (`EP-M1`, `EP-M2`, the
  ADR, and the translators' and developers' guide sections for the tier)
  carries the audit contract change and the German prose; PR B (`EP-M3` to
  `EP-M5`) carries the `FindingSink` API change across 24 rules, the messages,
  the sweep, and the remaining documentation. Rationale: separates an audit
  contract change and a catalogue review from a 24-rule API change; PR A alone
  unblocks 31.2.3.
- D15. Decision: keep `ortho_l10n`'s default formatting-issue reporter
  (one `tracing` warning per failed lookup, silent in `--json`).
  `INV-format-clean` is the gate; a once-per-key reporter would change the
  whole CLI and is out of scope.
- D16. Decision: do not reuse `crate::localization::LocalizedMessage` and its
  `MessageArg` in the lint core, because the core must stay framework-free and
  `LocalizedMessage` renders through the global localizer; the adapter converts
  core arguments to `LocalizationArgs`. Recorded per the `AGENTS.md`
  abstraction policy.

## Outcomes & retrospective

Not started. To be completed at each milestone and at closure, including the
reconciliation of the `Decision log` with ADR-0056, the new ADR, and the linter
design document.

## Context and orientation

Inspected base revision: `5c707ef339254a60d58f191c563bfe8597be9deb` (`main`,
"Align AGENTS.md with the Rust project template (#1135)"). Line numbers below
refer to that revision; relocate symbols with `rg -n '<symbol>' src tests`.

### The lint core

- `src/lint/rule.rs`: `RuleMeta` (lines 99-133) holds `name`, `category`,
  `stage`, `default_severity`, `summary`, `rationale`, `remediation`;
  `RuleMeta::code()` derives `netsuke::lint::<name with - replaced by _>`. Its
  doc comment already names 31.2.1. `FindingSink` (around lines 210-275) binds
  a rule to its findings; `at`, `detached`, and `at_or_detached` take
  `impl Into<String>` messages and locations.
- `src/lint/finding/mod.rs`:
  `Finding { meta, severity, message: String, location: Location }`;
  `Location::{Span, Symbol(String)}`; `sort_key()` orders by location
  (including the symbol text), rule name, then message; `display_message()`
  prefixes detached findings with their symbol. Sorting happens in the core
  (`src/lint/engine/mod.rs:80`), and `--limit` applies before any rendering.
- `src/lint/rules/**`: 24 `static … : RuleMeta` definitions across 11 files
  (suppression 3, determinism 2, caching 2, migration 2, hygiene 3,
  clarity/descriptions 2, clarity/recipe_shape 2, graph 2, portability 1,
  redundancy/declarations 3, redundancy/duplication 2). Shared label helpers:
  `Item::label()` in `src/lint/rules/recipes.rs:68-73` (member word plus name
  or 1-based index), and `label()` helpers in `caching/mod.rs:261`,
  `graph/mod.rs:66`, and `redundancy/duplication/mod.rs:185`.
  `portability/mod.rs:43-97` holds nine `CONSTRUCTS` entries with English
  `advice`.
- `src/lint/registry/mod.rs`: `all()`, `meta_by_name()`, `catalogue()`.
  `src/lint/registry/tests.rs` enforces kebab-case names and non-empty prose.
- `src/lint/test_support.rs`: fixtures (`lint_fixture!`) and
  `crate::lint_messages!`, which assert English `display_message()` text.
- Consumers of `display_message`: `src/lint/test_support.rs:111`,
  `src/lint/engine/tests.rs:39,45`, `src/lint/example_manifest_tests.rs:18`,
  `src/lint/finding/tests.rs`, and `FindingDiagnostic::new`. Nothing in
  telemetry, suppression, or deduplication reads the message.

### The runner boundary (`src/runner/check/`)

- `mod.rs` (196 lines): orchestrates a run; returns early for `--explain`
  (lines 58-61); `finish` (around 130-148) builds
  `RunnerError::LintThresholdExceeded { findings: report.diagnostics() }`.
- `diagnostics/mod.rs`: `CheckReport` and `FindingDiagnostic`, the `miette`
  projection; `FindingDiagnostic::new` sets `message`, `help`
  (`meta.remediation`), `code`, `url`, and the span `label` (rule name).
  Callers of `diagnostics()`: `json.rs:25`, `text.rs:22`, `check/mod.rs:145`,
  `diagnostics/tests.rs:44,81`.
- `json.rs`: `render_result`; `render_catalogue` and `RuleEntry::new` for
  `--explain --json`.
- `explain.rs`: text `--explain`, reading the prose fields (around lines
  79-112).
- `text.rs`: human rendering; `summary_line` and `truncation_line` localized
  through the global (`CHECK_SUMMARY_*`).
- Snapshots:
  `src/snapshots/check/netsuke__runner__check__tests__{result_document,truncated_result_document,rule_catalogue}.snap`.

### Localization

- `src/localization/mod.rs`: `define_keys!` (lines 13-22) generates
  `pub const NAME: &str = "dotted.id"` and a table slice; `ALL_KEYS` unions the
  tables (lines 45-78); the global localizer; `LocalizedMessage` (lines
  172-270), whose `render_with` passes the key as the fallback.
- Key tables: `keys.rs` (351 lines, re-exports the others),
  `check_keys.rs`, `clap_error_keys.rs`, `manifest_keys.rs`.
- `src/cli/localization/mod.rs`: `LayeredLocalizer` consults the requested
  catalogue, then `en-US`; `build_localizer(Option<&str>)`; a catalogue that
  fails to parse or register is dropped with a warning, and a failing `en-US`
  falls back to `NoOpLocalizer` (lines 109-168).
  `ortho_config::Localizer::message(id, args, fallback)` returns `lookup` or
  `fallback`; `FluentLocalizer::lookup` returns `None` for a missing message or
  a formatting error, after reporting it.
- `src/locale/catalogues.rs`: 35 embedded catalogues (`include_str!`);
  `en-US` is the source locale.
- Locale resolution: `--locale`, `NETSUKE_LOCALE`, configuration files (via
  `ortho_config`'s merged `Cli.locale`), then the system locale
  (`src/locale/resolution.rs`; `src/main.rs` `configure_runtime`). This task
  adds no flag or configuration key, so `docs/ortho-config-users-guide.md`
  applies only as background.
- `build.rs` calls `build_l10n_audit::audit_localization_keys()` on every
  build and lists `rerun-if-changed` paths (lines 13-15, 212-225).
  `build_l10n_audit/mod.rs` lists `KEY_SOURCES` and checks duplicate
  declarations (lines 156-168); `compare.rs::audit_catalogue` reports `missing`,
  `orphaned`, and `variable_mismatches`; `ftl.rs` extracts ids and `$word`
  variables.
- Tests: `tests/build_l10n_audit_tests.rs` (stages the four key files by
  name at lines 55-61), `build_l10n_audit_rules_tests.rs`,
  `build_l10n_keys_tests.rs`, `build_l10n_parser_tests.rs`;
  `tests/localization_tests.rs`
  (`every_catalogue_parses_without_the_english_fallback` at line 75);
  `tests/locale_catalogue_tests.rs` (render sweep, not-copies, identifiers
  preserved).

### Tests and docs bound to rule prose

- `tests/lint_rule_reference_tests.rs` binds `docs/netsuke-linter-rules.md`
  to the registry (summary cell verbatim; rationale and remediation verbatim in
  each section). It stays bound to the registry here (31.2.3 rebinds it);
  `INV-source-parity` makes registry and `en-US` equal, so the reference
  transitively matches the catalogue.
- `tests/check_command_tests/{explanation,output,policy}.rs` (end to end via
  `assert_cmd`, `--locale en-US`); `src/runner/check/tests.rs`;
  `diagnostics/tests.rs`; `tests/check_command_absent_tests.rs` (builds without
  `lint`).
- Docs: `docs/netsuke-linter-design.md` (sections 3.2, 5.2, 5.4, 9, 11, 12),
  `docs/netsuke-linter-rules.md`, `docs/users-guide.md` ("Lint a manifest with
  netsuke check", around lines 1640-1738), `docs/translators-guide.md` (section
  4 key domains; "Current limitation"), `docs/localization-styleguide.md`,
  `docs/localization-glossary.md`, `docs/developers-guide.md` (lint feature
  gating, around lines 1488-1510).

### Tooling and gates

`make check-fmt`, `make lint`, `make doc-coverage`, `make test` (nextest with
`--all-features`, then doctests), `make test-default-features`,
`make markdownlint` (includes spelling), `make nixie`, and, because
`build_l10n_audit/` and `build.rs` change, `make test-package-verify`. Verus
(`make verus`) and Kani (`make kani-*`) exist but are not used (`D9`).

### Signposts

Documentation: `docs/netsuke-design.md`, `docs/netsuke-linter-design.md`,
`docs/rust-testing-with-rstest-fixtures.md`, `docs/rust-doctest-dry-guide.md`,
`docs/reliable-testing-in-rust-via-dependency-injection.md`,
`docs/ortho-config-users-guide.md`, `docs/rstest-bdd-users-guide.md`,
`docs/translators-guide.md`, `docs/localization-styleguide.md`,
`docs/localization-glossary.md`, `docs/documentation-style-guide.md`. Skills:
`rust-router` (then `rust-types-and-apis` for the template and finding-text
types and `rust-unit-testing` for `rstest` tables), `proptest`,
`hexagonal-architecture` (port and adapter placement), `execplans`,
`en-gb-oxendict` for documentation, and `pr-creation` for the two PRs.

## Conformance basis

Upstream artefacts, inspected at `5c707ef33`:

- `docs/roadmap.md`, task 31.2.1 and the step 31.2 preamble; tasks 31.2.2 to
  31.2.4, 31.4.1, and 31.4.4 as scope boundaries.
- `docs/adrs/adr-0056-manifest-linting-under-netsuke-check.md` (Accepted,
  2026-08-30): "Rule text is not localized yet; rule identifiers never will
  be", the migration paragraph, and "Known risks and limitations".
- `docs/adrs/adr-0069-split-the-localization-key-tables.md`.
- `docs/netsuke-linter-design.md` sections 3.2, 5.2, 5.4, 9.1, 9.2, 12.
- `docs/translators-guide.md`, `docs/localization-styleguide.md`,
  `docs/developers-guide.md` ("Fluent keys stay ungated").
- No Terms of Reference document exists for this phase.

Requirements:

- `REQ-keys`: rule prose is keyed `lint.rule.<name>.summary`, `.rationale`,
  and `.remediation`, following the rule's stable name.
- `REQ-fallback`: a missing or unformattable translation degrades to the
  `en-US` catalogue, then to compiled English; never empty, never a key.
- `REQ-localized`: `netsuke --locale <tag> check` reports a finding's
  message and remediation in that locale.
- `REQ-ident`: `code`, `severity`, rule-name `label`, `url`, spans, order,
  and the `--explain --json` document are byte-identical across locales.
- `REQ-explain-json`: `--explain --json` stays in the source locale.
- `REQ-core-pure`: the lint core has no localization framework dependency.

Gaps:

- `GAP-message`: finding messages and locations are English `String`s.
- `GAP-audit`: the audit forbids catalogue gaps.
- `GAP-normalized`: no check that declared keys stay distinct after
  normalization.
- `GAP-visibility`: nothing reports translation coverage.
- `GAP-adr`: ADR-0056 states localization is deferred.

Trace:

```plaintext
roadmap 31.2.1 -> REQ-keys -> EP-M1 -> lint_rule_keys_are_declared
roadmap 31.2.1 -> REQ-fallback -> EP-M2 -> runner::check::prose::tests::fallback_chain
roadmap 31.2.1 -> REQ-localized -> EP-M2, EP-M3 -> tests/features_lint/check_localization.feature
roadmap 31.2.1 -> REQ-ident -> EP-M4 -> identifiers_are_locale_invariant
ADR-0056 -> REQ-explain-json -> EP-M2 -> rule_catalogue snapshot unchanged
roadmap 31.5.1 -> REQ-core-pure -> EP-M3 -> tests/lint_core_boundary_tests.rs
GAP-audit -> D2 -> EP-M1 -> optional-tier and atomicity audit cases
GAP-normalized -> L-normalized-injective -> EP-M1 -> declared_keys_stay_distinct_after_normalization
GAP-visibility -> D2 -> EP-M1 -> lint_translation_coverage snapshot
GAP-adr -> D10 -> EP-M2 (PR A) -> new ADR and ADR-0056 amendment banner
```

## Verification plan

### Abstract model

Let `R` be the finite set of registered rules, each with a name `n(r)` (a
lowercase kebab-case string, unique in `R`) and English prose `E(r, f)` for `f`
in `{summary, rationale, remediation}`. Let `M` be the finite set of declared
templates (rule messages and shared phrases), each a pair `(key(m), eng(m))`
where `eng(m)` is a string with `{$v}` placeholders. Let `K` be every declared
message key (all tables) and `N(k)` the `ortho_l10n` normalization replacing
every `.` with `-`. A catalogue `C_t` for tag `t` is a partial map from
normalized keys to Fluent patterns; `C_src` is the `en-US` catalogue.
`render(C, k, args)` is `Some(text)` when `C` has `N(k)` and formats without
error, otherwise `None`.

A finding `x` carries `rule(x)`, `location(x)` (a span or a phrase instance),
and a text instance `T(x) = (m, args)` whose arguments are verbatim text,
counts, or phrase instances `(p, plain args)`. `english(i)` interpolates `eng`
of an instance's template with the English of its arguments.

Resolution for tag `t` and fallback `e`:

```plaintext
resolve(t, k, args, e) = render(C_t, k, args)
                         orElse render(C_src, k, args)
                         orElse e
```

Prose: `prose_t(r, f) = resolve(t, key(r, f), {}, E(r, f))` where
`key(r, f) = "lint.rule." ++ n(r) ++ "." ++ f`. Instances render bottom-up:
phrase instances first, then the message with phrase results as text arguments,
each with `english(·)` as its fallback.

Output projection: `ident(x)` is (code, severity, label, url, spans), computed
from `rule(x)`, `severity(x)`, and `location(x)` alone. Order is
`sort(findings, sort_key)` with `sort_key` reading location (spans and
`english` of a location phrase), `n(rule(x))`, and `english(T(x))`.

### Invariants and lemmas

- `INV-ident` (`REQ-ident`): for every finding set and every pair of tags
  `t, u`, the sequence of `ident(x)` under `t` equals that under `u`; the
  `--explain --json` bytes are equal.
- `INV-order`: the output order and the `--limit` truncation set are equal
  under every tag.
- `INV-fallback` (`REQ-fallback`): `resolve` returns the first defined tier;
  its result is non-empty whenever `e` is non-empty and never equals the key.
- `INV-source-parity`: for every `r` and `f`, `render(C_src, key(r, f), {})`
  equals `E(r, f)`; for every template `m` in `M`, rendering `key(m)` through
  `C_src` with a distinct sentinel per placeholder, after removing isolation
  marks, equals `eng(m)` interpolated with the same sentinels. No `en-US` lint
  value contains a line feed.
- `INV-format-clean`: for every catalogue `C_t` and every lint key it
  contains, rendering with one sentinel per `en-US` variable produces no
  formatting issue and contains every sentinel.
- `INV-verbatim-spans`: every backtick-delimited span in an `en-US` lint
  value appears unchanged in every catalogue entry for the same key.
- `INV-key-binding` (`REQ-keys`): the declared `lint.` key set equals
  `{key(r, f)}` over `R` and the prose fields, together with `{key(m)}` over
  `M`; every rule template key starts with `"lint.rule." ++ n(r) ++ "."` for
  the rule that lists it; every finding's template is listed by its rule (or is
  a shared phrase).
- `INV-audit-tier`: the audit fails when `C_src` lacks any declared key, when
  any catalogue contains an undeclared key, when a present key's variables
  differ from `C_src`, or when a catalogue holds some but not all keys of a
  group; it succeeds when a non-source catalogue lacks whole groups of `lint.`
  keys.
- `INV-interp`: the core interpolator replaces each `{$v}` with the value of
  argument `v` exactly once, copies all other text unchanged, and never
  re-reads text taken from an argument value.
- `INV-spelling`: registry prose and templates contain none of the listed
  en-GB-only stems.
- `L-normalized-injective`: `N` is injective on `K`. Proof sketch: the audit
  requires `C_src` to contain every key in `K` (and no undeclared key). If
  `N(k1) = N(k2)` for `k1 ≠ k2`, loading `C_src` registers two messages with
  one identifier, which `fluent-bundle` `add_resource` rejects, so `C_src`
  fails to build; `every_catalogue_parses_without_the_english_fallback`
  (`tests/localization_tests.rs:75`) builds `C_src` successfully; hence no such
  pair exists. The dedicated test states the property directly so that a
  failure names the colliding keys instead of reporting a registration error.
  The `D7` naming rule (dot-free unique rule names, a head word after the name,
  hyphen-free variant segments that are not head words) excludes collisions
  such as `lint.rule.a.message.summary` with `lint.rule.a-message.summary`, and
  `message.no-rule` with `message.no.rule`, before they are written.
- `L-ident-independent`: `ident(x)` is computed without a localizer. Shown
  structurally (those projection functions take no localizer) and tested by
  `INV-ident`.
- `L-order-independent`: `sort_key` reads no localized value. Shown
  structurally by `REQ-core-pure` and tested by `INV-order`.

### Axioms (trusted, not verified here)

- `AXIOM-fluent-lookup`: `ortho_l10n::FluentLocalizer::lookup` returns
  `Some` only for a present message that formats without error, tries the
  dotted then the normalized identifier, inserts isolation marks around
  placeables, and reports formatting issues to the configured reporter
  (`ortho_l10n` 0.10.0, `src/lib.rs:236-308`). Exercised through the real
  catalogues in `EP-M2` to `EP-M4`.
- `AXIOM-fluent-duplicates`: `fluent-bundle` 0.16 `add_resource` reports a
  duplicate message identifier. Used by `L-normalized-injective`; exercised by
  a negative control in `EP-M1`.
- `AXIOM-fluent-syntax`: an unescaped `}` in a pattern is a parse error
  (`fluent-syntax-0.12.0/src/parser/pattern.rs:177-179`), and multiline values
  keep line feeds. Exercised by the parse-all test and the no-newline assertion.
- `AXIOM-layering`: `crate::cli_localization::build_localizer(Some(t))`
  layers `C_t` over `C_src`. Repository-owned; exercised directly.
- `AXIOM-locale-plumbing`: `configure_runtime` installs the localizer for the
  merged locale before `runner::run`. Existing behaviour, exercised end to end
  by the BDD feature.

### Methods per obligation

- Obligation: `INV-audit-tier`. Method: `rstest` table over
  `audit_catalogue` inputs (source vs other locale; required vs `lint.` key;
  present, absent, orphaned, variable mismatch; whole group vs partial group).
  Rationale: a finite partition. Artefact: `build_l10n_audit` unit tests and
  `tests/build_l10n_audit_rules_tests.rs`. Evidence: the "non-source lacks a
  whole lint group" case fails before the tier exists. Non-vacuity: cases that
  must still fail (`en-US` lacking a lint key; an orphan in `de`; `de` carrying
  a rule's summary but not its remediation).
- Obligation: translation coverage visibility (`GAP-visibility`). Method:
  `insta` snapshot of the ledger (one line per locale: translated rules, total
  rules, phrase group present). Artefact: `tests/locale_catalogue_tests.rs` (or
  a sibling), snapshot `lint_translation_coverage`. Evidence: shows `de`
  complete after `EP-M2` (prose) and `EP-M3` (messages) and every other
  non-English locale at zero. Non-vacuity: the snapshot changes when a rule
  group is added to `de`.
- Obligation: `L-normalized-injective`. Method: exhaustive test over
  `ALL_KEYS`. Artefact: `src/localization/mod.rs` tests,
  `declared_keys_stay_distinct_after_normalization`. Non-vacuity: a negative
  control feeds the helper the two colliding examples above and expects both
  pairs reported; a second control builds a two-message resource through
  `FluentLocalizerBuilder` and expects a registration error.
- Obligation: `INV-key-binding`. Method: exhaustive test over the registry,
  `RuleMeta::messages`, `phrases::ALL`, and the declared table; plus an
  assertion in `FindingSink` tests that a template not listed by the running
  rule is rejected. Artefact: `lint_rule_keys_are_declared` (lint-gated).
  Non-vacuity: removing one declared key, or listing another rule's template,
  fails it (seeded fault, reverted).
- Obligation: `INV-source-parity`. Method: exhaustive over `R` × three
  fields (byte equality) and over `M` with sentinel arguments, reading `C_src`
  through a `FluentLocalizer` built from the embedded `en-US` text only, plus a
  no-line-feed assertion. Artefacts: `en_us_prose_matches_the_registry`,
  `en_us_templates_match_the_core`. Non-vacuity: altering one character of one
  `en-US` entry, or of one template, fails it (seeded faults, reverted).
- Obligation: `INV-format-clean`. Method: for each of the 35 catalogues,
  build a localizer with defaults disabled and an issue-collecting reporter
  (`with_error_reporter`), render every lint key the catalogue holds with
  sentinels for the `en-US` variables, and assert zero issues and every
  sentinel present. About 35 × 110 lookups; milliseconds. Artefact:
  `every_present_lint_entry_formats_cleanly`. Non-vacuity: a `de` entry with
  `$name` as literal text, and one referencing an undefined term, each fail it
  (seeded faults, reverted).
- Obligation: `INV-verbatim-spans`. Method: for every catalogue and every
  lint key it holds, every backticked span of the `en-US` value appears in the
  translation. Artefact: `lint_entries_keep_identifiers_verbatim`, extending
  the spirit of `catalogues_preserve_netsuke_identifiers`. Non-vacuity:
  translating `` `order_only_deps` `` in a `de` entry fails it.
- Obligation: `INV-fallback`. Method: `rstest` table over a handwritten stub
  `Localizer` layered as `LayeredLocalizer` does: tier states {present, absent,
  format-error} for the requested and source tiers (nine cases), checking the
  returned text and that the key id is never returned; plus two real-catalogue
  cases (`ja`, which has no lint entries, returns the `en-US` text; a key
  absent everywhere returns the compiled English). Artefact:
  `src/runner/check/prose/tests.rs`. Non-vacuity: a mutant passing the key as
  the fallback (as `LocalizedMessage::render_with` does) fails the "absent
  everywhere" case.
- Obligation: `INV-interp`. Method: `proptest` on the core interpolator over
  generated templates (literal text including `{`, `}`, `{{`, `$`, and
  `{$`-like fragments that are not placeholders) and argument values (including
  `{$v}`, braces, backticks, newlines, and right-to-left text). Oracle: an
  independent reference that splits the template into literal and placeholder
  segments with a simple scanner written in the test, then concatenates.
  Classes recorded with `prop_assert!` counts in `Artefacts and notes`.
  Artefact: `src/lint/finding/text/proptests.rs`. Non-vacuity: a mutant that
  substitutes repeatedly (re-reading values) fails on a value containing `{$v}`.
- Obligation: `INV-ident` and `INV-order`. Method: in-process exhaustive
  sweep over all 35 tags × one fixture per rule (from `lint_fixture!`): lint
  each fixture once, then render the JSON result 35 times through
  `json::render_result` with an injected `build_localizer(Some(tag))`,
  comparing the identifier projection and order with `en-US`, and comparing
  `--explain --json` bytes. Artefact: `src/runner/check/locale_sweep_tests.rs`
  (name provisional). Non-vacuity: under `de`, at least one `message` and one
  `help` differ from `en-US`; a seeded fault routing `code` through the
  localizer fails it.
- Obligation: `REQ-core-pure`. Method: boundary test scanning `src/lint/**`
  for forbidden paths, reusing the approach of
  `tests/ninja_gen_hashmap_boundary.rs`. Artefact:
  `tests/lint_core_boundary_tests.rs`. Non-vacuity: the matcher is unit-tested
  against sample lines naming `ortho_config` (rejected) and
  `crate::localization::lint_keys` (accepted).
- Obligation: `INV-spelling`. Method: exhaustive scan of registry prose and
  templates. Artefact: `rule_prose_is_spelling_neutral`. Non-vacuity: a sample
  containing "behaviour" is rejected by the matcher.
- Obligation: `REQ-localized`, `REQ-explain-json` end to end. Method: BDD
  scenarios through the real binary (below) and `assert_cmd` cases in
  `tests/check_command_tests/`. Oracle: expected text read from
  `locales/<tag>/messages.ftl` and rendered by a fresh `FluentLocalizer`
  (independent of the runner), isolation marks stripped on both sides.

### Formal verification decision

No Verus proof and no Kani harness are planned (`D9`). The candidate properties
are `L-normalized-injective`, `INV-fallback`, `INV-ident`, and `INV-interp`.
The first three range over domains closed at compile time — 24 rules, a
declared key table, 35 embedded catalogues, three fallback tiers — and are
enumerated completely by the tests above on every `make test`, which is a
decision procedure for the shipped artefact. A Verus proof of
`L-normalized-injective` would cover keys not yet written, but any such key
enters the same exhaustive test before it can merge, so the proof would add no
assurance the gate does not already give, while leaving the real risk (the
Fluent adapter and catalogue contents) outside its model. `INV-interp` ranges
over an unbounded string domain, but the interpolator is a single-pass scanner
of about 30 lines whose correctness property is local; a property test against
an independent oracle gives proportionate assurance, and a Verus model of Rust
`String` scanning would verify a re-implementation rather than the code. Kani
is unsuited to `String`-heavy code that calls `fluent-bundle`. The proof sketch
is still recorded so the naming rule it implies is documented in the ADR and
design document. Escalate if implementation reveals an obligation that neither
an exhaustive test nor a property test can discharge.

### Suggested BDD features

Path: `tests/features_lint/check_localization.feature`, collected by
`#[cfg(feature = "lint")] scenarios!("tests/features_lint", fixtures = [world:
TestWorld]);`
in `tests/bdd_tests.rs`. Steps reuse the existing
`netsuke is run with arguments` and stream assertions; new steps live in
`tests/bdd/steps/check_localization.rs`, gated on `lint`. Each scenario maps to
requirements and milestones as noted.

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
    And the first check finding message should be the "de" rendering of its message key
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

The fallback and catalogue scenarios are added in `EP-M2` (PR A); the message
scenarios in `EP-M3` and `EP-M4`. Step wording adapts to the existing step
library; the inputs and observable outcomes are fixed here.

## Plan of work

### `EP-M1` key table, audit optional tier, and source entries

Assigned: `REQ-keys`, `GAP-audit`, `GAP-normalized`, `GAP-visibility`, `D2`,
`D5`.

1. Red: add the audit tier and atomicity cases, the coverage ledger
   snapshot test, and the normalization test with controls. Run them; the tier
   cases fail.
2. Green, audit: in `build_l10n_audit/mod.rs`, read
   `src/localization/lint_keys.rs` through the same `KEY_SOURCES` loop (so
   duplicate detection covers it), and classify declared keys by the `lint.`
   prefix; assert every `lint.` key comes from a lint key file and nothing else
   does. In `compare.rs::audit_catalogue`, take a
   `DeclaredKeys { required, optional }`; `missing` is
   `(required ∪ optional) − present` for `en-US` and `required − present` for
   other tags, plus any partially present group; `orphaned` uses the union.
   Update the module doc comment (lines 3-11), `build.rs` `rerun-if-changed`
   (lines 13-15 and 212-225), and the staged list in
   `tests/build_l10n_audit_tests.rs:55-61`.
3. Green, keys: add `src/localization/lint_keys.rs` with one `define_keys!`
   block holding the 72 prose keys; add it to `ALL_KEYS` and the `keys.rs`
   re-exports.
4. Add the 72 `en-US` entries under a `## Lint rules` group comment in
   `locales/en-US/messages.ftl`: one `##` comment per rule naming the rule,
   single-line values copied byte-for-byte from the registry, literal braces as
   Fluent string literals, and a `#` comment where the text contains
   identifiers that must stay verbatim.
5. Reword "behaviour" and "catalogue" in the registry and the matching
   sentences in `docs/netsuke-linter-rules.md` (`D5`).
6. Add `lint_rule_keys_are_declared`, `en_us_prose_matches_the_registry`,
   `rule_prose_is_spelling_neutral`,
   `every_present_lint_entry_formats_cleanly`, and
   `lint_entries_keep_identifiers_verbatim`.
7. Run the focused tests, then the milestone gates.

End state: catalogues and audit accept lint keys; nothing renders them yet.

### `EP-M2` rule prose localized at the runner boundary (end of PR A)

Assigned: `REQ-fallback`, `REQ-explain-json`, `D3` (prose), `D6`, `D10`, `D11`.

1. Red: `src/runner/check/prose/tests.rs` with the `INV-fallback` table, and
   a `de` case expecting German remediation; the module does not exist, so the
   red is a compile failure, recorded as such. Add the BDD fallback and
   catalogue scenarios with the `cfg`-gated `scenarios!` (red: steps missing).
2. Green: add `src/runner/check/prose/mod.rs` with `RuleProse`, `ProseField`,
   `prose_key`, and `rule_prose(meta, localizer)`. Pass `&dyn Localizer` into
   `CheckReport::diagnostics`, `FindingDiagnostic::new` (for `help`),
   `json::render_result`, `explain::render` (text path only), and
   `text::{summary_line, truncation_line}`; obtain the localizer once in
   `src/runner/check/mod.rs` before the `--explain` branch. Update callers and
   existing tests to pass a localizer explicitly.
3. Add the 72 `de` prose entries following the glossary and style guide,
   keeping manifest identifiers (`order_only_deps`, `{{ outs }}`,
   `dependency_order`, rule names, flags) and literal `$PATH`, `$in`, `$out`
   verbatim.
4. Implement the BDD steps for the fallback and catalogue scenarios.
5. Confirm the `rule_catalogue` snapshot is unchanged; add a `--locale de`
   text case to `tests/check_command_tests/explanation.rs`; update the coverage
   ledger snapshot (`de`: prose complete).
6. Write the new ADR and the ADR-0056 amendment (`D10`), the translators'
   guide section on the `lint.` tier, atomicity, comments, and verbatim
   identifiers, and the developers' guide note on lint keys and the injected
   localizer. Index the ADR in `docs/contents.md`.

End state: remediation and `--explain` prose are localized; messages are still
English. PR A is opened here.

### `EP-M3` finding messages localizable end to end

Assigned: `GAP-message`, `REQ-localized`, `REQ-core-pure`, `D1`, `D3`
(messages), `D7`, `D8`, `D13`, `D16`.

1. Inventory: list every `format!` message site, label helper, location
   string, and English parameter under `src/lint/rules/**` with its target
   template or phrase and variant name, and record the table in
   `Artefacts and notes` before converting anything.
2. Red: add the template parity test, `INV-interp` proptest, the boundary
   test, and a `de` end-to-end case expecting a German message; they fail
   (missing types and keys).
3. Green, core: add `src/lint/finding/text.rs` (see `Interfaces`) with
   `Template`, `FindingText`, `FindingArg`, `Phrase`, `PlainArg`, and the
   interpolator; add `crate::lint::finding::phrases` with the shared subject
   phrases and `ALL`. Change `Finding.message` to `Finding.text`,
   `Location::Symbol(String)` to `Location::Symbol(Phrase)`, and
   `FindingSink::{at, detached, at_or_detached}` to take `FindingText` and
   `Phrase`. `sort_key` reads `english()` of the text and location;
   `display_message` composes `"{location}: {message}"` in English.
4. Green, rules: add `RuleMeta::messages` and convert each rule's sites to
   templates declared beside its `RuleMeta`; convert label helpers to phrases;
   convert the nine `bashism` advice strings to phrase templates under
   `lint.rule.bashism.advice.<construct>`; convert caching `kind` and
   descriptions `consequence` to message variants.
5. Green, keys and catalogues: declare the template keys in
   `src/localization/lint_keys.rs` (or a second table file listed in
   `KEY_SOURCES` if the first nears 400 lines); add `en-US` and `de` entries
   with a `#` comment per message naming each variable and its meaning.
6. Green, adapter: add `src/runner/check/l10n.rs` with
   `render_text(text, localizer)` and `render_phrase(phrase, localizer)`; use
   them in `FindingDiagnostic::new` for `message`, composing the detached
   prefix in the adapter (`D13`).
7. Inflection probe (prototype, deleted): draft Polish translations of the
   three hardest message families (an item-label message, the detached-location
   form, and `serial-order-without-deps`) in a scratch file outside the
   repository, and judge whether phrases allow acceptable grammar. Record the
   drafts and verdict in `Artefacts and notes`. If unacceptable, stop and
   escalate with the `D8` contingency.
8. Update the `result_document` and `truncated_result_document` snapshots
   and add one snapshot with a detached finding; review that differences are
   only isolation marks. Update English string assertions that cross a
   placeable to normalize isolation marks. Update the coverage ledger.
9. Refactor: remove now-unused `String` constructors; keep
   `crate::lint_messages!` asserting English.

End state: every finding message, location, and remediation is localized; `de`
complete.

### `EP-M4` cross-locale contract sweep and BDD feature

Assigned: `REQ-ident`, `INV-order`.

1. Red: add the 35-locale sweep with its non-vacuity assertion and the
   remaining BDD scenarios; steps missing until step 2.
2. Green: implement the steps using the existing workspace fixtures and the
   `manual-ninja-escape` reported manifest from `docs/netsuke-linter-rules.md`.
3. Run every seeded fault listed in the methods, record transcripts, revert.

End state: all verification obligations discharged.

### `EP-M5` documentation and delivery closure (end of PR B)

Assigned: `GAP-adr` (completion), `D10`.

1. `docs/netsuke-linter-design.md`: replace "Rule text is not localized yet"
   in section 12; describe templates, phrases, and `RuleMeta::messages` in 5.2
   and 5.4, localized output and isolation marks in 9.1 and 9.2, and the new
   tests in 11; reference the new ADR.
2. `docs/translators-guide.md`: add `lint.*` to the key-domain table;
   explain phrases and their inflection limits, the right-to-left mark (U+200F)
   for messages starting with a phrase, and correct the "Current limitation"
   passage (lines 309-318).
3. `docs/developers-guide.md`: "Adding or changing a lint rule's text":
   declare templates beside the rule, list them in `RuleMeta::messages`, add
   `en-US` entries, spelling-neutral prose, the key naming rule.
4. `docs/users-guide.md` `netsuke check` section: findings follow
   `--locale`; identifiers and `--explain --json` do not; untranslated rules
   fall back to English; interpolated values carry isolation marks, including
   the doubled marks around item labels. Prose only; any fenced example gets a
   `tested-example` marker and an `EXPECTED_EXAMPLE_IDS` entry.
5. `CHANGELOG.md` entry; update the `RuleMeta` doc comment; add an
   implementation note to the new ADR if anything moved; mark roadmap 31.2.1
   done; set this plan to `COMPLETE` after reconciliation. Search for stale
   claims with
   `rg -n "not localized yet|English-only" docs --glob '!docs/execplans/**'`.

## Milestones and plateaus

- `EP-M1`: catalogues and audit accept the `lint.` tier; `en-US` carries the
  prose; coverage is visible. Requirements: `REQ-keys`, `GAP-audit`,
  `GAP-normalized`, `GAP-visibility`. Evidence: audit tier and atomicity cases,
  normalization test, prose parity, format-clean, verbatim-span, spelling, and
  parse-all tests. Conformance: no output change; no public interface change
  outside `build_l10n_audit` and `crate::localization`; audit change matches
  `D2`. Recovery: revert the milestone commits. Remaining: nothing rendered.
  Compatibility: none.
- `EP-M2`: prose localized; `de` prose present; ADR written. Requirements:
  `REQ-fallback`, `REQ-explain-json`. Evidence: fallback table, `de`
  explanation case, BDD fallback and catalogue scenarios, unchanged
  `rule_catalogue` snapshot. Conformance: core untouched; adapters take an
  injected localizer. Recovery: revert; `EP-M1` remains coherent. Remaining:
  messages English. Compatibility: none (internal adapters).
- `EP-M3`: messages and locations localized. Requirements: `GAP-message`,
  `REQ-localized`, `REQ-core-pure`. Evidence: template parity, interpolator
  property test, boundary test, updated snapshots differing only by isolation
  marks, inflection probe verdict. Conformance: core framework-free; JSON field
  set unchanged. Recovery: revert; `EP-M2` remains coherent. Compatibility: none
  (`Finding`, `FindingSink`, and `RuleMeta` are crate-internal pre-1.0 APIs
  updated with all callers).
- `EP-M4`: contracts proven across locales. Requirements: `REQ-ident`.
  Evidence: sweep and BDD. Recovery: tests only.
- `EP-M5`: documentation reconciled; roadmap ticked. Evidence: Markdown
  gates and the stale-claim search.

## Concrete steps

Working directory: the repository root of this worktree. Every gate pipes
through `tee` with `set -o pipefail`, logging to
`/tmp/$ACTION-netsuke-31-2-1-move-linter-messages-into-l10n-catalogues.out`.

Focused commands (red, then green):

```plaintext
EP-M1  cargo nextest run --all-features -E 'test(/audit|normaliz|lint_rule_keys|en_us_prose|formats_cleanly|verbatim|spelling|coverage/)'
EP-M2  cargo nextest run --all-features -E 'test(/prose|explanation|every_catalogue_parses/)'
EP-M3  cargo nextest run --all-features -E 'test(/template|interp|boundary|result_document|check_command/)'
EP-M4  cargo nextest run --all-features -E 'test(/locale_sweep/)' && \
       cargo nextest run --all-features --test bdd_tests -E 'test(/localized_lint/)'
```

Milestone gates, sequentially, delegated to `scrutineer`:

```sh
set -o pipefail
B=31-2-1-move-linter-messages-into-l10n-catalogues
make check-fmt             2>&1 | tee "/tmp/check-fmt-netsuke-$B.out"
make lint                  2>&1 | tee "/tmp/lint-netsuke-$B.out"
make doc-coverage          2>&1 | tee "/tmp/doc-coverage-netsuke-$B.out"
make test                  2>&1 | tee "/tmp/test-netsuke-$B.out"
make test-default-features 2>&1 | tee "/tmp/test-default-features-netsuke-$B.out"
make test-package-verify   2>&1 | tee "/tmp/test-package-verify-netsuke-$B.out"
make markdownlint          2>&1 | tee "/tmp/markdownlint-netsuke-$B.out"
make nixie                 2>&1 | tee "/tmp/nixie-netsuke-$B.out"
```

`make test-package-verify` is required for `EP-M1` (audit and `build.rs`
change) and at each PR's close; the others after every milestone. Commit after
each green milestone with a message naming the milestone.

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
- `make test` passes; each new test fails before its milestone's change
  (recorded red evidence) and passes after.

Acceptance after `EP-M2` (PR A): the remediation, `--explain` text, fallback,
and catalogue criteria above hold; messages are still English.

Quality criteria: every gate in `Concrete steps` passes; every obligation in the
`Verification plan` is discharged with its non-vacuity evidence recorded in
`Artefacts and notes`; no lint suppression added; no file over 400 lines.

## Idempotence and recovery

All steps are re-runnable. Seeded faults are applied only after the preceding
commit, then reverted with `git checkout -- <file>`; confirm with
`git status --short` that nothing remains. Proptest regression files created by
a seeded fault are deleted. The inflection probe lives outside the repository
and is deleted after its verdict is recorded. If a milestone must be abandoned,
`git revert` its commits; each earlier milestone is a coherent plateau.

## Artefacts and notes

Reconnaissance evidence (at `5c707ef33`):

```plaintext
$ grep -c '' locales/en-US/messages.ftl
506
$ sed -n 465p locales/en-US/messages.ftl
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}
$ grep -rn behaviour src/lint/rules
src/lint/rules/redundancy/duplication/mod.rs:99: "... hides which behaviour the author wanted."
src/lint/rules/migration/mod.rs:1://! Rules that detect workarounds for behaviour a release has since changed.
```

Design review (2026-11-15), three panels. Verdict: proceed with conditions. The
conditions adopted were: localize detached locations as phrases; carve
`lint_keys` out of the boundary test; correct the Fluent authoring rules; make
parity exhaustive through static templates; add per-rule atomicity, a coverage
ledger, a sentinel-format check, and a verbatim-span check; split delivery into
two PRs. Rejected or deferred proposals: messages rendered only from the
catalogue (offered as the `D7` alternative); a `List` argument kind (`D8`
accepts the separator limitation); a once-per-key warning reporter (`D15`).

Implementation transcripts (red, green, seeded faults, proptest class counts,
the message inventory, and the inflection probe) are appended here per
milestone.

## Interfaces and dependencies

No new dependencies. Signatures are targets; names may be refined during
implementation if the `Decision log` records why.

In `src/lint/finding/text.rs` (framework-free core):

```rust
/// A message form: its catalogue key and its English source pattern.
#[derive(Debug)]
pub struct Template {
    /// Fluent key, a constant from `crate::localization::lint_keys`.
    pub key: &'static str,
    /// English with `{$name}` placeholders.
    pub english: &'static str,
}

/// The localizable text of one finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingText {
    template: &'static Template,
    args: Vec<(&'static str, FindingArg)>,
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
    template: &'static Template,
    args: Vec<(&'static str, PlainArg)>,
}

/// A phrase argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlainArg {
    Verbatim(String),
    Count(usize),
}
```

with builders `FindingText::new(&TEMPLATE)`, `.verbatim(name, value)`,
`.count(name, n)`, `.phrase(name, phrase)` (and the same minus `phrase` on
`Phrase`), accessors `template()` and `args()`, and `english() -> String` using
the interpolator
`fn interpolate(pattern: &str, lookup: impl Fn(&str)
-> Option<String>) -> String`.
`RuleMeta` gains `pub messages: &'static [&'static Template]`, a
non-identifier field.

In `src/runner/check/prose/mod.rs`:

```rust
pub(super) enum ProseField { Summary, Rationale, Remediation }
pub(super) struct RuleProse { pub(super) summary: String, pub(super) rationale: String, pub(super) remediation: String }
pub(super) fn prose_key(name: &str, field: ProseField) -> String;
pub(super) fn rule_prose(meta: &RuleMeta, localizer: &dyn Localizer) -> RuleProse;
```

In `src/runner/check/l10n.rs`:

```rust
pub(super) fn render_text(text: &FindingText, localizer: &dyn Localizer) -> String;
pub(super) fn render_phrase(phrase: &Phrase, localizer: &dyn Localizer) -> String;
```

Both call `Localizer::message(id, args, fallback)` with the compiled English as
`fallback`, relying on `LayeredLocalizer` for the `en-US` tier.

Changed signatures:
`CheckReport::diagnostics(&self, localizer: &dyn Localizer)`,
`json::render_result(report, localizer)`, `explain::render(…, localizer)`,
`text::summary_line(report, localizer)`, `text::truncation_line(…, localizer)`,
`FindingSink::at(span, FindingText)`,
`FindingSink::detached(Phrase, FindingText)`,
`FindingSink::at_or_detached(Option<Span>, Phrase, FindingText)`. All callers
change together; no compatibility layer (pre-1.0, crate-internal).

In `build_l10n_audit`: a
`DeclaredKeys { required: BTreeSet<String>, optional: BTreeSet<String> }` built
from one `KEY_SOURCES` loop, passed to `audit_catalogue`, plus a group function
mapping a `lint.` key to its group (`lint.rule.<name>` or `lint.subject`).

Reuse policy: `Template` and `FindingText` are the only way a rule reports text;
`src/runner/check/l10n.rs` and `prose` are the only renderers of lint keys;
`prose_key` is the single place the prose key grammar is spelt. The optional
tier is reserved for the `lint.` prefix; extending it to another prefix
requires an ADR amendment.

## Revision note

- 2026-11-15: initial draft from Wyvern reconnaissance of the lint prose
  flow, localization infrastructure, and docs and tests at `5c707ef33`.
- 2026-11-15: applied the community-of-experts review. Detached locations
  and English parameters become phrases or variants; `D13` composes the
  detached prefix in the adapter; the boundary test carves out `lint_keys`;
  Fluent authoring rules corrected (single-line values, both braces escaped);
  `D7` replaced per-site English with static templates, making parity
  exhaustive and removing the corpus; added per-rule atomicity, a coverage
  ledger, sentinel-format, verbatim-span, and spelling tests; `D6` extended to
  the summary lines; `D14` splits delivery into two PRs; `D15` and `D16` added;
  budgets split per PR and raised from a 1,600-line to an 1,800-line code
  estimate; citations corrected (`tests/localization_tests.rs:75`); Purpose now
  states that the user's goal requires 31.2.4 before 31.4.4. Milestone order is
  unchanged; the ADR moves to `EP-M2`.

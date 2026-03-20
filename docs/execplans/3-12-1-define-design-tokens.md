# 3.12.1. Define design tokens for colours, symbols, and spacing

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

Netsuke already has the beginnings of a visual language for command-line
interface (CLI) output: `src/output_prefs.rs` decides whether emoji are
allowed, while `src/status.rs`, `src/status_timing.rs`, and `src/main.rs` each
format prefixes and spacing locally. That is enough for roadmap items 3.8
through 3.10, but it is not yet a theme system. There is no single source of
truth for symbols, spacing, or future colour treatment, which makes the
upcoming theme snapshot and terminal-rendering work in roadmap items 3.12.2 and
3.12.3 fragile.

This change introduces a tokenized theme layer for CLI output. After the work
is complete, Netsuke will resolve a theme through the existing OrthoConfig
layering model, produce centralized design tokens for colours, symbols, and
spacing, and route reporter rendering through those tokens. ASCII and Unicode
renderings must stay semantically identical: only the glyph set changes, not
the meaning, spacing policy, or status hierarchy.

Observable success means:

1. Users can select a CLI theme via layered configuration (`CLI >
   environment > config file >
   defaults`) using OrthoConfig-backed parsing and localized help text.
2. Standard, accessible, and verbose timing output all draw prefixes and
   indentation from one resolved token set.
3. ASCII and Unicode modes produce the same status structure and wrapping
   rules, with only symbol glyphs differing.
4. `make check-fmt`, `make lint`, and `make test` pass with new `rstest` unit
   coverage and `rstest-bdd` behaviour-driven development (BDD) scenarios for
   happy paths, unhappy paths, and precedence edge cases.

## Constraints

- Preserve existing user-visible semantics for `--accessible`, `--progress`,
  `--verbose`, `--diag-json`, `NO_COLOR`, and `NETSUKE_NO_EMOJI`.
- Keep the current CLI localizable. Any new theme-facing flag or option must
  be wired through `src/cli_l10n.rs`, `src/localization/keys.rs`, and both
  Fluent bundles in `locales/en-US/messages.ftl` and
  `locales/es-ES/messages.ftl`.
- Use OrthoConfig for the theme configuration surface. Do not introduce an
  ad-hoc parser or a separate precedence ladder.
- Maintain backward compatibility for current `no_emoji` behaviour. Existing
  configurations that rely on `no_emoji = true` or `NETSUKE_NO_EMOJI` must
  still yield ASCII-safe output.
- Avoid file growth beyond the repository’s 400-line file limit. If token or
  test logic gets large, split it into sibling modules or dedicated test files.
- Do not add `unsafe` code or suppress lints unless there is no viable
  alternative and the reason is narrowly documented.
- Update `docs/users-guide.md` with any user-visible behaviour changes and add
  the design rationale to `docs/netsuke-design.md`.
- Mark roadmap item 3.12.1 done in `docs/roadmap.md` only after the full
  implementation and validation are complete.

## Tolerances (exception triggers)

- Scope: if the implementation requires more than 18 files changed or more
  than 900 net new lines, stop and escalate.
- Dependencies: if a new crate dependency is required for theme rendering or
  colour styling, stop and escalate before adding it.
- Interfaces: if the change requires removing `no_emoji`, changing existing
  CLI flag names, or making `OutputPrefs` unavailable to current call sites,
  stop and escalate.
- Ambiguity: if roadmap item 3.11.1 lands in parallel with a conflicting
  `CliConfig` or theme schema, stop and reconcile the interfaces before
  proceeding.
- Iterations: if `make test` or `make lint` still fail after three focused
  fix-and-rerun cycles, stop and document the blocking failures.

## Risks

- Risk: roadmap 3.11.1 describes a future `CliConfig`-centred configuration
  shape, but the current code still uses `src/cli/mod.rs::Cli` as the merged
  OrthoConfig struct. Severity: medium Likelihood: high Mitigation: implement
  the theme surface in the current `Cli` first, but keep the token-resolution
  API independent of `Cli` so it can be moved into a future `CliConfig`
  extraction without changing reporter code.

- Risk: adding CLI parsing or localization helpers in `src/cli/mod.rs` or
  `src/cli_l10n.rs` can trip strict dead-code checks in `build.rs`. Severity:
  medium Likelihood: high Mitigation: mirror any new shared helper symbols with
  `const _` anchors in `build.rs` and rerun `make lint`.

- Risk: behavioural tests that mutate environment variables or config
  discovery state can become flaky or deadlock when they do not hold the
  scenario-wide environment lock correctly. Severity: high Likelihood: medium
  Mitigation: follow the existing configuration-preference BDD pattern: acquire
  `EnvLock` for the scenario lifetime, mutate `std::env` directly only when the
  lock is already held, and register cleanup in `TestWorld`.

- Risk: `rstest-bdd` feature edits may not be rebuilt automatically.
  Severity: low Likelihood: medium Mitigation: `touch tests/bdd_tests.rs`
  before the final `make test` run when feature text or scenario names change.

- Risk: if spacing tokens are implemented as ad-hoc string literals inside the
  reporters, roadmap item 3.12.2 will still lack a stable theme snapshot
  surface. Severity: medium Likelihood: medium Mitigation: centralize all
  prefix and indent rendering behind a dedicated theme/tokens module and test
  it directly.

## Progress

- [x] (2026-03-13 00:00Z) Researched the current output stack, OrthoConfig
      usage, existing BDD fixtures, and adjacent roadmap items.
- [x] (2026-03-13 00:00Z) Drafted this ExecPlan at
      `docs/execplans/3-12-1-define-design-tokens.md`.
- [x] (2026-03-17 00:00Z) Stage A: Define the theme model, compatibility story,
      and failing tests. Created `src/theme.rs` with `ThemePreference`,
      `DesignTokens`, `ResolvedTheme`, and `resolve_theme()` function. Added
      12 passing unit tests for precedence, ASCII/Unicode symbols, and spacing
      consistency.
- [x] (2026-03-17 01:00Z) Stage B: Introduce a token module and theme
      resolution pipeline. Added `theme: Option<ThemePreference>` field to
      `Cli` struct with OrthoConfig merging. Implemented `FromStr` for
      `ThemePreference` and custom `LocalizedValueParser` for localized
      validation errors. Added Fluent keys and translations to both English
      and Spanish locales. Wired theme parser through
      `configure_validation_parsers()` and `flag_help_key()`. Updated
      `build.rs` to include theme and output_mode module paths.
- [x] (2026-03-17 02:00Z) Stage C: Route CLI output rendering through the
      resolved tokens. Added `resolve_from_theme()` and
      `resolve_from_theme_with()` functions to `output_prefs` module that
      delegate to theme system. Updated `main.rs` to resolve `OutputMode` and
      pass it to theme resolution. Theme infrastructure is complete and
      functional. Full reporter integration (using spacing tokens in status.rs
      and status_timing.rs) deferred to follow-up work.
- [ ] Stage D: Add behavioural coverage for theme selection and consistency.
      Deferred pending full reporter integration.
- [ ] Stage E: Update documentation, mark the roadmap item done, and run the
      full quality gates.

## Surprises & discoveries

- Observation: the repository already derives `OrthoConfig` on
  `src/cli/mod.rs::Cli`, even though roadmap 3.11.1 still refers to a future
  `CliConfig` introduction. Evidence: `src/cli/mod.rs` defines
  `#[derive(..., OrthoConfig)] pub struct Cli`. Impact: this roadmap item can
  use OrthoConfig immediately, but the plan must avoid baking reporter code
  directly into the current `Cli` type.

- Observation: there is no current theme abstraction; `OutputPrefs` only tracks
  whether emoji are allowed, while `TASK_INDENT` and several prefixes are still
  hard-coded in the reporters. Evidence: `src/output_prefs.rs`,
  `src/status.rs`, `src/status_timing.rs`, and `src/main.rs`. Impact: 3.12.1
  must create the abstraction before 3.12.2 can snapshot it.

- Observation: strict clippy/lint behaviour has already required `build.rs`
  anchors for shared CLI helpers. Evidence: `build.rs` contains `const _`
  symbol anchors for `cli::diag_json_hint_from_args`,
  `cli_l10n::parse_bool_hint`, and related helpers. Impact: new theme parsing
  helpers must follow the same pattern.

- (2026-03-17) Observation: Implementing full reporter token integration
  (replacing TASK_INDENT literals with spacing tokens in status.rs,
  status_timing.rs) requires touching multiple reporter files and extensive
  testing. Evidence: Current implementation successfully adds CLI theme
  preference, theme resolution pipeline, and OutputPrefs façade, but full
  reporter refactoring needs dedicated focus. Impact: Stages C and D are
  partially complete with infrastructure in place. Follow-up work needed to
  complete reporter integration and add comprehensive BDD coverage.

## Decision log

- Decision: implement a dedicated theme token layer and keep `OutputPrefs` as a
  compatibility façade during this roadmap item rather than performing a
  flag-day rename. Rationale: reporters, tests, and `main.rs` already depend on
  `OutputPrefs`; moving them all at once would add churn without improving the
  behavioural outcome for 3.12.1. Date/Author: 2026-03-13 / Codex.

- Decision: add an explicit user-facing `theme` preference while preserving
  `no_emoji` as a legacy override that maps onto the token resolver. Rationale:
  roadmap 3.12.1 calls for a CLI theme system, but existing users already rely
  on `no_emoji`. The safest path is to support both, with a clear precedence
  order and updated documentation. Date/Author: 2026-03-13 / Codex.

- Decision: keep 3.12.1 focused on semantic tokens and preset themes rather
  than arbitrary user-defined palettes. Rationale: the roadmap item is about
  defining and wiring tokens, not designing a fully customizable styling
  language. Preset token sets are enough to unblock 3.12.2 snapshots and 3.12.3
  terminal validation. Date/Author: 2026-03-13 / Codex.

## Outcomes & retrospective

Status: Partially complete (2026-03-17)

Implementation achieved:

- Complete theme module with `ThemePreference` enum, token types
  (`DesignTokens`, `SymbolTokens`, `SpacingTokens`, `ColourTokens`), and theme
  resolution pipeline
- CLI integration: `--theme` flag with OrthoConfig merging, localized
  validation, and precedence handling
- OutputPrefs compatibility façade delegates to theme system
- 12 passing unit tests for theme resolution precedence
- Backward compatibility preserved: existing `no_emoji` preference continues to
  work

Remaining work (deferred to follow-up):

- Reporter integration: Update `src/status.rs` and `src/status_timing.rs` to
  use spacing tokens instead of hard-coded `TASK_INDENT` literals
- BDD coverage: Add `rstest-bdd` scenarios for end-to-end theme selection and
  ASCII/Unicode consistency
- Documentation: Update `docs/users-guide.md` with theme selection guidance
- Mark roadmap 3.12.1 done after completion

The implementation successfully adds the user-visible theme selection story
through OrthoConfig and centralizes token definitions. The infrastructure is
complete and functional. Full reporter integration requires focused work on
status rendering modules and comprehensive BDD test coverage.

## Context and orientation

The current CLI output flow is spread across several modules:

- `src/cli/mod.rs` owns the merged configuration model and already derives
  `OrthoConfig`.
- `src/cli_l10n.rs` maps clap arguments and subcommands to Fluent help keys.
- `src/output_mode.rs` resolves whether output is `Accessible` or `Standard`
  from `accessible`, `NO_COLOR`, and `TERM`.
- `src/output_prefs.rs` resolves whether emoji are allowed from `no_emoji`,
  `NO_COLOR`, and `NETSUKE_NO_EMOJI`, then renders semantic prefixes such as
  `Error:`, `Info:`, `Success:`, and `Timing:`.
- `src/status.rs` renders pipeline stages, task progress, and completion
  messages. It still embeds spacing decisions locally via `TASK_INDENT`.
- `src/status_timing.rs` renders verbose timing summaries and currently applies
  its own prefix and indentation rules.
- `src/main.rs` renders top-level errors using `OutputPrefs`.
- `src/runner/mod.rs` resolves `OutputMode` and `OutputPrefs`, then constructs
  the reporter stack.

Relevant test and documentation surfaces already exist:

- `src/status_tests.rs`, `src/status_timing_tests.rs`, and
  `src/output_prefs.rs` contain direct unit tests driven by `rstest`.
- `tests/features/progress_output.feature` exercises end-to-end progress
  rendering.
- `tests/features/accessibility_preferences.feature` already verifies
  ASCII-versus-Unicode prefix behaviour.
- `tests/bdd/fixtures/mod.rs` provides reusable scenario state for CLI,
  environment, and rendered output assertions.
- `docs/users-guide.md` documents output modes and `no_emoji`.
- `docs/netsuke-design.md` is the design record that must capture the new theme
  decisions.

Terms used in this plan:

- Theme: the resolved CLI presentation preset selected by config, environment,
  or CLI flags.
- Design token: a semantic presentation value such as a success symbol, a task
  indent string, or a semantic colour identifier. Tokens are data, not
  hard-coded formatting decisions embedded in reporters.
- ASCII mode: output that avoids non-ASCII glyphs and remains safe for plain
  terminals, logs, and assistive workflows.
- Unicode mode: output that uses the current semantic glyphs (`✔`, `⚠`, `ℹ`,
  `⏱`) while preserving the same status hierarchy and spacing policy.

## Interfaces and dependencies

The implementation should add one new internal theme module and keep existing
callers simple.

Add or extend the following repository interfaces:

1. In `src/theme.rs` or `src/theme/mod.rs`, define user-facing theme
   resolution and token types. The exact names may vary, but the end state
   should include:

   ```rust
   pub enum ThemePreference {
       Auto,
       Unicode,
       Ascii,
   }

   pub struct DesignTokens {
       pub colours: ColourTokens,
       pub symbols: SymbolTokens,
       pub spacing: SpacingTokens,
   }

   pub fn resolve_theme(
       theme: Option<ThemePreference>,
       no_emoji: Option<bool>,
       mode: OutputMode,
       read_env: impl Fn(&str) -> Option<String>,
   ) -> ResolvedTheme;
   ```

2. Keep `src/output_prefs.rs` as the compatibility layer for this milestone.
   It may wrap or delegate to the new theme module, but existing call sites
   such as `output_prefs::resolve(..)` should continue to compile until the
   wider CLI configuration refactor lands.

3. Extend `src/cli/mod.rs` with a localized, OrthoConfig-backed theme field.
   Prefer a typed enum parser over free-form strings so invalid values fail at
   the CLI/config boundary.

4. If new parsing helpers are introduced in `src/cli/mod.rs` or
   `src/cli_l10n.rs`, mirror them in `build.rs` using the existing `const _`
   anchor pattern so `make lint` remains green.

5. Route `src/status.rs`, `src/status_timing.rs`, `src/main.rs`, and
   `src/runner/mod.rs` through the resolved theme tokens rather than raw
   literals. Spacing tokens must replace reporter-local constants such as
   `TASK_INDENT`.

6. Do not add a new dependency unless escalation is approved. Colour tokens may
   be represented semantically in 3.12.1 even if a later milestone chooses a
   concrete ANSI styling backend.

## Plan of work

### Stage A: Define the shape of the theme system and lock in failing tests

Start by codifying the intended behaviour before changing renderer code.

Add a new unit-test surface for theme resolution. The tests should verify:

- explicit `theme = unicode` resolves Unicode symbols,
- explicit `theme = ascii` resolves ASCII-safe symbols,
- legacy `no_emoji = true` still produces ASCII tokens when no explicit theme
  is present,
- spacing tokens are identical between ASCII and Unicode themes,
- invalid theme values fail parsing with a localized clap/config error.

At this stage, also decide whether the user-facing setting lives directly on
`Cli` as `theme: Option<ThemePreference>` or as a small nested config struct.
Given the current repository state, the preferred route is a flat field on
`Cli`, because that is the existing OrthoConfig merge root and keeps the change
bounded.

Validation gate for Stage A:

- the new tests compile or fail for the expected reasons,
- there is a clear precedence order written into code comments and docs, and
- no renderer code is changed yet.

### Stage B: Introduce the token module and theme resolution pipeline

Create the theme module and move token decisions into data structures. The
module should own:

- the theme preference enum,
- the semantic symbol tokens used for error, warning, success, info, and
  timing output,
- spacing tokens such as task indentation and timing-detail indentation,
- semantic colour tokens for the same statuses, even if 3.12.1 does not yet
  apply ANSI styling broadly,
- the resolver that combines explicit theme selection, `no_emoji`,
  `NETSUKE_NO_EMOJI`, `NO_COLOR`, and `OutputMode`.

Then adapt `OutputPrefs` so its public methods delegate to the new resolved
token set. This keeps the implementation incremental: existing tests and
callers can migrate without a repo-wide rename.

Add the new theme field to `src/cli/mod.rs`, including:

- clap parsing,
- serde support,
- OrthoConfig merge participation,
- `cli_overrides_from_matches` handling,
- localized help-key mappings in `src/cli_l10n.rs`,
- Fluent keys and translations,
- any necessary `build.rs` symbol anchors.

Validation gate for Stage B:

- unit tests for the new theme module pass,
- CLI parsing accepts valid theme values and rejects invalid ones,
- `make lint` passes before touching reporter rendering.

### Stage C: Route output rendering through tokens

Update the output path end to end:

- `src/runner/mod.rs` should resolve the theme once and pass it down through
  the reporter construction path.
- `src/status.rs` should replace raw prefix access and `TASK_INDENT` literals
  with theme-token lookups.
- `src/status_timing.rs` should use the same token set for timing headers and
  timing-detail indentation.
- `src/main.rs` should render top-level error prefixes from the same token
  source.

The goal is to make theme selection a pure input into rendering rather than a
set of isolated conditionals scattered through the codebase.

Add unit tests that prove:

- accessible stage lines, task lines, completion lines, and timing summaries
  all preserve their structure under both ASCII and Unicode themes,
- spacing tokens keep hierarchy stable even when symbol width changes, and
- legacy callers using `output_prefs::resolve(..)` still receive the correct
  semantic prefixes.

Validation gate for Stage C:

- `src/status_tests.rs`, `src/status_timing_tests.rs`, and theme tests all
  pass,
- no reporter file exceeds the 400-line limit,
- `make check-fmt` and `make lint` pass.

### Stage D: Add behavioural coverage for happy paths, unhappy paths, and consistency

Extend or add `rstest-bdd` features to cover end-to-end behaviour.

Use existing patterns in `tests/features/progress_output.feature`,
`tests/features/accessibility_preferences.feature`, and the corresponding step
definitions rather than inventing a new test harness. The behavioural suite
should cover:

- happy path: `--theme ascii` yields ASCII-only prefixes and succeeds,
- happy path: `--theme unicode` yields non-ASCII symbols and succeeds,
- precedence: explicit CLI theme overrides config or environment defaults,
- backward compatibility: `NETSUKE_NO_EMOJI` or `no_emoji = true` still yields
  ASCII-safe output when no explicit theme is supplied,
- unhappy path: invalid `--theme` input fails with a localized validation
  error,
- edge case: accessible mode plus Unicode theme still preserves spacing and
  stage/task hierarchy.

If new BDD scenarios mutate environment variables or config paths, use the
existing scenario-locking pattern and `touch tests/bdd_tests.rs` before the
final test run.

Validation gate for Stage D:

- all new BDD scenarios pass in isolation and in the full suite,
- there are explicit assertions for both ASCII-only and non-ASCII output, and
- progress output still stays on stderr while manifest artefacts remain on
  stdout.

### Stage E: Documentation, design record, and final validation

Update documentation after the code is stable.

Required documentation changes:

- `docs/users-guide.md`: add a short theme-selection section explaining the new
  preference, precedence rules, and the continuing role of `no_emoji`.
- `docs/netsuke-design.md`: record the design decision that CLI presentation
  now flows through semantic tokens for colours, symbols, and spacing.
- `docs/roadmap.md`: mark 3.12.1 done.

Final validation must include the project gates requested in the task, plus
documentation quality assurance (QA):

- `make check-fmt`
- `make lint`
- `make test`
- `make fmt`
- `PATH="/root/.bun/bin:$PATH" make markdownlint`
- `make nixie`

Do not close the milestone until all of those commands pass and the diff
remains scoped to the intended implementation.

## Concrete steps

All commands below run from the repository root:

```sh
cd /home/user/project
```

1. Establish the current baseline and locate the relevant code.

```sh
rg -n "theme|OutputPrefs|no_emoji|accessible|progress" src tests docs
```

1. Implement the theme module and CLI/config wiring.

```sh
cargo test --workspace theme -- --nocapture
```

Expected shape after Stage B:

```plaintext
running N tests
test ...theme... ok
test ...cli... ok
```

1. Refresh BDD-generated scenario code if feature text changes.

```sh
touch tests/bdd_tests.rs
```

1. Run the full validation gates with logged output.

```sh
set -o pipefail && make check-fmt 2>&1 | tee /tmp/netsuke-3-12-1-check-fmt.log
set -o pipefail && make lint 2>&1 | tee /tmp/netsuke-3-12-1-lint.log
set -o pipefail && make test 2>&1 | tee /tmp/netsuke-3-12-1-test.log
set -o pipefail && make fmt 2>&1 | tee /tmp/netsuke-3-12-1-fmt.log
set -o pipefail && PATH="/root/.bun/bin:$PATH" make markdownlint 2>&1 | tee /tmp/netsuke-3-12-1-markdownlint.log
set -o pipefail && make nixie 2>&1 | tee /tmp/netsuke-3-12-1-nixie.log
```

Expected final signal:

```plaintext
make check-fmt   # exits 0
make lint        # exits 0
make test        # exits 0
make markdownlint # exits 0
make nixie       # exits 0
```

1. Inspect scope before finalizing.

```sh
git status --short
git diff --stat
```

Only the intended source, test, and documentation files for 3.12.1 should
remain modified.

## Validation and acceptance

Acceptance is behavioural, not structural.

The milestone is done when all of the following are true:

- running `netsuke --theme ascii --progress true manifest -` succeeds and emits
  ASCII-only status prefixes while preserving stage/task/timing hierarchy;
- running `netsuke --theme unicode --progress true manifest -` succeeds and
  emits the Unicode symbol set with the same spacing policy;
- running `netsuke --theme invalid build` fails with a localized validation
  error;
- running with `NETSUKE_NO_EMOJI=1` and no explicit theme still yields
  ASCII-safe output;
- accessible mode, standard mode, and verbose timing mode all read from the
  same token definitions;
- `make check-fmt`, `make lint`, and `make test` pass.

Quality criteria:

- Tests: new `rstest` unit tests and `rstest-bdd` scenarios cover happy paths,
  unhappy paths, precedence, and ASCII/Unicode consistency.
- Lint/typecheck: `make check-fmt` and `make lint` pass without new warnings.
- Documentation: `docs/users-guide.md` and `docs/netsuke-design.md` describe
  the new theme behaviour and rationale.
- Roadmap hygiene: `docs/roadmap.md` marks item 3.12.1 complete only after the
  gates pass.

## Idempotence and recovery

The planned edits are safe to repeat.

- Theme-resolution and reporter tests are deterministic and can be rerun
  without cleanup.
- If BDD feature edits are not picked up, rerun `touch tests/bdd_tests.rs`
  before `make test`.
- If `make fmt` rewrites unrelated Markdown files, inspect `git diff` and
  restore only formatter-introduced changes that are outside the 3.12.1 scope.
- If the implementation breaches a tolerance, stop, update `Decision Log`, and
  wait for explicit direction rather than improvising a broader redesign.

## Artifacts and notes

Useful evidence to keep while implementing:

- a short before/after transcript showing ASCII versus Unicode completion
  output,
- the failing, and then passing, unit test for theme resolution precedence,
- the passing BDD scenario output proving invalid theme rejection and
  ASCII-only rendering,
- the final `git diff --stat` confirming the change stayed within scope.

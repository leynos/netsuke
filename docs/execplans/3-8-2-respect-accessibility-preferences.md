# Respect accessibility preferences (roadmap 3.8.2)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

No `PLANS.md` file exists in this repository.

## Purpose / big picture

Netsuke's accessible output mode (roadmap 3.8.1, complete) auto-detects
`NO_COLOR` and `TERM=dumb` to switch from animated to static status output.
However, the tool does not yet honour emoji preferences, enforce ASCII-only
output when requested, or guarantee semantic prefixes (Error, Warning, and Success)
in every output mode. This plan completes the remaining accessibility
preferences so that:

- Users who set `NETSUKE_NO_EMOJI=1`, pass `--no-emoji`, or write
  `no_emoji = true` in a config file receive output free of Unicode emoji.
  `NO_COLOR` also implies no-emoji automatically (emoji are visual decoration;
  when colour is suppressed, emoji should be too).
- Semantic text prefixes ("Error: ", "Warning: ", "Success: ") appear in every
  mode so that meaning is never conveyed solely by colour, symbol, or emoji.
  In standard mode (when emoji is permitted), these prefixes include an emoji
  glyph alongside the text (e.g., "✖ Error: ..."); in no-emoji mode, the
  plain-text prefix alone provides the semantic marker.
- A new `OutputPrefs` struct encapsulates resolved preferences (emoji allowed
  or not) so that all formatting code can query a single authority rather than
  re-checking environment variables ad hoc.

Observable success: running `NETSUKE_NO_EMOJI=1 cargo run -- build` against a
valid manifest produces status and error output with no emoji glyphs and with
explicit "Error: " / "Success: " labels where applicable. Running
`make check-fmt && make lint && make test` passes with new unit and BDD tests
covering preference resolution, semantic prefix rendering, and edge cases.

## Constraints

- No file may exceed 400 lines.
- Comments and documentation must use en-GB-oxendict spelling.
- Module-level `//!` doc comments are required on every new module.
- No `unsafe` code. No `expect()` in production code.
- Use `rstest` fixtures for unit tests and `rstest-bdd` v0.5.0 for
  behavioural tests.
- Existing public API signatures must remain backward compatible.
- Fluent message keys must be added to both `.ftl` files.
- `docs/users-guide.md` must document the new preferences.
- `docs/roadmap.md` entry 3.8.2 must be marked done on completion.
- `make check-fmt`, `make lint`, and `make test` must pass.
- `clippy::print_stderr` is denied globally.

## Tolerances (exception triggers)

- Scope: more than 20 files or 900 net new lines triggers escalation.
- Dependencies: no new external crate dependencies.
- Interfaces: public API signature changes trigger escalation.
- Tests: three failed investigation cycles triggers escalation.
- File size: `src/cli/mod.rs` exceeding 400 lines triggers mitigation.

## Risks

- Risk: `src/cli/mod.rs` is 387 lines; adding `no_emoji` may push past 400.
  Severity: medium. Likelihood: high.
  Mitigation: keep doc comments minimal; shorten `accessible` comment if needed.

- Risk: `cli_overrides_from_matches` must be updated for `no_emoji`.
  Severity: high. Likelihood: high.
  Mitigation: add to `value_source` check array; covered by Behaviour-Driven
  Development (BDD) scenario.

## Progress

- [x] Write ExecPlan.
- [x] Stage A: Create `src/output_prefs.rs` with `OutputPrefs` and unit tests.
- [x] Stage B: Add `no_emoji` to `Cli`, update merge, add Fluent keys.
- [x] Stage C: Wire `OutputPrefs` into status, runner, and main.
- [x] Stage D: Write BDD tests, update docs, mark roadmap done.
- [x] Final quality gates.

## Surprises & discoveries

- rstest-bdd reads `.feature` files at compile time via proc macros; changing
  only the feature file does not trigger recompilation. Touch the test entry
  point (`tests/bdd_tests.rs`) to force a rebuild after feature file edits.
- The `#[then]` macro generates step names from function names by replacing
  underscores with spaces; feature file step text must use spaces, not hyphens
  (e.g., "no emoji mode" not "no-emoji mode").
- `clippy::unnecessary_wraps` expectations are only needed on step functions
  that return `Ok(())` unconditionally; steps using `?` legitimately return
  `Result` and do not need the suppression.

## Decision log

- `OutputPrefs` is separate from `OutputMode` (orthogonal concerns).
- `NO_COLOR` implies no-emoji (visual decoration suppressed with colour).
- `no_emoji` is `Option<bool>` (tri-state: None/auto, Some(true)/off,
  Some(false)/on).
- Semantic prefixes use localized Fluent messages with `$emoji` select.
- Emoji-allowed mode: `✖ Error:`, `⚠ Warning:`, `✔ Success:`.
  No-emoji mode: `Error:`, `Warning:`, `Success:`.

## Outcomes & retrospective

Implementation completed successfully. All quality gates pass:
`make check-fmt`, `make lint`, `make test`.

New tests: 18 unit tests in `output_prefs::tests` (10 parameterized resolve
cases + 8 prefix assertion tests) and 10 BDD scenarios in
`accessibility_preferences.feature`. Total test count rose from 149 to 159
BDD scenarios.

Files created: 3 (`src/output_prefs.rs`, `tests/features/accessibility_preferences.feature`,
`tests/bdd/steps/accessibility_preferences.rs`).
Files modified: 13 (within 20-file tolerance).

The `src/cli/mod.rs` 400-line risk materialized (387 + new field). Mitigated
by shortening the `accessible` doc comment from 4 lines to 1 line, landing at
390 lines.

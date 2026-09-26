# 3.12.3. Add terminal rendering regression tests

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT

This plan has not been approved. Do not begin implementation until the user
explicitly approves it, including the open decisions `D3`, `D4`, `D8`, `D9`, and
`D11` in the `Decision log`.

## Purpose / big picture

Netsuke exposes four display-policy flags: `--color auto|always|never`,
`--emoji auto|always|never`, `--progress auto|always|never`, and
`--accessibility auto|on|off`. Each has an environment equivalent
(`NETSUKE_COLOR`, `NETSUKE_EMOJI`, `NETSUKE_PROGRESS`, `NETSUKE_ACCESSIBILITY`)
and a configuration-file key of the same name, merged by `ortho_config`.
Roadmap task 3.12.3 asks for terminal rendering regression tests which verify
that each policy behaves as documented.

Today the *resolution* of these policies is well tested in isolation: a
648-case sweep in `tests/cli_tests/display_policy_domain.rs` checks the pure
resolver functions. What is not tested is what a user actually sees: the bytes
Netsuke writes to a real terminal or a pipe. No test attaches Netsuke to a
terminal, so every `auto` branch that depends on "is this stream a terminal?"
is unverified, and no test asserts the absence of American National Standards
Institute (ANSI) escape sequences.

A baseline probe taken while writing this plan (see `Artefacts and notes`)
shows that the gap hides real defects. On a pseudo-terminal (PTY, a kernel
device that makes a child process believe it is attached to an interactive
terminal):

- `--color never --verbose` writes 416 ANSI Select Graphic Rendition (SGR,
  the `ESC [ … m` colour and style codes) sequences through the `tracing` log
  formatter, and `NO_COLOR=1 --verbose` writes the same 416.
- `--color never` on a failing run writes coloured `miette` diagnostics and a
  coloured `tracing` error line.
- `--color never --help` writes 113 SGR sequences from `clap`.

After this change, a contributor can run `make test` and see a regression suite
that drives the real `netsuke` binary through pipes and PTYs for every policy
value, and fails with a reviewable diff when any policy stops being honoured.
The four roadmap sub-bullets become observable, repeatable facts:

1. Colour. `--color never`, and `NO_COLOR` under `--color auto`, produce no
   SGR sequence on any stream Netsuke styles itself (status output, logs,
   diagnostics, help). `--color auto` colours a stream only when it is a
   terminal, `NO_COLOR` is not in effect, and `TERM` is not `dumb`. What
   `--color always` does on a pipe is decision `D11`.
2. Emoji. `--emoji never` produces ASCII-only status glyphs, `--emoji always`
   produces Unicode glyphs even in accessible mode, and `--emoji auto` produces
   Unicode in standard mode and ASCII in accessible mode (or when `NO_COLOR` is
   in effect). Every status line keeps a text label, so meaning never depends
   on the glyph.
3. Progress. `--progress never` produces no stage or task reporting.
   `auto` and `always` report every stage; the reporting is live and redrawing
   only when the output mode is standard, stderr is a terminal, and `TERM`
   names a redraw-capable terminal. Otherwise it is static text. Task updates
   are plain text whenever stdout is not a terminal.
4. Accessibility. `--accessibility on` never emits cursor-movement,
   line-erasing, or carriage-return redraws, even on a terminal.
   `--accessibility off` keeps standard (live-capable) output even when
   `NO_COLOR` is set. `--accessibility auto` selects accessible output exactly
   when `--color never` is given, or `--color auto` is in effect and `NO_COLOR`
   is set, or `TERM` is `dumb`.

## Constraints

- Preserve every behaviour the users' guide already documents unless a
  decision in this plan, approved by the user, says otherwise. In particular,
  keep: `NO_COLOR` or `TERM=dumb` auto-selecting accessible mode;
  `--color never` acting as an internal `NO_COLOR` (and therefore selecting
  accessible mode under `--accessibility auto`); task updates falling back to
  text when **stdout** is not a terminal; and `--json` disabling progress and
  tracing.
- Do not change status text, labels, stage numbering, or glyph tokens. The
  `insta` snapshots under `src/snapshots/status/`,
  `src/snapshots/status_timing/`, and `src/snapshots/output_prefs/` must pass
  unchanged.
- Do not add a crate to `Cargo.lock`. Enabling the `term` feature of the
  existing `nix` 0.31.3 dev-dependency (for `nix::pty::openpty`), and adding
  the same `nix` requirement to `test_support` under
  `[target.'cfg(unix)'.dependencies]`, are within scope once this plan is
  approved (decision `D5`).
- No in-process environment mutation. Tests never call `std::env::set_var`
  or `std::env::remove_var`. Production code reads `NO_COLOR` and `TERM` only
  through the injected `LocaleEnvProvider` already carried by
  `RunWithArgsDependencies` in `src/main.rs`, and stream terminal status is
  measured once in `main()` and passed in as values
  ([ADR-008](../adr-008-environment-seam-taxonomy.md)).
- Every end-to-end child process starts from `Command::env_clear()` and then
  receives only `PATH`, `HOME` and `TMPDIR` (pointing into the test's temporary
  directory), `LC_ALL=C.UTF-8`, `NETSUKE_LOCALE=en-US`, an explicit `TERM`, and
  the variables the case under test sets. On Windows also forward `SystemRoot`.
  Removing a list of variables is not enough: `LANG`, `HOME` (user
  configuration), `NO_GRAPHICS`, `COLORTERM`, and continuous integration (CI)
  variables all change the output.
- The pure display-policy core must not import `std::io`, `std::env`,
  `tracing_subscriber`, `miette`, `clap`, or `indicatif`. It receives facts and
  returns decisions. Adapters apply the decisions and never re-derive them.
- Presentation decisions must not flow into build semantics. The resolved
  display plan travels beside the command, never inside an application request
  (RFC 0026, "Application requests and an owned execution port").
- Keep every file under 400 lines, including `src/main.rs` (383 lines today)
  and `tests/cli_tests/display_policy_domain.rs` (397 lines today). Extract
  before adding.
- The borrow checker is Polonius and the trait solver is next-generation
  (pinned `nightly-2026-08-23`). Do not add turbofish, clones, or bounds to
  appease an older analysis.
- `unsafe_code` is forbidden, so the PTY harness must not use `pre_exec` or
  `setsid`; it uses the safe `CommandExt::process_group(0)` instead.
- Gate PTY-only test files with `#[cfg(unix)]` on their `#[path]` module
  declaration, as `tests/logging_stderr_tests.rs` does for
  `command_list_failure`. Never gate shared support code, so Windows still
  compiles and runs the piped suite.
- Refactors land as separate, behaviour-preserving commits before the
  functional commits that depend on them.

If satisfying the objective requires violating a constraint, stop, record the
conflict in `Decision log`, set the status to `BLOCKED`, and escalate.

## Tolerances (exception triggers)

- Scope: if production (non-test) changes exceed 600 net lines or touch more
  than 18 production files, stop and escalate. Test and documentation lines are
  not capped, but every file stays under 400 lines.
- Interface: the public surface changes listed in
  `Interfaces and dependencies` are sanctioned, including the new `runner::run`
  signature and removal of the ambient wrappers `output_mode::resolve` and
  `output_prefs::resolve_from_theme`. Removing or renaming any other public
  item requires escalation.
- Dependencies: any new crate, or a new feature on any crate other than
  `nix`, requires escalation.
- Snapshots: any change to an existing `.snap` file requires escalation.
- Iterations: if a focused test still fails after three fix attempts, or a
  PTY test fails in any of 20 stress runs, stop and escalate.
- Time: if a milestone takes more than one working day, stop and record why.
- Ambiguity: if an adapter (`tracing-subscriber`, `miette`, `clap`,
  `indicatif`) cannot honour a decision without replacing the library's
  renderer, stop and present the options.

## Risks

- Risk: PTY tests are flaky.
  - Severity: high. Likelihood: medium.
  - Cause: `indicatif` redraws on a 12 Hz timer, a grandchild (the fake Ninja)
    can hold the PTY slave open, and a full pipe can deadlock the child.
  - Mitigation: assert only presence or absence facts, never exact bytes of
    live output. The harness drops the `Command` straight after `spawn` so the
    parent holds no slave descriptor, sets `FD_CLOEXEC` on both PTY ends,
    spawns into a new process group, drains the PTY master and every piped
    stream on dedicated threads, kills the whole group on timeout, and joins
    the readers with a bound. EP-M0 proves 20 clean stress runs before any
    Netsuke PTY test exists.
- Risk: the `tracing` ANSI decision must change after the subscriber is
  installed, because `--color` is known only after configuration merges.
  - Severity: medium. Likelihood: medium.
  - Mitigation: `fmt::Layer::set_ansi` exists in 0.3.23 and
    `reload::Handle::modify` rebuilds the interest cache. Start with ANSI off,
    so buffered startup warnings are always plain, and switch once the plan is
    known, before `configure_runtime` raises the level. Do not put a
    per-layer filter inside the reload wrapper (its `downcast_raw` hides it).
    Spans opened before the switch keep plain fields; record this.
- Risk: `miette::set_hook` fails silently once any `Report` exists.
  - Severity: medium. Likelihood: medium.
  - Mitigation: install the hook first thing in `main()`, before any
    `Report` can be built, and `debug_assert!` success there. The hook reads
    the diagnostic style from a process-global `OnceLock` set once the plan is
    resolved, defaulting to plain ASCII before then.
- Risk: `clap` rebuilds localized usage errors through
  `ortho_config`'s `ClapError::raw`, which fixes their colour choice to `Never`.
  - Severity: low. Likelihood: certain.
  - Mitigation: decision `D9` covers help and version output only; usage
    errors are pinned as plain under every policy.
- Risk: output printed before configuration merges (configuration-load
  errors) cannot follow a configured colour policy.
  - Severity: low. Likelihood: certain.
  - Mitigation: pin it plain with one test and document it.
- Risk: Fluent wraps interpolated values in bidirectional isolate marks
  (`U+2068`, `U+2069`), so an "output is ASCII" check fails under
  `--emoji never` (observed in the baseline).
  - Severity: medium. Likelihood: certain.
  - Mitigation: record isolate presence as its own fingerprint field before
    normalizing with `test_support::fluent::normalize_fluent_isolates`, then
    check glyphs. The contract says the emoji policy governs glyphs, not
    isolate marks; stray isolates in accessible output are a recorded
    follow-up.
- Risk: macOS PTYs differ from Linux (end-of-file reads return `Ok(0)`
  rather than `EIO`), and `script -qfec` is util-linux syntax.
  - Severity: low. Likelihood: medium.
  - Mitigation: treat both as end-of-file in the harness with a unit test;
    mark the manual commands in `Validation and acceptance` as Linux-only.
- Risk: Windows `TERM` semantics differ: `console` treats an unset `TERM` as
  dumb on Unix but not on Windows.
  - Severity: medium. Likelihood: certain.
  - Mitigation: the environment reduction computes `redraw_capable` with a
    per-platform rule, covered by `#[cfg]`-specific witness rows.

## Progress

- [x] (2026-09-26 22:25Z) Reconnoitred implementation, tests, documentation,
  and tooling with a Wyvern agent team; researched the `NO_COLOR`,
  `FORCE_COLOR`, and `CLICOLOR` conventions.
- [x] (2026-09-26 22:25Z) Captured a baseline probe of the current binary
  (see `Artefacts and notes`).
- [x] (2026-09-26 22:40Z) Drafted this ExecPlan.
- [x] (2026-09-26 23:10Z) Expert-panel review (accessibility, testing and
  verification, architecture, terminal systems, plan quality); revised the plan
  (see `Revision note`).
- [ ] User approval of the plan and of decisions `D3`, `D4`, `D8`, `D9`,
  `D11`.
- [ ] EP-M0: terminal test harness.
- [ ] EP-M1: regression suite for already-correct behaviour.
- [ ] EP-M2: display-plan core and single resolution.
- [ ] EP-M3: bind colour and glyph decisions to the emitters.
- [ ] EP-M4: contract snapshots, pinning scenarios, documentation, ADR,
  roadmap.

## Surprises & discoveries

- Observation: `--color` governs no ANSI emitter today; its only effect is on
  accessible-mode selection through `NO_COLOR` semantics. Evidence:
  `src/theme.rs` documents `ColourTokens` as placeholders;
  `src/main.rs::init_tracing` sets `with_ansi(io::stderr().is_terminal())`;
  `src/` contains no `miette::set_hook` or `clap::ColorChoice`; baseline rows
  `verbose-colnever-tty`, `missing-colnever-tty`, `help-colnever-tty`. Impact:
  the colour sub-bullet needs EP-M3.
- Observation: an explicit `with_ansi(true)` in `tracing-subscriber` 0.3.23
  overrides that library's own `NO_COLOR` check. Evidence: `fmt/fmt_layer.rs`
  lines 308–314 and 743; baseline `verbose-nocolor-tty`. Impact: `NO_COLOR` is
  ignored by verbose output on a terminal today.
- Observation: `--progress auto` and `--progress always` resolve
  identically. Evidence: `src/cli/preferences.rs::progress_enabled`; baseline
  rows `gen-auto-pipe` and `gen-prog-always-pipe` are the same size. Impact:
  decision `D3`.
- Observation: `indicatif` hides its live output when `TERM` is `dumb`, or
  unset on Unix, even in standard mode. Evidence:
  `indicatif-0.18.6/src/draw_target.rs:80`; `console-0.16.6/src/term.rs`
  (`is_dumb`). Impact: the display plan models this (`redraw_capable`) so the
  core states what actually renders; `--accessibility off` with `TERM=dumb` is
  static.
- Observation: display decisions are derived from ambient state in three
  places: `src/main.rs`, `src/runner/mod.rs::run_with_ninja_program_resolver`,
  and `src/runner/help.rs::resolved_prefs`; `ExecutionContext` recomputes
  `progress_enabled`, and `IndicatifReporter` relies on `indicatif`'s own
  terminal check. Impact: EP-M2 rewires all of them.
- Observation: `miette` 7.6.0's graphical handler is already selected on a
  pipe; piped diagnostics are uncoloured only because `color` is unset and
  `supports-color` reports no support. It also emits OSC 8 hyperlinks
  independently of `color`, and uses a Unicode `×` under `--emoji never`.
  Evidence: `miette-7.6.0/src/handler.rs` lines 273, 284–298, 344–363; baseline
  `missing-emoji-never-tty`. Impact: decision `D8`.
- Observation: a failed run whose error is not a `RunnerError` prints the
  error twice (a `tracing` `ERROR` event and the `Error:` line), and
  `RunnerError` failures show two severity glyphs (Netsuke's prefix and
  `miette`'s `×`). Evidence: `src/main.rs::handle_runner_error`; baseline
  `err-auto-tty`. Impact: out of scope; follow-up issue. The suite must not pin
  the duplication as intended.

## Decision log

- Decision D1: treat 3.12.3 as "verify, and repair what verification
  breaks", bounded by the tolerances. `--color never` and `NO_COLOR` emitting
  colour are defects against the documented contract ("`--color never` behaves
  like an internal `NO_COLOR`", `docs/netsuke-design.md`; the `NO_COLOR`
  convention) and are fixed. New colour *behaviour* is not assumed: it is
  decision `D11`. Date/Author: 2026-09-26, planning agent, revised after panel
  review.
- Decision D2: shape the change as a functional core with an imperative
  shell, not a trait-based port. A new pure module `src/display_plan.rs`
  resolves a `DisplayPlan` from `DisplayPolicies` and `TerminalFacts`. The
  composition root gathers facts once and hands the plan to four outbound
  adapters: the status reporter factory, the `tracing` formatter, the
  diagnostic hook, and `clap` help. Environment strings are reduced to booleans
  exactly once, by `EnvSignals::read` in `src/output_mode.rs`; precedence lives
  in `const fn`s over those booleans, which the existing `resolve_with` and
  `resolve_theme` also call, so there is one source of truth for precedence and
  for the empty-`NO_COLOR` rule. Rationale: the domain logic is a total
  function over a small finite domain; a value-in, value-out core is simpler
  and more testable than a `TerminalProbe` trait with mock implementations.
  `OutputPrefs` (a glyph façade) and `ThemeContext` (an input record) each
  capture one facet, so a new type is justified provided it absorbs
  `ReporterOptions` rather than sitting beside it. Date/Author: 2026-09-26,
  planning agent, revised after panel review.
- Decision D3 (open, requires approval): keep `--progress auto` and
  `--progress always` behaviourally identical, pin the equivalence with a test,
  and document the contract as: "`always` requests progress reporting; it never
  forces animation into a pipe or a dumb terminal, and never overrides
  accessible mode." Alternative: give `auto` a distinct meaning, such as no
  stage lines when stderr is not a terminal. That changes CI logs, contradicts
  the users' guide ("task progress falls back to text, so logs remain
  readable"), and is a product decision outside a verification task.
  Recommendation: pin the equivalence.
- Decision D4 (open, requires approval): whether an *empty* `NO_COLOR`
  counts as set. Today it does (`src/output_mode.rs`: "any value, including
  empty"). Option A: align with the `NO_COLOR` convention ("present and not an
  empty string"), which `tracing-subscriber` and `anstream` follow
  (`supports-color` does not: it treats empty as set and `"0"` as unset). This
  is a one-line change in `EnvSignals::read` plus a users' guide edit. Option
  B: keep current behaviour, pin it, and record the divergence in the ADR.
  Either way every emitter agrees, because after EP-M3 no adapter consults
  `NO_COLOR` itself. Recommendation: Option A. Netsuke is pre-1.0
  (`0.1.0-beta3`), the convention's current text is explicit, and ADR-003
  favours predictability over preserving an accident. If Option A is approved,
  it lands as its own red-green commit in EP-M3.
- Decision D5: build the PTY harness on `nix::pty::openpty` by enabling the
  `term` feature of the existing `nix` 0.31.3 dev-dependency. Rationale: no
  crate is added to `Cargo.lock`; `portable-pty`, `expectrl`, and `rexpect`
  would each add a dependency tree for a few dozen lines of harness. Windows
  pseudo-console (ConPTY) coverage is a residual gap; Windows keeps the piped
  suite and the pure-core tests. Date/Author: 2026-09-26, planning agent.
- Decision D6: verify the finite resolution domain by exhaustive
  enumeration in an ordinary test instead of a Kani harness; use `proptest` plus
  `rstest` witness tables for the two string-facing lemmas; write no Verus
  proof. Rationale: the domain is 3⁴ policy combinations × 2 (`json`) × 2⁵
  facts = 5,184 points. Enumeration of a total function over a finite domain is
  a complete proof by exhaustion relative to the truth model, runs in
  milliseconds under `make test`, and is guarded by completeness checks (an
  exhaustive `match` per enumeration array and an asserted case count). Kani
  would prove the same property at higher cost outside the default gate. Verus
  is reserved in this repository for a small proof kernel over unbounded
  structures
  (`docs/execplans/4-1-3-record-phase-1-scope-boundary-for-verus-and-stateright.md`);
  a finite, total resolver needs none. Date/Author: 2026-09-26, planning
  agent, revised after panel review.
- Decision D7: use one oracle per behaviour. The *contract* is the pure
  plan: `insta` YAML snapshots, one per policy, render deterministic
  projections of `resolve_display_plan` over that policy's matrix, so any
  contract change is a reviewable diff on every platform. End-to-end tests do
  not restate expectations; they compute them from `resolve_display_plan` for
  the case's policies and facts, and check the binary's bytes honour them.
  Rationale: live `indicatif` output is timing-dependent (the 3.12.2 plan
  reached the same conclusion), and duplicating expectations in rstest rows and
  snapshots invites the two to drift. Date/Author: 2026-09-26, planning agent,
  revised after panel review.
- Decision D8 (open, requires approval): the diagnostic hook maps the plan's
  `DiagnosticStyle` to `MietteHandlerOpts`: `color(style.colour)`,
  `unicode(style.unicode)`, and `terminal_links(style.links)`, where `links` is
  true only when colour is on and the mode is standard. It never sets
  `force_graphical`, so a user's `NO_GRAPHICS` still selects `miette`'s
  narratable handler. Alternative: additionally select the narratable
  (screen-reader oriented) handler for an explicit `--accessibility on`. That
  is a visible format change for every accessible-mode user. Recommendation:
  adopt the mapping; defer the narratable handler to a follow-up roadmap item,
  and pin an accessible-mode diagnostic fingerprint now so that switch later
  shows as a diff.
- Decision D9 (open, requires approval): `clap` help and version output
  follow a startup colour hint: `--color` on the command line (last occurrence
  wins, `--color=never` and localized, case-insensitive values accepted,
  nothing after `--`), then `NETSUKE_COLOR`, then `auto`. Configuration files
  are not consulted, mirroring `locale_resolution::resolve_startup_json`. The
  hint is resolved through the same pure colour function and `TerminalFacts`
  (stdout, where help is written), and always handed to `clap` as
  `ColorChoice::Always` or `ColorChoice::Never`, never `Auto`, so `anstream`'s
  own `NO_COLOR`, `CLICOLOR`, and CI heuristics cannot disagree with Netsuke.
  Usage errors stay plain (see `Risks`). Alternative: leave help colouring to
  `clap` and document that `--color` does not apply to help. Recommendation:
  adopt the startup hint.
- Decision D10: record the display-policy contract and verification
  strategy as ADR-041, status `Proposed` while implementation runs and
  `Accepted` (with date and summary) at EP-M4. ADR-039 and ADR-040 are claimed
  on open branches (`jm5/kani-change-scoped-gate`,
  `6-1-1-split-rfc-0006-set-into-focused-child-rfcs-and-task`). Re-check every
  remote branch immediately before committing the ADR, and renumber on a clash.
  Date/Author: 2026-09-26, planning agent.
- Decision D11 (open, requires approval): `--color always` forces ANSI
  styling on a non-terminal stream for every emitter Netsuke controls
  (`tracing` logs and `miette` diagnostics on stderr, `clap` help on stdout),
  matching the `CLICOLOR_FORCE` and `FORCE_COLOR` sense of "always". This is
  new behaviour: the users' guide currently says colour rendering is not
  implemented. Alternative: `always` only bypasses `NO_COLOR` and `TERM=dumb`
  detection on a terminal, and pipes stay plain. Recommendation: adopt, because
  "always" that still depends on the stream is surprising, and CI log viewers
  that render ANSI are the main reason to ask for it.
- Decision D12: gather facts through existing seams. `NO_COLOR` and `TERM`
  are read through `RunWithArgsDependencies::locale_env` (the `NETSUKE_JSON`
  precedent), and stream terminal status is two new `bool` values in
  `RunWithArgsDependencies`, measured once in `main()`. The ambient wrappers
  `output_mode::resolve` and `output_prefs::resolve_from_theme` are deleted
  once unused, and `std::io::IsTerminal::is_terminal` joins
  `disallowed-methods` in `clippy.toml`, so `make lint` enforces single
  resolution with an `#[expect]` only in `main()`. Rationale: pre-1.0 APIs need
  no compatibility layer; a lint makes the structural invariant self-enforcing
  rather than a manual `rg` check. Date/Author: 2026-09-26, planning agent,
  after panel review.

## Outcomes & retrospective

Not started. Complete this section at each milestone boundary and at
completion, reconciling discoveries with the documents in `Conformance basis`.

## Context and orientation

Netsuke is a Rust command-line build tool: it reads a YAML-and-Jinja manifest
(`Netsukefile`) and drives Ninja. The binary entry point is `src/main.rs`; the
library root is `src/lib.rs`; `test_support/` is a workspace crate of shared
test helpers.

Policy types and flags. `src/cli/config.rs` defines the `clap`-free enums
`ColourPolicy`, `EmojiPolicy`, `ProgressPolicy`, and `AccessibilityPolicy`, and
the `CliConfig` struct that `ortho_config` merges from defaults, configuration
files, `NETSUKE_*` environment variables, and the command line
(`#[ortho_config(prefix = "NETSUKE")]`). `src/cli/command.rs` declares the
flags; `src/cli/policy_values.rs` supplies localized, case-insensitive value
parsers; `src/cli/preferences.rs` maps parsed policies to runtime preferences
(`theme_preference`, `accessibility_override`, `progress_enabled`).
`src/cli_l10n.rs` holds `locale_hint_from_args` and `json_hint_from_args`, the
pre-parse argument scans re-exported from `src/cli/parser.rs`;
`cli::parse_with_localizer_from` in `src/cli/parser.rs` builds and parses the
localized `clap` command. This task adds no configuration field; see
`docs/ortho-config-users-guide.md` for how the layers merge.

Resolution. `src/output_mode.rs::resolve_with` chooses `OutputMode::Accessible`
or `OutputMode::Standard` from the accessibility override, the colour policy,
and an injected `read_env` closure. `src/theme.rs::resolve_theme` chooses
Unicode or ASCII glyph tokens (it has a private `EnvSignals` struct).
`src/output_prefs.rs` wraps the theme in the `OutputPrefs` value reporters
consume. `output_mode::resolve` and `output_prefs::resolve_from_theme` are thin
wrappers that read the real process environment.

Rendering. `src/runner/reporter.rs::make_reporter` selects `AccessibleReporter`
(static labelled lines), `IndicatifReporter` (live `indicatif::MultiProgress`
bars drawn to stderr, `src/status_indicatif.rs`, which mirrors messages to
plain stderr lines when its draw target is hidden), or `SilentReporter`
(`--progress never`), wrapped in `VerboseTimingReporter` under `--verbose`.
`should_force_text_task_updates` forces plain task lines when the mode is
accessible or stdout is not a terminal. `src/main.rs::init_tracing` installs the
`tracing` subscriber; `src/startup_tracing.rs::StartupWriter` buffers early
events until the output mode is known. `src/main.rs::handle_runner_error`
prints failures through `miette::Report`'s debug renderer, which uses the
process-global `miette` hook (currently the library default). `clap` renders
`--help` in `src/main.rs::parse_cli_or_exit`.

Existing tests. `tests/cli_tests/display_policy_domain.rs` sweeps the pure
resolvers against a handwritten truth model. `src/output_mode.rs`,
`src/theme.rs`, `src/output_prefs.rs`, and `src/runner/reporter.rs` have
`rstest` tables. Behavioural scenarios use `rstest-bdd` 0.5.0 with
`strict-compile-time-validation`: feature files in `tests/features/` (all
platforms) and `tests/features_unix/` (Unix only) are collected by `scenarios!`
in `tests/bdd_tests.rs`, sharing the `TestWorld` fixture in
`tests/bdd/fixtures/mod.rs`. Child processes are built by
`tests/bdd/steps/manifest_command_helpers.rs::build_netsuke_command`, which
already calls `env_clear()`. Existing steps this plan reuses verbatim:
`a minimal Netsuke workspace` and `an empty workspace`
(`tests/bdd/steps/manifest_command.rs`), `netsuke is run with arguments {args}`
(same file), `the command should succeed` and `the command should fail`
(`tests/bdd/steps/process.rs`), `stderr should contain {fragment}` and
`stderr should not contain {fragment}` (same file),
`the {name} environment variable is {value}`
(`tests/bdd/steps/configuration_preferences.rs`), and
`a fake ninja executable that emits task status lines`
(`tests/features/progress_output.feature`). The shared `test_support` crate
provides `fluent::normalize_fluent_isolates` and `fake_ninja`. No test uses a
PTY.

Terms. A *policy* is the user's request (`auto`, `always`, …). A *fact* is an
observation (`NO_COLOR` set, stderr is a terminal). A *decision* is what
Netsuke does (style stderr, draw live bars). The *display plan* is the complete
set of decisions for one run. An *escape sequence* is any byte sequence
beginning with `ESC` (0x1B). A *Control Sequence Introducer (CSI)* sequence is
`ESC [`, then parameter bytes 0x30–0x3F, intermediate bytes 0x20–0x2F, and one
final byte 0x40–0x7E. An *SGR sequence* is a CSI sequence with final byte `m`.
A *redraw sequence* is a CSI sequence with final byte in
`A B C D E F G H J K f`, a save or restore cursor (`ESC 7`, `ESC 8`), or a
*bare carriage return* (a `\r` not followed by `\n`; a PTY turns every `\n` into
`\r\n`, so `\r\n` is folded first). An *OSC sequence* is `ESC ]`, used for
hyperlinks.

Skills and guides to load. Load `rust-router` and follow it to
`rust-unit-testing` (rstest tables, `googletest` matchers, `pretty_assertions`,
`insta`), `proptest`, `domain-cli-and-daemons`, and `nll-to-polonius` if a
borrow question arises. Load `hexagonal-architecture` for the core/adapter
split, `nextest` for runner settings, `execplans` for maintaining this document,
`en-gb-oxendict-style` for prose, and `codegraph-mcp` for finding callers
before changing signatures. Read `docs/rust-testing-with-rstest-fixtures.md`,
`docs/rstest-bdd-users-guide.md`, `docs/rust-doctest-dry-guide.md`,
`docs/reliable-testing-in-rust-via-dependency-injection.md`,
`docs/ortho-config-users-guide.md`, the "Environment isolation" and
"Behavioural testing strategy" sections of `docs/developers-guide.md`, and
`docs/documentation-style-guide.md` before editing.

## Conformance basis

Upstream artefacts, all at commit `ebcedaef` on `main`:

- `docs/roadmap.md`, "3.12. Terminal rendering verification", task 3.12.3
  and its sub-bullets: `RM-3.12.3-COLOUR`, `RM-3.12.3-EMOJI`,
  `RM-3.12.3-PROGRESS`, `RM-3.12.3-ACCESS`. The phase 3 hypothesis is
  `RM-P3-HYP`.
- `docs/netsuke-design.md`, the paragraphs beginning "Real-time stage
  reporting" (`DES-PROGRESS`: text fallback when stdout is not a terminal;
  accessible mode never animates and takes precedence) and "Theme resolution
  for CLI output" (`DES-COLOUR`: `--color never` behaves like an internal
  `NO_COLOR`; `always` bypasses `NO_COLOR`).
- `docs/netsuke-cli-design-document.md`, the component design for the
  command-line interface, which states that `NO_COLOR` disables coloured output
  (`CLI-DES-NOCOLOR`).
- `docs/users-guide.md`, "Policy values and parsing", "Accessible output",
  and "Ninja status progress": `UG-ACCESS`, `UG-EMOJI`, `UG-TEXT-FALLBACK`, and
  `UG-NO-COLOUR-ALONE` ("meaning is not conveyed by colour alone").
- [ADR-003](../adr-003-agent-consistent-human-first-cli.md) (`ADR-003`) and
  [ADR-008](../adr-008-environment-seam-taxonomy.md) (`ADR-008`).
- RFC 0026, "Application requests and an owned execution port"
  (`RFC-0026-PRESENTATION`); RFC 0027's layer table, which places rendering in
  "Adapters and presentation" (`RFC-0027-LAYERS`).
- External conventions: `NO_COLOR` (<https://no-color.org/>; command-line
  arguments and user configuration override it) as `EXT-NO-COLOR`, and
  `FORCE_COLOR` (<https://force-color.org/>) as `EXT-FORCE-COLOR`, which this
  plan does not adopt as an input.

There is no Terms of Reference document for this task.

Trace links:

```plaintext
RM-3.12.3-COLOUR   -> DES-COLOUR, CLI-DES-NOCOLOR, EXT-NO-COLOR, D1, D11
                   -> INV-COLOUR-TRUTH, INV-NO-SGR-WHEN-OFF, INV-COLOUR-ADDITIVE -> EP-M2, EP-M3
                   -> display_plan_domain sweep, terminal_rendering colour cells, contract snapshot colour
RM-3.12.3-EMOJI    -> UG-EMOJI, UG-NO-COLOUR-ALONE, D8 -> INV-GLYPHS -> EP-M1, EP-M3
                   -> terminal_rendering emoji cells, contract snapshot emoji
RM-3.12.3-PROGRESS -> DES-PROGRESS, UG-TEXT-FALLBACK, D3 -> INV-PROGRESS -> EP-M1, EP-M2
                   -> terminal_rendering progress cells, contract snapshot progress
RM-3.12.3-ACCESS   -> UG-ACCESS, DES-PROGRESS -> INV-MODE-TRUTH, INV-ACCESS-STATIC -> EP-M1, EP-M2
                   -> terminal_rendering accessibility cells, contract snapshot accessibility
ADR-008, D12       -> INV-SINGLE-RESOLUTION, L-ENV-SIGNALS -> EP-M2 -> clippy disallowed-methods, witness tables
RFC-0026-PRESENTATION, RFC-0027-LAYERS -> EP-M2 conformance check (plan never enters a build request)
```

## Verification plan

The implementation is decomposed so each obligation lands either on a pure
function, verified exhaustively, or on a byte-level observation of the real
binary, verified end to end. Libraries are axioms; their observable effects are
exercised, not trusted blindly.

Axioms:

- `AX-TTY`: `IsTerminal::is_terminal` is true for a PTY slave and false for
  a pipe on Linux and macOS.
- `AX-TRACING`: a `tracing_subscriber::fmt::Layer` with `ansi` false emits no
  escape sequence; `set_ansi` through `reload::Handle::modify` applies to
  subsequent events (not to spans already open).
- `AX-MIETTE`: `MietteHandlerOpts::color(false)` emits no SGR;
  `color(true)` emits SGR regardless of stream and `NO_COLOR`; `unicode(false)`
  selects ASCII drawing characters; `terminal_links(false)` emits no OSC 8
  hyperlink; `set_hook` succeeds only before any `Report` exists.
- `AX-CLAP`: `Command::color(ColorChoice::Never)` suppresses styling in help
  and version output, and `ColorChoice::Always` forces it.
- `AX-INDICATIF`: a `ProgressDrawTarget::stderr_with_hz` target is hidden
  when stderr is not a terminal or `TERM` is dumb (unset counts as dumb on
  Unix); `ProgressDrawTarget::hidden()` never draws.
- `AX-PTY`: `nix::pty::openpty` returns a connected pair; once every slave
  descriptor is closed, reading the master returns `EIO` (Linux) or `Ok(0)`
  (macOS).

Obligations over the pure core (artefact
`tests/cli_tests/display_plan_domain.rs`, using the truth model extracted to
`tests/cli_tests/display_policy_truth.rs`; command
`cargo nextest run --test cli_tests -E 'test(display_plan)'`):

- `INV-MODE-TRUTH`: the output mode is `Accessible` exactly when
  accessibility is `on`, or accessibility is `auto` and (colour is `never`, or
  colour is `auto` and `no_color_set`, or `term_dumb`).
- `INV-COLOUR-TRUTH`: `json` forces colour off on every stream; otherwise
  `never` is off, `always` is on (decision `D11`), and `auto` is
  `stream_is_terminal ∧ ¬no_color_set ∧ ¬term_dumb`. Evaluated for stderr in
  the plan and for stdout by the startup help hint.
- `INV-PROGRESS`: the reporter is `Silent` when progress is `never` or `json`
  is set, otherwise `Accessible` in accessible mode and `Standard` in standard
  mode; `live_progress` is `Standard ∧ stderr_is_terminal ∧ redraw_capable`;
  `force_text_task_updates` is `accessible ∨ ¬stdout_is_terminal`; `auto` and
  `always` give equal plans at every other coordinate (`D3`).

Method: exhaustive enumeration of all 5,184 points against the independent
truth model. Rationale: complete by exhaustion (decision `D6`). Non-vacuity: an
exhaustive `match` on each enum next to its enumeration array makes a new
variant a compile error; the test asserts the case count; it asserts every
value of every plan field is reached (both modes, both glyph sets, all three
reporter kinds, both values of each boolean). Red: before
`resolve_display_plan` exists the file does not compile, which is not a red
test; the real red is the seeded faults, each of which must fail the sweep and
be recorded in `Artefacts and notes` with its failing-case count: swapping the
`TERM=dumb` and `NO_COLOR` precedence, dropping the `ColourPolicy::Never` arm,
and ignoring `stdout_is_terminal` in `force_text_task_updates`.

Lemmas over string inputs:

- `L-ENV-SIGNALS`: `EnvSignals::read` maps `NO_COLOR` and `TERM` strings to
  `no_color_set`, `term_dumb` (exactly `"dumb"`), and `redraw_capable` (Unix:
  `TERM` set and not `"dumb"`; Windows: `TERM` not `"dumb"`), and
  `resolve_with` and `resolve_theme` give the same results as before the
  refactor for every environment. Method: an `rstest` witness table (absent,
  `""`, `"dumb"`, `"DUMB"`, `" dumb"`, `"0"`, `"xterm-256color"`, with `#[cfg]`
  rows for the platform rule) plus a `proptest` over arbitrary `Option<String>`
  pairs comparing the refactored resolvers against a copy of the pre-refactor
  functions kept in the test module. Artefact: `src/output_mode.rs` tests.
  Non-vacuity: the witness table exhibits each class explicitly; a seeded fault
  that lower-cases `TERM` must fail the `"DUMB"` row.
- `L-CLASSIFIER`: `test_support::terminal::classify` reports SGR, redraw,
  OSC, any escape, bare carriage return, bidi isolates, and non-ASCII glyphs
  exactly when the input contains them, as defined in
  `Context and orientation`. Method: a model-based `proptest`: generate a
  vector of tokens (plain printable ASCII excluding `ESC` and `[`, Unicode
  glyphs, isolate marks, SGR with random parameter bytes including `;`, `:`, and
  `?`, cursor-movement, erase-line, private modes such as `ESC [ ? 25 l`,
  save/restore cursor, OSC 8 links, `\r`, `\r\n`), render to bytes, classify,
  and compare with facts computed from the tokens. Plus an `rstest` witness
  table with one row per token kind and the baseline byte files as fixtures.
  Artefact: `test_support/src/terminal/classify.rs` tests. Non-vacuity: the
  witness table guarantees each class is exercised (`proptest` has no coverage
  mechanism); a seeded fault that accepts only digit parameters must fail on
  `ESC [ 38;5;208 m`.

End-to-end obligations (artefacts `tests/terminal_rendering_tests.rs` and
`tests/terminal_rendering/*.rs`; each cell derives its expectation from
`resolve_display_plan` for the same policies and facts):

- `INV-NO-SGR-WHEN-OFF`: when the plan's colour decision for a stream is off,
  no byte Netsuke writes to that stream contains an SGR or OSC sequence, across
  every emitter (status reporter, verbose `tracing`, error `tracing` line,
  `miette` diagnostics, `clap` help). Red: the baseline cells
  `verbose-colnever-tty` (416), `verbose-nocolor-tty` (416),
  `missing-colnever-tty` (6), `err-colnever-tty` (12), and `help-colnever-tty`
  (113), re-run by the new tests immediately before each EP-M3 fix.
  Non-vacuity: every "off" cell has a paired "on" cell on the same emitter and
  transport which must observe at least one SGR sequence, proving the emitter
  ran and the classifier can see it.
- `INV-COLOUR-ADDITIVE` (`UG-NO-COLOUR-ALONE`): for the fixed-output paths
  (accessible reporter, `miette` diagnostic, `--help`), removing SGR sequences
  from the colour-on output yields exactly the colour-off output. Colour
  therefore adds emphasis and never carries meaning. `tracing` is excluded
  because timestamps differ between runs. Non-vacuity: the colour-on output
  must contain SGR.
- `INV-ACCESS-STATIC`: in accessible mode the terminal stream contains no
  redraw sequence, no bare carriage return, and no escape sequence other than
  SGR when colour is on; stderr carries every `Stage N/6` label in order.
  Checked with `Attach::StderrOnly` so stdout (the generated Ninja file) does
  not interleave. Non-vacuity: the paired `--accessibility off` cell on the
  same PTY must observe redraw sequences (baseline: 189).
- `INV-GLYPHS`: after isolate normalization, status lines contain only ASCII
  when the plan selects ASCII glyphs, and at least one Unicode status glyph
  when it selects Unicode; with `D8`, the same holds for `miette` drawing
  characters. Every status line starts with a text label (`Stage`, `Task`, or
  the localized completion message) whatever the glyph set. Non-vacuity: the
  `--emoji always` cell must contain a non-ASCII glyph.
- `INV-SINGLE-RESOLUTION`: one run measures terminal state once and every
  consumer receives the same `DisplayPlan`. Method: structural, enforced by
  `clippy.toml` `disallowed-methods` (decision `D12`). Negative control: calling
  `is_terminal()` in `src/runner/mod.rs` must fail `make lint`; record the
  error.

Residual gaps: Kani and Verus are not used (`D6`). Windows terminal behaviour
is covered only by the pure core and the piped suite (no ConPTY). `FORCE_COLOR`,
`CLICOLOR`, and `CLICOLOR_FORCE` are not Netsuke inputs. Bidirectional isolate
marks in accessible output, the narratable diagnostic handler (`D8`),
duplicated error reporting, and whether Netsuke should pass `NO_COLOR` to Ninja
are recorded follow-ups. Ninja and recipe output is forwarded unchanged, so
`--color never` does not strip colour a subprocess emits itself.

## Plan of work

Stage A (understand and propose) is this document, the baseline probe, and the
expert review. Stage B is EP-M0 and EP-M1: a harness and a green suite over
behaviour that is already correct, with no production change. Stage C is EP-M2
and EP-M3: the display-plan core, then the adapter fixes, each fix committed
with the red cells that prove it. Stage D is EP-M4: contract snapshots, pinning
scenarios, documentation, the ADR, and the roadmap.

The go/no-go point is the end of EP-M0: if the PTY smoke test cannot pass 20
stress runs, stop and escalate before writing any Netsuke PTY test.

Run `make check-fmt`, `make typecheck`, `make lint`, and `make test` at the end
of every milestone (and before every commit that touches code), plus
`make markdownlint` and `make nixie` whenever Markdown changes.

## Milestones and plateaus

### EP-M0: terminal test harness

Requirements: enables every `RM-3.12.3-*` item; discharges `L-CLASSIFIER`.

Before writing code, sweep for an existing equivalent
(`rg -n 'openpty|strip_ansi|x1b\\[|is_terminal' test_support tests src`) and
record the result in `Decision log`. Planning found only `strip-ansi-escapes` in
`tests/yaml_error_tests.rs`, which strips rather than classifies.

Outcome: `test_support/src/terminal/mod.rs` with two children. `classify.rs`
(all platforms) exposes `classify(&[u8]) -> TerminalBytes`. `pty.rs`
(`#[cfg(unix)]` on its module declaration) exposes
`run_in_pty(PtyRequest) -> Result<PtyOutput>`. The harness:

1. opens the pair with an explicit window size of 120 columns by 40 rows;
2. sets `FD_CLOEXEC` on both ends under a process-wide mutex held until
   `spawn` returns, so concurrently spawned children cannot inherit them;
3. attaches the slave to stdout, stderr, or both according to `Attach`
   (`Both`, `StderrOnly`, `StdoutOnly`), pipes the other stream, and sets stdin
   to `Stdio::null()` (otherwise `terminal_size` falls back to stdin and picks
   up the developer's real terminal);
4. spawns with `process_group(0)`, then drops the `Command` so the parent
   holds no slave descriptor;
5. drains the master and the pipe on separate threads, treating `EIO` and
   `Ok(0)` as end-of-file;
6. waits with a 60-second timeout (inside nextest's 300-second limit),
   killing the whole process group on expiry and joining the readers with a
   bound;
7. fails loudly, never skips, if `/dev/ptmx` is unavailable.

Document the module's scope in its `//!` comment: terminal output observation
for tests only; callers are integration tests and BDD steps; no production use.

Tests: `L-CLASSIFIER` (property and witness table); an end-of-file classifier
unit test; a PTY smoke test running
`/bin/sh -c 'test -t 1 && test -t 2 && printf ok'` under `Attach::Both` (expects
`ok`), and the `StderrOnly` inverse (`test -t 1` fails).

Acceptance: `make test` passes;
`cargo nextest run -p test_support --stress-count 20 -E 'test(pty_smoke)'`
passes, once idle and once while another `make test` loads the host.
Conformance: no production change; `git diff Cargo.lock` shows no new package.
Recovery: additive; revert. Remaining gaps: no Netsuke behaviour tested.
Compatibility: none.

### EP-M1: regression suite for already-correct behaviour

Requirements: `RM-3.12.3-EMOJI`, `RM-3.12.3-PROGRESS`, `RM-3.12.3-ACCESS`
(partially; the plan-derived oracle arrives in EP-M2).

Outcome: a new integration-test binary `tests/terminal_rendering_tests.rs`
declaring `#[path = "terminal_rendering/<name>.rs"] mod <name>;` children (no
`mod.rs`, satisfying `tests/integration_test_wiring_tests.rs`): `support.rs`,
`emoji.rs`, `progress.rs`, `accessibility.rs`, and `json.rs`, plus
`#[cfg(unix)]` PTY files `emoji_pty.rs`, `progress_pty.rs`, and
`accessibility_pty.rs`. `support.rs` builds the child with
`assert_cmd::cargo::cargo_bin_cmd!` (piped) or `std::process::Command` for the
PTY harness, applies the `env_clear()` rule from `Constraints`, and stages
`tests/data/minimal.yml` into a temporary workspace through `cap_std` (the
Whitaker `no_std_fs_operations` lint applies). Task-update cells run `build`
with a fake Ninja that prints `[1/2]` status lines: promote the script writer
from `tests/logging_stderr/support.rs` into `test_support` (sweep first; its
helpers are `pub(super)` and not reachable from another binary), keeping the
old call sites working in the same commit.

Cells (expectations handwritten here from the users' guide and baseline; EP-M2
replaces them with plan-derived expectations):

- Emoji, piped on every platform and on a PTY on Unix: `never`, `always`,
  and `auto` crossed with `--accessibility on|off`, checking `INV-GLYPHS`.
- Progress: `never` has no `Stage` label and no redraw on either transport,
  paired with an `always` cell that has the labels (so the absence is not
  vacuous); `auto` and `always` on a pipe produce the six labels in order and
  no redraw; on a PTY in standard mode with `TERM=xterm-256color` both redraw;
  with stdout piped and stderr on the PTY, task updates are plain text.
- Accessibility: `on` on a PTY is static with all six labels; `off` with
  `NO_COLOR=1` on a PTY redraws; `off` with `TERM=dumb` on a PTY is static
  (`indicatif` hides itself); `auto` with `NO_COLOR=1`, `TERM=dumb`, or
  `--color never` is static.
- JSON: `--json --color always` on a PTY has no escape sequence on either
  stream.
- Pre-merge output: a configuration-load error under `--color always` is
  plain.

Use `googletest` (`expect_that!`, `contains_substring`, `not(…)`) so each
failing cell names its policy combination, and `pretty_assertions::assert_eq`
for ordered label lists. Parameterize with `#[rstest]` `#[case::name(…)]` rows,
one per semantically distinct cell.

No red cells are written in this milestone; the baseline table is the
milestone's record of the defects.

Acceptance: `make test` passes; every PTY file passes `--stress-count 20`.
Conformance: no production change; assertions cover only documented behaviour.
Recovery: additive; revert. Remaining gaps: colour. Compatibility: none.

### EP-M2: display-plan core and single resolution

Requirements: `INV-MODE-TRUTH`, `INV-COLOUR-TRUTH`, `INV-PROGRESS`,
`INV-SINGLE-RESOLUTION`, `L-ENV-SIGNALS`; `RFC-0026-PRESENTATION`.

Commit 1 (refactor, behaviour-preserving). Move `init_tracing`,
`set_tracing_filter`, and the reload handle (with a type alias) from
`src/main.rs` into a binary-local module `src/main_tracing.rs`, declared with
`#[path]` like `startup_tracing`; `startup_filter` and `DiagMode` stay where
they are. In `src/cli_l10n.rs` (344 lines), extract a shared
`find_option_value` scanner from `locale_hint_from_args`, as the comments on
`locale_hint_from_args` and `json_hint_from_args` ask for once "a second valued
pre-clap option" exists (`--color` is that option); move the file's tests to a
sibling file first if it would exceed 400 lines. Gates must pass with no test
change.

Commit 2 (refactor, behaviour-preserving). Introduce the public
`output_mode::EnvSignals` with `EnvSignals::read(read_env)` and `const fn`s
over it; make `resolve_with` and `theme::resolve_theme` delegate to them,
deleting `theme.rs`'s private `EnvSignals`. Add the `L-ENV-SIGNALS` witness
table and property test first (red: the type does not exist yet; the property
compares against a verbatim copy of the old functions).

Commit 3 (the core). Before creating `src/display_plan.rs`, record the sweep
(`OutputPrefs`, `ThemeContext`, `ResolvedTheme`, `ReporterOptions` each capture
one facet) and the re-use policy (pure; no I/O; callers are the binary
composition root, `runner`, and tests; adapters read the plan but never
re-derive it) in its `//!` comment. Extract the truth model from
`tests/cli_tests/display_policy_domain.rs` into
`tests/cli_tests/display_policy_truth.rs`, write
`tests/cli_tests/display_plan_domain.rs` (the 5,184-point sweep plus the
`auto == always` test), and run the seeded faults. Retire the 648-case sweep
and the redundant `proptest` from `display_policy_domain.rs`, keeping its
single-field `rstest` cases. Add doctests on `resolve_display_plan` and
`TerminalFacts::from_signals`.

Commit 4 (rewire, all callers at once). `main()` measures
`stdout().is_terminal()` and `stderr().is_terminal()` (the only
`#[expect(clippy::disallowed_methods, …)]` for `is_terminal`) and adds them to
`RunWithArgsDependencies`; `run_with_args` reads `NO_COLOR` and `TERM` through
`locale_env`, builds `TerminalFacts` once, resolves the plan after
configuration merges, and passes it to `runner::run(&cli, plan)`.
`runner::run_with_ninja_program_resolver`, `runner/help.rs::resolved_prefs`,
`ExecutionContext.progress_enabled`, and
`reporter::make_reporter(plan, verbose)` consume the plan; `ReporterOptions`
loses `mode` and `stdout_is_tty`. `IndicatifReporter` receives
`ProgressDrawTarget::hidden()` when `plan.live_progress()` is false, so the
core and the renderer agree. Delete `output_mode::resolve` and
`output_prefs::resolve_from_theme`; update test callers (about twelve) to
`resolve_display_plan(cli.display_policies(), TerminalFacts::default())`, and
update `docs/developers-guide.md` and `docs/test-isolation-with-ninja-env.md`,
which show `runner::run`. Add `std::io::IsTerminal::is_terminal` to
`disallowed-methods` in `clippy.toml` and run the negative control. Switch
EP-M1's handwritten expectations to plan-derived ones.

Acceptance: all four gates pass after each commit; EP-M1's suite is green
throughout, proving behaviour preservation; no snapshot changes. Conformance:
the plan never enters a build request; the core imports none of the forbidden
crates; public changes match `Interfaces and dependencies`. Recovery: each
commit reverts independently; EP-M1 detects partial rewiring. Remaining gaps:
adapters still ignore the colour decision. Compatibility: none; every caller is
updated in the same commit.

### EP-M3: bind colour and glyph decisions to the emitters

Requirements: `RM-3.12.3-COLOUR`, `INV-NO-SGR-WHEN-OFF`, `INV-COLOUR-ADDITIVE`,
`INV-GLYPHS` (diagnostics); decisions `D4`, `D8`, `D9`, `D11` as approved.

Each commit starts by adding that commit's end-to-end cells and behavioural
scenario, running them, and pasting the red output into `Artefacts and notes`;
then applies the fix; then shows them green.

1. `tracing`: build the formatting layer with ANSI off inside a
   `reload::Layer`, and call `set_tracing_ansi(plan.stderr_colour())` before
   `configure_runtime` raises the level. Cells: `--verbose` under `never`,
   `NO_COLOR=1`, `auto` on a PTY, and `always` piped (per `D11`); the
   non-`RunnerError` failure line under `never`.
2. `miette`: `DisplayPlan::diagnostic_style()` returns `DiagnosticStyle`;
   `main()` installs the hook first, before any `Report` can exist, and the
   hook maps the style stored in a `OnceLock` (plain ASCII until set) to
   `MietteHandlerOpts` per `D8`. Cells: a missing manifest (the existing
   `an empty workspace` step) under `never`, `NO_COLOR=1`, `always` piped,
   `--emoji never`, and `--accessibility on` (the accessible diagnostic
   fingerprint).
3. `clap`: add `colour_hint_from_args` beside `json_hint_from_args` in
   `src/cli_l10n.rs` (reusing the `policy_values` parser), a pure
   `display_plan::startup_colour`, and a `ColorChoice` parameter on
   `parse_with_localizer_from`, per `D9`. Cells: `--help` under `never`,
   `NO_COLOR=1`, `NO_COLOR=` (empty), `always` piped, and `auto` on a PTY; a
   usage error pinned plain; hint parsing rows for `--color=never`,
   `--color NEVER`, a repeated flag, and a value after `--`.
4. If `D4` Option A is approved: change `EnvSignals::read`, flipping the
   empty-`NO_COLOR` witness rows first (red) and updating the pinned cells.

The behavioural scenarios added here (red, then green) are:

```gherkin
Feature: Terminal colour policy

  Scenario: Colour never keeps verbose logs plain
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--color never --verbose generate"
    Then the command should succeed
    And stderr should contain no SGR sequence

  Scenario: Colour always styles piped verbose logs
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--color always --verbose generate"
    Then the command should succeed
    And stderr should contain an SGR sequence
```

```gherkin
Feature: Terminal colour policy on a pseudo-terminal

  Background:
    Given the "TERM" environment variable is "xterm-256color"

  Scenario: Colour never removes colour from terminal diagnostics
    Given an empty workspace
    When netsuke is run on a terminal with arguments "--color never generate"
    Then the command should fail
    And the terminal output should contain no SGR sequence
    And the terminal output should contain "manifest_not_found"

  Scenario: NO_COLOR removes colour from terminal logs
    Given a minimal Netsuke workspace
    And the "NO_COLOR" environment variable is "1"
    When netsuke is run on a terminal with arguments "--verbose generate"
    Then the command should succeed
    And the terminal output should contain no SGR sequence
    And the terminal output should contain "Stage 6/6"
```

The first feature lives in `tests/features/terminal_colour.feature` and the
second in `tests/features_unix/terminal_colour_pty.feature`, with steps in
`tests/bdd/steps/terminal_rendering.rs`. The PTY `When` step needs
`build_netsuke_command` refactored to build a `std::process::Command` (wrapped
into `assert_cmd` for the piped path), a terminal-output slot on `TestWorld`,
and `#[cfg(unix)]` on the PTY step functions themselves. Every "contains no"
step normalizes isolates before matching, as `assert_output_not_contains` in
`manifest_command_helpers.rs` does, and is paired with a positive fragment in
the same scenario.

Acceptance: every red cell turns green; each paired colour-on cell observes
SGR; all gates pass after each commit. Conformance: adapters read the plan and
never consult `NO_COLOR` or `is_terminal()`; no new dependency. Recovery: each
commit reverts independently. Remaining gaps: contract snapshots and
documentation. Compatibility: none.

### EP-M4: contract snapshots, pinning scenarios, documentation

Requirements: all `RM-3.12.3-*` items closed; `D7`, `D10`.

Snapshots: `tests/cli_tests/display_plan_contract.rs` renders, per policy, a
YAML table of `(policy value, facts) -> projected plan fields` and stores it
with an explicit `insta::assert_yaml_snapshot!` name
(`display_plan_contract_colour`, `…_emoji`, `…_progress`, `…_accessibility`).
These snapshots are pure, so they run on every platform.

Pinning scenarios for behaviour that EP-M1 already proved (these are not
red-green; they document the contract in executable form), in
`tests/features/terminal_rendering.feature`:

```gherkin
Feature: Terminal rendering policies

  Scenario Outline: Emoji policy selects the status glyph set
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--emoji <emoji> --accessibility <access> --progress always generate"
    Then the command should succeed
    And stderr status glyphs should be <glyphs>
    And every stderr status line should carry a text label

    Examples:
      | emoji  | access | glyphs  |
      | never  | off    | ASCII   |
      | always | on     | Unicode |
      | auto   | off    | Unicode |
      | auto   | on     | ASCII   |

  Scenario: Progress never suppresses stage reporting
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--progress never generate"
    Then the command should succeed
    And stderr should not contain "Stage 1/6"
    And stdout should contain "rule"
```

and in `tests/features_unix/terminal_rendering_pty.feature`:

```gherkin
Feature: Terminal rendering on a pseudo-terminal

  Background:
    Given a minimal Netsuke workspace
    And the "TERM" environment variable is "xterm-256color"

  Scenario: Accessible mode never redraws on a terminal
    When netsuke is run on a terminal with arguments "--accessibility on generate"
    Then the command should succeed
    And the terminal output should contain no redraw sequence
    And the terminal output should contain "Stage 6/6"

  Scenario: Standard mode draws live progress on a terminal
    When netsuke is run on a terminal with arguments "--accessibility off generate"
    Then the command should succeed
    And the terminal output should contain a redraw sequence
```

Confirm every step phrase against `tests/bdd/steps/` before writing, and update
this section if one changes.

Documentation:

- `docs/users-guide.md`, "Control output and accessibility": replace
  "Colour rendering is not implemented in beta3 …" with the colour contract
  (which outputs `--color` governs; the `D11` behaviour; help follows the
  command line and `NETSUKE_COLOR` only; usage errors and pre-configuration
  errors are plain; subprocess output is forwarded unchanged). State the `D3`
  wording, the `D4` outcome, and that `TERM=dumb` keeps standard mode static.
- `docs/netsuke-design.md`: extend "Theme resolution for CLI output" with the
  display-plan core, its facts, its four adapters, and a link to ADR-041;
  update the "guarded by `insta` snapshots" sentence to name the PTY suite and
  the contract snapshots.
- `docs/netsuke-cli-design-document.md`: document `DisplayPlan`,
  `TerminalFacts`, and the adapters as the component's internal interface, and
  correct the `NO_COLOR` statement to match `D4`.
- `docs/adr-041-display-policy-contract-and-terminal-verification.md`:
  context, the contract table, `D2`, `D3`, `D4`, `D6` to `D9`, `D11`, `D12`,
  and known limitations (no `FORCE_COLOR` input, no ConPTY coverage). Status
  `Accepted` with the date and a one-sentence summary. Add it to the ADR list in
  `docs/contents.md`.
- `docs/developers-guide.md`: a "Terminal rendering tests" subsection under
  "Behavioural testing strategy": the `test_support::terminal` harness and its
  re-use policy, the `env_clear()` rule, why PTY tests are Unix-only,
  stress-running PTY tests, the plan-derived-oracle convention, and the
  `is_terminal` lint.
- `docs/repository-layout.md`: the new integration binary and
  `test_support/src/terminal/`.
- `docs/roadmap.md`: tick 3.12.3 and its four sub-bullets.

Acceptance: `make check-fmt`, `make typecheck`, `make lint`, `make test`,
`make markdownlint`, and `make nixie` pass. Conformance: every trace link
resolves to a passing test; the ADR, design documents, and code agree.
Recovery: documentation commits revert independently. Remaining gaps: those
under `Verification plan`. Compatibility: none.

## Concrete steps

Work from the repository root on branch
`3-12-3-terminal-rendering-regression-tests`. Capture long output with `tee`:

```sh
BRANCH=$(git branch --show-current)
make test 2>&1 | tee "/tmp/test-netsuke-${BRANCH}.out"
```

Focused loops while developing:

```sh
cargo nextest run -p test_support -E 'test(terminal)'
cargo nextest run --test terminal_rendering_tests
cargo nextest run --test cli_tests -E 'test(display_plan)'
cargo nextest run --test bdd_tests -E 'test(terminal)'
cargo nextest run --test terminal_rendering_tests --stress-count 20 -E 'test(pty)'
```

Gates run sequentially, never in parallel (delegate to the `scrutineer` agent
where available):

```sh
make check-fmt
make typecheck
make lint
make test
make markdownlint   # when Markdown changes
make nixie          # when Markdown changes
```

A passing `make test` ends with the nextest summary
(`Summary [...] N tests run: N passed`) and a doctest summary with no failures.

## Validation and acceptance

Manual acceptance on Linux (the `script -qfec` form is util-linux syntax; on
macOS use `script -q /dev/null <command>`):

```sh
cargo build --bin netsuke
BIN="$PWD/target/debug/netsuke"
cd "$(mktemp -d)" && cp "$OLDPWD/tests/data/minimal.yml" Netsukefile
script -qfec "$BIN --color never --verbose generate" /dev/null \
  | grep -o $'\x1b\\[[0-9;]*m' | wc -l        # expect 0 (baseline 416)
script -qfec "$BIN --accessibility on generate" /dev/null \
  | grep -o $'\x1b\\[[0-9;?]*[A-HJKf]' | wc -l # expect 0
script -qfec "$BIN --color never --help" /dev/null \
  | grep -o $'\x1b\\[[0-9;]*m' | wc -l        # expect 0 (baseline 113)
```

Red-Green-Refactor evidence to record in `Artefacts and notes`: the seeded
faults failing the EP-M2 sweep with their case counts; the `L-ENV-SIGNALS`
witness rows failing before `EnvSignals` exists; each EP-M3 commit's red cells
and scenarios failing on the unfixed binary, then passing; the `is_terminal`
lint negative control; the gate results after each milestone.

Quality criteria:

- Tests: `make test` passes, including the new integration binary, BDD
  scenarios, contract snapshots, property tests, and doctests.
- Verification: every obligation in `Verification plan` has its artefact,
  passing evidence, and recorded non-vacuity check.
- Lint and type checks: `make check-fmt`, `make typecheck`, and `make lint`
  pass, with no new suppression other than the one sanctioned `is_terminal`
  `#[expect]` in `main()`.
- Flakiness: every PTY test passes 20 stress runs, idle and under load.

## Idempotence and recovery

Every step is re-runnable. Tests create workspaces in `tempfile` directories
and leave no residue. The PTY harness kills the child's process group on
timeout, so an interrupted run leaves no orphan. Each commit reverts
independently, and the EP-M1 suite guards EP-M2 and EP-M3 against partial
application. Never park work in the shared `git stash`; use a work-in-progress
commit. If `insta` writes a `.snap.new` by mistake, delete it rather than
accepting it.

## Artefacts and notes

Baseline probe, 2026-09-26, commit `ebcedaef`, debug build, Linux. PTY cells
used `script -qfec` (which gives the child a controlling terminal and copies
the parent's window size, unlike the planned harness, so counts are indicative
rather than comparable); pipe cells used plain redirection. `NO_COLOR`,
`FORCE_COLOR`, `CLICOLOR`, and `CLICOLOR_FORCE` were removed and
`TERM=xterm-256color` set unless stated. Columns are SGR sequences and
redraw-style CSI sequences. Every row contained non-ASCII bytes, all of them
Fluent isolate marks except where noted.

```plaintext
gen-auto-tty                       sgr=0    cursor=189
gen-auto-pipe                      sgr=0    cursor=0
gen-acc-on-tty                     sgr=0    cursor=0
gen-colnever-tty                   sgr=0    cursor=0
gen-nocolor-tty                    sgr=0    cursor=0
gen-nocolor-empty-tty              sgr=0    cursor=0
gen-termdumb-tty                   sgr=0    cursor=0
gen-emoji-never-tty                sgr=0    cursor=189
gen-prog-never-tty                 sgr=0    cursor=0
gen-prog-always-pipe               sgr=0    cursor=0
gen-json-colalways-tty             sgr=0    cursor=0
err-colnever-tty (tracing error)   sgr=12   cursor=0
missing-colnever-tty (miette)      sgr=6    cursor=0
missing-nocolor-tty (miette)       sgr=0    cursor=0
missing-emoji-never-tty (miette)   sgr=6    cursor=92   Unicode "×" present
missing-colalways-pipe (miette)    sgr=0    cursor=0
verbose-colnever-tty               sgr=416  cursor=0
verbose-nocolor-tty                sgr=416  cursor=0
verbose-colalways-pipe             sgr=0    cursor=0
help-colnever-tty                  sgr=113  cursor=0
help-nocolor-tty                   sgr=0    cursor=0
```

## Interfaces and dependencies

Libraries, all already in the dependency graph: `nix` 0.31.3 (`term` feature)
for `openpty`; `googletest` 0.14.3 and `pretty_assertions` 1.4.1; `rstest`
0.26.1 and `rstest-bdd` 0.5.0; `insta` 1 (`yaml`); `proptest` 1.11.0;
`assert_cmd` 2; `cap-std` for fixture staging.

`FORCE_COLOR`, `CLICOLOR`, and `CLICOLOR_FORCE` are not Netsuke inputs. The
`CLICOLOR` page now describes itself as deprecated in favour of `NO_COLOR` and
`FORCE_COLOR`, and adopting `FORCE_COLOR` is a feature, not a verification
task. ADR-041 lists it under known limitations.

In `src/output_mode.rs` (existing public functions keep their signatures):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EnvSignals {
    pub no_color_set: bool,
    pub term_dumb: bool,
    pub redraw_capable: bool,
}

impl EnvSignals {
    /// Reduce `NO_COLOR` and `TERM` to signals; the only place strings are read.
    #[must_use]
    pub fn read(read_env: impl Fn(&str) -> Option<String>) -> Self;
}
```

In `src/display_plan.rs` (fields private; queries are `const fn`, so a
contradictory plan cannot be built):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayPolicies {
    pub colour: ColourPolicy,
    pub emoji: EmojiPolicy,
    pub progress: ProgressPolicy,
    pub accessibility: AccessibilityPolicy,
    pub json: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TerminalFacts {
    pub env: EnvSignals,
    pub stdout_is_terminal: bool,
    pub stderr_is_terminal: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReporterKind { Silent, Accessible, Standard }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticStyle { pub colour: bool, pub unicode: bool, pub links: bool }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayPlan { /* private */ }

impl DisplayPlan {
    pub const fn output_mode(self) -> OutputMode;
    pub const fn prefs(self) -> OutputPrefs;
    pub const fn reporter(self) -> ReporterKind;
    pub const fn progress_enabled(self) -> bool;
    pub const fn live_progress(self) -> bool;
    pub const fn force_text_task_updates(self) -> bool;
    pub const fn stderr_colour(self) -> bool;
    pub const fn diagnostic_style(self) -> DiagnosticStyle;
}

impl TerminalFacts {
    pub const fn from_signals(env: EnvSignals, stdout_is_terminal: bool, stderr_is_terminal: bool) -> Self;
}

#[must_use]
pub fn resolve_display_plan(policies: DisplayPolicies, facts: TerminalFacts) -> DisplayPlan;

/// Colour for help output, from the startup hint and stdout's facts.
#[must_use]
pub const fn startup_colour(hint: ColourPolicy, facts: TerminalFacts) -> bool;
```

In `src/cli/preferences.rs`, add
`Cli::display_policies(&self) -> DisplayPolicies`, and move the
`EmojiPolicy -> Option<ThemePreference>` and
`AccessibilityPolicy -> Option<bool>` mappings onto the enums in
`src/cli/config.rs` as `const fn`s that the existing `Cli` accessors delegate
to. ADR-041 records these enums as approved value dependencies of the core (an
open item in RFC 0027).

Changed public signatures: `runner::run(cli: &Cli, plan: DisplayPlan)`,
`runner::run_with_ninja_program(cli: &Cli, plan: DisplayPlan, program:
&Utf8Path)`,
and `cli::parse_with_localizer_from(…, colour: ColorChoice)`. Removed:
`output_mode::resolve` and `output_prefs::resolve_from_theme`. Internal:
`reporter::make_reporter(plan: DisplayPlan, verbose: bool)`. Binary-local:
`src/main_tracing.rs` with `init_tracing`, `set_tracing_filter`, and
`set_tracing_ansi(bool)`; the `miette` hook installer beside
`handle_runner_error`. In `src/cli_l10n.rs`:
`colour_hint_from_args(&[OsString]) -> Option<ColourPolicy>`.

In `test_support/src/terminal/`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TerminalBytes {
    pub has_sgr: bool,
    pub has_redraw: bool,
    pub has_osc: bool,
    pub has_any_escape: bool,
    pub has_bare_cr: bool,
    pub has_bidi_isolates: bool,
    pub has_non_ascii_glyphs: bool,
}

pub fn classify(bytes: &[u8]) -> TerminalBytes;

#[cfg(unix)]
pub enum Attach { Both, StderrOnly, StdoutOnly }

#[cfg(unix)]
pub struct PtyRequest { pub command: std::process::Command, pub attach: Attach, pub timeout: std::time::Duration }

#[cfg(unix)]
pub struct PtyOutput { pub status: std::process::ExitStatus, pub terminal: Vec<u8>, pub piped: Vec<u8> }

#[cfg(unix)]
pub fn run_in_pty(request: PtyRequest) -> anyhow::Result<PtyOutput>;
```

## Revision note

- 2026-09-26: initial draft from Wyvern reconnaissance, external convention
  research, and a baseline PTY probe.
- 2026-09-26: revised after a five-member expert review. Split `D1` and
  added `D11` (`--color always` on pipes) and `D12` (fact gathering through
  existing seams, lint-enforced single resolution, deletion of ambient
  wrappers). Replaced the environment-removal list with `env_clear()`. Moved
  red cells from EP-M1 into the EP-M3 commits that fix them and moved the
  colour scenarios into EP-M3. Replaced the facts-to-string round-trip with
  `EnvSignals` shared by all resolvers. Added the missed consumers
  (`runner/help.rs`, `ExecutionContext`, `indicatif`'s own terminal check),
  `redraw_capable` for `TERM=dumb` and unset `TERM`, `INV-COLOUR-ADDITIVE`,
  label and isolate checks, a richer byte classifier, PTY harness flake
  controls, and completeness guards for enumeration. Dropped `force_graphical`
  from `D8` and scoped `D9` to help and version. Corrected BDD step phrases,
  the Verus rationale, the ADR status handling, and documentation coverage (CLI
  design document, repository layout). The remaining work is unchanged in
  shape: approval, then EP-M0 to EP-M4.

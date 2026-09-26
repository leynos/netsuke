# 3.12.3. Add terminal rendering regression tests

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT

This plan has not been approved. Do not begin implementation until the user
explicitly approves it, including the open decisions `D3`, `D4`, `D8`, and `D9`
in the `Decision log`.

## Purpose / big picture

Netsuke exposes four display-policy flags: `--color auto|always|never`,
`--emoji auto|always|never`, `--progress auto|always|never`, and
`--accessibility auto|on|off`. Each has an environment equivalent
(`NETSUKE_COLOR`, `NETSUKE_EMOJI`, `NETSUKE_PROGRESS`, `NETSUKE_ACCESSIBILITY`)
and a configuration-file key of the same name. Roadmap task 3.12.3 asks for
terminal rendering regression tests which verify that each policy behaves as
documented.

Today the *resolution* of these policies is well tested in isolation: a
648-case sweep in `tests/cli_tests/display_policy_domain.rs` checks the pure
resolver functions. What is not tested is what a user actually sees: the bytes
Netsuke writes to a real terminal or a pipe. No test attaches Netsuke to a
terminal at all, so every `auto` branch that depends on "is this stream a
terminal?" is unverified, and no test asserts the absence of American National
Standards Institute (ANSI) escape sequences.

A baseline probe taken while writing this plan (see `Artefacts and notes`)
shows that the gap hides real defects. On a pseudo-terminal:

- `--color never --verbose` writes 416 ANSI Select Graphic Rendition (SGR,
  the `ESC [ … m` colour and style codes) sequences through the `tracing` log
  formatter, and so does `NO_COLOR=1 --verbose`.
- `--color never` on a failing run writes coloured `miette` diagnostics and a
  coloured `tracing` error line.
- `--color never --help` writes 113 SGR sequences from `clap`.
- `--color always` writes no colour at all when output is piped.

After this change, a contributor can run `make test` and see a regression suite
that drives the real `netsuke` binary through both a pipe and a pseudo-terminal
(PTY, a kernel device that makes a child process believe it is attached to an
interactive terminal) for every policy value, and fails with a reviewable diff
when any policy stops being honoured. The four roadmap sub-bullets become
observable, repeatable facts:

1. `--color never` produces no SGR sequence on any stream; `--color always`
   produces coloured logs and diagnostics even when piped; `--color auto`
   colours only a terminal, and only when `NO_COLOR` is absent and `TERM` is not
   `dumb`.
2. `--emoji never` produces ASCII-only status glyphs; `--emoji always`
   produces Unicode glyphs even in accessible mode; `--emoji auto` follows the
   output mode.
3. `--progress never` produces no stage or task reporting; `auto` and
   `always` produce live, redrawing progress on a terminal and static text
   lines when piped.
4. `--accessibility on` never emits cursor-movement or line-erasing control
   sequences, even on a terminal; `off` keeps live output even when `NO_COLOR`
   or `TERM=dumb` is set; `auto` selects accessible output exactly when
   `NO_COLOR`, `TERM=dumb`, or `--color never` is in effect.

## Constraints

- Preserve every behaviour the users' guide already documents unless a
  decision in this plan, approved by the user, says otherwise. In particular,
  keep: `NO_COLOR` or `TERM=dumb` auto-selecting accessible mode;
  `--color never` acting as an internal `NO_COLOR` (and therefore selecting
  accessible mode under `--accessibility auto`); task updates falling back to
  text when **stdout** is not a terminal; and `--json` disabling progress and
  tracing.
- Do not change the status text, labels, stage numbering, or glyph tokens.
  The `insta` snapshots under `src/snapshots/status/`,
  `src/snapshots/status_timing/`, and `src/snapshots/output_prefs/` must pass
  unchanged. A change to any of them is a tolerance breach.
- Do not add a new crate to the dependency graph. Enabling the `term` feature
  of the existing `nix` dev-dependency (for `nix::pty::openpty`) and adding
  that same `nix` version as a Unix-only dependency of the `test_support` crate
  are within scope once this plan is approved (decision `D5`). Anything else is
  an escalation.
- No in-process environment mutation. Tests must never call
  `std::env::set_var` or `std::env::remove_var`. Environment differences are
  either injected through closures or `mockable::Env`, or applied to a child
  process through `Command::env` and `Command::env_remove` (see
  [ADR-008](../adr-008-environment-seam-taxonomy.md)).
- Every end-to-end test must neutralize ambient terminal signals in the child:
  remove `NO_COLOR`, `FORCE_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE`, and every
  `NETSUKE_*` display variable, and set `TERM` explicitly. A developer with
  `NO_COLOR` exported must get the same results as continuous integration (CI).
- The pure display-policy core must not import `std::io`, `std::env`,
  `tracing_subscriber`, `miette`, `clap`, or `indicatif`. It receives facts and
  returns a decision. Adapters apply the decision.
- Presentation decisions must not flow into build semantics. The resolved
  display plan travels beside the command, never inside an application request
  (RFC 0026, "Application requests and an owned execution port").
- Keep `src/main.rs` (383 lines today) and every other file under the
  400-line cap. Extract before adding.
- The borrow checker is Polonius and the trait solver is next-generation
  (pinned `nightly-2026-08-23`). Do not add turbofish, clones, or bounds to
  appease an older analysis.
- PTY tests are Unix-only and must be gated per test with `#[cfg(unix)]`
  (or placed in `tests/features_unix/`), never by gating a whole shared module,
  so Windows still compiles and runs the piped suite.

If satisfying the objective requires violating a constraint, stop, record the
conflict in `Decision log`, set the status to `BLOCKED`, and escalate.

## Tolerances (exception triggers)

- Scope: if production (non-test) changes exceed 450 net lines or touch more
  than 14 production files, stop and escalate. Test and documentation lines are
  not capped, but any single test file must stay under 400 lines.
- Interface: the public library surface may gain the display-plan types
  described in `Interfaces and dependencies`. Removing or renaming an existing
  public item (for example `output_mode::resolve_with`, `theme::resolve_theme`,
  or `output_prefs::resolve_from_theme`) requires escalation.
- Dependencies: any new crate, or any new feature on a crate other than
  `nix`, requires escalation.
- Snapshots: any change to an existing `.snap` file requires escalation.
- Iterations: if a focused test still fails after three fix attempts, or a
  PTY test fails intermittently in more than one of 20 consecutive local runs,
  stop and escalate.
- Time: if any single milestone takes more than one working day, stop and
  record why.
- Ambiguity: if an adapter (`tracing-subscriber`, `miette`, `clap`) cannot
  honour a decision without replacing the library's renderer, stop and present
  the options.

## Risks

- Risk: PTY-driven tests could be flaky because `indicatif` redraws on a
  12 Hz timer and a PTY read can race process exit. Severity: high. Likelihood:
  medium. Mitigation: assert only order-independent, presence-or-absence facts
  (for example "at least one cursor-movement sequence"), never exact bytes of
  live output. Drain the PTY master until end-of-file (`EIO` on Linux after the
  child closes the slave) on a dedicated reader thread before waiting for the
  child, and bound every PTY test with a timeout. Stage B measures 20
  consecutive runs before any PTY test is committed.
- Risk: ANSI binding for `tracing` must change after the subscriber is
  installed, because `--color` is only known once configuration is merged.
  Severity: medium. Likelihood: medium. Mitigation:
  `tracing_subscriber::fmt::Layer::set_ansi` exists in the locked 0.3.23
  release; wrap the formatting layer in a `reload::Layer` (as the level filter
  already is) and start with ANSI disabled. Buffered startup warnings are
  therefore always plain, which is the safe default. If the reload wrapper does
  not type-check against the existing layered subscriber, escalate with the
  compiler error rather than replacing the subscriber design.
- Risk: `miette::set_hook` can be installed only once per process, and unit
  tests share a process. Severity: low. Likelihood: medium. Mitigation: install
  the hook only in the binary composition root, after the display plan is
  resolved; test the handler options it builds as a pure function, and test the
  installed hook end to end in child processes.
- Risk: `clap` renders `--help` and usage errors before configuration loads,
  so a configuration-file `color = "never"` cannot affect help output.
  Severity: low. Likelihood: certain. Mitigation: mirror the existing
  `resolve_startup_json` precedent: derive a startup colour hint from
  command-line arguments, then `NETSUKE_COLOR`, then `auto`. Document the
  limitation in the users' guide.
- Risk: Fluent wraps interpolated values in Unicode bidirectional isolate
  marks (`U+2068`, `U+2069`), so a naïve "output is ASCII" check fails even
  under `--emoji never` (observed in the baseline). Severity: medium.
  Likelihood: certain. Mitigation: normalize with the existing
  `test_support::fluent::normalize_fluent_isolates` before any ASCII check, and
  state in the contract that the emoji policy governs glyphs, not isolate marks.
- Risk: continuous integration runners may export `NO_COLOR`, `TERM=dumb`,
  or `CI` values that change `auto` decisions. Severity: medium. Likelihood:
  medium. Mitigation: the end-to-end helpers remove and set every relevant
  variable explicitly (see `Constraints`).
- Risk: the macOS PTY driver differs from Linux (no `EIO` at end-of-file;
  reads return 0). Severity: low. Likelihood: medium. Mitigation: treat both
  `Ok(0)` and `EIO` as end-of-file in the harness and cover both with a unit
  test of the end-of-file classifier.

## Progress

- [x] (2026-09-26 22:25Z) Reconnoitred implementation, tests, documentation,
  and tooling with a Wyvern agent team; researched `NO_COLOR`, `FORCE_COLOR`,
  and `CLICOLOR` conventions.
- [x] (2026-09-26 22:25Z) Captured a baseline PTY probe of the current binary
  (see `Artefacts and notes`).
- [x] (2026-09-26 22:40Z) Drafted this ExecPlan.
- [ ] Expert-panel review of the draft and revision.
- [ ] User approval of the plan and of decisions `D3`, `D4`, `D8`, `D9`.
- [ ] EP-M0: terminal test harness (PTY runner and byte classifier).
- [ ] EP-M1: piped and PTY regression suite pinning already-correct
  behaviour.
- [ ] EP-M2: display-plan core and single composition-root resolution.
- [ ] EP-M3: bind colour and glyph decisions to `tracing`, `miette`, and
  `clap`.
- [ ] EP-M4: behavioural scenarios, snapshots, documentation, ADR, and
  roadmap completion.

## Surprises & discoveries

- Observation: `--color` governs no ANSI emitter today. Its only effect is to
  influence accessible-mode selection through `NO_COLOR` semantics. Evidence:
  `src/theme.rs` documents `ColourTokens` as placeholders;
  `src/main.rs::init_tracing` sets `with_ansi(io::stderr().is_terminal())`; no
  `miette::set_hook` or `clap::ColorChoice` exists in `src/`; baseline rows
  `verbose-colnever-tty`, `missing-colnever-tty`, `help-colnever-tty`. Impact:
  the `--color` sub-bullet cannot be verified without fixing the emitters
  (EP-M3).
- Observation: an explicit `with_ansi(true)` in `tracing-subscriber` 0.3.23
  overrides that library's own `NO_COLOR` check, so `NO_COLOR=1` does not
  suppress coloured logs on a terminal. Evidence: `fmt_layer.rs` lines 308–314
  and 743 in the registry source; baseline row `verbose-nocolor-tty` (416 SGR).
  Impact: `NO_COLOR` compliance is currently broken for verbose output.
- Observation: `--progress auto` and `--progress always` resolve identically.
  `Cli::progress_enabled` is false only for `never`, and `indicatif` hides its
  own bars when stderr is not a terminal, mirroring stage messages to plain
  stderr lines instead. Evidence: `src/cli/preferences.rs::progress_enabled`;
  `src/status_indicatif.rs` (`is_hidden` mirroring); baseline rows
  `gen-auto-pipe` and `gen-prog-always-pipe` are byte-identical in size.
  Impact: decision `D3`.
- Observation: the output mode and the "stdout is a terminal" fact are
  resolved twice from ambient state, once in `src/main.rs` and again in
  `src/runner/mod.rs::run_with_ninja_program_resolver`. Evidence: both call
  `output_mode::resolve(…)` which reads `std::env::var`. Impact: EP-M2 resolves
  once and passes the result down.
- Observation: a failed run whose error is not a `RunnerError` prints the
  error twice: once as a `tracing` `ERROR` event and once as the
  `Error:`-prefixed line. Evidence: `src/main.rs::handle_runner_error`; baseline
  `err-auto-tty`. Impact: out of scope; recorded for a follow-up issue. The
  regression suite must not pin the duplicate as intended behaviour.
- Observation: `miette` renders its graphical handler with a Unicode `×`
  glyph even under `--emoji never`. Evidence: baseline row
  `missing-emoji-never-tty`. Impact: decision `D8`.

## Decision log

- Decision D1: treat 3.12.3 as "verify, and repair what verification
  breaks", bounded by the tolerances above. Regression tests that pin a
  documented contract are written first; where the baseline shows the contract
  is broken, the fix lands in the same commit as the test that proves it (red,
  then green). Rationale: a regression suite that encoded `--color never`
  emitting colour would enshrine a defect. The documented contract
  ("`--color never` behaves like an internal `NO_COLOR`",
  `docs/netsuke-design.md`) is clear enough to fix against without a new
  product decision. Date/Author: 2026-09-26, planning agent.
- Decision D2: shape the change as a functional core with an imperative
  shell, not a trait-based port. A new pure module resolves a `DisplayPlan` from
  `DisplayPolicies` (the four policies plus `json`) and a `TerminalFacts` value
  (`NO_COLOR` presence, `TERM=dumb`, stdout-is-terminal, stderr-is-terminal).
  The composition root gathers the facts once and hands the plan to three
  outbound adapters: the status reporter factory, the `tracing` formatter, and
  the diagnostic hook. Rationale: the domain logic is a total function over a
  small finite domain; a value-in, value-out core is simpler and more testable
  than a `TerminalProbe` trait with mock implementations. It also removes the
  duplicated ambient resolution in `src/runner/mod.rs`. This protects the
  boundary without transplanting a pattern. Date/Author: 2026-09-26, planning
  agent.
- Decision D3 (open, requires approval): keep `--progress auto` and
  `--progress always` behaviourally identical, pin that equivalence with a
  test, and document it in the users' guide ("`always` currently behaves like
  `auto`; non-terminal output always falls back to static text"). Alternative:
  give `auto` a distinct meaning, for example suppressing stage lines when
  stderr is not a terminal. That changes CI logs that users rely on,
  contradicts the users' guide ("task progress falls back to text, so logs
  remain readable"), and is a product decision outside a verification task.
  Recommendation: pin the equivalence.
- Decision D4 (open, requires approval): keep treating an *empty* `NO_COLOR`
  as present, as `src/output_mode.rs` documents ("any value, including empty")
  and the users' guide implies ("when … `NO_COLOR` is present"). Pin it with a
  test and record in the ADR that this diverges from the current text of the
  `NO_COLOR` convention, which says "present and not an empty string".
  Alternative: align with the convention, which also matches
  `tracing-subscriber` and `supports-color`. That is a one-line change in
  `no_color_active_with`, but it changes documented behaviour. Recommendation:
  retain current behaviour in this task and raise a follow-up issue; the
  adapters in EP-M3 read the resolved plan, not `NO_COLOR`, so every emitter
  agrees either way.
- Decision D5: build the PTY harness on `nix::pty::openpty` from the existing
  `nix` 0.31.3 dev-dependency by enabling its `term` feature, and add the same
  `nix` requirement to `test_support` under
  `[target.'cfg(unix)'.dependencies]`. Rationale: this adds no crate to
  `Cargo.lock`. `portable-pty`, `expectrl`, and `rexpect` would each add a new
  dependency tree for a few dozen lines of harness. Windows pseudo-console
  (ConPTY) coverage is deferred; Windows keeps the piped suite and the
  pure-core tests. Date/Author: 2026-09-26, planning agent.
- Decision D6: verify the finite policy domain by exhaustive enumeration in
  an ordinary `rstest` test instead of a Kani harness, use `proptest` for the
  two string-facing lemmas (environment abstraction and the escape-sequence
  classifier), and do not use Verus. Rationale: the resolution domain is 3⁴
  policy combinations × 2 (`json`) × 2⁴ facts = 2,592 points, so enumeration is
  a complete proof by exhaustion that runs in milliseconds under `make test`;
  Kani would prove the same property at far higher cost and outside the default
  gate. No unbounded lemma is introduced, and Verus is not adopted in this
  repository
  (`docs/execplans/4-1-3-record-phase-1-scope-boundary-for-verus-and-stateright.md`).
  Date/Author: 2026-09-26, planning agent.
- Decision D7: snapshot a *rendering fingerprint* rather than raw terminal
  bytes. For each policy the suite renders a matrix of cases to a small record
  (`sgr`, `cursor_control`, `ascii_glyphs`, `stage_lines_present`) and stores
  the whole matrix as one `insta` YAML snapshot per policy. Rationale: live
  `indicatif` output is timing-dependent and cannot be snapshotted byte for
  byte (the 3.12.2 plan reached the same conclusion), but the fingerprint is
  deterministic and a changed cell produces a one-line, reviewable diff.
  Date/Author: 2026-09-26, planning agent.
- Decision D8 (open, requires approval): the diagnostic hook built from the
  display plan sets `miette`'s `color` from the stderr colour decision and
  `unicode` from the resolved glyph theme, and under `--color always` also sets
  `force_graphical(true)` so that piped diagnostics are coloured. Alternative
  A: bind colour only, leaving the `×` glyph under `--emoji never` and
  uncoloured piped diagnostics under `--color always`. Alternative B:
  additionally select `miette`'s narratable (screen-reader oriented) handler in
  accessible mode. That is a visible format change for every `NO_COLOR` user,
  because `NO_COLOR` selects accessible mode. Recommendation: adopt the
  proposal; defer Alternative B to a follow-up roadmap item.
- Decision D9 (open, requires approval): `clap` help and usage output follow
  a *startup* colour hint: `--color` on the command line, then `NETSUKE_COLOR`,
  then `auto` (where `clap` applies its own terminal and `NO_COLOR` detection).
  Configuration files are not consulted, mirroring
  `locale_resolution::resolve_startup_json`. Alternative: leave help colouring
  to `clap` and document that `--color` does not apply to help. Recommendation:
  adopt the startup hint; it is a small change with an existing precedent and
  closes the `help-colnever-tty` defect.
- Decision D10: record the display-policy contract and verification strategy
  as ADR-041. ADR-039 and ADR-040 are already claimed on open branches
  (`jm5/kani-change-scoped-gate`,
  `6-1-1-split-rfc-0006-set-into-focused-child-rfcs-and-task`). Re-check every
  remote branch for a clash immediately before committing the ADR, and renumber
  if needed. Date/Author: 2026-09-26, planning agent.

## Outcomes & retrospective

Not started. Complete this section at each milestone boundary and at
completion, reconciling discoveries with the documents in `Conformance basis`.

## Context and orientation

Netsuke is a Rust command-line build tool: it reads a YAML-and-Jinja manifest
(`Netsukefile`) and drives Ninja. The binary entry point is `src/main.rs`; the
library root is `src/lib.rs`. The pieces relevant to this task are listed below.

Policy types and flags. `src/cli/config.rs` defines the four `Clap`-free enums
`ColourPolicy`, `EmojiPolicy`, `ProgressPolicy`, and `AccessibilityPolicy`, and
the `CliConfig` struct that `ortho_config` merges from defaults, configuration
files, `NETSUKE_*` environment variables, and the command line
(`#[ortho_config(prefix = "NETSUKE")]`). `src/cli/command.rs` declares the
`--color`, `--emoji`, `--progress`, and `--accessibility` arguments;
`src/cli/policy_values.rs` supplies localized, case-insensitive value parsers;
`src/cli/preferences.rs` maps the parsed policies to runtime preferences
(`theme_preference`, `accessibility_override`, `progress_enabled`).
`ortho_config` is the layered-configuration library; see
`docs/ortho-config-users-guide.md`. This task adds no configuration field.

Resolution. `src/output_mode.rs::resolve_with` chooses `OutputMode::Accessible`
or `OutputMode::Standard` from the accessibility override, the colour policy,
and an injected `read_env` closure (it only asks whether `NO_COLOR` is present
and whether `TERM` equals `dumb`). `src/theme.rs::resolve_theme` chooses
Unicode or ASCII glyph tokens. `src/output_prefs.rs::resolve_from_theme` wraps
the theme in the `OutputPrefs` value the reporters consume. The thin wrappers
`output_mode::resolve` and `output_prefs::resolve_from_theme` read the real
process environment; they are the only places allowed to call `std::env::var`
here, under `#[expect(clippy::disallowed_methods, …)]`.

Rendering. `src/runner/reporter.rs::make_reporter` selects `AccessibleReporter`
(static labelled lines), `IndicatifReporter` (live `indicatif::MultiProgress`
bars drawn to stderr, `src/status_indicatif.rs`), or `SilentReporter`
(`--progress never`), and wraps it in `VerboseTimingReporter` under `--verbose`.
`should_force_text_task_updates` forces plain task lines when the mode is
accessible or stdout is not a terminal. `src/main.rs::init_tracing` installs the
`tracing` subscriber that formats log events to stderr;
`src/startup_tracing.rs::StartupWriter` buffers early events until the output
mode is known. `src/main.rs::handle_runner_error` prints failures, formatting
`RunnerError` through `miette::Report`'s debug renderer (the process-global
`miette` hook, currently the library default). `clap` renders `--help` and
usage errors in `src/main.rs::parse_cli_or_exit`.

Existing tests. `tests/cli_tests/display_policy_domain.rs` sweeps the pure
resolvers. `src/output_mode.rs`, `src/theme.rs`, `src/output_prefs.rs`, and
`src/runner/reporter.rs` have `rstest` unit tables. Behavioural scenarios use
`rstest-bdd` 0.5.0: feature files live in `tests/features/` (all platforms) and
`tests/features_unix/` (Unix only), are collected by `scenarios!` in
`tests/bdd_tests.rs`, and use the `TestWorld` fixture in
`tests/bdd/fixtures/mod.rs`. Child-process environment changes go through
`tests/bdd/helpers/env_mutation.rs::mutate_env_var`.
`tests/features/progress_output.feature` already drives the binary through a
pipe. `tests/logging_stderr_tests.rs` owns stderr logging contracts with helper
fixtures in `tests/logging_stderr/support.rs`. The shared `test_support` crate
(workspace member) provides `fluent::normalize_fluent_isolates`, `fake_ninja`,
and workspace helpers. No test uses a PTY.

Terms. A *policy* is the user's request (`auto`, `always`, …). A *fact* is an
observation about the environment (`NO_COLOR` present, stderr is a terminal). A
*decision* is what Netsuke does (emit colour on stderr, draw live bars). The
*display plan* is the complete set of decisions for one run. An *SGR sequence*
is `ESC [ params m`, the ANSI code that sets colour or style. A *cursor-control
sequence* is a Control Sequence Introducer (CSI, `ESC [`) sequence that moves
the cursor or erases part of a line (final byte one of `A B C D E F G H J K`),
which is how live progress redraws itself.

Skills and guides to load. Implementers should load the `rust-router` skill and
follow it to `rust-unit-testing` (rstest tables, `googletest` matchers,
`pretty_assertions`, `insta`), `proptest`, and `domain-cli-and-daemons`; the
`hexagonal-architecture` skill for the core/adapter split; `nextest` for runner
configuration; `execplans` for maintaining this document; and
`en-gb-oxendict-style` for prose. Read
`docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`,
`docs/rust-doctest-dry-guide.md`,
`docs/reliable-testing-in-rust-via-dependency-injection.md`,
`docs/ortho-config-users-guide.md`, the "Environment isolation" and
"Behavioural testing strategy" sections of `docs/developers-guide.md`, and
`docs/documentation-style-guide.md` before editing.

## Conformance basis

Upstream artefacts, all at commit `ebcedaef` on `main`:

- `docs/roadmap.md`, section "3.12. Terminal rendering verification", task
  3.12.3 and its four sub-bullets, identified here as `RM-3.12.3-COLOUR`,
  `RM-3.12.3-EMOJI`, `RM-3.12.3-PROGRESS`, and `RM-3.12.3-ACCESS`. The phase 3
  hypothesis ("pleasant, accessible local command-line experience while making
  every command predictable for CI, editor integrations, and agents") is
  `RM-P3-HYP`.
- `docs/netsuke-design.md`, the paragraphs beginning "Real-time stage
  reporting" and "Theme resolution for CLI output": `DES-PROGRESS` (text
  fallback when stdout is not a terminal; accessible mode never animates;
  accessible mode takes precedence) and `DES-COLOUR` (`--color never` behaves
  like an internal `NO_COLOR`; `always` bypasses `NO_COLOR`).
- `docs/users-guide.md`, sections "Policy values and parsing", "Accessible
  output", and "Ninja status progress": `UG-ACCESS` (automatic accessible mode
  on `TERM=dumb` or `NO_COLOR`), `UG-EMOJI` (`always`, `never`, `auto` glyph
  semantics), `UG-TEXT-FALLBACK`.
- [ADR-003](../adr-003-agent-consistent-human-first-cli.md): human-first,
  agent-consistent command-line contract (`ADR-003`).
- [ADR-008](../adr-008-environment-seam-taxonomy.md): environment seams
  (`ADR-008`).
- RFC 0026 (`docs/rfcs/0026-hexagonal-domain-hardening.md`), "Application
  requests and an owned execution port": presentation settings must not decide
  build semantics (`RFC-0026-PRESENTATION`).
- External conventions: `NO_COLOR` (<https://no-color.org/>, "present and not
  an empty string … prevents the addition of ANSI color"; command-line
  arguments and user configuration override it) as `EXT-NO-COLOR`, and
  `FORCE_COLOR` (<https://force-color.org/>) as `EXT-FORCE-COLOR`, which this
  plan does not adopt (see `Interfaces and dependencies`).

There is no Terms of Reference document for this task.

Trace links:

```plaintext
RM-3.12.3-COLOUR   -> DES-COLOUR, EXT-NO-COLOR -> INV-COLOUR-TRUTH, INV-NO-SGR-WHEN-OFF
                   -> EP-M2, EP-M3 -> display_plan::tests::colour_decision_matches_truth_table,
                      terminal_rendering_tests::colour_matrix (PTY), snapshot colour_fingerprint
RM-3.12.3-EMOJI    -> UG-EMOJI -> INV-GLYPHS -> EP-M1, EP-M3
                   -> terminal_rendering_tests::emoji_matrix, snapshot emoji_fingerprint
RM-3.12.3-PROGRESS -> DES-PROGRESS, UG-TEXT-FALLBACK, D3 -> INV-PROGRESS -> EP-M1, EP-M2
                   -> terminal_rendering_tests::progress_matrix, snapshot progress_fingerprint
RM-3.12.3-ACCESS   -> UG-ACCESS, DES-PROGRESS -> INV-ACCESS-STATIC, INV-MODE-TRUTH -> EP-M1, EP-M2
                   -> terminal_rendering_tests::accessibility_matrix, snapshot accessibility_fingerprint
ADR-008            -> INV-SINGLE-RESOLUTION, L-ENV-ABSTRACTION -> EP-M2 -> display_plan property tests
RFC-0026-PRESENTATION -> EP-M2 conformance check (plan never enters a build request)
```

## Verification plan

The implementation is decomposed so that every obligation lands on a pure
function or a byte-level observation of the real binary. The pure core is
verified exhaustively; the adapters are verified end to end against the real
libraries, which are treated as axioms.

Axioms (trusted, not verified here):

- `AX-TTY`: `std::io::IsTerminal::is_terminal` reports `true` for a PTY slave
  and `false` for a pipe on Linux and macOS.
- `AX-TRACING`: `tracing_subscriber::fmt::Layer` emits no SGR sequence when
  `ansi` is false, and `set_ansi` through a `reload::Handle` takes effect for
  subsequent events.
- `AX-MIETTE`: `MietteHandlerOpts::color(false)` yields a handler that emits no
  SGR sequence; `unicode(false)` selects ASCII drawing characters;
  `force_graphical(true)` selects the graphical handler on a pipe.
- `AX-CLAP`: `Command::color(ColorChoice::Never)` suppresses styling in
  rendered help and errors; `ColorChoice::Always` forces it.
- `AX-INDICATIF`: a `ProgressDrawTarget::stderr_with_hz` target is hidden when
  stderr is not a terminal and draws with cursor-control sequences when it is.
- `AX-PTY`: `nix::pty::openpty` returns a connected master/slave pair; after
  the child exits and every slave descriptor is closed, reading the master
  returns `EIO` (Linux) or `Ok(0)` (macOS).

Each adapter-facing axiom is exercised end to end by EP-M1 and EP-M3 tests
rather than assumed silently: if an axiom is false, a PTY test fails.

Obligations:

- Obligation `INV-MODE-TRUTH`: for every policy combination and fact vector,
  `DisplayPlan::output_mode` is `Accessible` exactly when accessibility is
  `on`, or accessibility is `auto` and (colour is `never`, or colour is `auto`
  and `NO_COLOR` is present, or `TERM` is `dumb`). Method: exhaustive
  enumeration against an independent truth model. Rationale: the domain is
  finite (2,592 points), so enumeration is complete. Domain: all
  `ColourPolicy × EmojiPolicy × ProgressPolicy ×
  AccessibilityPolicy × json × no_color × term_dumb × stdout_tty × stderr_tty`.
  Artefact: `src/display_plan/tests.rs::display_plan_matches_truth_model`.
  Evidence: `cargo nextest run -E 'test(display_plan)'`; red before
  `resolve_display_plan` exists (compile failure is the expected red), green
  after. Non-vacuity: the test also counts, per output field, how many cases
  produce each value and asserts every value is reached (for example both
  modes, both glyph sets, all three reporter kinds, both colour decisions per
  stream). Seeded fault: swapping the `TERM=dumb` and `NO_COLOR` precedence, or
  dropping the `ColourPolicy::Never` arm, must fail the sweep; record the
  failing case count in `Artefacts and notes`.
- Obligation `INV-COLOUR-TRUTH`: `stderr_colour` is `false` for `never`,
  `true` for `always`, and for `auto` equals
  `stderr_is_terminal ∧ ¬no_color ∧ ¬term_dumb`; `json` forces `false`. The
  same holds for `stdout_colour` with `stdout_is_terminal`. Method, domain,
  artefact, and non-vacuity: as `INV-MODE-TRUTH` (same sweep).
- Obligation `INV-PROGRESS`: the reporter kind is `Silent` when progress is
  `never` or `json` is set; otherwise `Accessible` in accessible mode and
  `Live` in standard mode. `force_text_task_updates` equals
  `accessible ∨ ¬stdout_is_terminal`. `auto` and `always` yield identical plans
  (decision `D3`). Method: same sweep, plus a dedicated `rstest` asserting
  `plan(auto) == plan(always)` for every other coordinate, so a future
  divergence is a deliberate, visible change.
- Obligation `INV-SINGLE-RESOLUTION`: one run consults the environment and
  terminal state once, and every consumer (reporter factory, `tracing`,
  diagnostic hook, runner) receives the same `DisplayPlan`. Method: structural.
  `run_with_ninja_program_resolver` and `make_reporter` accept the plan instead
  of calling `output_mode::resolve` or `is_terminal`; a `rg` check in
  `Concrete steps` shows no remaining `is_terminal()` or
  `output_mode::resolve(` call outside the composition root. Negative control:
  reintroducing the call in the runner makes the check print a match.
- Lemma `L-ENV-ABSTRACTION`: for every environment (arbitrary strings, absent
  or present, for `NO_COLOR` and `TERM`), resolving through
  `TerminalFacts::from_env` gives the same output mode and theme as the existing
  `output_mode::resolve_with` and `theme::resolve_theme` given the raw
  environment. This justifies reducing the environment to two booleans. Method:
  `proptest` over `Option<String>` pairs, with a generator biased to include
  `""`, `"dumb"`, `"DUMB"`, `" dumb"`, and `"0"`. Artefact:
  `src/display_plan/tests.rs::terminal_facts_preserve_resolution`. Evidence:
  default 256 cases; record the class counts (`prop_assert` with a
  classification of absent, empty, `dumb`, other) and require each class to be
  observed. Non-vacuity: a seeded fault that lower-cases `TERM` before
  comparing must be rejected by the `"DUMB"` class.
- Lemma `L-CLASSIFIER`: the test-support byte classifier reports an SGR
  sequence exactly when the input contains `ESC [ P* m` (with `P` in `0-9;:`),
  reports cursor control exactly when it contains a CSI sequence whose final
  byte is in `A–K`, and never reports either for input without `ESC`. Method:
  model-based `proptest`. Generate a vector of tokens (plain ASCII text,
  Unicode text, SGR with random parameters, cursor-movement, erase-line,
  carriage return, a lone `ESC`), render them to bytes, classify, and compare
  with facts computed from the token vector. Artefact:
  `test_support/src/terminal/classify.rs` tests. Non-vacuity: assert every
  token kind appears in at least 5% of cases; include named witnesses
  (`"\x1b[31mred\x1b[0m"`, `"\x1b[2K\r"`, `"\x1b[1A"`) and the baseline byte
  files as fixtures; a seeded fault that accepts only digit parameters must
  fail on `ESC [ 38;5;208 m`-style colon or semicolon parameters.
- Obligation `INV-NO-SGR-WHEN-OFF`: when the stderr colour decision is off,
  no byte written by the real binary to stderr contains an SGR sequence, for
  every emitter Netsuke controls (status reporter, verbose `tracing`, error
  `tracing` line, `miette` diagnostics); likewise for stdout and `clap` help.
  Method: end-to-end PTY and pipe matrix (EP-M1 green cells, EP-M3 red then
  green cells). Artefact: `tests/terminal_rendering_tests.rs` and
  `tests/terminal_rendering/*.rs`. Evidence: red on the current binary for
  `verbose-colnever-tty`, `missing-colnever-tty`, `help-colnever-tty`, and
  `verbose-nocolor-tty` (baseline counts 416, 6, 113, and 416); green after
  EP-M3. Non-vacuity: the paired `--color always` cells must observe at least
  one SGR sequence on the same emitter, proving the emitter was exercised and
  the classifier can see it.
- Obligation `INV-ACCESS-STATIC`: in accessible mode no stream contains a
  cursor-control sequence, and stderr contains every `Stage N/6` label in
  order. Method: end-to-end PTY matrix over `--accessibility on`, `NO_COLOR=1`,
  `TERM=dumb`, and `--color never` on a terminal. Non-vacuity: the paired
  `--accessibility off` cell on the same PTY must observe cursor-control
  sequences (baseline: 189).
- Obligation `INV-GLYPHS`: after Fluent isolate normalization, status lines
  are ASCII-only when the plan selects ASCII glyphs and contain at least one
  Unicode status glyph when it selects Unicode; with decision `D8`, the same
  holds for `miette` diagnostic drawing characters. Method: end-to-end pipe
  matrix (all platforms) plus PTY cells for the diagnostic path. Non-vacuity:
  the `--emoji always` cell must contain a non-ASCII glyph.

Rejected methods and residual gaps: Kani and Verus are not used (decision
`D6`). Windows terminal behaviour is covered only by the pure core and the
piped suite; ConPTY coverage is a residual gap. `FORCE_COLOR`, `CLICOLOR`, and
`CLICOLOR_FORCE` remain unsupported by Netsuke's own decision (libraries may
still honour them under `auto`, which the tests neutralize).

## Plan of work

Stage A (understand and propose) is complete: this document, the baseline
probe, and the expert review. Stages B to D follow as milestones.

Stage B builds the test harness and the green regression suite (EP-M0, EP-M1).
It changes no production code, so it proves the harness against behaviour that
is already correct and records red evidence for the defects.

Stage C introduces the display-plan core and routes every consumer through it
(EP-M2), then binds the adapters (EP-M3). Each red cell recorded in stage B
turns green in the commit that fixes it.

Stage D adds the behavioural scenarios and fingerprint snapshots, updates the
documentation, writes the ADR, and marks the roadmap entry done (EP-M4).

The go/no-go point is the end of EP-M0: if the PTY harness cannot reach 20
consecutive clean runs of its own smoke test, stop and escalate before writing
any PTY regression test.

## Milestones and plateaus

### EP-M0: terminal test harness

Outcome: `test_support` gains a `terminal` module with two parts. The first is
`classify.rs` (all platforms): a pure function
`classify(bytes: &[u8]) -> TerminalBytes` returning whether the bytes contain
SGR sequences, cursor-control sequences, and non-ASCII glyphs after Fluent
isolate normalization. The second is `pty.rs` (`#[cfg(unix)]`):
`run_in_pty(command: std::process::Command, attach: Attach, timeout: Duration)
-> Result<PtyOutput>`,
where `Attach` selects which of stdout and stderr go to the PTY slave (the
other is piped), so the stdout-terminal and stderr-terminal facts can be varied
independently. The harness spawns the child with the slave as its controlling
streams, closes the parent's slave descriptor, drains the master on a reader
thread until end-of-file, then waits for the child, killing it on timeout.

Before writing either file, sweep for an existing equivalent
(`rg -n 'openpty|strip_ansi|x1b\\[' test_support tests src`) and record the
result in `Decision log`; the baseline sweep found only `strip-ansi-escapes`
usage in `tests/yaml_error_tests.rs`, which strips rather than classifies.
Document the module's scope (terminal output observation for tests only; no
production use; call sites are integration tests and BDD steps) in its `//!`
comment and in `docs/developers-guide.md`.

Tests: `L-CLASSIFIER` property and witnesses; end-of-file classification unit
test (`EIO` and `Ok(0)` both end the read); a PTY smoke test that runs
`/bin/sh -c 'test -t 1 && test -t 2 && printf ok'` and expects `ok`, plus the
inverse with stdout piped.

Requirements: enables every `RM-3.12.3-*` item. Acceptance: `make test` passes;
the smoke test passes 20 consecutive runs
(`for i in $(seq 20); do cargo nextest run -p test_support -E
'test(pty_smoke)' || break; done`).
Conformance check: no production change; no new crate in `Cargo.lock`
(`git diff --stat Cargo.lock` shows only feature metadata, if anything).
Recovery: the module is additive; revert the commit. Remaining gaps: no Netsuke
behaviour tested yet. Compatibility decision: none.

### EP-M1: regression suite for already-correct behaviour

Outcome: a new integration-test binary `tests/terminal_rendering_tests.rs` with
child modules under `tests/terminal_rendering/` (`support.rs`, `emoji.rs`,
`progress.rs`, `accessibility.rs`, `colour_mode.rs`), each under 400 lines.
`support.rs` builds a `netsuke` `Command` via `assert_cmd::cargo::cargo_bin`,
removes `NO_COLOR`, `FORCE_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE`, `CI`, and every
`NETSUKE_*` display variable, sets `TERM=xterm-256color` unless the case
overrides it, and runs the `generate` command against `tests/data/minimal.yml`
copied into a temporary directory (reuse the fixture pattern in
`tests/logging_stderr/support.rs`).

Cells pinned here are those the baseline shows to be correct already:

- Emoji (pipe, all platforms; PTY on Unix): `never`, `always`, `auto`
  crossed with `--accessibility on|off`, asserting `INV-GLYPHS` on status lines.
- Progress: `never` has no `Stage` label and no cursor control on either
  transport; `auto` and `always` on a pipe produce the six `Stage N/6` labels
  in order and no cursor control; on a PTY with standard mode both produce
  cursor control; stdout piped with stderr on the PTY still forces text task
  updates (use the fake Ninja from `tests/logging_stderr/support.rs` emitting
  `[1/2]` status lines).
- Accessibility: `on` on a PTY has no cursor control and all six labels;
  `off` with `NO_COLOR=1` or `TERM=dumb` on a PTY still has cursor control;
  `auto` with `NO_COLOR=1`, `NO_COLOR=` (empty, decision `D4`), `TERM=dumb`, or
  `--color never` is static.
- JSON: `--json --color always` on a PTY has no SGR and no cursor control on
  either stream.

Use `googletest` matchers (`expect_that!`, `contains_substring`, `not(…)`) for
byte-classification assertions so each failing cell names the policy
combination, and `pretty_assertions::assert_eq` for ordered label lists.
Parameterize with `#[rstest]` `#[case]` rows, one row per semantically distinct
cell.

Also write, but do not commit, the red cells for `INV-NO-SGR-WHEN-OFF`
(`--color never --verbose`, `NO_COLOR=1 --verbose`, `--color never` on a
missing manifest, `--color never --help`, all on a PTY) and the
`--color always` piped cells. Run them and paste the failures into
`Artefacts and notes`; they are committed with their fixes in EP-M3.

Acceptance: `make test` passes with the new green suite; the red run is
recorded. Conformance: no production change; the suite asserts only documented
behaviour. Recovery: additive; revert. Remaining gaps: colour. Compatibility
decision: none.

### EP-M2: display-plan core and single resolution

Outcome: a new library module `src/display_plan.rs` (with
`src/display_plan/tests.rs`) owning `DisplayPolicies`, `TerminalFacts`,
`DisplayPlan`, `ReporterKind`, and `resolve_display_plan`. Before creating it,
sweep for an equivalent (`OutputPrefs`, `ThemeContext`, `ResolvedTheme`, and
`ReporterOptions` were reviewed during planning: each captures one facet, none
captures the whole decision); record the sweep and the module's re-use policy
(pure; no I/O; callers are the binary composition root and tests; adapters read
the plan but never re-derive it) in its `//!` comment and in the "Theme
resolution for CLI output" paragraph of `docs/netsuke-design.md`.

`resolve_display_plan` composes the existing precedence functions instead of
duplicating them: it calls `output_mode::resolve_with` and
`output_prefs::resolve_from_theme_with` with a reader derived from
`TerminalFacts`, so `src/output_mode.rs` and `src/theme.rs` remain the single
source of precedence rules.
`TerminalFacts::from_env(read_env, stdout_is_terminal, stderr_is_terminal)` is
the only constructor that inspects raw environment strings.

Rewire consumers in the same commit (no compatibility shims):
`src/main.rs::run_with_args` and `run_cli` gather facts once (the
`is_terminal()` calls and the `std::env::var` reader move to one composition
root helper), resolve the plan, and pass it to `runner::run`; the runner and
`reporter::make_reporter` take the plan instead of calling
`output_mode::resolve` and `is_terminal()`. Move `init_tracing`,
`set_tracing_filter`, and the new ANSI setter from `src/main.rs` into a
binary-local module `src/main_tracing.rs` (declared with `#[path]` like
`startup_tracing`) to keep `src/main.rs` under 400 lines.

Tests (red first): the exhaustive sweep (`INV-MODE-TRUTH`, `INV-COLOUR-TRUTH`,
`INV-PROGRESS`), the `auto == always` row, and the `L-ENV-ABSTRACTION`
property, written before `resolve_display_plan` exists. Doctests on
`resolve_display_plan` and `TerminalFacts::from_env` show one example each,
following `docs/rust-doctest-dry-guide.md`.

Acceptance: `make check-fmt`, `make typecheck`, `make lint`, and `make test`
pass; the EP-M1 suite is unchanged and green, proving behaviour preservation;
`rg -n 'is_terminal\(\)|output_mode::resolve\(' src` lists only the
composition-root helper. Conformance: the plan never enters a build request
(`RFC-0026-PRESENTATION`); public additions match
`Interfaces and dependencies`; no snapshot changes. Recovery: revert the single
commit; the EP-M1 suite detects partial rewiring. Remaining gaps: adapters
still ignore the colour decision. Compatibility decision: none; every caller is
updated in the same commit.

### EP-M3: bind colour and glyph decisions to the emitters

Three atomic commits, each pairing the previously recorded red cells with the
fix.

1. `tracing`: build the formatting layer with ANSI disabled, wrap it in a
   `reload::Layer`, and after the plan is resolved call
   `set_ansi(plan.stderr_colour)` through the stored handle. Commit the
   `--verbose` colour cells (`never`, `NO_COLOR`, `always` piped, `auto` on a
   PTY).
2. `miette`: add a pure function
   `diagnostic_handler_options(plan: &DisplayPlan) -> MietteHandlerOpts`
   (unit-tested for every relevant plan) and install it once with
   `miette::set_hook` in the composition root after the plan is resolved, per
   decision `D8`. Commit the missing-manifest diagnostic cells, including the
   ASCII-glyph cell for `--emoji never`.
3. `clap`: add `cli::colour_hint_from_args` mirroring
   `cli::json_hint_from_args`, and `resolve_startup_colour(args, env)` mirroring
   `locale_resolution::resolve_startup_json`, and apply `Command::color`
   before parsing, per decision `D9`. Commit the `--help` and usage-error cells.

Acceptance: every red cell recorded in EP-M1 is green; the paired non-vacuity
cells observe SGR; all gates pass after each commit. Conformance: adapters read
the plan and never consult `NO_COLOR` or `is_terminal()` themselves; no new
dependency. Recovery: each commit reverts independently. Remaining gaps:
behavioural scenarios and documentation. Compatibility decision: none.

### EP-M4: behavioural scenarios, fingerprints, documentation

Outcome: `tests/features/terminal_rendering.feature` (piped, all platforms) and
`tests/features_unix/terminal_rendering_pty.feature` (PTY), with steps in
`tests/bdd/steps/terminal_rendering.rs` registered in the steps module and
reusing `mutate_env_var`. Four `insta` YAML fingerprint snapshots (decision
`D7`), one per policy, generated from the EP-M1 matrices in
`tests/terminal_rendering/fingerprints.rs`.

The feature specification (keep it synchronized with the implementation):

```gherkin
Feature: Terminal rendering policies

  Background:
    Given a minimal Netsuke workspace
    And the child environment has no terminal colour signals

  Scenario Outline: Emoji policy selects the status glyph set
    When netsuke is run with arguments "--emoji <emoji> --accessibility <access> --progress always generate"
    Then the command should succeed
    And stderr status glyphs should be <glyphs>

    Examples:
      | emoji  | access | glyphs  |
      | never  | off    | ASCII   |
      | always | on     | Unicode |
      | auto   | off    | Unicode |
      | auto   | on     | ASCII   |

  Scenario: Progress never suppresses stage reporting
    When netsuke is run with arguments "--progress never generate"
    Then the command should succeed
    And stderr should not contain "Stage 1/6"

  Scenario: Colour always colours piped verbose logs
    When netsuke is run with arguments "--color always --verbose generate"
    Then the command should succeed
    And stderr should contain an SGR sequence

  Scenario: Colour never keeps piped output plain
    When netsuke is run with arguments "--color never --verbose generate"
    Then the command should succeed
    And stderr should not contain any terminal escape sequence
```

```gherkin
Feature: Terminal rendering on a pseudo-terminal

  Background:
    Given a minimal Netsuke workspace
    And the child environment has no terminal colour signals

  Scenario: Accessible mode never redraws on a terminal
    When netsuke is run on a terminal with arguments "--accessibility on generate"
    Then the command should succeed
    And the terminal output should not contain cursor control sequences
    And the terminal output should contain "Stage 6/6"

  Scenario: Standard mode draws live progress on a terminal
    When netsuke is run on a terminal with arguments "--accessibility off generate"
    Then the command should succeed
    And the terminal output should contain cursor control sequences

  Scenario: NO_COLOR selects accessible output on a terminal
    Given the child environment variable "NO_COLOR" is "1"
    When netsuke is run on a terminal with arguments "generate"
    Then the terminal output should not contain cursor control sequences

  Scenario: Colour never removes colour from terminal diagnostics
    Given an empty Netsuke workspace
    When netsuke is run on a terminal with arguments "--color never generate"
    Then the command should fail
    And the terminal output should not contain an SGR sequence
```

Reuse existing steps (`a minimal Netsuke workspace`,
`netsuke is run with arguments …`, `the command should succeed`,
`stderr should contain …`) and add only the new ones. Confirm the step wording
against `tests/bdd/steps/` before writing and adjust this block if an existing
step already covers a phrase.

Documentation:

- `docs/users-guide.md`, "Control output and accessibility": replace
  "Colour rendering is not implemented in beta3 …" with the colour contract
  (which outputs `--color` governs; `always` colours piped logs and
  diagnostics; help follows the command line and `NETSUKE_COLOR` only); state
  the `auto`/`always` progress equivalence (`D3`) and the empty `NO_COLOR`
  behaviour (`D4`).
- `docs/netsuke-design.md`: extend "Theme resolution for CLI output" with the
  display-plan core, its facts, its three adapters, and a reference to the ADR;
  replace the "guarded by `insta` snapshots" sentence with one that also names
  the PTY regression suite.
- `docs/adr-041-display-policy-contract-and-terminal-verification.md`
  (status `Accepted` once this plan is approved): the contract table, the
  functional-core decision, decisions `D3`, `D4`, `D6` to `D9`, and the
  divergence from `NO_COLOR`'s empty-string rule. Add it to the ADR list in
  `docs/contents.md`.
- `docs/developers-guide.md`: a "Terminal rendering tests" subsection under
  "Behavioural testing strategy" covering the `test_support::terminal` harness,
  the ambient-signal neutralization rule, why PTY tests are Unix only, and the
  fingerprint-snapshot convention.
- `docs/roadmap.md`: tick 3.12.3 and its four sub-bullets.

Acceptance: `make check-fmt`, `make typecheck`, `make lint`, `make test`,
`make markdownlint`, and `make nixie` pass. Conformance: every trace link in
`Conformance basis` resolves to a passing test; the ADR and design document
agree with the code. Recovery: documentation commits revert independently.
Remaining gaps: those listed under `Verification plan`. Compatibility decision:
none.

## Concrete steps

Run everything from the repository root,
`/home/leynos/.lody/repos/github---leynos---netsuke/worktrees/<worktree>`, on
branch `3-12-3-terminal-rendering-regression-tests`. Capture long output with
`tee` into `/tmp`, for example:

```sh
BRANCH=$(git branch --show-current)
make test 2>&1 | tee "/tmp/test-netsuke-${BRANCH}.out"
```

Focused loops while developing:

```sh
cargo nextest run -p test_support -E 'test(terminal)'
cargo nextest run --test terminal_rendering_tests
cargo nextest run --lib -E 'test(display_plan)'
cargo nextest run --test bdd_tests -E 'test(terminal_rendering)'
```

Structural check for `INV-SINGLE-RESOLUTION` after EP-M2 (expect exactly the
composition-root helper):

```sh
rg -n 'is_terminal\(\)|output_mode::resolve\(' src
```

Gates after each milestone, sequentially, never in parallel (delegate to the
`scrutineer` agent where available):

```sh
make check-fmt
make typecheck
make lint
make test
make markdownlint   # documentation milestones
make nixie          # documentation milestones
```

Expected tail of a passing `make test` includes the nextest summary line
(`Summary […] N tests run: N passed`) and the doctest summary with no failures.

## Validation and acceptance

Behavioural acceptance, observable by a human on Linux or macOS:

```sh
cargo build --bin netsuke
cd "$(mktemp -d)" && cp "$OLDPWD/tests/data/minimal.yml" Netsukefile
script -qfec "$OLDPWD/target/debug/netsuke --color never --verbose generate" /dev/null \
  | grep -c $'\x1b\\[[0-9;]*m'        # expect 0 (baseline: 416 lines matched)
script -qfec "$OLDPWD/target/debug/netsuke --accessibility on generate" /dev/null \
  | grep -c $'\x1b\\[[0-9;]*[A-K]'    # expect 0
"$OLDPWD/target/debug/netsuke" --color always --verbose generate 2>&1 >/dev/null \
  | grep -c $'\x1b\\[[0-9;]*m'        # expect > 0 (baseline: 0)
```

Red-Green-Refactor evidence to record in `Artefacts and notes`:

- Red: EP-M1's uncommitted colour cells fail on the unmodified binary with
  messages naming the cell and the SGR count; EP-M2's sweep fails to compile
  before `resolve_display_plan` exists.
- Green: each EP-M3 commit turns its cells green; EP-M2's sweep passes.
- Refactor: after each milestone, rerun the focused loop and the full gates.

Quality criteria:

- Tests: `make test` passes, including the new integration binary, BDD
  scenarios, fingerprint snapshots, property tests, and doctests.
- Verification: every obligation in `Verification plan` has its artefact,
  passing evidence, and recorded non-vacuity check.
- Lint and type checks: `make check-fmt`, `make typecheck`, and `make lint`
  pass with no new suppression.
- Flakiness: each PTY test passes 20 consecutive local runs before commit.

## Idempotence and recovery

Every step is re-runnable. Tests create their workspaces in `tempfile`
directories and leave no residue. The PTY harness kills and reaps the child on
timeout, so an interrupted run leaves no orphan. Each milestone is one or a few
commits that revert independently; the EP-M1 suite guards EP-M2 and EP-M3
against partial application. If a snapshot is generated by mistake, delete the
new `.snap.new` file rather than accepting it.

## Artefacts and notes

Baseline probe, taken on 2026-09-26 against commit `ebcedaef` with a debug
build, using `script -qfec` for the PTY cells and plain redirection for the
pipe cells, with `NO_COLOR`, `FORCE_COLOR`, `CLICOLOR`, and `CLICOLOR_FORCE`
removed and `TERM=xterm-256color` unless stated. Counts are SGR sequences,
cursor-control sequences, and whether any byte is non-ASCII (every row is
non-ASCII because of Fluent isolate marks):

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
missing-emoji-never-tty (miette)   sgr=6    cursor=92   (Unicode "×" present)
missing-colalways-pipe (miette)    sgr=0    cursor=0
verbose-colnever-tty               sgr=416  cursor=0
verbose-nocolor-tty                sgr=416  cursor=0
verbose-colalways-pipe             sgr=0    cursor=0
help-colnever-tty                  sgr=113  cursor=0
help-nocolor-tty                   sgr=0    cursor=0
```

## Interfaces and dependencies

Libraries: `nix` 0.31.3 (`term` feature) for `openpty`; `googletest` 0.14.3 and
`pretty_assertions` 1.4.1 for assertions; `rstest` 0.26.1 and `rstest-bdd`
0.5.0 for tables and scenarios; `insta` 1 (`yaml`) for fingerprints; `proptest`
1.11.0 for the two lemmas; `assert_cmd` 2 for building the child command. All
are already in the dependency graph.

`FORCE_COLOR`, `CLICOLOR`, and `CLICOLOR_FORCE` are deliberately not added to
`TerminalFacts`. The `CLICOLOR` convention now describes itself as deprecated
in favour of `NO_COLOR` and `FORCE_COLOR`, and adopting `FORCE_COLOR` is a
user-visible feature, not a verification task. The ADR records this as an
outstanding decision.

In `src/display_plan.rs`, define (names may be refined during review, but the
shape is fixed by this plan):

```rust
/// The four display policies plus JSON mode, as merged by OrthoConfig.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayPolicies {
    pub colour: ColourPolicy,
    pub emoji: EmojiPolicy,
    pub progress: ProgressPolicy,
    pub accessibility: AccessibilityPolicy,
    pub json: bool,
}

/// Observations about the process's terminal environment, gathered once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalFacts {
    pub no_color: bool,
    pub term_dumb: bool,
    pub stdout_is_terminal: bool,
    pub stderr_is_terminal: bool,
}

impl TerminalFacts {
    /// Reduce raw environment reads and stream checks to facts.
    pub fn from_env(
        read_env: impl Fn(&str) -> Option<String>,
        stdout_is_terminal: bool,
        stderr_is_terminal: bool,
    ) -> Self;
}

/// How stage and task progress is reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReporterKind { Silent, Accessible, Live }

/// Every rendering decision for one run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayPlan {
    pub output_mode: OutputMode,
    pub prefs: OutputPrefs,
    pub reporter: ReporterKind,
    pub force_text_task_updates: bool,
    pub stdout_colour: bool,
    pub stderr_colour: bool,
}

/// Resolve the display plan; pure and total.
pub fn resolve_display_plan(policies: DisplayPolicies, facts: TerminalFacts) -> DisplayPlan;
```

`DisplayPolicies` gains a `From<&Cli>` conversion in `src/cli/preferences.rs`.
In the binary, `src/main_tracing.rs` owns `init_tracing`, `set_tracing_filter`,
and `set_tracing_ansi(bool)`; a pure
`diagnostic_handler_options(&DisplayPlan) -> miette::MietteHandlerOpts` lives
beside `handle_runner_error`. In `src/cli/`, add
`colour_hint_from_args(&[OsString]) -> Option<ColourPolicy>`; in
`src/locale_resolution.rs`, add
`resolve_startup_colour(&[OsString], &impl LocaleEnvProvider) -> ColourPolicy`.

In `test_support/src/terminal/`, define `classify(&[u8]) -> TerminalBytes`
(fields `has_sgr`, `has_cursor_control`, `has_non_ascii_glyphs`) and, under
`#[cfg(unix)]`, `run_in_pty(Command, Attach, Duration) -> Result<PtyOutput>`
with `enum Attach { Both, StderrOnly, StdoutOnly }` and
`PtyOutput { status, terminal: Vec<u8>, piped: Vec<u8> }`.

## Revision note

- 2026-09-26: initial draft from Wyvern reconnaissance, external convention
  research, and a baseline PTY probe. Awaiting expert-panel review and user
  approval.

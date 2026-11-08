# 3.12.3. Add terminal rendering regression tests

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: IN PROGRESS

On 2026-10-23 the user accepted the recommended outcomes of decisions `D3`,
`D4` (Option A), `D8`, `D9`, and `D11`, and deferred implementation. The user
approved implementation on 2026-10-24.

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
   terminal, `NO_COLOR` is not set to a non-empty value, and `TERM` is not
   `dumb`. `--color always` styles logs, diagnostics, and help even when the
   stream is a pipe (decision `D11`).
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
  ([ADR-0008](../adrs/adr-0008-environment-seam-taxonomy.md)).
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
- Keep every file under 400 lines, including `src/main.rs` (379 lines today),
  `src/runner/mod.rs` (385 lines today) and
  `tests/cli_tests/display_policy_domain.rs` (397 lines today). Extract before
  adding.
- The borrow checker is Polonius and the trait solver is next-generation
  (pinned `nightly-2026-10-22`). Do not add turbofish, clones, or bounds to
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
    Netsuke PTY test exists. The mutex coordinates PTY launches through this
    helper only; an unrelated process spawn can race the short interval between
    `openpty` and setting `FD_CLOEXEC`. Such a child may briefly retain an end
    until it exits, so bounded EOF and reader waits plus stress runs under
    concurrent load cover the residual risk.
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
- [x] (2026-10-23) User accepted decisions `D3`, `D4` (Option A), `D8`, `D9`,
  and `D11` as recommended; implementation was not yet authorized.
- [x] (2026-10-24) User approved implementation.
- [x] (2026-10-24) Rebased the approved branch onto the live PR target,
  `origin/main` at `f8df5d808f1bee4eb8554df0160119864ade6f39`; retained the
  branch commits and dropped an obsolete sync merge already contained in the
  target.
- [x] (2026-10-24) `make check-fmt` passed. The first `make test` reached
  4,839 passing tests, then timed out in a nested public API `cargo check` at
  the default 300-second limit while compiler admission was busy. Decision
  `D13` records the scoped timeout adjustment; rerun the gates after that fix.
- [x] (2026-10-24) Rechecked harness ownership and the locked `nix` APIs;
  record the test-only scope, dependency feature gates, and PTY descriptor
  inheritance limits in the relevant milestones below.
- [x] (2026-10-24) Baseline `make lint`, `make doc-coverage`,
  `make typecheck`, `make markdownlint`, `make nixie`, and
  `make test-workflow-contracts` passed. The second `make test` run passed
  4,856 tests, then timed out `case_2_compiler_diagnostic` at 900.139 seconds;
  seven tests were not run. A contemporaneous status snapshot shows the
  fixture's `rustc` request queued behind one retained cache drain and three
  active compiler groups. This confirms shared contention at that snapshot, but
  not that it accounts for the full test duration; Decision `D14` records the
  limit and follow-up evidence.
- [x] (2026-10-25) Implemented EP-M0's byte classifier and Unix PTY harness.
  Focused `test_support` nextest passed 137/137 tests, and the PTY smoke suite
  passed 20 iterations of all three cases while the host compile queue was
  active. The full serial gates passed: `make check-fmt`, `make lint`,
  `make doc-coverage` (98.21%), `make typecheck`, `make test` (4,884 passed, 6
  skipped; all doctests passed), `make markdownlint`, `make nixie`, and
  `make test-workflow-contracts` (1,339 passed, 3 skipped, plus 27 passed). The
  separate PTY smoke stress run while another `make test` loads the host also
  passed 20 iterations of all three cases (60/60 executions); an idle-pool run
  remains to be recorded before EP-M0 is complete.
- [x] (2026-10-25) Implemented the EP-M1 regression binary with 31 piped and
  PTY cases. Its focused run passed 31/31. The PTY filter selected all 16 Unix
  cases, and all 16 passed in each of 20 stress iterations while unrelated
  `make test` jobs were active on other worktrees. The first full gate sequence
  found seven Clippy diagnostics and a stale local permissions helper left by
  the fake-Ninja promotion; both were corrected. The focused suite again passed
  31/31 and the PTY stress run passed 16/16 for 20 iterations. The full gates
  are being rerun on the corrected candidate.
- [x] (2026-10-30) Rebased onto the current PR target, `origin/main` at
  `7929a3fc24759d98ed3ffe70c8abf7b66861f17d`, which had advanced ten commits
  past the previous base. Resolved conflicts in `.config/nextest.toml`,
  `docs/developers-guide.md`, `tests/logging_stderr/support.rs`, and
  `tests/workflow_contracts/test_execution_coverage_test.py`. ADR-0059 had
  moved compile-time contracts out of timed tests, so decisions `D13` and `D14`
  are superseded and the branch kept its fake-Ninja promotion plus the new
  normalization helpers. The full gate set is being rerun on the rebased
  candidate.
- [x] (2026-11-04) Rebased again onto the verified PR target,
  `origin/main` at `a6009675e017303367cd2821837af2cebf5b94cd`. Resolved the
  `test_support/src/lib.rs` conflict by retaining both the target's
  `temp_path_filter` and this branch's fake-Ninja and terminal modules; kept
  the target-side lockfile and regenerated it with `make typecheck`. All eight
  post-rebase gates passed on `a9ba648db2dbec89df8d82fbc3fbecfa261ad4a0`:
  `make check-fmt`, `make lint`, `make doc-coverage` (98.33%), `make typecheck`,
  `make test` (5,078 passed, 7 skipped; doctests passed), `make markdownlint`
  (217 files, zero issues), `make nixie`, and `make test-workflow-contracts`
  (1,666 passed, 3 skipped, plus 28 passed). The worktree was clean. The EP-M0
  PTY smoke test still needs its idle-pool run; its loaded-host run passed
  earlier.
- [x] (2026-11-04) The interim PR head
  `b940bd2eae5b13629808169f64a1b2c02afbefdf` passed hosted Linux, Kani,
  default-feature, and release jobs, but its Windows test and lint jobs failed.
  The test build exposed PTY-only JSON cases and imports that were not
  Unix-gated; Clippy required the non-Unix no-op script permission helper to be
  `const`. Fix these before marking EP-M1 complete.
- [x] (2026-11-04) The PTY smoke suite passed 20/20 stress iterations with all
  three tests passing in each iteration after the compiler pool reached zero
  held and queued work. Together with the earlier loaded-host run (60/60
  executions), this completes EP-M0.
- [x] (2026-11-04) Corrected EP-M1's Windows compile boundaries by moving the
  JSON PTY case behind a Unix-only module declaration, gating only the PTY
  imports in shared support, and providing a separate non-Unix `const` no-op
  permission helper. Candidate `2c3d43540eb7a62744915e290cfadc236d14bac8` passed
  `make check-fmt`, `make lint`, `make doc-coverage` (98.33%),
  `make typecheck`, `make test` (5,078 passed, 7 skipped; doctests passed),
  `make markdownlint` (217 files, zero issues), and `make nixie`; run the
  hosted Windows lanes before marking EP-M1 complete.
- [x] (2026-11-04) Rebased the seven-commit series from old head
  `2c3d43540eb7a62744915e290cfadc236d14bac8` onto target
  `01c7190e12943255cda00b1affc65efec0249726`, using
  `a6009675e017303367cd2821837af2cebf5b94cd` as the exclusive replay boundary
  established by the earlier rebase. The rebase was linear and clean;
  `range-diff` matched all seven commits and the post-rebase audit preserved
  all 18 target-only paths, including the expected release-help test deletion.
  The only shared path, `docs/developers-guide.md`, retains both the target's
  parser-parity guidance and this branch's terminal test guidance. The host's
  global `~/.config/git/attributes` selects Weave for `*.rs`, but the
  repository has no tracked Weave opt-in; followed the Weave skill's
  unattended-rebase policy by using built-in Git merge with
  `core.attributesFile=/dev/null` and `zdiff3`. Recovery refs preserve the old
  head, boundary, target, and first rebase result. New head:
  `940ee40c9248ae429effd3f5d49ad76173dd1585`.
- [x] (2026-11-04) Hosted Windows validation of the rebased candidate found
  that `tests/logging_stderr/support.rs` still marked the shared fake-Ninja
  re-export Unix-only. The helper itself writes `.cmd` scripts on Windows, and
  `tests/logging_stderr/verbose.rs` uses it in the piped suite. Removed the
  unnecessary gate; all seven serial candidate gates passed, including doc
  coverage (98.33%) and 5,094 tests plus doctests. Markdown lint reported zero
  issues across 217 files, and Nixie passed.
- [x] (2026-11-04) Completed EP-M2 commit 1: moved process-wide tracing setup
  into `src/main_tracing/mod.rs` and extracted the shared pre-clap option-value
  scanner. Updated `config_load` to use the relocated filter setter and kept
  `SubscriberExt` available to the existing test module under `cfg(test)`; test
  source and behaviour remain unchanged. All seven serial gates passed:
  check-fmt, lint, doc coverage (98.33%), typecheck, test (5,094 passed, 7
  skipped; doctests passed), markdownlint (217 files, zero issues), and Nixie.
- [x] (2026-11-04) Completed EP-M2 commit 2: added public
  `output_mode::EnvSignals`, its `NO_COLOR` policy and mode methods, and moved
  `resolve_with` plus `theme::resolve_theme` onto the shared signal read. The
  old theme-local signal type is gone. The witness table covers nine explicit
  environment classes, and the property compares both resolvers with copies of
  their previous rules over arbitrary strings; the focused nextest run passed
  all 10 cases. Whitaker required moving the output-mode unit tests into
  `src/output/mode/tests.rs` to keep each source file below 400 lines. All
  seven serial gates passed: doc coverage (98.33%), typecheck, 5,104 nextest
  tests (7 skipped) plus doctests, Markdown lint (217 files, zero issues), and
  Nixie. The host sccache was unavailable during tests; healthy admission ran
  the test binaries uncached.
- [x] (2026-11-04) Completed EP-M2 commit 3: added the
  pure `display_plan` core, its public re-export, and the 5,184-case sweep with
  an extracted independent truth model. The focused sweep passed 1/1 and the
  plan unit cases passed 14/14. Three seeded faults were detected with 288,
  864, and 1,080 mismatching points respectively. The initial `auto == always`
  assertion also compared a `never` input with `always`; narrowed it to the
  `auto` policy coordinate, as D3 requires. Clippy then exposed long test
  helpers, indexed coverage arrays, and a boolean-heavy expected-value struct;
  split the sweep into named helpers, used bitsets for coverage, and grouped
  progress expectations. All seven serial candidate gates passed: check-fmt,
  lint, doc coverage (98.35%), typecheck, 5,117 tests (7 skipped) plus
  doctests, Markdown lint (217 files, zero issues), and Nixie.
- [x] (2026-11-04) Completed EP-M2 commit 4's runtime rewire: the binary
  captures terminal and environment facts once, runner and reporter paths
  consume `DisplayPlan`, ambient output resolvers are removed, and EP-M1
  expectations use plan projections. Hosted CI on the previous head found that
  `src/output/plan/mod.rs` was missing from Kani proof scope; the current
  candidate adds the module. Its first lint run found that runtime-only policy
  helpers in `cli/config/mod.rs` become dead code in the schema-only `build.rs`
  compilation. Moved those helpers into `cli/preferences.rs` and kept the
  configuration types in their existing module to avoid an unused re-export.
  Clippy also required `main_tracing::init_tracing` to consume the
  stderr-terminal fact captured by `main`, rather than make its own terminal
  query. The full serial gates passed on candidate base
  `9dc62a0b7522d998e6a1a2e506398286174e1fbc` with an unchanged worktree diff
  (SHA-256 `d4b3701c6f0ddd57a31d28b18453aecfbdb85937677e91bc713fbf91def9fd52`):
  `make check-fmt`, `make lint`, `make doc-coverage` (98.35%), `make typecheck`,
  `make test` (5,114 passed, 7 skipped; doctests passed), `make markdownlint`
  (217 files, zero issues), `make nixie`, and `make test-workflow-contracts`
  (1,666 passed, 3 skipped, plus 28 passed). No snapshots changed.
- [x] (2026-11-05) Isolate the mutation compile allowance in a dedicated Kani
  Nextest profile. The mutation test timed out at 600 s on hosted run
  41840367244 and at 900.093 s in a warm local gate; neither produced a
  compiler diagnostic. The ordinary and coverage profiles stay at 300 s per
  test and the CI coverage run at 960 s, while the Kani profile grants 2,400 s
  to the mutation test and 2,520 s to the full Nextest run. Hosted run
  41840367244 spent 824 s before Nextest began the test and 7 s finalizing, so
  the 60-minute `kani-smoke` job leaves 249 s beyond setup, the run cap, and
  finalization. Profile selection and effective override binding are now
  observed rather than assumed: the local gate re-ran under `--profile kani`
  and was terminated by nextest at 2,400.056 s, which is the profile's own
  allowance and not the 300 s default, so the override reached the test.
  `test_execution_coverage_test.py` and `timeout_budget_properties_test.py`
  hold that wiring and passed. Budget adequacy is settled by measurement rather
  than by raising the number again: see D17. The local timeout is the shared
  `build-limits` pool saturating, and the hosted gate each patch waits for is
  green on `main`, so a successful local Kani run is not achievable while
  several hundred foreign compile requests are queued.
- [x] (2026-11-05) EP-M3 tracing red-green: the new terminal policy tests first
  failed for `--color always` on a pipe and `NO_COLOR=1` on a PTY, while the
  `never` pipe and `auto` PTY controls passed. A reloadable formatting layer
  now starts without ANSI, receives the CLI startup policy before configuration
  discovery, and receives the merged `DisplayPlan` before runtime logging. The
  old configuration-failure test now checks that `--color always` styles its
  tracing events, as D11 requires. The terminal-rendering suite passed 35/35,
  the BDD suite passed 309/309, and the non-RunnerError failure pair passed
  2/2. The new `kani-smoke` timeout contract still needs the final full gate
  sequence.
- [x] (2026-11-07) Answered the hosted ExecPlan assessment
  (`NETSUKE-PR809-EXECPLAN-1103317A7`) and the proof assessment
  (`NETSUKE-PR809-PROOF-1103317A7`). All five ExecPlan finding groups are
  addressed: runner-rendered help takes the plan's stdout colour (D18, with the
  `NETSUKE_COLOR` correction that made `help` and `help build` agree with
  `--help`); the byte-level invariants are measured per emitter with colour as
  the only variable (D24); the PTY-only cases moved into Unix-gated modules so
  pipe coverage compiles on every platform; the configuration-load split is
  documented in the users' guide and ADR-0072; and the structural deviations
  are recorded as D20 to D22. The proof assessment accepted the branch as
  preserving the existing bounded verification claims and asked for two
  evidence qualifications, recorded in D17 and D23.
  `cargo test --test terminal_rendering_tests` passed 65/65 and the
  `main_startup` unit tests 20/20. Remaining: the full sequential gate batch,
  publication, and the lifecycle.
- [x] (2026-11-07) Cleared the rebase's two CodeScene findings and passed the
  full ten-gate set. `run_with_args` split again — `resolve_startup_window`
  owns the pre-configuration window and returns a `StartupWindow` — taking
  `src/main.rs` to 378 lines, and the three environment providers written out in
  `src/main_startup/tests.rs` collapsed into one table-driven `FixedEnv`, so
  no test spells out an environment provider by hand. `cs check` reproduced
  both hosted scores before and cleared them after (9.58 → 10.00 and 9.68 →
  10.00). Two gate batches were needed: the first failed Clippy on a `match`
  that was exactly `?`, and the second failed `spelling` on the single word
  "artefact" in this plan's own D25 text, which also aborted
  `markdownlint-cli2` before it ran. All ten gates then passed sequentially
  against `19a49efe0` with the batch's tree byte-identical at its start and
  end. This entry was then committed, so the candidate that carries it is a
  later commit whose only change is documentation: `check-fmt` (220 Markdown
  files unchanged), `lint` (`doc`, Clippy, Whitaker, `yamllint`, `actionlint`),
  `doc-coverage` (98.38% against 80%), `test` (5,303 passed, 7 skipped,
  doctests passed), `test-workflow-contracts` (1,792 passed, 3 skipped, plus
  28), `test-package-verify`, and `test-coverage-artifact` (52),
  `test-documentation-contracts` (15), `markdownlint` (0 issues across 220
  files), and `nixie` (221 diagrams). The earlier nine-green batch on
  `81162c9c9` is superseded, and the prior claim that `test-package-verify` and
  `test-coverage-artifact` were "not applicable" no longer holds: both were run
  here and both passed. Remaining: publication and the lifecycle.
- [x] EP-M0: terminal test harness.
- [x] EP-M1: regression suite for already-correct behaviour. Its 31 piped/PTy
  cases and 20-run loaded-host stress evidence are recorded above; hosted
  Windows build and lint lanes passed on the rebased candidate.
- [x] EP-M2: display-plan core and single resolution.
- [x] EP-M3: bind colour and glyph decisions to the emitters.
- [x] EP-M4: contract snapshots, pinning scenarios, documentation, ADR,
  roadmap.

## Surprises & discoveries

- Observation: task-update text remains plain when stderr is a PTY and stdout
  is piped, even while stage progress redraws on the PTY. Evidence:
  `IndicatifReporter::report_task_progress` writes task lines separately from
  the stage draw target, and the test captures the `Task` lines before
  classifying them. Impact: the PTY assertion is scoped to task-update lines
  rather than rejecting the valid stage redraws in the same stream.
- Observation: moving the fake-Ninja script writer left the old local
  executable-permission helper behind. Evidence: `make typecheck` and
  `make test` both reported that `Permissions::set_mode` lacked its trait
  import in `tests/logging_stderr/support.rs`. Impact: removed the now-unused
  duplicate helper; the shared writer remains the single owner of platform
  script permissions.

- Observation: `--color` governs no ANSI emitter today; its only effect is on
  accessible-mode selection through `NO_COLOR` semantics. Evidence:
  `src/theme/mod.rs` documents `ColourTokens` as placeholders; the pre-EP-M2
  `init_tracing` sampled `io::stderr().is_terminal()`; `src/` contains no
  `miette::set_hook` or `clap::ColorChoice`; baseline rows
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
  and `src/runner/help/mod.rs::resolved_prefs`; `ExecutionContext` recomputes
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
- Observation: between the draft (`ebcedaef`) and 2026-10-23, `main` moved
  the modules this plan touches into directories and renumbered every ADR.
  Evidence: `src/output/mode.rs` and `src/output/prefs/mod.rs` (re-exported as
  `output_mode` and `output_prefs`), `src/theme/mod.rs`,
  `src/status/indicatif.rs`, `src/cli/l10n/mod.rs`, `src/runner/help/mod.rs`,
  `src/cli/config/mod.rs`, `src/cli/policy/values.rs`,
  `src/locale/resolution.rs`, `src/startup_tracing/mod.rs`; ADRs now live in
  `docs/adrs/adr-nnnn-*.md`, where the agent-consistent CLI record is ADR-0046
  and ADR-0041 is the canonical recipe shell quoting surface. Impact: paths,
  ADR citations, and the planned ADR serial (then ADR-0060) were remapped. Code
  inspection of the merged tree on 2026-10-23 shows the defects the plan
  targets are still present: `src/main.rs` still calls
  `with_ansi(io::stderr().is_terminal())`; `src/` still has no `set_hook` or
  `ColorChoice`; and the ambient resolution sites remain at `src/main.rs`,
  `src/runner/mod.rs`, and `src/runner/help/mod.rs`. The binary was not
  re-probed; EP-M3's red cells re-run each defect against the implementation
  base before fixing it.
- Observation: the second baseline `make test` run timed out
  `split_build_fixture_compiles_through_the_direct_rustc_harness::case_2_compiler_diagnostic`
  at 900.139 seconds after 4,856 passes. The captured build-limits status
  overlaps the case: all four tokens were held, with one retained
  `server-drain` group, three active compiler groups from other worktrees, and
  the fixture's `rustc` request waiting for 57 seconds. Impact: shared
  admission delayed the fixture at that point, but the single snapshot does not
  explain the full timeout. Keep the cap at 900 seconds and capture a process
  timeline if the test fails again; do not intervene in automatic recovery.
- Observation: Whitaker's `no_std_fs_operations` rejects `std::fs::File` even
  when it only wraps a PTY descriptor. Impact: the harness keeps descriptors as
  `OwnedFd`, passes them to `Stdio`, and reads the PTY master with nix so
  terminal capture does not introduce an ambient filesystem API.
- Observation: the JSON rendering file contained both piped and PTY tests,
  while only the module declaration can reliably keep Unix-only APIs out of a
  Windows build. Evidence: hosted Windows CI could not resolve the gated PTY
  exports or `TerminalWorkspace::run_pty` from `json.rs` and shared support.
  Impact: move the PTY case into `json_pty.rs`, gate that module declaration,
  and gate the matching imports and method surface.
- Observation: `cfg_attr` cannot inject the `const` function modifier.
  Evidence: the first Windows-lint fix attempt failed during Rust parsing on
  Linux at `#[cfg_attr(not(unix), const)]`. Impact: define the Unix fallible
  helper and non-Unix `const` no-op as separate target-gated functions.
- Observation: the Kani mutation compile gate produces no per-patch progress,
  so a timeout on it cannot be attributed. Evidence: `compile_patched_tree` runs
  `cargo kani` through `.output()`, which captures and discards both streams,
  and the loop applies the next patch only after the previous build finishes. A
  run terminated at the allowance therefore leaves one
  `Compiling netsuke-build` line for the whole gate and no indication of which
  of the 30 patches it reached. Determining how far the timed-out run got
  required reading artefact timestamps in the shared target directory by hand.
  Impact: record it as a diagnostic gap rather than repairing it here. The fix
  is to report the patch name and its elapsed time per iteration through
  `tracing`, which the gate's own module documentation already claims it does
  for reverts. Out of scope for 3.12.3; note it for the Kani gate's owner.

## Decision log

- Decision D1: treat 3.12.3 as "verify, and repair what verification
  breaks", bounded by the tolerances. `--color never` and `NO_COLOR` emitting
  colour are defects against the documented contract ("`--color never` behaves
  like an internal `NO_COLOR`", `docs/netsuke-design.md`; the `NO_COLOR`
  convention) and are fixed. New colour *behaviour* is not assumed: it is
  decision `D11`. Date/Author: 2026-09-26, planning agent, revised after panel
  review.
- Decision D2: shape the change as a functional core with an imperative
  shell, not a trait-based port. A new pure module `src/output/plan/mod.rs`
  resolves a `DisplayPlan` from `DisplayPolicies` and `TerminalFacts`. The
  composition root gathers facts once and hands the plan to four outbound
  adapters: the status reporter factory, the `tracing` formatter, the
  diagnostic hook, and `clap` help. Environment strings are reduced to booleans
  exactly once, by `EnvSignals::read` in `src/output/mode.rs`; precedence lives
  in `const fn`s over those booleans, which the existing `resolve_with` and
  `resolve_theme` also call, so there is one source of truth for precedence and
  for the empty-`NO_COLOR` rule. Rationale: the domain logic is a total
  function over a small finite domain; a value-in, value-out core is simpler
  and more testable than a `TerminalProbe` trait with mock implementations.
  `OutputPrefs` (a glyph façade) and `ThemeContext` (an input record) each
  capture one facet, so a new type is justified provided it absorbs
  `ReporterOptions` rather than sitting beside it. Date/Author: 2026-09-26,
  planning agent, revised after panel review.
- Decision D3 (accepted): keep `--progress auto` and
  `--progress always` behaviourally identical, pin the equivalence with a test,
  and document the contract as: "`always` requests progress reporting; it never
  forces animation into a pipe or a dumb terminal, and never overrides
  accessible mode." Alternative: give `auto` a distinct meaning, such as no
  stage lines when stderr is not a terminal. That changes CI logs, contradicts
  the users' guide ("task progress falls back to text, so logs remain
  readable"), and is a product decision outside a verification task.
  Recommendation: pin the equivalence. Outcome: accepted as recommended.
  Date/Author: 2026-10-23, user.
- Decision D4 (accepted, Option A): whether an *empty* `NO_COLOR`
  counts as set. Today it does (`src/output/mode.rs`: "any value, including
  empty"). Option A: align with the `NO_COLOR` convention ("present and not an
  empty string"), which `tracing-subscriber` and `anstream` follow
  (`supports-color` does not: it treats empty as set and `"0"` as unset). This
  is a one-line change in `EnvSignals::read` plus a users' guide edit. Option
  B: keep current behaviour, pin it, and record the divergence in the ADR.
  Either way every emitter agrees, because after EP-M3 no adapter consults
  `NO_COLOR` itself. Recommendation: Option A. Netsuke is pre-1.0
  (`0.1.0-beta4`), the convention's current text is explicit, and ADR-0046
  favours predictability over preserving an accident. Outcome: Option A
  accepted. An empty `NO_COLOR` counts as unset everywhere Netsuke reads it.
  The change lands as its own red-green commit in EP-M3 (step 4), after the
  emitters are bound, and the users' guide and CLI design document are updated
  to say "set to a non-empty value". Date/Author: 2026-10-23, user.
- Decision D5: build the PTY harness on `nix::pty::openpty` by enabling the
  `term`, `fs`, and `signal` features of the existing `nix` 0.31.3
  dev-dependency. The safe `fcntl` calls for `FD_CLOEXEC` require `fs`, and
  `killpg` requires `signal`; `term` exposes `openpty`. Rationale: no crate is
  added to `Cargo.lock`; `portable-pty`, `expectrl`, and `rexpect` would each
  add a dependency tree for a few dozen lines of harness. Windows
  pseudo-console (ConPTY) coverage is a residual gap; Windows keeps the piped
  suite and the pure-core tests. Date/Author: 2026-09-26, planning agent;
  feature gates verified 2026-10-24.
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
- Decision D8 (accepted): the diagnostic hook maps the plan's
  `DiagnosticStyle` to `MietteHandlerOpts`: `color(style.colour)`,
  `unicode(style.unicode)`, and `terminal_links(style.links)`, where `links` is
  true only when colour is on and the mode is standard. It never sets
  `force_graphical`, so a user's `NO_GRAPHICS` still selects `miette`'s
  narratable handler. Alternative: additionally select the narratable
  (screen-reader oriented) handler for an explicit `--accessibility on`. That
  is a visible format change for every accessible-mode user. Recommendation:
  adopt the mapping; defer the narratable handler to a follow-up roadmap item,
  and pin an accessible-mode diagnostic fingerprint now so that switch later
  shows as a diff. Outcome: accepted as recommended; the narratable handler is
  a follow-up. Date/Author: 2026-10-23, user.
- Decision D9 (accepted): `clap` help and version output
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
  adopt the startup hint. Outcome: accepted as recommended. Date/Author:
  2026-10-23, user.
- Decision D10: record the display-policy contract and verification
  strategy as ADR-0072, status `Proposed` while implementation runs and
  `Accepted` (with date and summary) at EP-M4. ADRs now live in
  `docs/adrs/adr-nnnn-title-slug.md` with four-digit serials. The 2026-09-26
  draft named ADR-041, which `main` has since given to the canonical recipe
  shell quoting surface. An all-branch sweep on 2026-11-04 found ADR-0060
  already used and ADR-0071 reserved, while ADR-0072 had no filename or prose
  reservation. Re-sweep every remote branch, in both the old `docs/adr-NNN-`
  and new `docs/adrs/adr-nnnn-` layouts, immediately before committing the ADR,
  and renumber on a clash. Date/Author: 2026-09-26, planning agent; serial
  revised 2026-10-23.
- Decision D11 (accepted): `--color always` forces ANSI
  styling on a non-terminal stream for every emitter Netsuke controls
  (`tracing` logs and `miette` diagnostics on stderr, `clap` help on stdout),
  matching the `CLICOLOR_FORCE` and `FORCE_COLOR` sense of "always". This is
  new behaviour: the users' guide currently says colour rendering is not
  implemented. Alternative: `always` only bypasses `NO_COLOR` and `TERM=dumb`
  detection on a terminal, and pipes stay plain. Recommendation: adopt, because
  "always" that still depends on the stream is surprising, and CI log viewers
  that render ANSI are the main reason to ask for it. Outcome: accepted as
  recommended. Date/Author: 2026-10-23, user.
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
- Decision D13: retain the 300-second default Nextest timeout, and give only
  `cli_configuration_fixture_compiles` a 900-second allowance in the serialized
  `nested-cargo-builds` group. Its nested `cargo check --all-features` timed
  out at 300 seconds during the first full run after rebase while compiler
  admission was healthy but heavily queued; the other 4,839 completed tests
  passed. This is a measured external build constraint, the same narrowly
  scoped policy used by existing cold-build tests, and remains below the
  960-second CI whole-run cap. No retry is added. Date/Author: 2026-10-24,
  planning agent. Superseded on 2026-10-30: ADR-0059 removed
  `cli_configuration_fixture_compiles` from the timed suite, so this allowance
  no longer exists.
- Decision D14: keep the
  `split_build_fixture_compiles_through_the_direct_rustc_harness` allowance at
  900 seconds and do not widen it. The second baseline run timed out
  `case_2_compiler_diagnostic` at 900.139 seconds. Its contemporaneous
  admission snapshot shows one retained cache drain, three active compiler
  groups, and a waiting compiler request under the fixture target. This proves
  shared admission contention at that point but does not account for the full
  duration. A larger cap would conflict with the 960-second CI whole-run cap;
  no source defect has been established. If reproduced, capture the child
  process tree and admission state through termination. Date/Author:
  2026-10-24, planning agent. Superseded on 2026-10-30: ADR-0059 removed
  `split_build_fixture_compiles_through_the_direct_rustc_harness` from the
  timed suite, so this allowance no longer exists.
- Decision D15: superseded by D16 on 2026-11-05. The 900-second mutation
  allowance still timed out locally without a diagnostic, and a one-patch cold
  compile took 1,864 seconds under heavy compiler admission.
- Decision D16: isolate the mutation compile allowance in a dedicated Kani
  Nextest profile. Keep the default and coverage profiles at 300 seconds per
  test and the CI coverage run at 960 seconds; give only the Kani mutation test
  2,400 seconds and the Kani run 2,520 seconds. Set `kani-smoke` to 60 minutes:
  the measured 824-second setup plus the run cap and 7-second finalization
  totals 3,351 seconds, leaving 249 seconds for runner variance and reporting.
  The 1,864-second one-patch result was observed under heavy shared admission,
  so it is evidence for the allowance rather than a predictable build time.
  Date/Author: 2026-11-05, implementation agent.
- Decision D17: keep D16's 2,400-second allowance and read the local timeout as
  local capacity rather than an undersized budget. Measured on 2026-11-05 in
  this worktree: a cold `cargo kani --only-codegen` over the sandbox took 342 s
  from an empty shared target directory; six seconds later the same command
  re-ran in 11 s; one patched tree took 57 s. Thirty patches therefore cost
  roughly 342 + 29 x 57 = 1,995 s of compilation on an idle host, so a single
  2,400-second cap clears it by about 400 s and a cold start plus contention
  does not. The budget is not the defect: the hosted `kani-smoke` job on `main`
  run 42058628930 spent 665 s on the whole mutation compile gate (15:58:35 to
  16:09:40) and passed, while this branch's hosted run 41840367244 failed at
  the 600-second allowance this change replaces. The local failures are the
  shared `build-limits` pool: capacity 4 with several hundred requests queued
  throughout, so each of the 30 sequential patches waits behind other agents'
  compiles inside the test's own wall clock. A re-run confirmed it: with the
  target directory fully warm the gate still compiled nothing in fourteen
  minutes while 189 requests were queued, because every patch re-enters the
  same queue. Do not raise the allowance again on local evidence; measure on
  CI. The gate is validated by its hosted result, and the local re-run is not
  evidence of a defect. One qualification, added after the proof assessment
  (`NETSUKE-PR809-PROOF-1103317A7`): a timeout with no compiler diagnostic does
  not by itself prove admission-queue contention. Treat the timed-out run as
  inconclusive and cite the separate contention measurements — the warm-target
  re-run that compiled nothing in fourteen minutes while 189 requests were
  queued — and the successful hosted execution as the supporting evidence, not
  the absence of output. Date/Author: 2026-11-05, implementation agent.
- Decision D18 (assessment follow-up): the runner-rendered help paths take the
  colour from the resolved plan rather than clap's `ColorChoice`.
  `render_long_help` always embeds clap's styles in its `StyledStr`;
  `StyledStr::Display` strips them and `StyledStr::ansi()` keeps them, so the
  renderer decides. `DisplayPlan` gained a `StreamColour { stdout, stderr }`
  for this, resolved from one function, so `--help`, `help`, and `help build`
  cannot disagree. The two flags travel as one value rather than as loose
  booleans: they are always decided together and consumed together, and a
  fourth independent bool on the plan would also have tripped
  `clippy::struct_excessive_bools`, whose remedy — merging fields into an enum
  — is the right shape here. The first version of this fix was still wrong for
  `NETSUKE_COLOR`: the informational-help path runs ahead of the configuration
  merge, so its plan was resolved from parsed argv alone, where an absent
  `--color` is indistinguishable from `--color auto`. Only clap's own `--help`
  saw the environment value, through `resolve_startup_colour`. The measured
  evidence was 50 SGR lines on `--help` against 0 on `help` and `help build`
  under `NETSUKE_COLOR=always`. `StartupSignals` now carries the resolved
  startup colour hint and applies it through `policies_before_configuration`,
  which restores the precedence the users' guide states — `--color`, then
  `NETSUKE_COLOR`, then `auto` — on all three routes. Date/Author: 2026-11-07,
  implementation agent, after the hosted ExecPlan assessment
  (`NETSUKE-PR809-EXECPLAN-1103317A7`).
- Decision D19 (assessment follow-up): a configuration-load failure splits into
  a plain user-facing message and policy-styled `tracing` events. The message
  is plain because the configuration that would select a colour policy has not
  been read; the events follow the startup hint because `D11` says `--color`
  governs every emitter Netsuke controls, and exempting startup tracing would
  make the emitter's behaviour depend on which phase it happens to run in. Both
  halves are pinned in one test per policy. Date/Author: 2026-11-07,
  implementation agent.
- Decision D20 (assessment follow-up): `src/main.rs` was 463 lines at the
  assessed head, above the 400-line rule in `AGENTS.md`. The pre-configuration
  startup window — `StartupSignals`, `StartupParse`, `parse_cli_or_exit`,
  `settle_startup_diagnostics`, `startup_filter`, `startup_localizer`, and
  `requests_help_or_version` — moved to `src/main_startup/mod.rs` (226 lines)
  with its unit tests in `src/main_startup/tests.rs`. The `main_` prefix group
  in `tests/workflow_contracts/rust_module_layout_test.py` now names that third
  entry and extends its rationale. It is a directory module rather than a
  sibling `main_startup.rs` because the repository lint requires `mod.rs` as
  soon as a module owns a child file, and `tests.rs` is that child.
  Date/Author: 2026-11-07, implementation agent.
- Decision D21 (assessment follow-up): the 22-file production diff is approved
  as it stands. The tolerance escalates at 18 production files or 600 net
  lines; the diff touches 22 files for 510 net lines, so the line budget is met
  and only the file count is over. The files are not extra scope: the change
  replaces `output_mode::resolve` and `output_prefs::resolve_from_theme` with a
  core plus four adapters, and the deletion side of that refactor is spread
  across every module that used to make its own decision. Splitting it into
  fewer files would mean leaving ambient resolution in place. Date/Author:
  2026-11-07, implementation agent.
- Decision D22 (assessment follow-up): two test-support `#[expect]`s are
  approved as scope-narrowed exceptions to the plan's "no new suppression" rule.
  `clippy::struct_excessive_bools` on `TerminalBytes` is a data carrier whose
  whole purpose is to expose independent byte-level facts; the lint's remedy is
  to merge fields, which would delete facts the assertions need.
  `clippy::unnecessary_wraps` on the non-Unix `make_script_executable` is the
  platform-mirror shape the codebase already uses at
  `tests/bdd/steps/progress_output.rs`, where the fallible signature exists so
  the caller needs no `cfg`. Neither suppression hides production behaviour.
  Date/Author: 2026-11-07, implementation agent.
- Decision D23 (proof assessment follow-up): the mutation gate proves
  compilation, not detection.
  `tests/kani_mutation_evidence_tests/compile_guard.rs` invokes
  `cargo kani --only-codegen --lib --all-features`, which establishes that the
  30 patched trees compile under the Kani frontend; it does not establish that
  their intended assertions produce counterexamples, and the 30 patches include
  ordinary property-test controls rather than 30 Kani harnesses. The compile
  gate stands on its own terms. Any claim that every negative control remains
  effective needs mutation-replay results naming the intended failed assertion,
  and this branch does not make that claim. The same assessment confirms the
  display-domain arithmetic — 3⁴ × 2 × 2⁵ = 5,184, not 20,736 — and asks that
  enumeration over a finite domain not be described as inherently stronger than
  an equivalent Kani proof. Its advantages here are complete finite coverage
  against the independent truth model and a cost small enough to run in the
  default gate. Date/Author: 2026-11-07, implementation agent, after
  `NETSUKE-PR809-PROOF-1103317A7`.
- Decision D24 (assessment follow-up): `INV-COLOUR-ADDITIVE` is proven per
  emitter with the comparison held to one variable, and the accessible reporter
  is recorded as an intentionally excluded emitter. Stripping SGR from a
  `--color always` stream must reproduce the `--color never` stream byte for
  byte — but only when glyph and accessibility policy are pinned identically on
  both sides, because `--color never` also selects accessible mode when
  accessibility is `auto`, which swaps every Unicode glyph and every progress
  line for its ASCII or static counterpart. Letting that drift compares two
  differently rendered streams and blames the difference on colour. The
  accessible status reporter emits no escape at all — a prefix glyph plus a
  localized label — so for it the invariant holds by construction and the
  assertion that keeps it true is the stronger "no escape", not "stripping SGR
  changes nothing". Date/Author: 2026-11-07, implementation agent.

- **D25. The rebase onto `921320086` needed two CodeScene clearances, and the
  CLI reproduced both.** After replaying 22 commits onto the newer main, four
  conflicts were resolved and one merge artefact was removed:
  `test_support/src/lib.rs` had gained a second copy of the re-export block,
  because weave reported the file "auto-resolved" while the duplicated
  `pub use` lines produced ten E0252 errors. The hosted CodeScene delta then
  flagged the final commit for `Large Method` in `src/main.rs`
  (`run_with_args`, score 9.58) and `String Heavy Function Arguments` in
  `src/main_startup/tests.rs` (9.68). `cs check` reproduced both scores on the
  pre-fix files, so the local tool is a usable oracle for this job rather than
  a guess.

  Neither finding was suppressed. `run_with_args` was split along the three
  stages it already contained — resolve the startup window, render the
  informational-help route, run after the merge — which took the function from
  74 to 59 lines and its file to 10.00. `src/main_startup/tests.rs` collapsed
  three near-identical `LocaleEnvProvider` implementations into one table-driven
  `FixedEnv`, which is a genuine deduplication and not an evasion; the file
  moved to 10.00 with the `String Heavy Function Arguments` biomarker clear.
  What remains in that file is four rstest `#[case] args: &[&str]` tables,
  which the repository already spells the same way in
  `src/shell_selection/built_in/tests.rs`: the case *is* the raw argv under
  test, so a newtype would add a conversion at every case while enforcing
  nothing. Date/Author: 2026-11-07, implementation agent.

- **D26. The pre-merge checks found one real defect, one real test gap, and
  three stale rows.** CodeRabbit's second-round findings arrived as pre-merge
  check rows rather than inline threads, so each was verified against current
  source rather than applied. `Testing (Overall)` was correct: every colour
  case passed its policy on the command line, which exercises
  `resolve_startup_colour`, and the merged path this branch adds had no
  configuration-file case at all. `Testing (Property / Proof)` was correct in a
  way the row did not state — `test_support::terminal::classify` consumed an
  unterminated escape sequence wholesale, so a bare carriage return after
  `\x1b[` was reported absent. A truncated capture with a live redraw would
  have read as a static stream. Both are fixed. `User-Facing Documentation` was
  correct that the README still claimed colour rendering was unimplemented,
  which this branch makes false, and that the guide's `NO_COLOR` sentence
  omitted the `--color always` precedence. `Developer Documentation` was
  correct that `settle_startup_diagnostics` was still placed in `src/main.rs`
  after D20 moved it. `Unit Architecture` and `Domain Architecture` were not
  adopted: the startup side effects it names (`observability::init_metrics`,
  `localization::set_localizer`) are unchanged from the base commit and merely
  moved with the function, and the policy-type dependency it names already
  exists at base in `src/theme/mod.rs` and `src/output/mode.rs`. Date/Author:
  2026-11-08, implementation agent.

- **D27. Three findings from CodeRabbit review `5815758338` were real, and one
  of them was a genuine defect in the classifier fix.** All three were
  reproduced before changing anything.
  - The classifier treated DCS, SOS, PM and APC introducers as two-byte
    escapes, so `ESC _` consumed only the introducer and the string's payload
    was scanned as live control bytes. An APC payload containing `\r` therefore
    reported a bare carriage return, which is redraw evidence the terminal
    never acted on. The D26 fix for truncated CSI sequences had closed the same
    class of hole for CSI but left it open for the other control strings. They
    are now scanned to their terminator, with two rules the finding is right to
    insist on: only ST ends them, so a BEL inside the payload is data, and CAN
    or SUB cancels the string so the terminal renders what follows. `has_osc`
    stays specific to OSC. Reverting the parser fails six of the new witnesses,
    so they are not vacuous.
  - The nested-cargo-timeout guidance said `ci` and `default` both have a
    960-second whole-run cap. Only `ci` declares one; it inherits the rest of
    its settings from `default`, and `.config/nextest.toml` states explicitly
    that a local `default` run has no whole-run cap. The guidance now
    distinguishes the CI cap from the shared-setting reason, which is what the
    restriction actually rests on.
  - The Clippy terminal-probe contract searched the whole remainder of
    `src/main.rs` after the `#[expect(...)]`, so any later `fn main()` satisfied
    it. It now reads only the decorated item's header. Moving the attribute
    onto `pub fn probe_terminal()` fails the test with that function's name in
    the message, where the old form passed. Date/Author: 2026-11-08,
    implementation agent, after `5815758338`.

- **D28. Six further pre-merge rows were adopted and one was found already
  satisfied.** Each was checked against current source first; two were real
  documentation defects and one was a real coverage gap.
  - *User-Facing Documentation* was correct, and the error was mine. I had
    written that `--color always` suppresses both the `NO_COLOR` and
    `TERM=dumb` triggers. `no_color_active` special-cases `ColourPolicy::Always`
    to `false`, but `resolve_output_mode` tests `self.term_dumb` with a plain
    `||` that no colour policy reaches. A probe confirms `always + TERM=dumb`
    still resolves to accessible mode. ADR-0072 states the intended rule —
    `TERM=dumb` is not a colour hint but a statement that the terminal cannot
    redraw, and it prevents live redraws even under `--accessibility off` — so
    the code is right and the prose was wrong. The guide now says what happens
    and points at `--accessibility off` as the override.
  - The same row asked for the output changes and the Rust API migration in
    `docs/v0-1-0-migration-guide.md`. The API part is a genuine breaking
    change: `output_prefs::resolve_from_theme` and `resolve_from_theme_with`
    were public at base and are gone, replaced by `resolve_from_signals`, and
    `netsuke::display_plan` is new. A "Colour, emoji and progress are resolved
    once" section now carries both halves.
  - It also asked to remove the obsolete colour limitation from the translated
    READMEs. All six carried it — `README.de.md:349`, `README.es.md:355`,
    `README.fr.md:357`, `README.ja.md:308`, `README.pt-BR.md:338` and
    `README.zh-CN.md:280` — while the English `README.md` was already clean, so
    the translations had drifted. Limitations tracked the English bullet at
    the printed line numbers, and all six are removed.
  - *Developer Documentation* was right that the tracing module path was stale:
    `src/main_tracing/mod.rs` is now `src/main_tracing.rs`. The same row asked
    for how the display plan sets ANSI output, which was not documented; the
    guide now records that each adapter applies the plan's own decision —
    tracing via `ansi` at `init_tracing`, Miette by choosing a graphical or
    narratable handler, Clap via `StyledStr::ansi` rather than `Display`, and
    the reporter via `live_progress`.
  - *Testing (Unit And Behavioural)* asked for bounded PTY tests of the timeout
    error and of cleanup when a descendant holds a descriptor open. Neither
    existed, though the harness implements both. `pty_reports_a_child_that
    _exceeds_its_timeout` and `pty_stops_reading_when_a_descendant_holds_the
    _terminal` now cover them. The second takes the full five-second timeout,
    so it exercises the process-group kill rather than the ordinary exit; the
    first fails when its timeout is raised, so neither is vacuous.
  - *Module-Level Documentation* asked `json.rs` to describe the
    configuration-error and tracing checks accurately. The docstring now states
    the split: JSON mode carries no escape at all, while a configuration-load
    failure keeps its human-readable message plain and lets the tracing events
    beside it follow the startup colour hint.
  - *Testing (Overall)*, *Testing (Property / Proof)*, *Testing (Compile-Time /
    Ui)*, *Unit Architecture* and *Domain Architecture* were already addressed
    by this branch or by earlier decisions (D21, D22, D26, D27), and re-reading
    each against the current tree found nothing further to change. The
    task-update row in particular is satisfied: `progress_pty.rs` collects each
    whole task line and classifies it, which includes the bytes before
    `Task `. Date/Author: 2026-11-08, implementation agent, after
    `5815758338`.

## Outcomes & retrospective

### Milestone EP-M0 to EP-M4: complete

All four milestones are implemented and committed. The observable outcome is
that the display policies 3.12.3 names are now measured through the real
`netsuke` binary on a kernel pseudo-terminal and on a pipe, rather than only
through the pure resolvers. Three defects the plan's baseline probe recorded —
416 SGR sequences under `--color never --verbose`, coloured Miette diagnostics
on `--color never`, and unformatted help output — are closed, and each has a
regression test that fails when the wiring is removed.

The functional core arrived later than the tests: EP-M1 pinned already-correct
behaviour on the harness before EP-M2 introduced `DisplayPlan`, so the tests
were a specification the implementation had to satisfy rather than a
description of it. That ordering paid for itself when EP-M3 moved the emitters
onto the plan, because the PTY harness could show the pre-plan and post-plan
bytes were identical for the policies that had already been correct.

### Gates against the delivered candidate

Every applicable repository gate ran sequentially against `09e48138d` after the
rebase onto `main`, and all thirteen exited `0`: `check-fmt`, `markdownlint`,
`typecheck`, `lint-clippy`, `lint-whitaker`, `lint-python`,
`github-actions-lint`, `lint-workflow-scripts`, `doc-coverage` (98.35% against
the 80% threshold), `nixie`, `test` (5,183 tests across 125 binaries, plus the
doctest pass), `test-workflow-contracts`, and `test-documentation-contracts`.

`test-kani-mutations` is the exception to reading a local result as evidence
about the diff, for the reason D17 records: its local cost is the shared
`build-limits` pool, so a local run measures host capacity. Its adequacy rests
on the hosted `kani-smoke` job, and the hosted result for this head is the
authoritative one. Treating a local timeout there as a red gate would be the
mistake D17 exists to prevent.

It passed locally anyway on the candidate that followed: 625.818 s inside the
2,400 s allowance, 633 s including nextest's own start and finish. The pool
happened to be lightly loaded, which is the same explanation as the earlier
timeouts arrived at from the other side. So both readings agree, and neither
turns the local number into a claim about the patch set.

### Windows compile gaps the hosted runner found

The hosted `Windows / lint-windows` and `Windows / build-test-windows` jobs
failed on the same head, each for a reason invisible on Linux, and each is a
`#[cfg(unix)]` boundary rather than a behavioural defect:

- `tests/terminal_rendering/tracing.rs` was compiled on every platform but
  imported `Attach` from `test_support::terminal`, which is gated on `unix`.
  The PTY case moved to `tracing_pty.rs`, matching the `*_pty.rs` convention
  the rest of that directory already follows, and the module is gated in
  `tests/terminal_rendering_tests.rs`.
- `tests/bdd/steps/terminal_rendering.rs` imported the `when` macro
  unconditionally, but its only `when` step needs the Unix PTY harness. The
  macro import is now `#[cfg(unix)]`, which is sound because the step's
  scenarios live in `tests/features_unix`, itself gated at its `scenarios!`
  declaration.

Neither gap could be caught by a local `cargo check`: cross-compiling to
`x86_64-pc-windows-msvc` stops in `ring`'s C build, which needs a Windows
toolchain this host does not have. The audit therefore checks the boundary
directly — every ungated module is confirmed to name no Unix-only item — and
the hosted Windows jobs remain the authority.

Three repository targets were recorded as not applicable for that round and
were not run: `test-package-verify`, `test-coverage-artifact`, and the
coverage-artefact validator itself, which is operator-run against a downloaded
artefact directory. Only the last is genuinely inapplicable. The other two
seemed to be, on the reasoning that the package `include` list, `build.rs`,
`build_l10n_audit/` and `scripts/` were untouched, but `AGENTS.md` scopes them
to the change as a whole rather than to a hand-picked path list, and this
branch touches the Makefile and the packaging tests. Both were run on the final
candidate and both passed; see the gate entry in `Progress`. The lesson is that
"not applicable" is a claim about a rule's scope, not about which files a
diffstat happens to name.

### CodeScene's blocking gate, and the extraction it asked for

The hosted CodeScene review failed one *blocking* gate, "Prevent hotspot
decline", against `src/main.rs`: `run_with_args` had crossed the threshold for
lines of code and the hotspot's health fell 10.00 → 9.57. The rest of its
report — five advisory findings, including two duplication notices and a
complexity note on `nextest_budgets.py` — is advisory only and was not treated
as a failure.

The function genuinely was a Large Method, 84 lines doing four separable
things, so the remedy is the extraction the rule asks for rather than a
suppression. `resolve_startup_signals` now owns everything that depends only on
raw argv and the environment — the JSON hint, the terminal facts, and Clap's
help colour choice — and returns them as a `StartupSignals` value.
`run_configured_command` owns resolving the runtime display plan, configuring
the process, and dispatching the runner. `run_with_args` keeps the sequence
between them and is now 69 lines.

`make lint`'s Clippy pass accepts that; CodeScene did not, on the first
attempt: extracting the configuration load gave `load_merged_cli` five
parameters and the delta report flagged "Excess Number of Function Arguments".
That wrapper was a pure pass-through of `config_load::resolve_configuration`
adding no behaviour, so it was inlined instead. `cs delta` then measured 9.57 →
10.00 with no new issue, confirming the blocking gate clears before the push
rather than one CI round trip later.

The blocking gate did clear on the hosted run for `867e7d8bf`, and `main.rs`
left the report entirely. The *advisory* gate still failed, on four findings
that reproduce against the merge base and are this branch's own:

- `tests/cli_tests/display_plan_contract.rs` — four snapshot tests identical
  apart from which field they replace.
- `tests/bdd/steps/terminal_rendering.rs` — six step functions identical apart
  from which classifier predicate they assert.
- `tests/workflow_contracts/nextest_budgets.py::largest_test_allowance` —
  cyclomatic complexity 9, from inlining profile inheritance, override
  collection and budget folding in one function.
- `tests/cli_tests/parsing.rs::parse_cli` — grown 83 → 87 lines by this
  branch's colour-choice argument, a *degraded* pre-existing finding rather
  than a new one.

Each got the extraction its rule names, and `cs delta` then reported no new
issues at all. Repairing `parse_cli` also exposed two pre-existing emoji tests
as duplicated, so they now share one resolver call. The Rust files both read
10.00. Confirming the fix by measurement rather than by inference is the
practice the local CLI exists for: a push is a slow, shared-resource round trip
and an advisory failure there costs a full CI cycle to learn.

The advisory gate is not a required check — the branch ruleset requires
`build-test`, `kani-smoke`, `netsukefile`, and `release / metadata` — but every
recent merged pull request on this repository had it green, and a red check on
a head is worth clearing when the findings are genuinely this branch's.

### A near miss on the gate-exclusivity rule

While this was in flight a targeted `cargo nextest` run was started alongside
the still-running mutation gate. Two Rust test suites on one candidate is the
arrangement the repository's gate rules forbid, so it was stopped immediately
rather than left to finish. Nothing had reached a verdict, so no gate result
was contaminated; the mutation gate kept exclusive use of the candidate for the
remainder of its run.

## Context and orientation

Netsuke is a Rust command-line build tool: it reads a YAML-and-Jinja manifest
(`Netsukefile`) and drives Ninja. The binary entry point is `src/main.rs`; the
library root is `src/lib.rs`; `test_support/` is a workspace crate of shared
test helpers.

Policy types and flags. `src/cli/config/mod.rs` defines the `clap`-free enums
`ColourPolicy`, `EmojiPolicy`, `ProgressPolicy`, and `AccessibilityPolicy`, and
the `CliConfig` struct that `ortho_config` merges from defaults, configuration
files, `NETSUKE_*` environment variables, and the command line
(`#[ortho_config(prefix = "NETSUKE")]`). `src/cli/command.rs` declares the
flags; `src/cli/policy/values.rs` supplies localized, case-insensitive value
parsers; `src/cli/preferences.rs` maps parsed policies to runtime preferences
(`theme_preference`, `accessibility_override`, `progress_enabled`).
`src/cli/l10n/mod.rs` holds `locale_hint_from_args` and `json_hint_from_args`,
the pre-parse argument scans re-exported from `src/cli/parser/mod.rs`;
`cli::parse_with_localizer_from` in `src/cli/parser/mod.rs` builds and parses
the localized `clap` command. This task adds no configuration field; see
`docs/ortho-config-users-guide.md` for how the layers merge.

Resolution. `src/output/mode.rs::resolve_with` chooses `OutputMode::Accessible`
or `OutputMode::Standard` from the accessibility override, the colour policy,
and an injected `read_env` closure. `src/theme/mod.rs::resolve_theme` chooses
Unicode or ASCII glyph tokens (it has a private `EnvSignals` struct).
`src/output/prefs/mod.rs` wraps the theme in the `OutputPrefs` value reporters
consume. `output_mode::resolve` and `output_prefs::resolve_from_theme` are thin
wrappers that read the real process environment.

Rendering. `src/runner/reporter.rs::make_reporter` selects `AccessibleReporter`
(static labelled lines), `IndicatifReporter` (live `indicatif::MultiProgress`
bars drawn to stderr, `src/status/indicatif.rs`, which mirrors messages to
plain stderr lines when its draw target is hidden), or `SilentReporter`
(`--progress never`), wrapped in `VerboseTimingReporter` under `--verbose`.
`should_force_text_task_updates` forces plain task lines when the mode is
accessible or stdout is not a terminal. `src/main.rs::init_tracing` installs the
`tracing` subscriber; `src/startup_tracing/mod.rs::StartupWriter` buffers
early events until the output mode is known. `src/main.rs::handle_runner_error`
prints failures through `miette::Report`'s debug renderer, which uses the
process-global `miette` hook (currently the library default). `clap` renders
`--help` in `src/main.rs::parse_cli_or_exit`.

Existing tests. `tests/cli_tests/display_policy_domain.rs` sweeps the pure
resolvers against a handwritten truth model. `src/output/mode.rs`,
`src/theme/mod.rs`, `src/output/prefs/mod.rs`, and `src/runner/reporter.rs` have
`rstest` tables. Behavioural scenarios use `rstest-bdd` 0.6.0 with
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
observation (`NO_COLOR` set, stderr is a terminal). `NO_COLOR` is *set* when it
is present and not empty (decision `D4`); until EP-M3 step 4 lands, the code
still treats an empty value as set. A *decision* is what Netsuke does (style
stderr, draw live bars). The *display plan* is the complete set of decisions
for one run. An *escape sequence* is any byte sequence beginning with `ESC`
(0x1B). A *Control Sequence Introducer (CSI)* sequence is `ESC [`, then
parameter bytes 0x30–0x3F, intermediate bytes 0x20–0x2F, and one final byte
0x40–0x7E. An *SGR sequence* is a CSI sequence with final byte `m`. A *redraw
sequence* is a CSI sequence with final byte in `A B C D E F G H J K f`, a save
or restore cursor (`ESC 7`, `ESC 8`), or a *bare carriage return* (a `\r` not
followed by `\n`; a PTY turns every `\n` into `\r\n`, so `\r\n` is folded
first). An *OSC sequence* is `ESC ]`, used for hyperlinks.

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

Upstream artefacts are referenced at commit
`f8df5d808f1bee4eb8554df0160119864ade6f39` on `main` (the live PR target at the
2026-10-24 rebase; the draft was written against `ebcedaef`):

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
- [ADR-0046](../adrs/adr-0046-agent-consistent-human-first-cli.md) (`ADR-0046`)
  and [ADR-0008](../adrs/adr-0008-environment-seam-taxonomy.md) (`ADR-0008`).
- [RFC 0026](../rfcs/rfc-0026-harden-the-compiler-and-execution-boundaries.md),
  "Application requests and an owned execution port" (`RFC-0026-PRESENTATION`),
  and the layer table in
  [RFC 0027](../rfcs/rfc-0027-check-an-executable-architecture-contract.md),
  which places rendering in "Adapters and presentation" (`RFC-0027-LAYERS`).
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
ADR-0008, D12       -> INV-SINGLE-RESOLUTION, L-ENV-SIGNALS -> EP-M2 -> clippy disallowed-methods, witness tables
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
  functions kept in the test module. Artefact: `src/output/mode.rs` tests.
  Non-vacuity: the witness table exhibits each class explicitly; a seeded fault
  that lower-cases `TERM` must fail the `"DUMB"` row. Decision `D4` (Option A)
  changes one class on purpose: until EP-M3 step 4 the `""` row expects
  `no_color_set` (behaviour preservation during the refactor); step 4 flips
  that row to unset and changes the reference copy's `NO_COLOR` rule in the
  same red-green commit, so the property keeps comparing against a stated rule
  rather than silently drifting. A seeded fault that keeps `is_some()` after
  step 4 must fail the `""` row.
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
  because timestamps differ between runs. The paired comparison must hold glyph
  and accessibility policy fixed: `--color never` also selects accessible mode
  when accessibility is `auto`, so an unpinned pair differs in glyph set and
  progress rendering as well as in colour, and the comparison would no longer
  be about colour. The accessible status reporter is an intentionally excluded
  emitter: it renders a prefix glyph and a localized label and emits no escape
  at all, so the invariant holds for it by construction and the assertion that
  keeps it true is the stronger "no escape sequence of any kind". Non-vacuity:
  the colour-on output must contain SGR.
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
2. sets `FD_CLOEXEC` on both ends while holding the PTY runner's spawn mutex
   until `spawn` returns; the mutex serializes this helper's PTY launches, and
   the close-on-exec flag prevents descriptors surviving successful `exec`;
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
- Pre-merge output: the human-readable configuration-load error message is
  plain under `--color always`, while the structured `tracing` events emitted
  alongside it carry SGR. Both halves are asserted in one case per policy, so
  neither a plain-message regression nor a dropped-style regression can hide
  behind the other.

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
`src/main.rs` into a binary-local module `src/main_tracing/mod.rs`, declared
with a plain `mod main_tracing;` exactly as `main.rs` declares `startup_tracing`
(`src/startup_tracing/mod.rs`); `startup_filter` and `DiagMode` stay where
they are. In `src/cli/l10n/mod.rs` (360 lines; its tests already live in
`src/cli/l10n/tests.rs`), extract a shared `find_option_value` scanner from
`locale_hint_from_args`, as the comments on `locale_hint_from_args` and
`json_hint_from_args` ask for once "a second valued pre-clap option" exists
(`--color` is that option). If the file would exceed 400 lines, move the
pre-parse scanners into a child module `src/cli/l10n/hints.rs` first. Gates
must pass with no test change.

Commit 2 (refactor, behaviour-preserving). Introduce the public
`output_mode::EnvSignals` with `EnvSignals::read(read_env)` and `const fn`s
over it; make `resolve_with` and `theme::resolve_theme` delegate to them,
deleting the private `EnvSignals` in `src/theme/mod.rs`. Add the
`L-ENV-SIGNALS` witness table and property test first (red: the type does not
exist yet; the property compares against a verbatim copy of the old functions).

Commit 3 (the core). The core lives at `src/output/plan/mod.rs`, beside
`output::mode` and `output::prefs`, with its unit tests in
`src/output/plan/tests.rs`, and `src/lib.rs` re-exports it as
`netsuke::display_plan` in the same way it re-exports `output_mode` and
`output_prefs`. Before creating it, record the sweep (`OutputPrefs`,
`ThemeContext`, `ResolvedTheme`, `ReporterOptions` each capture one facet) and
the re-use policy (pure; no I/O; callers are the binary composition root,
`runner`, and tests; adapters read the plan but never re-derive it) in its
`//!` comment. Extract the truth model from
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
`runner::run_with_ninja_program_resolver`, `runner/help/mod.rs::resolved_prefs`,
`ExecutionContext.progress_enabled`, and
`reporter::make_reporter(plan, verbose)` consume the plan; `ReporterOptions`
loses `mode` and `stdout_is_tty`. `IndicatifReporter` receives
`ProgressDrawTarget::hidden()` when `plan.live_progress()` is false, so the
core and the renderer agree. Delete `output_mode::resolve` and
`output_prefs::resolve_from_theme`; update test callers (about twelve) to
`resolve_display_plan(cli.display_policies(), TerminalFacts::default())`, and
update any documentation example that calls `runner::run` or
`run_with_ninja_program` (sweep with `rg -n 'runner::run' docs`; none exists on
`main` at `f8df5d808f1bee4eb8554df0160119864ade6f39`, but re-check). Add
`std::io::IsTerminal::is_terminal` to `disallowed-methods` in `clippy.toml` and
run the negative control. Switch EP-M1's handwritten expectations to
plan-derived ones.

Acceptance: all four gates pass after each commit; EP-M1's suite is green
throughout, proving behaviour preservation; no snapshot changes. Conformance:
the plan never enters a build request; the core imports none of the forbidden
crates; public changes match `Interfaces and dependencies`. Recovery: each
commit reverts independently; EP-M1 detects partial rewiring. Remaining gaps:
adapters still ignore the colour decision. Compatibility: none; every caller is
updated in the same commit.

### EP-M3: bind colour and glyph decisions to the emitters

Requirements: `RM-3.12.3-COLOUR`, `INV-NO-SGR-WHEN-OFF`, `INV-COLOUR-ADDITIVE`,
`INV-GLYPHS` (diagnostics); decisions `D4` (Option A), `D8`, `D9`, and `D11`,
all accepted on 2026-10-23.

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
   `src/cli/l10n/mod.rs` (reusing the `policy_values` parser), a pure
   `display_plan::startup_colour`, and a `ColorChoice` parameter on
   `parse_with_localizer_from`, per `D9`. Cells: `--help` under `never`,
   `NO_COLOR=1`, `always` piped, and `auto` on a PTY; a usage error pinned
   plain; hint parsing rows for `--color=never`, `--color NEVER`, a repeated
   flag, and a value after `--`.
4. Empty `NO_COLOR` (`D4`, Option A): first flip the `""` row of the
   `L-ENV-SIGNALS` witness table and its reference copy, and add end-to-end
   cells for `NO_COLOR=` (set but empty) on a PTY: `generate` selects standard,
   live output; `--verbose generate` is coloured; `--help` is coloured. Run
   them red, then change `EnvSignals::read` to treat an empty value as unset
   (`is_some_and(|value| !value.is_empty())`), and run them green. No other
   cell changes, because every other cell sets `NO_COLOR` to a non-empty value
   or not at all.

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
    And the terminal output should contain "Manifest 'Netsukefile' not found"

  Scenario: NO_COLOR removes colour from terminal logs
    Given a minimal Netsuke workspace
    And the "NO_COLOR" environment variable is "1"
    When netsuke is run on a terminal with arguments "--color auto --verbose generate"
    Then the command should succeed
    And the terminal output should contain no SGR sequence
    And the terminal output should contain "Stage 6/6"
```

The positive fragment is the human diagnostic text, not the JSON code: a
pseudo-terminal run renders the human report, so `manifest_not_found` never
appears on that stream. The `NO_COLOR` row states `--color auto` rather than
relying on an omitted flag, so the scenario pins the interaction of the two
inputs instead of depending on the default.

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
Feature: Terminal rendering policy contracts

  Scenario: Emoji never keeps status glyphs ASCII
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--emoji never --accessibility off --progress always generate"
    Then the command should succeed
    And stderr status glyphs should be ASCII
    And every stderr status line should carry a text label

  Scenario: Emoji always selects Unicode status glyphs
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--emoji always --accessibility on --progress always generate"
    Then the command should succeed
    And stderr status glyphs should be Unicode
    And every stderr status line should carry a text label

  Scenario: Emoji auto on a piped run selects Unicode glyphs
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--emoji auto --accessibility off --progress always generate"
    Then the command should succeed
    And stderr status glyphs should be Unicode
    And every stderr status line should carry a text label

  Scenario: Accessibility mode keeps status glyphs ASCII under emoji auto
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--emoji auto --accessibility on --progress always generate"
    Then the command should succeed
    And stderr status glyphs should be ASCII
    And every stderr status line should carry a text label

  Scenario: Progress never suppresses stage reporting
    Given a minimal Netsuke workspace
    When netsuke is run with arguments "--progress never generate"
    Then the command should succeed
    And stderr should not contain "Stage 1/6"
    And stdout should contain "rule"
```

The four emoji rows are written out rather than kept as a Scenario Outline.
`rstest-bdd` 0.6.0's outline codegen inserts a hidden
`#[case] __rstest_bdd_case_idx: usize` parameter ahead of the outline columns,
so a three-column outline expands to a five-parameter test function and trips
`clippy::too_many_arguments` under `-D warnings`. The `glyphs` column was also
redundant: glyph family is a function of the two policy columns, so naming it
invited the outline to disagree with the policies it claimed to test. Writing
the rows out keeps one source of truth per scenario and left the lint truthful
instead of suppressed.

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
  "Colour rendering is not implemented in beta4 …" with the colour contract
  (which outputs `--color` governs; the `D11` behaviour; help follows the
  command line and `NETSUKE_COLOR` only; usage errors and pre-configuration
  errors are plain; subprocess output is forwarded unchanged). State the `D3`
  wording, the `D4` outcome, and that `TERM=dumb` keeps standard mode static.
- `docs/netsuke-design.md`: extend "Theme resolution for CLI output" with the
  display-plan core, its facts, its four adapters, and a link to ADR-0072;
  update the "guarded by `insta` snapshots" sentence to name the PTY suite and
  the contract snapshots.
- `docs/netsuke-cli-design-document.md`: document `DisplayPlan`,
  `TerminalFacts`, and the adapters as the component's internal interface, and
  correct the `NO_COLOR` statement to match `D4`.
- `docs/adrs/adr-0072-display-policy-contract-and-terminal-verification.md`:
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

EP-M3 tracing red-green evidence, 2026-11-05: the focused red run
(`/tmp/terminal-tracing-red-3-12-3.out`) failed the piped `always` case because
no SGR was emitted and the PTY `NO_COLOR=1` case because SGR was emitted. Its
`never` pipe and `auto` PTY controls passed. After adding a reloadable tracing
ANSI setting and applying it from startup and merged display policies,
`/tmp/terminal-tracing-green-3-12-3.out` reports 35/35 terminal-rendering
tests, including the update to the configuration-failure `always` contract.
`/tmp/terminal-colour-bdd-3-12-3.out` reports 309/309 behavioural tests,
including both new pipe scenarios, and
`/tmp/terminal-tracing-error-green-3-12-3.out` reports 2/2 for the
non-`RunnerError` failure's `never` and `always` cases.

EP-M2 commit 3 red-green evidence, 2026-11-04: the focused command
`cargo nextest run --test cli_tests -E 'test(display_plan)'` passed 1/1 on the
clean candidate
(`/tmp/display-plan-focused-7d2cdc6e-6293-4c60-8e07-9edea901ad54.out`); the
plan unit tests passed 14/14
(`/tmp/display-plan-unit-7d2cdc6e-6293-4c60-8e07-9edea901ad54.out`). Deliberate
faults made the same sweep report 288 mismatches after exchanging the
`NO_COLOR` and `TERM=dumb` facts for mode resolution, 864 after enabling colour
for `ColourPolicy::Never`, and 1,080 after ignoring `stdout_is_terminal` for
task updates. Their logs are
`/tmp/display-plan-seed-signal-swap-7d2cdc6e-6293-4c60-8e07-9edea901ad54.out`,
`/tmp/display-plan-seed-never-colour-7d2cdc6e-6293-4c60-8e07-9edea901ad54.out`,
and
`/tmp/display-plan-seed-stdout-fact-7d2cdc6e-6293-4c60-8e07-9edea901ad54.out`.
Each seeded production fault was reverted before the next experiment. The full
gate logs for the clean candidate use the `-3.out` suffix: `check-fmt`, `lint`,
`doc-coverage`, `typecheck`, `test`, `markdownlint`, and `nixie`. Doc coverage
measured 98.35%; nextest passed 5,117 tests with 7 skipped, and doctests
passed. The earlier Nixie temporary-directory failure did not recur.

## Interfaces and dependencies

Libraries, all already in the dependency graph: `nix` 0.31.3 (`term`, `fs`, and
`signal` features) for PTY operations; `googletest` 0.14.3 and
`pretty_assertions` 1.4.1; `rstest` 0.26.1 and `rstest-bdd` 0.6.0; `insta` 1
(`yaml`); `proptest` 1.11.0; `assert_cmd` 2; `cap-std` for fixture staging.

`FORCE_COLOR`, `CLICOLOR`, and `CLICOLOR_FORCE` are not Netsuke inputs. The
`CLICOLOR` page now describes itself as deprecated in favour of `NO_COLOR` and
`FORCE_COLOR`, and adopting `FORCE_COLOR` is a feature, not a verification
task. ADR-0072 lists it under known limitations.

In `src/output/mode.rs` (existing public functions keep their signatures):

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

In `src/output/plan/mod.rs` (fields private; queries are `const fn`, so a
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
`Cli::display_policies(&self) -> DisplayPolicies`. Put the
`EmojiPolicy -> Option<bool>`, `AccessibilityPolicy -> Option<bool>`, and
`ProgressPolicy -> bool` projections on their policy enums as `const fn`s, and
have the existing `Cli` accessors delegate to them. Keep these runtime-only
implementations in `cli/preferences.rs`, beside the `Cli` accessors: `build.rs`
compiles `cli/config/mod.rs` as a schema-only slice, where the unused policy
methods trigger denied dead-code warnings. `Cli::theme_preference` converts the
emoji projection to `ThemePreference`. Clippy also required
`main_tracing::init_tracing` to consume the stderr-terminal fact captured by
`main`, rather than make its own terminal query. ADR-0072 records these enums
as approved value dependencies of the core (an open item in RFC 0027).

Changed public signatures: `runner::run(cli: &Cli, plan: DisplayPlan)`,
`runner::run_with_ninja_program(cli: &Cli, plan: DisplayPlan, program:
&Utf8Path)`,
and `cli::parse_with_localizer_from(…, colour: ColorChoice)`. Removed:
`output_mode::resolve` and `output_prefs::resolve_from_theme`. Internal:
`reporter::make_reporter(plan: DisplayPlan, verbose: bool)`. Binary-local:
`src/main_tracing/mod.rs` with `init_tracing`, `set_tracing_filter`, and
`set_tracing_ansi(bool)`; the `miette` hook installer beside
`handle_runner_error`. In `src/cli/l10n/mod.rs`:
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
  (`runner/help/mod.rs`, `ExecutionContext`, `indicatif`'s own terminal check),
  `redraw_capable` for `TERM=dumb` and unset `TERM`, `INV-COLOUR-ADDITIVE`,
  label and isolate checks, a richer byte classifier, PTY harness flake
  controls, and completeness guards for enumeration. Dropped `force_graphical`
  from `D8` and scoped `D9` to help and version. Corrected BDD step phrases,
  the Verus rationale, the ADR status handling, and documentation coverage (CLI
  design document, repository layout). The remaining work is unchanged in
  shape: approval, then EP-M0 to EP-M4.
- 2026-10-23: recorded the user's acceptance of decisions `D3`, `D4`
  (Option A), `D8`, `D9`, and `D11`; implementation was not yet authorized.
  Made the `D4` outcome concrete: `NO_COLOR` is set only when non-empty, the
  change lands as EP-M3 step 4 with its own red cells, and the `L-ENV-SIGNALS`
  empty-string row flips in that commit. Merged `origin/main` (`8a9610008`) and
  remapped every path, ADR citation, and line count to the reorganized tree.
  The core now lives at `src/output/plan/` and the binary-local tracing module
  uses a plain `mod` declaration. The ADR serial moved from ADR-041 (since
  taken) to ADR-0060 in `docs/adrs/`. Milestone structure, obligations, and
  tolerances are unchanged.
- 2026-10-24: the user approved implementation. Replaced the old base and
  dependency-version references with the rebased tree after rebasing onto
  `origin/main` at `f8df5d808f1bee4eb8554df0160119864ade6f39`. Nix source
  inspection showed `fs` and `signal` are required for safe descriptor flags
  and process-group termination. Corrected the PTY mutex claim: it coordinates
  only this helper, while `FD_CLOEXEC` protects descriptors across successful
  `exec`; unrelated spawns can race the short interval before the flag is set.
  Added D13 after the full suite timed out in a nested public API compile under
  shared compiler pressure. The remaining baseline gates passed except
  `make test`, where `case_2_compiler_diagnostic` reached the 900-second limit.
  The captured admission evidence and its limits are recorded in D14.

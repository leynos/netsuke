# Issue 800: Rewrite release admission as a Python gate script

Status: COMPLETE

Delivered, validated, and reviewed. Two CodeRabbit passes have concluded: the
first raised three findings, all triaged; the second read the revision that
answered them and returned none. The pull request remains open for the
repository's own review and merge, which is a separate concern from this
plan's completion.

## Purpose

Release admission is roughly 300 lines of Bash under `.github/scripts/`
(`require-release-admission-canaries.sh` and its two siblings). The estate rule
in `docs/scripting-standards.md` requires gate logic to be Python: loops, `;`
chains, and conditionals in workflows become a `uv`-run Cyclopts script with
cmd-mox tests. Admission is netsuke's own policy, so it stays in netsuke; only
its form must change.

Success is observable when `.github/scripts/release_admission.py` runs under
`uv run --script`, reproduces the Bash behaviour case for case, has cmd-mox
tests for every reachable RFC 0005 admission and refusal path, keeps every
refusal's wording or states why it differs, and is what `release.yml` invokes
once the shell scripts are deleted.

## Constraints

- Behaviour must match the current shell case for case: every input, operation
  order, argument vector, classification, record, sink rule, and exit status.
- The fail-closed `fresh` contradiction and Bash's trailing-newline semantics
  are preserved, not repaired.
- Refusals keep their current wording, or the change states why the wording
  differs.
- Observation mode and publication independence are preserved.
- `release-dry-run.yml` is unchanged.
- The Python is the estate shape: Python 3.14, PEP 723 `uv` block, shebang
  `#!/usr/bin/env -S uv run --script`, Cyclopts env-first, tests under
  `scripts/tests/` using cmd-mox.

## Progress

- [x] Freeze the shell behaviour. The measurements are recorded in
      `scripts/tests/release_admission_test_support.py`'s module docstring:
      exit statuses, stderr, JSONL key order and types, default paths, and
      environment variables.
- [x] Implement the Python gate. `.github/scripts/release_admission.py` is the
      entry point; `.github/scripts/_release_admission/` holds seven modules
      (`policy`, `records`, `delivery`, `commands`, `telemetry`,
      `configuration`, `gate`); the last of those was split out of `gate` when
      the CodeScene refactor pushed it over the module ceiling. The
      runtime-test support in `scripts/tests/` is a facade and five modules,
      after the two-stage split recorded under *Decisions and findings*.
- [x] Clear the Python gates. `make lint-python` passes all five stages:
      Ruff 0.16.4, Pylint 4.0.9 at 10.00/10, the df12 house lints, ambrleaks,
      and interrogate at 100%.
- [x] Repoint the runtime tests at the Python gate. The 33-case frozen oracle
      passes against `.github/scripts/release_admission.py`, including the
      PR #770 newline-revision cases in both modes.
- [x] Replace the executable fakes with cmd-mox and port the case matrix.
      `release_admission_test_fakes.py` is deleted, the four runtime modules
      run through registered doubles, and the modules pass with 37 cases in
      under 90 seconds. The design the port settled on, and the reasons:
      - The subprocess boundary stays. The gate is still invoked as a real
        process, because only that exercises its shebang, its exit status, its
        artefact files, and its redirections. cmd-mox replaces the *fake
        executables*, intercepting at the command-name level.
      - `python3` is never shimmed. cmd-mox's POSIX shims are symlinks whose
        shebang is `#!/usr/bin/env python3`, so a shim named `python3` would
        resolve its own interpreter back to itself. The clock therefore runs
        as the real `python3` by default, and the tests that need a
        deterministic or failing clock point
        `NETSUKE_RELEASE_ADMISSION_CLOCK_ADAPTER` at a shim named `clock`.
      - `gh` and `git` are shimmed under their documented default names, so
        the default resolution path is what the tests exercise; an
        operator-supplied adapter path is covered by pointing an adapter
        variable at a shim by absolute path.
      - The in-process handler cannot ignore `SIGTERM`, so the
        term-ignoring-timeout case moves out of the gate matrix and becomes a
        direct test of `commands.run_bounded_sync` against a real
        TERM-trapping child. In the gate's records a TERM-ignoring child and a
        compliant one are indistinguishable: `run_bounded` returns `None` on
        either, and `classify_result` maps that to `124`.
      - `NETSUKE_FAKE_*` variables disappear. Behaviour is configured by
        registering doubles, which is cmd-mox's own idiom, and the assertions
        move onto the doubles' recorded invocations.
      - The doubles are spies, and every count assertion is a direct
        comparison against the recorded calls. cmd-mox verifies expectations
        for mocks only, so a spy's `times_called` would set an expectation
        nothing reads; `in_order()` on a spy is worse, because an ordered
        expectation nothing consumes makes `verify()` raise. Ordering is
        therefore asserted on the recorded calls, where it is data.
- [x] Switch `release.yml` to the Python entry point and delete the shell. The
      admission step now runs `uv run --script .github/scripts/release_admission.py`;
      the three shell scripts are deleted; `release-dry-run.yml` is unchanged.
- [x] Update the workflow contract, ADR-020, and the developers' guide. The
      contract asserts the Python invocation, the `astral-sh/setup-uv`
      provisioning, observation mode, publication independence, and the
      absence of any `*release-admission*.sh` reference. The developers' guide
      describes the entry point, the module split, and the cmd-mox conventions.
- [x] Clear the CodeRabbit findings and the CodeScene advisory findings. The
      six review findings are one commit; the advisory findings on code this
      change introduces are a second, atomic refactor commit, together with the
      module split that refactor forced. The split could not be a third commit:
      the refactor left `gate.py` at 442 lines, so the state between them fails
      Pylint's `C0302` and no commit may be red. Everything is measured locally
      with `cs check` against the file rather than the commit, because
      `cs check HEAD:./file` reads the committed blob and reports the previous
      revision's issues.
- [x] Clear the first review pass over the delivered port. Three findings: a
      dead `or 0` and this plan's revision-note ordering were accepted, and a
      proposed counter suppression on an unusable clock reading was rejected
      against measurement. The rejection also corrected a false sentence in the
      frozen oracle's docstring, which was the finding's premise, and added the
      regression case that pins the measured behaviour.

## Decisions and findings

- Cyclopts' default result action, `print_non_int_return_int_as_exit_code`,
  returns an integer command result unchanged. The first version of the entry
  point called `app()` and then returned a literal `0`, which discarded the
  gate's own status and made every enforced failure exit successfully. The fix
  is to return `app()`'s value; the alternative considered was an explicit
  `sys.exit(admission.finish())` inside `main`, which would move the process
  boundary into the command and make the function untestable in process.

- Bounding is native. The shell wrapped each command in GNU `timeout`;
  `commands.run_bounded` reproduces both the one-second grace and the `124`
  (SIGTERM) / `137` (SIGKILL) classification through cuprum's cancellation
  path, so a timeout no longer depends on an external binary being present.
- The configured adapter names are the cuprum allowlist catalogue, because the
  adapter environment variables are a documented part of this gate's contract.
- A launch the operating system refuses (a missing adapter, a file without its
  execute bit) is not a new failure kind. GNU `timeout` printed its own
  sentence and exited `126` or `127`; cuprum raises instead, so the port prints
  `release-admission adapter could not be run: <program>: <reason>` and maps
  the exception back to the same status. The status and classification are
  preserved; the sentence is a stated wording difference.
- `os.environ.get(VAR, default)` is not equivalent to Bash's `${VAR-default}`.
  The dash substitutes only when the variable is *unset*, so an exported empty
  `NETSUKE_RELEASE_ADMISSION_ENFORCE` was kept and refused by the gate's own
  mode check rather than replaced with the observation default. The membership
  test in `configuration.load_configuration` is therefore load-bearing and
  carries a comment saying so.
- The 400-line module ceiling applies to Python as well as Rust (Pylint
  `C0302`, `max-module-lines = 400`). `release_admission_test_support.py` was
  exactly 400 lines at HEAD, so the runtime-test support was split into a facade
  (`..._test_support.py`), the subprocess harness (`..._test_harness.py`), and
  the record assertions (`..._test_records.py`). The cmd-mox port then split
  three further modules out of the facade: the doubles (`..._test_doubles.py`),
  the wrappers each case composes (`..._test_scenarios.py`), and the case table
  (`..._test_cases.py`). The support is therefore a facade and five modules,
  against the three of the first split.
- The repository has no `pylint: disable` comment anywhere and no function
  over four parameters outside the new gate. AGENTS.md requires grouping
  parameters into meaningfully named structs, so `records.py` gained the
  `MetricFields`, `TraceFields`, `OperationRecord`, and `GateRecord` value
  objects rather than argument-count suppressions.
- `configuration` is a module of its own because the split that cleared the
  CodeScene complexity finding left `gate` at 442 lines. The division is by
  question asked, not by line count: `configuration` owns everything the gate
  is *told* -- the required variables, their diagnostics, Bash's two default
  forms, and the artefact paths -- and `gate` owns everything it then *does*.
  `gate` therefore imports one name, `Configuration`, and the entry point
  reaches `configuration` directly for the three variable names Cyclopts binds
  and for `load_configuration`/`ConfigurationError`. The alternative, having
  `gate` re-bind every moved name, was rejected: it would have made `gate` an
  indirection no reader needed, and one of the names it would have carried,
  `DIAGNOSTIC_PREFIX`, turned out to be referenced nowhere at all and was
  deleted instead of moved.
- `from __future__ import annotations` is redundant on the 3.14 baseline and
  the df12 house lints reject it (`C9112`). PEP 649 defers annotation
  evaluation, so a `TYPE_CHECKING`-only `Path` in a dataclass annotation is
  safe without it; the repository does not support runtime annotation
  introspection on these modules (ADR-038).

## Deferred

Producer-backed refusals are deferred. The gate has no scan producer of its own
yet, so `NETSUKE_RELEASE_ADMISSION_EVIDENCE_STATE` arrives from the environment
and the `missing_evidence`, `stale_evidence`, and `mismatch` categories are
reachable; the categories that require a real producer to observe are named as
deferred in the test module rather than simulated.

## Outcomes and retrospective

The port is complete, validated, and open for review. The Bash gate is now a
Cyclopts `uv run --script` entry point with its implementation split across
seven modules, its behaviour held to a frozen oracle, and its tests running
through cmd-mox doubles. `release.yml` invokes the Python entry point; the
three shell scripts are deleted.

Two review passes have run over it. The first, at `30b961d8`, raised three
findings: two were accepted and fixed, and one was rejected against a
measurement of the Bash gate — see the revision note for the account. The
second, at `c6a9d023`, read the revision that answered the first and returned
no findings across all thirty changed files. Both revisions were gated in full
before their pass was requested; the second revision's gates are
`make check-fmt`, `make lint-python`, `make typecheck-python`, `make lint` (all
twelve sub-suites, with Clippy clean so Whitaker genuinely ran),
`make doc-coverage` (98.87%), `make test` (3917 passed, 6 skipped, 129
doctests), `make test-release-admission` (38 passed),
`make test-workflow-contracts` (1150 passed, 3 skipped), `make markdownlint`,
and `make github-actions-lint`.

### Behaviour preserved

- The five operations, their order, argument vectors, classification, records,
  sink rules, and exit statuses are unchanged, and the frozen oracle in
  `scripts/tests/release_admission_test_support.py` remains the evidence.
- Every refusal keeps its wording. The seven stderr diagnostics operators may
  key on are byte-identical.
- Observation mode and publication independence are unchanged; the contract
  still asserts that no publication job depends on the canary job.
- Bounding is native: `commands.run_bounded` reproduces GNU `timeout`'s
  one-second grace and its `124`/`137` classification without an external
  binary.

### Stated differences

Two differences exist, both deliberate and both recorded in the entry point's
module docstring:

1. An adapter the operating system refuses to launch is reported as
   `release-admission adapter could not be run: <program>: <reason>` where the
   shell's GNU `timeout` printed its own sentence. Same position, same exit
   status, same classification; only the sentence differs.
2. A refused configuration and a refused metric write have no other stderr at
   all, exactly as before, so a refused observation mode still fails the step.

A third difference was expected and does not exist. The port renders a duration
with `str(float)` where the shell interpolated its value with `printf %s`,
which looked like it would diverge on float formatting. It does not: the shell
computed every duration by piping both readings through a `python3` helper that
ended in `print(...)`, so both sides are `str(float)` of the same value. A
nine-case probe over sub-second, large, exponent, `nan`, and integer readings
produced byte-identical output on both sides, and the records are
byte-identical with it.

One behaviour is deliberately preserved although it reads as a contradiction:
`check_scan_freshness` admits `fresh`, but `verify_evidence` refuses it unless
the state is *not* fresh and a workflow run identifier is present. The port
preserves this exactly rather than repairing it, because repairing it would
change admission semantics beyond the scope of a form change.

### Lessons

- `cs check HEAD:./file` reads the committed blob, not the working tree, so a
  refactor that has not been committed yet is scored against the previous
  revision and reports the very issues it just fixed. Measure with
  `cs check ./file` until the refactor is committed.
- Cyclopts' default result action returns an integer command result unchanged.
  Returning a literal `0` after `app()` silently discards every enforced
  failure's status; return `app()` itself.
- The 400-line module ceiling is enforced for Python too (Pylint `C0302`), so
  the runtime-test support had to be split twice: into a facade and three
  modules before the port, then into a facade and five once cmd-mox replaced
  the executable fakes.
- A refactor that clears a complexity finding lengthens the code it clarifies,
  and the ceiling applies to the result. Extracting `_OutputPaths`,
  `_optional`, and `_defaulted_only_when_unset` from `load_configuration` took
  `gate.py` from well inside the ceiling to 442 lines, so the refactor could
  not be committed on its own: it had to carry the split that made the module
  fit. "Separate atomic refactors" is a rule about intent, not a licence to
  commit a red tree.
- No gate in this repository collects doctests from `.github/scripts/`.
  `make test` runs the Rust suites, and `make test-workflow-contracts` passes
  `--doctest-modules` to `scripts/*.py` rather than to the implementation
  package. Two examples in `records.py` had therefore been raising
  `SyntaxError` since they were written, with every gate green over them. Run a
  new `Examples` block rather than reading it: a docstring containing `>>>` is
  not evidence that the block executes.
- `os.environ.get(VAR, default)` is not Bash's `${VAR-default}`: the dash
  substitutes only when the variable is unset. An exported empty
  `NETSUKE_RELEASE_ADMISSION_ENFORCE` must reach the mode check, not be
  replaced by the observation default, so `configuration.load_configuration`
  tests membership rather than truthiness.
- A review finding can cite this repository's own prose as its authority. The
  counter-suppression finding quoted a sentence from the frozen oracle's
  docstring that asserted the shell dropped an operation's counter with its
  duration. The sentence was false, and the shell's `emit_metric` validates the
  counter's literal `1` separately from the duration, so adopting the fix would
  have *introduced* the divergence it claimed to prevent. Measure the claim
  against the shell before acting on it, even when the claim appears to quote
  the oracle; a false sentence in a document is not evidence about behaviour.
- The frozen oracle's docstring is prose, not a test. Nothing collects it —
  no test module reads it, and no gate runs it — so a wrong sentence there
  survives every gate and then misleads a reviewer. When a measurement
  contradicts the docstring, correcting the docstring is part of the fix rather
  than an afterthought; the missing regression case for the garbage clock was
  the second half of the same gap.
- `make spelling` regenerates `typos.toml` from the shared dictionary on every
  run. The dictionary's colour-flag rule had changed since the committed copy,
  leaving a clean checkout dirty after a spelling run. That one-line
  regeneration is committed separately at the head of this branch so a reviewer
  can split it without touching the port's commits.

## Revision note

- 2026-10-10 — Initial ExecPlan for `#800`: freeze the Bash gate's behaviour,
  port it to a Cyclopts `uv run --script` entry point, port the runtime cases
  to cmd-mox, switch `release.yml` over, and delete the shell.
- 2026-10-10 — Review pass. Two contradictions in this plan were corrected
  rather than papered over. The ExecPlan claimed a *facade and four modules*
  while its own account of the first split named two, and the case count had
  gone stale at 35; both counts are now stated per stage, because the support
  was split twice — into a facade and three modules before the port, and into a
  facade and five once cmd-mox replaced the executable fakes. The status stays
  `IN PROGRESS` with a review caveat beneath the header, since the style guide
  reserves `COMPLETE` for a plan whose review has concluded. The port is
  unaffected: no source file, gate, workflow, or test case changed with this
  note.
- 2026-10-10 — CodeScene pass and the module split it forced. The advisory
  review named three issues on code this change introduces, and a fourth
  surfaced from a test this pass had already lengthened. The refactor: `policy`
  gained a `Vocabulary` base whose `contains` class method replaced five
  same-shaped predicates; `load_configuration` was split so each Bash default
  form has one named helper (`_optional` for `${VAR:-default}`,
  `_defaulted_only_when_unset` for `${VAR-default}`) and the artefact paths
  travel as an `_OutputPaths` value; and the two oversized tests were reduced
  by reusing `assert_failure_trace_sequence` and by extracting
  `_run_under_generated_identifiers`. That work took `gate.py` to 442 lines, so
  the same commit moves the configuration half into the new `configuration`
  module. The entry point and the developers' guide now name `configuration`
  where they named `gate`; `DIAGNOSTIC_PREFIX`, a constant the port introduced
  but nothing ever read, is deleted rather than moved. CodeScene scores every
  touched file at 10.00, and the 37-case runtime suite is unchanged, which is
  the evidence that the refactor preserved behaviour. The developers' guide
  records both conventions, as the house abstraction policy requires. One
  instrument detail is worth keeping: `cs check HEAD:./file` scores the
  *committed* blob, so a working-tree refactor must be measured with
  `cs check ./file`.
- 2026-10-10 — Doctest repair. Checking the moved examples by hand turned up a
  defect no gate had run: `MetricFields.is_valid`'s two examples wrapped as
  ``MetricFields(...)`` then ``.is_valid()``. The first line is a complete
  statement, so the second was a second statement and doctest compiled the pair
  as one — every run raised ``SyntaxError``. Nothing collected these doctests:
  `make test` runs the Rust suites, `make test-workflow-contracts` runs
  `--doctest-modules` over `scripts/*.py` rather than the package, and the
  runtime suite invokes the gate as a subprocess. The examples now break inside
  the call, which is the shape `TraceFields.is_valid` already used, and all 24
  examples in the package pass. Recorded because the same wrap is easy to
  reintroduce, and because "a docstring with an `Examples` block" is not
  evidence that the block runs.
- 2026-10-10 — The first CodeRabbit pass over the delivered port. Three
  findings. Two were accepted: a dead `or 0` in `telemetry.write_output`, whose
  expression could never differ from its operand, and this revision note's own
  ordering (the initial entry sat below two later ones, with a blank line
  splitting the list). The third proposed suppressing an operation's counter
  together with its duration when the clock returns garbage. That was
  **rejected, and the rejection is the interesting part**: it would have
  introduced the divergence it claimed to prevent. The shell validates the
  counter's literal `1` separately from the duration, so a garbage reading
  drops the duration and the operation trace and still writes the counter. The
  finding's premise was a sentence in this branch's own oracle docstring
  asserting the opposite, which measurement disproved; that sentence is now
  corrected, and the reviewer's confidence in it was the reason to measure
  rather than to patch. A nine-shape clock matrix (garbage on every read, on
  the first read only, on the finish reads only, and a non-zero exit) confirms
  the shell writes one counter per operation and the port matches it
  byte-for-byte on the records.
- 2026-10-10 — The second CodeRabbit pass, over the revision that answered the
  first. It returned **no findings** across all thirty changed files. The
  interesting part is what that silence cost to earn: the port was re-gated in
  full on the exact bytes committed, and the three defects the first pass found
  were each either fixed or falsified before the pass was requested. A clean
  review is evidence about a revision, not about a change, so the revision it
  read is named here — `c6a9d023` — and the gates that ran against it are
  recorded under *Outcomes and retrospective*.

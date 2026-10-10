# Issue 800: Rewrite release admission as a Python gate script

Status: IN PROGRESS

Delivered and validated; the status stays `IN PROGRESS` because the change is
still open for review. The style guide reserves `COMPLETE` for a plan whose
work is delivered *and* whose review has concluded, and this one has not
concluded yet.

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
      entry point; `.github/scripts/_release_admission/` holds six modules
      (`policy`, `records`, `delivery`, `commands`, `telemetry`, `gate`). The
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
  test in `gate.load_configuration` is therefore load-bearing and carries a
  comment saying so.
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
Cyclopts `uv run --script` entry point with its implementation split across six
modules, its behaviour held to a frozen oracle, and its tests running through
cmd-mox doubles. `release.yml` invokes the Python entry point; the three shell
scripts are deleted.

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

- Cyclopts' default result action returns an integer command result unchanged.
  Returning a literal `0` after `app()` silently discards every enforced
  failure's status; return `app()` itself.
- The 400-line module ceiling is enforced for Python too (Pylint `C0302`), so
  the runtime-test support had to be split twice: into a facade and three
  modules before the port, then into a facade and five once cmd-mox replaced
  the executable fakes.
- `os.environ.get(VAR, default)` is not Bash's `${VAR-default}`: the dash
  substitutes only when the variable is unset. An exported empty
  `NETSUKE_RELEASE_ADMISSION_ENFORCE` must reach the mode check, not be
  replaced by the observation default, so `gate.load_configuration` tests
  membership rather than truthiness.
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

# Issue 800: Rewrite release admission as a Python gate script

Status: IN PROGRESS

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
      (`policy`, `records`, `delivery`, `commands`, `telemetry`, `gate`).
- [x] Clear the Python gates. `make lint-python` passes all five stages:
      Ruff 0.16.4, Pylint 4.0.9 at 10.00/10, the df12 house lints, ambrleaks,
      and interrogate at 100%.
- [ ] Replace the executable fakes with cmd-mox and port the case matrix.
- [ ] Switch `release.yml` to the Python entry point and delete the shell.
- [ ] Update the workflow contract, ADR-020, and the developers' guide.

## Decisions and findings

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
  the record assertions (`..._test_records.py`).
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

Not yet complete.

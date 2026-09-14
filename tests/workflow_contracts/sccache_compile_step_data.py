"""Hold the compile-step recognizer used by `sccache_contract_test.py`.

`sccache_contract_test.py` needs to know which workflow steps compile the
Rust tree so it can check that a job's sccache reset and report steps bracket
every one of them. That recognizer, and the shared-action constant it checks
alongside a step's `run` script, are data and heuristics rather than
contracts, so they live here rather than growing the test file past the
repository's 400-line file limit. This module holds no tests of its own.

Run via ``make test-workflow-contracts``.
"""

from pathlib import PurePosixPath

#: The shared `generate-coverage` action, which runs an instrumented
#: `cargo llvm-cov` build rather than a plain `run:` compiler invocation.
GENERATE_COVERAGE_ACTION = "leynos/shared-actions/.github/actions/generate-coverage@"

#: The Cyclopts helper every compiling lane reports through; it writes the
#: text and JSON files and the summary section, covered by
#: `scripts/tests/test_ci_report_sccache_stats.py`.
SCCACHE_REPORT_SCRIPT = "uv run --script scripts/ci/report_sccache_stats.py"
#: The one lane that only prints the JSON form and keeps no files: the
#: release smoke job has no uv and nothing downstream reads its statistics.
JSON_ONLY_REPORTS = frozenset({("release.yml", "windows-native-recipe-smoke")})


def assert_report_command(workflow_name: str, job_name: str, command: str) -> None:
    """Assert a job's statistics step reports the way its lane is allowed to.

    Parameters
    ----------
    workflow_name
        Workflow file the job lives in.
    job_name
        The compiling job under test.
    command
        The ``run`` script of its ``Show sccache statistics`` step.
    """
    command = command.strip()
    if (workflow_name, job_name) in JSON_ONLY_REPORTS:
        assert "sccache --show-stats --stats-format=json" in command, (
            f"{workflow_name} {job_name} must export machine-readable sccache "
            "statistics"
        )
        return
    assert command == SCCACHE_REPORT_SCRIPT, (
        f"{workflow_name} {job_name} must report through the checked-in "
        f"{SCCACHE_REPORT_SCRIPT!r} rather than inline shell, got {command!r}"
    )


def invokes_build_command(line: str) -> bool:
    """Return whether a shell line's first word is `make` or `cargo`.

    Parameters
    ----------
    line
        A single line from a step's `run` script.

    Returns
    -------
    bool
        ``True`` when the line's first token is `make` or `cargo`, ignoring
        any leading path.

    Notes
    -----
    Checking the first word, rather than scanning the whole line for these
    substrings, matters: `ci-windows.yml`'s `Install GNU Make` step runs
    `choco install make`, where `make` is the installer's argument rather
    than an invoked command, and a substring match would misclassify that
    installer as the step that starts compiling. A leading path, such as
    `/usr/bin/make` in `ci.yml`'s `Lint` step, is normalized to its
    basename so the pinned invocation is still recognized.
    """
    stripped = line.strip()
    if not stripped:
        return False
    first_token = stripped.split(maxsplit=1)[0]
    return PurePosixPath(first_token).name in {"make", "cargo"}


def is_compile_step(step: dict[str, object]) -> bool:
    """Return whether a workflow step compiles the Rust tree.

    Parameters
    ----------
    step
        A single normalized workflow step.

    Returns
    -------
    bool
        ``True`` when the step invokes `make` or `cargo`, or calls the
        shared `generate-coverage` action.

    Notes
    -----
    A step counts as compiling when it invokes `make` or `cargo` as the
    first word of one of its script's lines, which covers `cargo build`
    directly and every `make` target that in turn reaches `cargo`, or when
    it calls the shared `generate-coverage` action. The recognizer is
    deliberately coarse: this module has no record of which `make` targets
    compile and which only lint, format, or check documentation, so a
    housekeeping step such as `Show rustc version` (`cargo --version`) is
    counted alongside the real build. That overcount is harmless here
    because the statistics contract only needs the position of the first
    and last compiling step, not an exhaustive inventory of them.
    """
    if str(step.get("uses", "")).startswith(GENERATE_COVERAGE_ACTION):
        return True
    script = str(step.get("run", ""))
    return any(invokes_build_command(line) for line in script.splitlines())

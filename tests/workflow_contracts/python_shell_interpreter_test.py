"""Contract tests holding every ``shell: python`` step to the Python baseline.

GitHub Actions' ``python`` shell runs whichever ``python`` is first on
``PATH``. setup-uv's ``python-version`` input only sets ``UV_PYTHON``, so
without ``activate-environment`` the trusted coverage steps ran under the
runner's system interpreter (3.12 on ubuntu-2404). The trusted modules are
written to the baseline, and under 3.12 three of them raised ``NameError`` as
they loaded, because annotations were evaluated at definition time against
imports made only under ``typing.TYPE_CHECKING``. ``Coverage (PR submission)``
then failed on every pull request, and no pull request could fix it, because
the workflow always runs the default branch's copy.

These tests make that invariant explicit for every workflow: a job that runs
``shell: python`` must first check out the trusted tree and activate a uv
environment at the Makefile's ``PYTHON_BASELINE``, and its first Python step
must be the checked-in interpreter check, which loads anywhere and names the
mismatch. ``make lint-workflow-scripts`` covers the other half: every trusted
module must load under the baseline.

Run via ``make test-workflow-contracts``.
"""

import argparse
import ast
import importlib.util
import re
import runpy
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the script boundary is under test.
import sys
import types
import typing as typ

import pytest
from python_action_dispatch import assert_python_action_dispatch
from workflow_loading import (
    MAKEFILE_PATH,
    REPO_ROOT,
    load_workflow,
    require_mapping,
)

if typ.TYPE_CHECKING:
    from pathlib import Path


WORKFLOWS_DIR = REPO_ROOT / ".github" / "workflows"
VERIFY_MODULE = ".github/scripts/verify_python_baseline.py"
VERIFY_COMMAND = "verify"
SETUP_UV_PREFIX = "astral-sh/setup-uv@"
CHECKOUT_PREFIX = "actions/checkout@"


def _makefile_python_baseline() -> str:
    """Return the Makefile's ``PYTHON_BASELINE ?=`` default."""
    text = MAKEFILE_PATH.read_text(encoding="utf-8")
    matches = re.findall(r"^PYTHON_BASELINE \?= (\S+)$", text, flags=re.MULTILINE)
    assert len(matches) == 1, "the Makefile must declare PYTHON_BASELINE once"
    return matches[0]


def _python_shell_jobs() -> list[tuple[str, str, list[dict[str, object]]]]:
    """Return every job, in every workflow, that runs a ``shell: python`` step."""
    found = []
    for path in sorted(WORKFLOWS_DIR.glob("*.yml")):
        jobs = require_mapping(load_workflow(path).get("jobs"), f"{path.name} jobs")
        for job_name, job in jobs.items():
            job_mapping = require_mapping(job, f"{path.name} job {job_name}")
            match job_mapping.get("steps"):
                case list() as steps:
                    pass
                case _:
                    continue
            step_mappings = [
                require_mapping(step, f"{path.name} {job_name} step") for step in steps
            ]
            if any(step.get("shell") == "python" for step in step_mappings):
                found.append((path.name, job_name, step_mappings))
    return found


#: Jobs that run ``shell: python`` but cannot provision an interpreter under
#: the current trust contracts. ``report-excluded-fork`` is held by
#: ``trust_boundary_test.py`` to ``checks: write`` alone and to telemetry
#: steps that bracket the publication, which leaves no room for the checkout
#: and setup-uv steps this contract requires. Every other invariant here still
#: applies to it. Removing a job from this set is the intended path once its
#: trust contract admits the provisioning steps.
UNPROVISIONED_JOBS = frozenset({("coverage-pr-submit.yml", "report-excluded-fork")})

PYTHON_SHELL_JOBS = _python_shell_jobs()
JOB_IDS = [f"{workflow}:{job}" for workflow, job, _ in PYTHON_SHELL_JOBS]
PROVISIONED_JOBS = [
    (workflow, job, steps)
    for workflow, job, steps in PYTHON_SHELL_JOBS
    if (workflow, job) not in UNPROVISIONED_JOBS
]
PROVISIONED_IDS = [f"{workflow}:{job}" for workflow, job, _ in PROVISIONED_JOBS]


def _first_index(steps: list[dict[str, object]], key: str, needle: str) -> int:
    """Return the first step index whose ``key`` value starts with ``needle``."""
    for index, step in enumerate(steps):
        if str(step.get(key, "")).startswith(needle):
            return index
    return -1


def test_the_trusted_coverage_workflow_is_covered() -> None:
    """The contract is live: the workflow that motivated it still runs Python."""
    assert "coverage-pr-submit.yml:submit-coverage" in PROVISIONED_IDS, (
        f"expected the trusted coverage job among {PROVISIONED_IDS!r}"
    )


def test_unprovisioned_jobs_still_exist_and_run_python() -> None:
    """The exemption names live jobs only, so it is pruned rather than forgotten."""
    live = {(workflow, job) for workflow, job, _ in PYTHON_SHELL_JOBS}
    stale = sorted(UNPROVISIONED_JOBS - live)
    assert not stale, (
        f"UNPROVISIONED_JOBS names jobs that no longer run shell: python: {stale!r}"
    )


@pytest.mark.parametrize(
    ("workflow", "job", "steps"), PROVISIONED_JOBS, ids=PROVISIONED_IDS
)
def test_python_shell_jobs_activate_the_baseline_interpreter(
    workflow: str, job: str, steps: list[dict[str, object]]
) -> None:
    """Every ``shell: python`` job puts a baseline uv environment on PATH first."""
    baseline = _makefile_python_baseline()
    first_python = next(
        index for index, step in enumerate(steps) if step.get("shell") == "python"
    )
    setup_index = _first_index(steps, "uses", SETUP_UV_PREFIX)
    checkout_index = _first_index(steps, "uses", CHECKOUT_PREFIX)

    assert 0 <= checkout_index < first_python, (
        f"{workflow} {job} must check out the trusted tree before running "
        f"checked-in Python (checkout at {checkout_index}, python at {first_python})"
    )
    assert 0 <= setup_index < first_python, (
        f"{workflow} {job} must set up uv before its first python step "
        f"(setup at {setup_index}, python at {first_python})"
    )
    with_ = require_mapping(steps[setup_index].get("with"), f"{job} setup-uv inputs")
    assert with_.get("python-version") == baseline, (
        f"{workflow} {job} must install the Makefile PYTHON_BASELINE "
        f"({baseline!r}) through setup-uv, got {with_.get('python-version')!r}"
    )
    assert with_.get("activate-environment") in {True, "true"}, (
        f"{workflow} {job} must set activate-environment so the python shell "
        f"resolves the baseline interpreter rather than the runner's own; "
        f"python-version alone only sets UV_PYTHON, got {with_!r}"
    )


@pytest.mark.parametrize(
    ("workflow", "job", "steps"), PROVISIONED_JOBS, ids=PROVISIONED_IDS
)
def test_python_shell_jobs_verify_the_interpreter_first(
    workflow: str, job: str, steps: list[dict[str, object]]
) -> None:
    """The first Python step is the interpreter check, which loads anywhere."""
    first_python = next(step for step in steps if step.get("shell") == "python")
    try:
        assert_python_action_dispatch(first_python, VERIFY_MODULE, VERIFY_COMMAND)
    except AssertionError as error:
        pytest.fail(
            f"{workflow} {job} must run {VERIFY_MODULE} {VERIFY_COMMAND!r} as its "
            f"first python step, so an interpreter mismatch is named rather than "
            f"surfacing as a NameError in a later module: {error}"
        )


def _load_verify_module() -> types.ModuleType:
    """Load the interpreter check module from its checked-in path."""
    path: Path = REPO_ROOT / VERIFY_MODULE
    specification = importlib.util.spec_from_file_location(
        "verify_python_baseline", path
    )
    assert specification is not None, f"{VERIFY_MODULE} must be importable"
    assert specification.loader is not None, f"{VERIFY_MODULE} must have a loader"
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


def test_verify_module_baseline_matches_the_makefile() -> None:
    """The module's BASELINE and the Makefile's PYTHON_BASELINE cannot drift."""
    module = _load_verify_module()
    baseline = ".".join(str(part) for part in module.BASELINE)
    assert baseline == _makefile_python_baseline(), (
        f"{VERIFY_MODULE} BASELINE ({baseline!r}) must equal the Makefile "
        f"PYTHON_BASELINE; bump both in the same commit"
    )


def test_verify_module_uses_only_builtin_names_in_annotations() -> None:
    """The check itself must load on any interpreter, or it cannot report one."""
    source = (REPO_ROOT / VERIFY_MODULE).read_text(encoding="utf-8")
    guarded_imports = [
        node
        for node in ast.walk(ast.parse(source))
        if isinstance(node, ast.If) and "TYPE_CHECKING" in ast.unparse(node.test)
    ]
    assert not guarded_imports, (
        f"{VERIFY_MODULE} must not import under TYPE_CHECKING: an annotation "
        f"naming such an import raises NameError on the interpreters this "
        f"module exists to reject"
    )


def _baseline_mismatch_cases() -> list[tuple[tuple[int, ...], str | None, str | None]]:
    """Return baseline-derived interpreter cases and their expected messages."""
    module = _load_verify_module()
    major, minor = module.BASELINE
    required = f"need Python {major}.{minor}"
    runner_system = (major, minor - 2, 13)
    newer = (major, minor + 1, 0)
    return [
        ((major, minor, 0), None, None),
        ((major, minor, 7), None, None),
        (
            runner_system,
            f"found {'.'.join(str(part) for part in runner_system)}",
            required,
        ),
        (
            newer,
            f"found {'.'.join(str(part) for part in newer)}",
            required,
        ),
    ]


@pytest.mark.parametrize(
    ("version_info", "expected", "required"),
    _baseline_mismatch_cases(),
    ids=(
        "baseline",
        "baseline-later-micro",
        "runner-system-python",
        "newer-than-baseline",
    ),
)
def test_baseline_mismatch_names_the_found_interpreter(
    version_info: tuple[int, ...], expected: str | None, required: str | None
) -> None:
    """Only the baseline major and minor pass; a mismatch names both versions."""
    module = _load_verify_module()
    message = module.baseline_mismatch(version_info, "/opt/python/bin/python")
    if expected is None:
        assert message is None, f"{version_info!r} is the baseline, got {message!r}"
    else:
        assert message is not None, f"{version_info!r} must be rejected"
        assert expected in message, message
        assert required is not None, "mismatch cases must name the required version"
        assert required in message, message
        assert "/opt/python/bin/python" in message, (
            "the message must name the interpreter so the PATH culprit is visible"
        )


def _main_verifier_cases() -> list[tuple[tuple[int, ...], int]]:
    """Return baseline-derived interpreter cases for the command boundary."""
    module = _load_verify_module()
    major, minor = module.BASELINE
    baseline = (major, minor, 2)
    runner_system = (major, minor - 2, 13)
    return [
        (baseline, 0),
        (runner_system, 1),
    ]


@pytest.mark.parametrize(
    ("version_info", "status"),
    _main_verifier_cases(),
    ids=("baseline", "runner-system-python"),
)
def test_verify_main_reports_the_interpreter_boundary(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    version_info: tuple[int, ...],
    status: int,
) -> None:
    """Report success or a named mismatch through the verifier's public command."""
    module = _load_verify_module()
    executable = "/opt/python/bin/python"
    version = ".".join(str(part) for part in version_info)
    monkeypatch.setattr(
        module,
        "sys",
        types.SimpleNamespace(
            executable=executable,
            stderr=sys.stderr,
            version=version,
            version_info=version_info,
        ),
    )

    assert module.main([VERIFY_COMMAND]) == status, "the verifier status is contractual"
    captured = capsys.readouterr()
    if status == 0:
        assert captured.out == f"python {version} at {executable}\n", (
            "a baseline interpreter must report its resolved executable"
        )
        assert not captured.err, "a baseline interpreter must not report a mismatch"
    else:
        required = ".".join(str(part) for part in module.BASELINE)
        assert not captured.out, "a mismatched interpreter must not report success"
        assert f"need Python {required}" in captured.err, (
            "a mismatch must name the required baseline"
        )
        assert f"found {version}" in captured.err, (
            "a mismatch must name the detected interpreter version"
        )
        assert executable in captured.err, "a mismatch must name the executable"


def test_verify_script_exits_with_a_named_mismatch(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    """Exercise the ``__main__`` path with a controlled wrong interpreter."""
    module = _load_verify_module()
    major, minor = module.BASELINE
    version_info = (major, minor - 2, 13)
    version = ".".join(str(part) for part in version_info)
    executable = "/opt/runner-python/bin/python"
    fake_sys = types.SimpleNamespace(
        argv=[str(REPO_ROOT / VERIFY_MODULE), VERIFY_COMMAND],
        executable=executable,
        stderr=sys.stderr,
        version=version,
        version_info=version_info,
    )
    monkeypatch.setitem(sys.modules, "sys", fake_sys)
    monkeypatch.setattr(argparse, "sys", fake_sys)

    with pytest.raises(SystemExit) as exited:
        runpy.run_path(str(REPO_ROOT / VERIFY_MODULE), run_name="__main__")

    assert exited.value.code == 1, "the script must reject a non-baseline interpreter"
    captured = capsys.readouterr()
    assert not captured.out, "a mismatch must not report success"
    assert f"need Python {major}.{minor}" in captured.err, (
        "the script mismatch must name the required baseline"
    )
    assert f"found {version}" in captured.err, (
        "the script mismatch must name the detected version"
    )
    assert executable in captured.err, "the script mismatch must name the executable"


def test_verify_script_runs_successfully_under_the_test_interpreter() -> None:
    """Run the checked-in ``__main__`` path and report the active interpreter."""
    result = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed interpreter and checked-in script.
        [sys.executable, str(REPO_ROOT / VERIFY_MODULE), VERIFY_COMMAND],
        capture_output=True,
        check=False,
        shell=False,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    assert result.stdout == f"python {sys.version.split()[0]} at {sys.executable}\n", (
        "the script must report the subprocess interpreter"
    )
    assert not result.stderr, "the baseline verifier must not emit an error"

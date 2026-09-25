"""Execute the coverage lane's staging and validation path end to end.

``coverage-main.yml`` stages the generated ``lcov.info`` into a directory of
its own and runs ``scripts/validate_coverage_artifact.py`` over it before the
CodeScene upload sees it. The suites that read that step
(``codescene_report_validation_invariants``, ``codescene_validation_step_test``,
``codescene_upload_contract_test``) all treat it as text: they assert that the
step *names* the validator, *creates* a directory, and *copies* the report into
it. None of them runs it, and a script satisfying every one of those clauses
can still fail on a runner — a ``sed`` expression matching nothing, a baseline
passed to the wrong flag, a path absent from the job's working directory —
with every structural check still green.

These cases close that gap by running the lane's own script, read from the
workflow rather than restated here, inside a workspace holding the three things
it reads: the real ``Makefile``, the real validator, and one ``lcov.info``.
Nothing below re-implements the step, so its ``sed`` expression, flag spelling,
and argument list are each held to account as written.

The toolchain resolver is stubbed, following ``release_glibc_floor_test``: the
step's subject is its own script, not ``uv``'s interpreter download. The stub
records the baseline the step extracted and checks the flag shape it was handed,
so stubbing removes the download without removing the assertion.

What "reaches the upload" means here is modelled rather than observed. GitHub
Actions runs a step only while its job is succeeding — a step-level ``if``
carries an implicit ``success()``, and the upload step declares no status
function — so the boundary is exactly "the job got past this step". The
sentinel below records that boundary; it is not an upload, and no credential or
network is involved.

Run via ``make test-workflow-contracts``.
"""

import re
import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the step's own script is under test.
import sys
import typing as typ

import pytest
from codescene_report_validation_invariants import REPORT_VALIDATION_STEP
from workflow_loading import (
    COVERAGE_MAIN_WORKFLOW_PATH,
    REPO_ROOT,
    job_steps,
    load_workflow,
    named_step,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

#: The job in ``coverage-main.yml`` that owns the staging and upload steps.
COVERAGE_JOB = "coverage-upload"

#: The validator the step invokes, by the path the step spells.
VALIDATOR = REPO_ROOT / "scripts" / "validate_coverage_artifact.py"

#: The Makefile declaration the step reads its interpreter from. The same
#: reader as ``release_workflow_hoist_test``, so the one source stays one.
BASELINE_PATTERN = r"^PYTHON_BASELINE \?= (\S+)$"

#: Where the stub writes the baseline the step extracted for it.
BASELINE_LOG = "uv-baseline.txt"

#: The file the sentinel is written to, when the step returns zero.
UPLOAD_SENTINEL = "upload-reached"

#: A report satisfying every record the validator requires.
VALID_LCOV = "TN:\nSF:src/lib.rs\nDA:1,1\nLF:1\nLH:1\nend_of_record\n"

#: Stands in for the step's own script, and is run as a shell script.
STEP_SCRIPT = "step.sh"

UV_STUB = """\
#!/bin/sh
# Stands in for `uv run --no-project --python <baseline>`, which fetches the
# baseline interpreter on demand. The subject here is the step's script, so the
# prefix is checked as spelled and the remainder is handed to the test's own
# interpreter. Nothing downloads, so nothing needs the network.
set -eu
test "$1" = run
test "$2" = --no-project
test "$3" = --python
printf '%s' "$4" > "$UV_BASELINE_LOG"
shift 4
exec "$UV_PYTHON" "$@"
"""

#: Runs the step, then records whether the job would have carried on.
#:
#: GitHub Actions runs a step only while the job is succeeding, so the upload
#: that follows the validation step is reached exactly when it returned zero.
#: The step is run as a child process because its own `set -euo pipefail` is
#: scoped to itself rather than to this harness.
HARNESS = """\
bash step.sh
status=$?
if [ "$status" -eq 0 ]; then
  : > upload-reached
fi
exit "$status"
"""


def _step_script() -> str:
    """Return the lane's staging and validation script, as the workflow writes it."""
    steps = job_steps(load_workflow(COVERAGE_MAIN_WORKFLOW_PATH), COVERAGE_JOB)
    return str(named_step(steps, REPORT_VALIDATION_STEP).get("run", ""))


def _makefile_baseline() -> str:
    """Return the interpreter the Makefile pins, which the step must read."""
    text = (REPO_ROOT / "Makefile").read_text(encoding="utf-8")
    match = re.search(BASELINE_PATTERN, text, flags=re.MULTILINE)
    assert match is not None, "the Makefile must declare PYTHON_BASELINE ?="
    return match.group(1)


def _stage_workspace(tmp_path: Path, report: str) -> Path:
    """Lay out the files the step reads, with ``report`` as the generated one."""
    workspace = tmp_path / "workspace"
    (workspace / "scripts").mkdir(parents=True)
    shutil.copy2(REPO_ROOT / "Makefile", workspace / "Makefile")
    shutil.copy2(VALIDATOR, workspace / "scripts" / VALIDATOR.name)
    (workspace / "lcov.info").write_text(report, encoding="utf-8")
    (workspace / STEP_SCRIPT).write_text(_step_script(), encoding="utf-8")
    return workspace


def _run_step(tmp_path: Path, workspace: Path) -> subprocess.CompletedProcess[str]:
    """Run the step's script in ``workspace`` and report what it did."""
    stubs = workspace / "bin"
    stubs.mkdir()
    uv = stubs / "uv"
    uv.write_text(UV_STUB, encoding="utf-8")
    uv.chmod(0o755)
    scratch = tmp_path / "scratch"
    scratch.mkdir()
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - the harness is the bash script above.
        ["bash", "-c", HARNESS],  # ruff: ignore[start-process-with-partial-path] - resolved from the fixed PATH.
        cwd=workspace,
        check=False,
        env={
            "PATH": f"{stubs}:/usr/bin:/bin",
            # Keeps the step's `mktemp --directory` inside the test's own
            # scratch space rather than leaving a directory in the system /tmp.
            "TMPDIR": str(scratch),
            "UV_BASELINE_LOG": str(tmp_path / BASELINE_LOG),
            "UV_PYTHON": sys.executable,
        },
        text=True,
        capture_output=True,
    )


def _reached_upload(workspace: Path) -> bool:
    """Whether the job would have carried on to the upload step."""
    return (workspace / UPLOAD_SENTINEL).exists()


def test_a_valid_report_reaches_the_upload_boundary(tmp_path: Path) -> None:
    """Carry a well-formed report through to the step that would submit it."""
    workspace = _stage_workspace(tmp_path, VALID_LCOV)
    result = _run_step(tmp_path, workspace)
    assert result.returncode == 0, (
        f"the staged report is valid, so the step must pass: {result.stderr!r}"
    )
    assert _reached_upload(workspace), (
        "a report the validator accepted must reach the upload step"
    )


def test_the_step_reads_the_baseline_the_makefile_pins(tmp_path: Path) -> None:
    """Hand the validator the one interpreter pin, read rather than copied.

    The step reads ``PYTHON_BASELINE`` out of the Makefile instead of spelling
    the version, so a pin bump changes one file. That holds only while the
    ``sed`` expression still matches the declaration; a reader that quietly
    extracted nothing would pass an empty `--python` on a runner.
    """
    workspace = _stage_workspace(tmp_path, VALID_LCOV)
    _run_step(tmp_path, workspace)
    recorded = (tmp_path / BASELINE_LOG).read_text(encoding="utf-8")
    assert recorded == _makefile_baseline(), (
        f"the step must pass the Makefile's pin to uv, got {recorded!r}"
    )


@pytest.mark.parametrize(
    ("report", "issue"),
    [
        pytest.param("", "EMPTY_REPORT", id="empty"),
        pytest.param("unexpected data\n", "INVALID_RECORD", id="malformed"),
    ],
)
def test_a_rejected_report_stops_before_the_upload(
    tmp_path: Path, report: str, issue: str
) -> None:
    """Stop at the validator, naming the fault, rather than at CodeScene.

    An empty or malformed report is what the step exists to catch: the upload
    asserts only that the file exists, so without this it reaches CodeScene,
    which refuses it hours later against a commit whose every step passed.
    """
    workspace = _stage_workspace(tmp_path, report)
    result = _run_step(tmp_path, workspace)
    assert result.returncode == 1, (
        f"a {issue} report is a controlled validation failure: {result.stderr!r}"
    )
    assert not _reached_upload(workspace), (
        f"a {issue} report must stop before the upload step"
    )
    assert "error:" in result.stderr, (
        f"the step must report the fault it found, got {result.stderr!r}"
    )

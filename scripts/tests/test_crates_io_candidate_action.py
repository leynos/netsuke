"""Execute the package action's candidate step against packaged artefacts.

``release_package_contract_test`` reads the step's shell text, and
``test_crates_io_candidate_cli`` calls the CLI in process. Neither shows the
two working together: this module runs the step body from
``.github/actions/package-crate/action.yml`` under Bash, from a workspace that
holds the record and archive the packaging step would have written, and checks
that an accepted candidate hands the record on while a rejected one fails the
step. The only substitution is the ``uv`` launcher, which names the test
interpreter instead of resolving the repository's Python baseline.

Run via ``make test-release-publication``.
"""

import copy
import dataclasses
import json
import os
import pathlib
import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the step runs as a child.
import sys

import crates_io_candidate_handoff as handoff
import pytest
from crates_io_candidate_support import (
    BIN_NAME,
    COMMIT,
    PACKAGE_NAME,
    TAG,
    VERSION,
    CandidateOverrides,
    write_candidate,
)

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
CANDIDATE_STEP = "Validate the packaged candidate offline"
OUTPUT_NAME = "crates-io-candidate.json"


def _action_steps() -> list[dict[str, object]]:
    """Return the package action's declared steps."""
    return handoff.load_action_steps()


def _candidate_script() -> str:
    """Return the candidate step's shell body from the package action."""
    matching = [step for step in _action_steps() if step.get("name") == CANDIDATE_STEP]
    assert len(matching) == 1, "the package action must declare one candidate step"
    run = matching[0]["run"]
    assert isinstance(run, str), "the candidate step must carry a script"
    return run


def _rehearsal_identity() -> dict[str, object]:
    """Return the identity a dry-run packaging step records."""
    return {
        "tag": None,
        "commit": COMMIT,
        "is_prerelease": False,
        "package_name": PACKAGE_NAME,
        "version": VERSION,
        "bin_name": BIN_NAME,
    }


def _workspace(
    tmp_path: pathlib.Path, *, overrides: CandidateOverrides | None = None
) -> pathlib.Path:
    """Build the workspace the packaging and candidate steps share."""
    workspace = tmp_path / "workspace"
    archive_directory = workspace / "target" / "package"
    record_directory = workspace / "artifacts" / "package"
    archive_directory.mkdir(parents=True)
    record_directory.mkdir(parents=True)
    record, _ = write_candidate(archive_directory, overrides=overrides)
    _ = shutil.copyfile(record, record_directory / "identity.json")
    # The step runs from the repository root, so its relative script path has
    # to resolve from the workspace instead.
    (workspace / "scripts").symlink_to(REPO_ROOT / "scripts", target_is_directory=True)
    return workspace


def _uv_stub(directory: pathlib.Path) -> None:
    """Write a ``uv`` that runs the step's script under the test interpreter."""
    stub = directory / "uv"
    _ = stub.write_text(
        "#!/usr/bin/env bash\n"
        "set -euo pipefail\n"
        "# The step launches `uv run --no-project --python <version> <script>`.\n"
        "# Dropping the launcher arguments leaves the command the action runs.\n"
        "shift 4\n"
        'exec "$STEP_PYTHON" "$@"\n',
        encoding="utf-8",
    )
    stub.chmod(0o755)


def _step_environment(
    tmp_path: pathlib.Path, *, dry_run: str, tag: str = TAG
) -> dict[str, str]:
    """Build the step environment the action would supply for one run."""
    stub_directory = tmp_path / "bin"
    stub_directory.mkdir()
    _uv_stub(stub_directory)
    runner_temp = tmp_path / "runner-temp"
    runner_temp.mkdir()
    github_output = tmp_path / "github-output"
    _ = github_output.write_text("", encoding="utf-8")
    environment = os.environ.copy()
    environment.update({
        "PATH": os.pathsep.join((str(stub_directory), environment.get("PATH", ""))),
        "STEP_PYTHON": sys.executable,
        "RUNNER_TEMP": str(runner_temp),
        "GITHUB_OUTPUT": str(github_output),
        "RELEASE_TAG": tag,
        "RELEASE_DRY_RUN": dry_run,
        "RELEASE_COMMIT": COMMIT,
        "RELEASE_PACKAGE_NAME": PACKAGE_NAME,
        "RELEASE_VERSION": VERSION,
    })
    return environment


def _run_candidate_step(
    workspace: pathlib.Path, environment: dict[str, str]
) -> subprocess.CompletedProcess[str]:
    """Run the action's candidate step body from the supplied workspace."""
    bash = shutil.which("bash")
    assert bash is not None, "the candidate step is a Bash script"
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed script; shell is disabled.
        [bash, "--noprofile", "--norc", "-c", _candidate_script()],
        capture_output=True,
        check=False,
        cwd=workspace,
        env=environment,
        text=True,
    )


def _candidate_record(tmp_path: pathlib.Path) -> dict[str, object]:
    """Return the record the step wrote to the runner's temporary directory."""
    output = tmp_path / "runner-temp" / OUTPUT_NAME
    assert output.is_file(), "an accepted candidate must hand on its record"
    loaded = json.loads(output.read_text(encoding="utf-8"))
    assert isinstance(loaded, dict), "the candidate record must be a mapping"
    return loaded


def test_candidate_step_validates_a_publishing_candidate(
    tmp_path: pathlib.Path,
) -> None:
    """Hand on the record when a publishing candidate matches its release tag."""
    workspace = _workspace(tmp_path)
    environment = _step_environment(tmp_path, dry_run="false")

    completed = _run_candidate_step(workspace, environment)

    assert completed.returncode == 0, (
        f"the candidate step must accept the packaged release: {completed.stderr}"
    )
    candidate = _candidate_record(tmp_path)
    assert candidate["crate"] == f"{PACKAGE_NAME}-{VERSION}.crate", (
        "record the archive the packaging step produced"
    )


def test_candidate_step_validates_a_rehearsal_candidate(
    tmp_path: pathlib.Path,
) -> None:
    """Hand on the record when a dry run checks a tagless candidate."""
    workspace = _workspace(
        tmp_path,
        overrides=CandidateOverrides(identity=_rehearsal_identity()),
    )
    environment = _step_environment(tmp_path, dry_run="true")

    completed = _run_candidate_step(workspace, environment)

    assert completed.returncode == 0, (
        f"the candidate step must accept a rehearsal candidate: {completed.stderr}"
    )
    identity = _candidate_record(tmp_path)["identity"]
    assert isinstance(identity, dict), "the record carries the validated identity"
    assert identity["tag"] is None, "a rehearsal record retains its absent tag"


def test_candidate_step_fails_the_hand_off_for_a_rejected_candidate(
    tmp_path: pathlib.Path,
) -> None:
    """Fail the step, and write no record, when the candidate is rejected."""
    # The fixture identity records a release tag, which a dry run must refuse.
    workspace = _workspace(tmp_path)
    environment = _step_environment(tmp_path, dry_run="true")

    completed = _run_candidate_step(workspace, environment)

    assert completed.returncode != 0, "a rejected candidate must fail the step"
    assert "must not record a release tag" in completed.stderr, (
        "report the rejection through the step's error stream"
    )
    assert not (tmp_path / "runner-temp" / OUTPUT_NAME).exists(), (
        "a rejected candidate must hand nothing on"
    )


def test_candidate_step_rejects_an_ambiguous_dry_run_input(
    tmp_path: pathlib.Path,
) -> None:
    """Fail the step when its dry-run input names neither release mode."""
    workspace = _workspace(tmp_path)
    environment = _step_environment(tmp_path, dry_run="maybe")

    completed = _run_candidate_step(workspace, environment)

    assert completed.returncode != 0, "an unusable dry-run input must fail the step"
    assert "dry-run must be true or false" in completed.stderr, (
        "name the input the step could not interpret"
    )


@dataclasses.dataclass(frozen=True, slots=True)
class JobFixture:
    """Collect the workspace identity and documents a case may substitute.

    Each is an independent replacement for one fixture input, and a case that
    needs none of them supplies nothing, so they travel together rather than as
    three more keyword arguments to :func:`_run_package_job`.
    """

    overrides: CandidateOverrides | None = None
    workflow: dict[str, object] | None = None
    action_steps: list[dict[str, object]] | None = None


def _run_package_job(
    tmp_path: pathlib.Path,
    *,
    dry_run: str,
    fixture: JobFixture | None = None,
) -> handoff.JobOutcome:
    """Run the package job's hand-off steps over the fixture workspace."""
    substituted = fixture if fixture is not None else JobFixture()
    context = handoff.JobContext(
        workspace=_workspace(tmp_path, overrides=substituted.overrides),
        environment=_step_environment(tmp_path, dry_run=dry_run),
    )
    runner = handoff.Runner(
        action_steps=tuple(
            substituted.action_steps
            if substituted.action_steps is not None
            else _action_steps()
        ),
        upload_root=tmp_path / "uploaded",
    )
    return handoff.run_package_job(
        context,
        runner,
        substituted.workflow
        if substituted.workflow is not None
        else handoff.load_workflow(),
    )


def test_accepted_candidate_hands_the_package_to_the_next_job(
    tmp_path: pathlib.Path,
) -> None:
    """Hand on the identity record and crate archive, not the temporary record."""
    outcome = _run_package_job(tmp_path, dry_run="false")

    assert not outcome.failed, "an accepted candidate must not fail the job"
    assert outcome.uploaded == ("identity.json", f"{PACKAGE_NAME}-{VERSION}.crate"), (
        "hand on the identity record and the crate archive"
    )
    assert OUTPUT_NAME not in outcome.uploaded, (
        "the temporary candidate record is not part of the hand-off"
    )


def test_rehearsal_candidate_hands_the_package_to_the_next_job(
    tmp_path: pathlib.Path,
) -> None:
    """Hand on a tagless rehearsal candidate, which has no release tag to check.

    A pull-request run has a merge ref rather than a tag, so the whole job —
    not only the candidate step — must accept the record its packaging step
    wrote and still pass the identity record and archive to the next job.
    """
    outcome = _run_package_job(
        tmp_path,
        dry_run="true",
        fixture=JobFixture(
            overrides=CandidateOverrides(identity=_rehearsal_identity())
        ),
    )

    assert not outcome.failed, "an accepted rehearsal must not fail the job"
    assert outcome.uploaded == ("identity.json", f"{PACKAGE_NAME}-{VERSION}.crate"), (
        "hand on the identity record and the crate archive"
    )


def test_rejected_candidate_stops_the_package_hand_off(
    tmp_path: pathlib.Path,
) -> None:
    """Hand nothing on when the candidate step rejects the packaged release."""
    # The fixture identity records a tag, which a dry run must refuse.
    outcome = _run_package_job(tmp_path, dry_run="true")

    assert outcome.failed, "a rejected candidate must fail the package job"
    assert not outcome.uploaded, "a rejected candidate must hand nothing on"
    assert not (tmp_path / "uploaded").exists(), "the upload step must not run"


@pytest.mark.parametrize(
    "mutation",
    [
        "upload-always",
        "package-continue-on-error",
        "candidate-continue-on-error",
    ],
)
def test_the_hand_off_harness_detects_a_failure_bypass(
    tmp_path: pathlib.Path, mutation: str
) -> None:
    """Show each failure-bypassing edit hands a rejected package on."""
    workflow = copy.deepcopy(handoff.load_workflow())
    action_steps = copy.deepcopy(_action_steps())
    handoff.mutate_hand_off(workflow, action_steps, mutation)

    outcome = _run_package_job(
        tmp_path,
        dry_run="true",
        fixture=JobFixture(workflow=workflow, action_steps=action_steps),
    )

    assert outcome.uploaded == ("identity.json", f"{PACKAGE_NAME}-{VERSION}.crate"), (
        "the harness must show the bypass handing a rejected package on"
    )

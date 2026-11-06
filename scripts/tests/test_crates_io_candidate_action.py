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

import json
import os
import pathlib
import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the step runs as a child.
import sys

import yaml
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
ACTION_MANIFEST = REPO_ROOT / ".github" / "actions" / "package-crate" / "action.yml"
CANDIDATE_STEP = "Validate the packaged candidate offline"
OUTPUT_NAME = "crates-io-candidate.json"


def _action_steps() -> list[dict[str, object]]:
    """Return the package action's declared steps."""
    loaded = yaml.safe_load(ACTION_MANIFEST.read_text(encoding="utf-8"))
    assert isinstance(loaded, dict), "the action manifest must be a mapping"
    runs = loaded["runs"]
    assert isinstance(runs, dict), "the action manifest must declare its runs"
    steps = runs["steps"]
    assert isinstance(steps, list), "the action runs must be a step list"
    assert all(isinstance(step, dict) for step in steps), (
        "each action step must be a mapping"
    )
    return steps


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
    environment = os.environ.copy()
    environment.update({
        "PATH": os.pathsep.join((str(stub_directory), environment.get("PATH", ""))),
        "STEP_PYTHON": sys.executable,
        "RUNNER_TEMP": str(runner_temp),
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

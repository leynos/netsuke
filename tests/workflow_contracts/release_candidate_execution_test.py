"""Run the two steps that decide whether a producer built the candidate.

The candidate travels as a string from ``release.yml``'s metadata job, through
every producer's ``commit`` input, to the record the recorder writes. Two
shell steps stand between that string and the bytes published under it: the
metadata job refuses a value that is not a full lowercase commit, and each
producer compares the checkout's own ``HEAD`` against it before building.

The structural cases in ``release_candidate_binding_test.py`` assert those
steps are wired and spell the right variables. They never run them, so a
comparison that dropped a character, inverted its arms, or read the wrong
variable would keep every structural check green. These cases execute both
bodies, read from the workflows rather than restated, over a real temporary
repository: valid values must pass and invalid ones must stop the job.

Run via ``make test-workflow-contracts``.
"""

import subprocess  # ruff: ignore[suspicious-subprocess-import] - the steps' own scripts are under test.
import typing as typ

import pytest
from workflow_loading import (
    PACKAGE_WORKFLOW_PATH,
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
)

if typ.TYPE_CHECKING:
    import pathlib

CANDIDATE_STEP = "Bind the release candidate commit"
CHECKOUT_STEP = "Verify the checkout is the release candidate"
METADATA_JOB = "metadata"
BUILD_JOB = "build"

#: The candidate the metadata step accepts: a full lowercase hexadecimal
#: commit. Written out rather than derived, because this step's subject is the
#: spelling rule and a generated value could only restate it.
VALID_CANDIDATE = "a" * 40

#: Values the metadata step must refuse, paired with how the case names the
#: refusal. One row per rule, so a broken fragment names itself.
INVALID_CANDIDATES = (
    pytest.param("A" * 40, id="uppercase"),
    pytest.param("g" * 40, id="non-hexadecimal"),
    pytest.param("a" * 39, id="short"),
    pytest.param("a" * 41, id="long"),
    pytest.param("", id="empty"),
    pytest.param("v0.1.0", id="tag"),
    pytest.param("main", id="branch"),
)

#: The author identity every temporary commit is made with, so the cases do
#: not depend on the reader's Git configuration.
_IDENTITY = (
    "-c",
    "user.name=Netsuke Contract",
    "-c",
    "user.email=netsuke@example.invalid",
)


def _environment(home: pathlib.Path) -> dict[str, str]:
    """Return a Git-clean environment for one executed step.

    The caller's Git configuration is dropped rather than inherited: a
    ``GIT_CONFIG_*`` injection or a commit-signing default would change what
    ``git init`` and ``git rev-parse`` do, and the subject here is the step's
    own logic rather than the reader's setup.

    Returns
    -------
    dict[str, str]
        The base environment every step inherits.
    """
    return {
        "PATH": "/usr/bin:/bin",
        "HOME": str(home),
        "XDG_CONFIG_HOME": str(home / ".config"),
        "GIT_CONFIG_GLOBAL": str(home / "gitconfig"),
        "GIT_CONFIG_SYSTEM": "/dev/null",
        "GIT_AUTHOR_NAME": "Netsuke Contract",
        "GIT_AUTHOR_EMAIL": "netsuke@example.invalid",
        "GIT_COMMITTER_NAME": "Netsuke Contract",
        "GIT_COMMITTER_EMAIL": "netsuke@example.invalid",
    }


def _run_script(
    workspace: pathlib.Path,
    home: pathlib.Path,
    body: str,
    environment: dict[str, str],
) -> subprocess.CompletedProcess[str]:
    """Run one step body in ``workspace`` and report what it did.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The completed step.
    """
    script = workspace / "step.sh"
    script.write_text(body, encoding="utf-8")
    output = workspace / "github-output"
    output.touch()
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - the step body is the shell script.
        ["bash", str(script)],  # ruff: ignore[start-process-with-partial-path] - resolved from the fixed PATH.
        cwd=workspace,
        check=False,
        env=_environment(home) | {"GITHUB_OUTPUT": str(output)} | environment,
        text=True,
        capture_output=True,
    )


def _step_body(workflow_path: pathlib.Path, job: str, name: str) -> str:
    """Return one step's shell body as the workflow writes it.

    Returns
    -------
    str
        The step's ``run`` block.
    """
    step = named_step(job_steps(load_workflow(workflow_path), job), name)
    return str(step.get("run", ""))


def _candidate_step(
    tmp_path: pathlib.Path, candidate: str
) -> subprocess.CompletedProcess[str]:
    """Execute the metadata job's candidate check over ``candidate``.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The completed step.
    """
    workspace = tmp_path / "metadata"
    workspace.mkdir()
    return _run_script(
        workspace,
        tmp_path,
        _step_body(RELEASE_WORKFLOW_PATH, METADATA_JOB, CANDIDATE_STEP),
        {"CANDIDATE_COMMIT": candidate},
    )


def _git(workspace: pathlib.Path, home: pathlib.Path, *args: str) -> str:
    """Run one Git command in ``workspace`` and return its standard output.

    Returns
    -------
    str
        The command's output, stripped of the trailing newline.
    """
    completed = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - a fixed argument list, never a shell string.
        ["git", *args],  # ruff: ignore[start-process-with-partial-path] - resolved from the fixed PATH.
        cwd=workspace,
        check=True,
        env=_environment(home),
        text=True,
        capture_output=True,
    )
    return completed.stdout.strip()


def _commit(workspace: pathlib.Path, home: pathlib.Path) -> str:
    """Create one commit in ``workspace`` and return its full SHA.

    Returns
    -------
    str
        The commit ``HEAD`` resolves to.
    """
    workspace.mkdir()
    _git(workspace, home, "init", "--quiet")
    _git(
        workspace,
        home,
        *_IDENTITY,
        "commit",
        "--quiet",
        "--allow-empty",
        "-m",
        "candidate",
    )
    return _git(workspace, home, "rev-parse", "HEAD")


def _checkout_step(
    tmp_path: pathlib.Path, workspace: pathlib.Path, candidate: str
) -> subprocess.CompletedProcess[str]:
    """Execute the producer's HEAD comparison in a real repository.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The completed step.
    """
    return _run_script(
        workspace,
        tmp_path,
        _step_body(PACKAGE_WORKFLOW_PATH, BUILD_JOB, CHECKOUT_STEP),
        {"CANDIDATE_COMMIT": candidate},
    )


def test_a_full_lowercase_commit_is_published(tmp_path: pathlib.Path) -> None:
    """Publish the candidate the metadata step accepted.

    The output is what every producer's ``commit`` input receives, so a step
    that validated the value and then published something else would bind the
    whole release to a commit nobody checked.
    """
    result = _candidate_step(tmp_path, VALID_CANDIDATE)
    assert result.returncode == 0, result.stderr
    written = (tmp_path / "metadata" / "github-output").read_text(encoding="utf-8")
    assert written == f"value={VALID_CANDIDATE}\n", written


@pytest.mark.parametrize("candidate", INVALID_CANDIDATES)
def test_a_candidate_that_is_not_a_full_commit_is_refused(
    tmp_path: pathlib.Path, candidate: str
) -> None:
    """Refuse every spelling that is not a full lowercase hexadecimal commit."""
    result = _candidate_step(tmp_path, candidate)
    assert result.returncode == 1, f"{candidate!r} must be refused: {result.stdout}"
    written = (tmp_path / "metadata" / "github-output").read_text(encoding="utf-8")
    assert "value=" not in written, "a refused candidate must not be published"


def test_the_checkout_of_the_candidate_passes(tmp_path: pathlib.Path) -> None:
    """Accept a producer whose checkout is the candidate it was told to build."""
    workspace = tmp_path / "producer"
    head = _commit(workspace, tmp_path)
    result = _checkout_step(tmp_path, workspace, head)
    assert result.returncode == 0, result.stderr


def test_a_checkout_of_another_commit_is_refused(tmp_path: pathlib.Path) -> None:
    """Refuse a producer that built something other than the candidate.

    This is the check that makes the rest of the provenance chain mean
    anything: the recorder would happily bind bytes built from a moved branch
    to the candidate's name, because the only thing it is told is the name.
    """
    workspace = tmp_path / "producer"
    head = _commit(workspace, tmp_path)
    assert head != VALID_CANDIDATE, "the fixture commit must differ from the case"
    result = _checkout_step(tmp_path, workspace, VALID_CANDIDATE)
    assert result.returncode == 1, result.stdout
    assert head in result.stderr, (
        f"the refusal must name the commit on disk, got {result.stderr!r}"
    )
    assert VALID_CANDIDATE in result.stderr, (
        f"the refusal must name the candidate, got {result.stderr!r}"
    )


def test_an_unreadable_checkout_is_refused(tmp_path: pathlib.Path) -> None:
    """Stop a producer whose tree is not a checkout at all.

    ``git rev-parse HEAD`` fails outside a repository, and the step runs under
    ``set -euo pipefail``. Without that, an empty reading would be compared
    against the candidate and reported as a mere mismatch, which reads as a
    moved branch rather than as a tree that was never checked out.
    """
    workspace = tmp_path / "producer"
    workspace.mkdir()
    result = _checkout_step(tmp_path, workspace, VALID_CANDIDATE)
    assert result.returncode != 0, result.stdout
    assert "checkout is" not in result.stderr, (
        f"an unreadable checkout is not a mismatched one, got {result.stderr!r}"
    )


def test_the_checkout_step_reads_the_candidate_from_the_environment() -> None:
    """Take the candidate from the step's ``env``, never from the script text.

    A step that inlined an expression would receive literal ``${{ ... }}`` on
    a runner and compare HEAD against it. The structural cases assert the
    ``env`` block; this one asserts the body reads no other variable.
    """
    body = _step_body(PACKAGE_WORKFLOW_PATH, BUILD_JOB, CHECKOUT_STEP)
    assert "CANDIDATE_COMMIT" in body, body
    assert "${{" not in body, f"a run block must not carry an expression: {body}"

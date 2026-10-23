"""Behavioural tests for ``scripts/kani_proof_scope.py``.

The script decides whether `kani-smoke` runs its proofs, so each way it can
be wrong is tested: a non-pull-request event that skips, a pull request whose
change set cannot be read and skips, a path matched by prefix rather than by
directory, and a scope file whose damage turns into a decision instead of an
error. The git half runs against real repositories built in a temporary
directory, and the command-line half runs the script as a child process with
its environment passed to that child alone.

Run via ``make test-workflow-contracts``.
"""

import dataclasses
import json
import os
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the tests drive git and the script as children.
import sys
import typing as typ

import pytest
from workflow_loading import REPO_ROOT

sys.path.insert(0, str(REPO_ROOT / "scripts"))

# The module under test lives in scripts/, outside any package, so the import
# cannot precede the sys.path insertion above.
import kani_proof_scope as scope_mod

if typ.TYPE_CHECKING:
    from pathlib import Path

SCRIPT = REPO_ROOT / "scripts" / "kani_proof_scope.py"
SCOPE = ("src/ir/", "Cargo.toml")
VALID_SCOPE = '[scope]\nsources = ["src/ir/"]\ninfrastructure = ["Cargo.toml"]\n'
#: A child environment for git that ignores the developer's own configuration,
#: so a signing hook or a default branch name cannot change what is built.
GIT_ENVIRONMENT = {
    "PATH": os.environ.get("PATH", ""),
    "HOME": "/nonexistent",
    "GIT_CONFIG_GLOBAL": os.devnull,
    "GIT_CONFIG_NOSYSTEM": "1",
    "GIT_AUTHOR_NAME": "Test",
    "GIT_AUTHOR_EMAIL": "test@example.invalid",
    "GIT_COMMITTER_NAME": "Test",
    "GIT_COMMITTER_EMAIL": "test@example.invalid",
}


@dataclasses.dataclass(frozen=True, slots=True)
class ScriptOptions:
    """Group the optional event and metrics inputs passed to the script."""

    event_name: str = "pull_request"
    metrics_file: Path | None = None


def _git(repository: Path, *arguments: str) -> None:
    """Run one git command in ``repository`` and require it to succeed."""
    # The argv is fixed by the test; no untrusted input reaches the child.
    subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - shell is False.
        ["git", *arguments],  # ruff: ignore[start-process-with-partial-path] - git from the test PATH.
        cwd=repository,
        env=GIT_ENVIRONMENT,
        check=True,
        capture_output=True,
    )


def _commit(repository: Path, path: str, message: str) -> None:
    """Write ``path`` and commit it."""
    target = repository / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(message, encoding="utf-8")
    _git(repository, "add", path)
    _git(repository, "commit", "--quiet", "--message", message)


def _merge_commit_repository(tmp_path: Path, changed: str) -> Path:
    """Return a repository whose HEAD merges a branch that changed ``changed``."""
    # The shape `actions/checkout` produces for a pull request: HEAD is the
    # merge commit and its first parent is the base branch tip.
    repository = tmp_path / "repository"
    repository.mkdir()
    _git(repository, "init", "--quiet", "--initial-branch=main")
    _commit(repository, "README.md", "base")
    _git(repository, "switch", "--quiet", "--create", "topic")
    _commit(repository, changed, "topic")
    _git(repository, "switch", "--quiet", "main")
    _commit(repository, "CHANGELOG.md", "main moved on")
    _git(repository, "merge", "--quiet", "--no-ff", "--no-edit", "topic")
    return repository


@pytest.mark.parametrize("event_name", ["push", "schedule", "workflow_dispatch"])
def test_every_other_event_runs_the_proofs(event_name: str) -> None:
    """Run in full on every event but a pull request, whatever changed."""
    decision = scope_mod.decide(event_name, SCOPE, [])
    assert decision.run_proofs, event_name
    assert decision.category is scope_mod.DecisionCategory.FULL_RUN, event_name
    assert scope_mod.decide(event_name, SCOPE, None).category is (
        scope_mod.DecisionCategory.FULL_RUN
    ), f"unreadable changes must not change the {event_name} category"


def test_unreadable_change_set_runs_the_proofs() -> None:
    """Fail closed when the pull request's change set could not be read."""
    decision = scope_mod.decide("pull_request", SCOPE, None)
    assert decision.run_proofs, "an unreadable change set must run the proofs"
    assert decision.category is scope_mod.DecisionCategory.UNREADABLE_DIFF, (
        "unreadable changes must have their own fixed category"
    )


@pytest.mark.parametrize(
    ("changes", "expected"),
    [
        (["src/ir/graph.rs"], True),
        (["docs/users-guide.md", "Cargo.toml"], True),
        (["src/irony.rs", "Cargo.toml.orig"], False),
        (["docs/users-guide.md"], False),
        ([], False),
    ],
)
def test_pull_request_runs_only_when_a_proof_input_changes(
    changes: list[str], *, expected: bool
) -> None:
    """Match directories by whole segment and files exactly."""
    decision = scope_mod.decide("pull_request", SCOPE, changes)
    assert decision.run_proofs is expected, decision.reason
    expected_category = (
        scope_mod.DecisionCategory.IN_SCOPE
        if expected
        else scope_mod.DecisionCategory.OUT_OF_SCOPE
    )
    assert decision.category is expected_category, decision.reason
    assert ("none of them" in decision.reason) is not expected, decision.reason


def test_matched_paths_are_named_and_elided() -> None:
    """Name the first matched paths and count the rest."""
    changes = [f"src/ir/file_{index}.rs" for index in range(12)]
    reason = scope_mod.decide("pull_request", SCOPE, changes).reason
    assert "`src/ir/file_0.rs`" in reason, reason
    assert "`src/ir/file_10.rs`" not in reason, reason
    assert "and 2 more" in reason, reason


@pytest.mark.parametrize(
    "text",
    [
        "not toml [",
        "[other]\n",
        '[scope]\nsources = ["src/ir/"]\n',
        '[scope]\nsources = []\ninfrastructure = ["Cargo.toml"]\n',
        '[scope]\nsources = ["src/ir/", 3]\ninfrastructure = ["Cargo.toml"]\n',
        '[scope]\nsources = ["src/ir/", ""]\ninfrastructure = ["Cargo.toml"]\n',
        '[scope]\nsources = "src/ir/"\ninfrastructure = ["Cargo.toml"]\n',
        "scope = 3\n",
    ],
)
def test_damaged_scope_is_an_error(tmp_path: Path, text: str) -> None:
    """Refuse a scope file rather than decide from a damaged one."""
    scope_file = tmp_path / "proof-scope.toml"
    scope_file.write_text(text, encoding="utf-8")
    with pytest.raises(scope_mod.ScopeFileError):
        scope_mod.read_scope(scope_file)


def test_missing_scope_is_an_error(tmp_path: Path) -> None:
    """Refuse a scope file that does not exist."""
    with pytest.raises(scope_mod.ScopeFileError):
        scope_mod.read_scope(tmp_path / "absent.toml")


def test_undecodable_scope_is_an_error_with_its_decode_cause(tmp_path: Path) -> None:
    """Keep the decoding failure as the cause of an invalid scope decision."""
    scope_file = tmp_path / "proof-scope.toml"
    scope_file.write_bytes(b"[scope]\nsources = [\xff]\n")
    with pytest.raises(scope_mod.ScopeFileError) as raised:
        scope_mod.read_scope(scope_file)
    assert isinstance(raised.value.__cause__, UnicodeDecodeError), (
        "the decoding failure must remain available as the cause"
    )


def test_checked_in_scope_reads() -> None:
    """Read the repository's own scope, sources first."""
    entries = scope_mod.read_scope(scope_mod.DEFAULT_SCOPE_FILE)
    assert "src/ir/" in entries, entries
    assert entries.index("src/ir/") < entries.index("Cargo.toml"), entries


def test_merge_commit_change_set_is_the_pull_request_diff(tmp_path: Path) -> None:
    """Read the merge commit's diff against the base tip, not the whole branch."""
    repository = _merge_commit_repository(tmp_path, "src/ir/graph.rs")
    changes = scope_mod.pull_request_changes(repository)
    assert changes == ["src/ir/graph.rs"], changes


def test_rename_reports_both_paths(tmp_path: Path) -> None:
    """Report a file moved out of the scope under its old path as well."""
    repository = _merge_commit_repository(tmp_path, "src/ir/graph.rs")
    _git(repository, "switch", "--quiet", "--create", "rename", "HEAD")
    (repository / "docs").mkdir()
    _git(repository, "mv", "src/ir/graph.rs", "docs/graph.rs")
    _git(repository, "commit", "--quiet", "--message", "move")
    _git(repository, "switch", "--quiet", "--detach", "main")
    _git(repository, "merge", "--quiet", "--no-ff", "--no-edit", "rename")
    changes = sorted(scope_mod.pull_request_changes(repository) or [])
    assert changes == ["docs/graph.rs", "src/ir/graph.rs"], changes


def test_single_parent_head_is_unreadable(tmp_path: Path) -> None:
    """Treat a HEAD that is not a merge commit as an unreadable change set."""
    repository = _merge_commit_repository(tmp_path, "src/ir/graph.rs")
    _commit(repository, "docs/after.md", "a plain commit on top")
    assert scope_mod.pull_request_changes(repository) is None, "HEAD is not a merge"


def test_git_failure_is_unreadable(tmp_path: Path) -> None:
    """Treat a failing git command, here in a repository with no HEAD, as unreadable."""
    _git(tmp_path, "init", "--quiet")
    assert scope_mod.pull_request_changes(tmp_path) is None, "git must have failed"


def _run_script(
    tmp_path: Path,
    repository: Path,
    scope_text: str,
    options: ScriptOptions | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run the script as the workflow does, with inputs in the child's environment."""
    script_options = options or ScriptOptions()
    scope_file = tmp_path / "proof-scope.toml"
    scope_file.write_text(scope_text, encoding="utf-8")
    environment = {
        **GIT_ENVIRONMENT,
        "INPUT_EVENT_NAME": script_options.event_name,
        "INPUT_REPOSITORY": str(repository),
        "INPUT_SCOPE_FILE": str(scope_file),
        "GITHUB_OUTPUT": str(tmp_path / "output"),
        "GITHUB_STEP_SUMMARY": str(tmp_path / "summary.md"),
        "INPUT_METRICS_FILE": str(
            script_options.metrics_file or tmp_path / "decision.json"
        ),
    }
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed argv, no shell.
        [sys.executable, str(SCRIPT)],
        env=environment,
        capture_output=True,
        text=True,
        check=False,
    )


@pytest.mark.parametrize(
    ("changed", "expected"), [("src/ir/graph.rs", "true"), ("docs/guide.md", "false")]
)
def test_script_publishes_the_decision(
    tmp_path: Path, changed: str, expected: str
) -> None:
    """Write the step output and a summary from the workflow's environment."""
    repository = _merge_commit_repository(tmp_path, changed)
    completed = _run_script(tmp_path, repository, VALID_SCOPE)
    assert completed.returncode == 0, completed.stderr
    output = (tmp_path / "output").read_text(encoding="utf-8")
    assert output == f"run-proofs={expected}\n", output
    heading = "Kani proofs run" if expected == "true" else "Kani proofs skipped"
    summary = (tmp_path / "summary.md").read_text(encoding="utf-8")
    assert f"### {heading}" in summary, summary
    assert f"::notice title={heading}::" in completed.stdout, completed.stdout
    expected_category = "in_scope" if expected == "true" else "out_of_scope"
    record = json.loads((tmp_path / "decision.json").read_text(encoding="utf-8"))
    assert record == {"decision": expected_category}, record


def test_script_records_an_unreadable_pull_request_diff(tmp_path: Path) -> None:
    """Record the fail-closed decision when HEAD is not a merge commit."""
    repository = tmp_path / "single-commit"
    repository.mkdir()
    _git(repository, "init", "--quiet", "--initial-branch=main")
    _commit(repository, "docs/guide.md", "guide")
    completed = _run_script(tmp_path, repository, VALID_SCOPE)
    assert completed.returncode == 0, completed.stderr
    assert (tmp_path / "output").read_text(encoding="utf-8") == "run-proofs=true\n", (
        "an unreadable diff must fail closed and run the proofs"
    )
    record = json.loads((tmp_path / "decision.json").read_text(encoding="utf-8"))
    assert record == {"decision": "unreadable_diff"}, record


@pytest.mark.parametrize("event_name", ["push", "schedule", "workflow_dispatch"])
def test_script_runs_every_other_event_in_full(tmp_path: Path, event_name: str) -> None:
    """Publish a full run for every event but a pull request, whatever changed.

    The merge commit changes only a path outside the scope, which a pull
    request would skip, so a run here comes from the event alone.
    """
    repository = _merge_commit_repository(tmp_path, "docs/guide.md")
    completed = _run_script(
        tmp_path, repository, VALID_SCOPE, ScriptOptions(event_name=event_name)
    )
    assert completed.returncode == 0, completed.stderr
    output = (tmp_path / "output").read_text(encoding="utf-8")
    assert output == "run-proofs=true\n", output
    summary = (tmp_path / "summary.md").read_text(encoding="utf-8")
    assert "### Kani proofs run" in summary, summary
    assert f"`{event_name}`" in summary, summary
    record = json.loads((tmp_path / "decision.json").read_text(encoding="utf-8"))
    assert record == {"decision": "full_run"}, record


def test_script_fails_on_a_damaged_scope(tmp_path: Path) -> None:
    """Fail without step outputs while recording `invalid_scope`."""
    repository = _merge_commit_repository(tmp_path, "docs/guide.md")
    completed = _run_script(tmp_path, repository, "[scope]\n")
    assert completed.returncode != 0, "a damaged scope must fail the step"
    assert not (tmp_path / "output").exists(), "a failed step must decide nothing"
    record = json.loads((tmp_path / "decision.json").read_text(encoding="utf-8"))
    assert record == {"decision": "invalid_scope"}, record


def test_metrics_record_failure_warns_without_changing_the_decision(
    tmp_path: Path,
) -> None:
    """Keep a valid decision when its separate metrics record is unwritable."""
    repository = _merge_commit_repository(tmp_path, "src/ir/graph.rs")
    metrics_parent = tmp_path / "not-a-directory"
    metrics_parent.write_text("file", encoding="utf-8")
    completed = _run_script(
        tmp_path,
        repository,
        VALID_SCOPE,
        ScriptOptions(metrics_file=metrics_parent / "decision.json"),
    )
    assert completed.returncode == 0, completed.stderr
    assert (tmp_path / "output").read_text(encoding="utf-8") == "run-proofs=true\n", (
        "a metrics write failure must not change the proof decision"
    )
    assert completed.stderr == (
        "::warning::Kani scope decision metrics could not be recorded.\n"
    ), "a metrics write failure must produce one fixed warning"


def test_metrics_record_failure_does_not_mask_an_invalid_scope(
    tmp_path: Path,
) -> None:
    """Retain the scope error and failed step if its record cannot be written."""
    repository = _merge_commit_repository(tmp_path, "docs/guide.md")
    metrics_parent = tmp_path / "not-a-directory"
    metrics_parent.write_text("file", encoding="utf-8")
    completed = _run_script(
        tmp_path,
        repository,
        "[scope]\n",
        ScriptOptions(metrics_file=metrics_parent / "decision.json"),
    )
    assert completed.returncode != 0, "invalid scope must keep failing the step"
    assert not (tmp_path / "output").exists(), "invalid scope must decide nothing"
    assert (
        completed.stderr.count(
            "::warning::Kani scope decision metrics could not be recorded."
        )
        == 1
    ), "an invalid scope with a failed record must produce one fixed warning"
    assert "ScopeFileError" in completed.stderr, completed.stderr

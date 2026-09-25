"""Verify the release-candidate resolver against real temporary repositories.

The resolver decides which Netsuke commit the downstream canaries build, so
each selection rule is exercised against a Git repository with real tags and
a real ``refs/remotes/origin/main``, not a mocked Git adapter.
"""

import importlib.util
import os
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the tests build real Git repositories.
import sys
from pathlib import Path

import pytest

SCRIPT = (
    Path(__file__).resolve().parents[2]
    / ".github"
    / "scripts"
    / "resolve_release_candidate.py"
)
_SPEC = importlib.util.spec_from_file_location("resolve_release_candidate", SCRIPT)
if _SPEC is None or _SPEC.loader is None:
    message = f"cannot load {SCRIPT}"
    raise ImportError(message)
resolver = importlib.util.module_from_spec(_SPEC)
sys.modules[_SPEC.name] = resolver
_SPEC.loader.exec_module(resolver)

#: A Git identity and environment independent of the developer's configuration.
GIT_ENVIRONMENT = {
    "GIT_AUTHOR_NAME": "Canary Test",
    "GIT_AUTHOR_EMAIL": "canary@example.invalid",
    "GIT_COMMITTER_NAME": "Canary Test",
    "GIT_COMMITTER_EMAIL": "canary@example.invalid",
    "GIT_CONFIG_GLOBAL": os.devnull,
    "GIT_CONFIG_NOSYSTEM": "1",
    "PATH": os.environ.get("PATH", ""),
}


def git(repository: Path, *arguments: str) -> str:
    """Run Git in ``repository`` with an isolated configuration.

    Parameters
    ----------
    repository
        The working tree to run in.
    *arguments
        Git arguments, excluding the program name.

    Returns
    -------
    str
        Git's standard output, stripped.
    """
    completed = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed program, list arguments.
        ["git", "-C", str(repository), *arguments],  # ruff: ignore[start-process-with-partial-path] - Git on PATH.
        check=True,
        capture_output=True,
        text=True,
        env=GIT_ENVIRONMENT,
    )
    return completed.stdout.strip()


def commit_version(repository: Path, version: str) -> str:
    """Commit a manifest declaring ``version`` and return the new commit.

    Parameters
    ----------
    repository
        The working tree to commit in.
    version
        The package version to declare.

    Returns
    -------
    str
        The full identifier of the new commit.
    """
    (repository / "Cargo.toml").write_text(
        f'[package]\nname = "netsuke"\nversion = "{version}"\n', encoding="utf-8"
    )
    git(repository, "add", "Cargo.toml")
    git(repository, "commit", "--quiet", "-m", f"Release {version}")
    return git(repository, "rev-parse", "HEAD")


@pytest.fixture
def repository(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """Create a repository with a beta tag and a ``main`` tracking ref.

    The resolver runs Git in the working directory, so the test changes into
    the repository; ``monkeypatch.chdir`` restores the directory afterwards.

    Returns
    -------
    Path
        The repository's working tree.
    """
    git(tmp_path, "init", "--quiet", "--initial-branch=main")
    beta = commit_version(tmp_path, "0.1.0-beta3")
    git(tmp_path, "tag", "v0.1.0-beta3", beta)
    git(tmp_path, "tag", "vTEST", beta)
    git(tmp_path, "update-ref", resolver.MAIN_REF, beta)
    monkeypatch.chdir(tmp_path)
    return tmp_path


def run_git_in_repository(repository: Path):  # ruff: ignore[missing-return-type-undocumented-public-function] - returns the adapter.
    """Return a Git adapter bound to ``repository``."""
    return lambda arguments: git(repository, *arguments) + "\n"


@pytest.mark.parametrize(
    ("lower", "higher"),
    [
        ("v0.1.0-beta3", "v0.1.0-rc1"),
        ("v0.1.0-rc1", "v0.1.0-rc2"),
        ("v0.1.0-rc.2", "v0.1.0-rc.10"),
        ("v0.1.0-beta10", "v0.1.0-beta3"),
        ("v0.1.0-rc2", "v0.1.0"),
        ("v0.1.0", "v0.1.1-rc1"),
        ("v0.9.0", "v0.10.0"),
    ],
)
def test_version_key_follows_semver_precedence(lower: str, higher: str) -> None:
    """Pre-releases order by SemVer 2.0 precedence, not by string or tag date."""
    assert resolver.version_key(lower) < resolver.version_key(higher), (
        f"{lower} should precede {higher}"
    )


@pytest.mark.parametrize("tag", ["vTEST", "v0.1", "0.1.0", "v01.0.0", "v0.1.0+build"])
def test_version_key_ignores_non_version_tags(tag: str) -> None:
    """Tags outside the ``vMAJOR.MINOR.PATCH[-PRE]`` shape are not candidates."""
    assert resolver.version_key(tag) is None, f"{tag} is not a version tag"


@pytest.mark.parametrize(
    ("tags", "expected"),
    [
        (["v0.1.0-beta3", "v0.1.0-rc1"], ("refs/tags/v0.1.0-rc1", "rc-tag")),
        (["v0.1.0-rc1", "v0.1.0-rc2", "vTEST"], ("refs/tags/v0.1.0-rc2", "rc-tag")),
        (["v0.1.0-rc1", "v0.1.0"], (resolver.MAIN_REF, "main")),
        (["v0.1.0-beta3"], (resolver.MAIN_REF, "main")),
        ([], (resolver.MAIN_REF, "main")),
    ],
)
def test_auto_prefers_only_the_newest_release_candidate(
    tags: list[str], expected: tuple[str, str]
) -> None:
    """``auto`` picks an ``-rcN`` tag only while no later version tag exists."""
    assert resolver.select_auto_ref(tags) == expected, f"auto rule for {tags}"


def test_default_request_resolves_the_workflow_commit(repository: Path) -> None:
    """An empty request builds the commit the workflow itself runs on."""
    head = git(repository, "rev-parse", "HEAD")

    candidate = resolver.resolve("", head, run_git_in_repository(repository))

    assert candidate == resolver.Candidate(head, "0.1.0-beta3", "workflow"), (
        "unexpected candidate"
    )


def test_auto_resolves_a_newer_release_candidate_tag(repository: Path) -> None:
    """``auto`` resolves the tag's commit and reports that commit's version."""
    rc = commit_version(repository, "0.1.0-rc1")
    git(repository, "tag", "-a", "-m", "rc1", "v0.1.0-rc1", rc)
    commit_version(repository, "0.1.0-rc2-dev")

    candidate = resolver.resolve("auto", "0" * 40, run_git_in_repository(repository))

    assert candidate == resolver.Candidate(rc, "0.1.0-rc1", "rc-tag"), (
        "unexpected candidate"
    )


def test_auto_falls_back_to_main_without_a_release_candidate(
    repository: Path,
) -> None:
    """With only a beta tag, ``auto`` builds the ``main`` tracking ref."""
    main = commit_version(repository, "0.1.0-dev")
    git(repository, "update-ref", resolver.MAIN_REF, main)

    candidate = resolver.resolve("auto", "0" * 40, run_git_in_repository(repository))

    assert candidate == resolver.Candidate(main, "0.1.0-dev", "main"), (
        "unexpected candidate"
    )


def test_explicit_ref_is_resolved_to_its_commit(repository: Path) -> None:
    """A named ref, such as a tag, is peeled to the commit it identifies."""
    beta = git(repository, "rev-parse", "v0.1.0-beta3^{commit}")

    candidate = resolver.resolve(
        "v0.1.0-beta3", "0" * 40, run_git_in_repository(repository)
    )

    assert candidate == resolver.Candidate(beta, "0.1.0-beta3", "explicit"), (
        "unexpected candidate"
    )


def test_explicit_branch_resolves_through_its_remote_tracking_ref(
    repository: Path,
) -> None:
    """A branch the checkout has only as ``origin/<name>`` still resolves."""
    branch = commit_version(repository, "0.1.0-rc1-dev")
    git(repository, "update-ref", "refs/remotes/origin/release/0.1", branch)
    git(repository, "reset", "--quiet", "--hard", "HEAD~1")

    candidate = resolver.resolve(
        "release/0.1", "0" * 40, run_git_in_repository(repository)
    )

    assert candidate == resolver.Candidate(branch, "0.1.0-rc1-dev", "explicit"), (
        "the remote-tracking branch should be the candidate"
    )


def test_a_ref_that_resolves_as_given_wins_over_the_remote(repository: Path) -> None:
    """A tag of the same name as a remote branch keeps its own meaning."""
    tag = git(repository, "rev-parse", "v0.1.0-beta3^{commit}")
    other = commit_version(repository, "9.9.9")
    git(repository, "update-ref", "refs/remotes/origin/v0.1.0-beta3", other)

    candidate = resolver.resolve(
        "v0.1.0-beta3", "0" * 40, run_git_in_repository(repository)
    )

    assert candidate.commit == tag, "the tag should win over the remote branch"


@pytest.mark.parametrize("ref", ["--output=/tmp/owned", "no-such-ref"])
def test_unresolvable_refs_are_refused(repository: Path, ref: str) -> None:
    """Option-shaped and unknown refs fail rather than selecting anything."""
    with pytest.raises(subprocess.CalledProcessError):
        resolver.resolve(ref, "0" * 40, run_git_in_repository(repository))


def test_manifest_without_a_version_is_refused() -> None:
    """A candidate whose manifest has no package version cannot be verified."""
    with pytest.raises(ValueError, match="no \\[package\\] version"):
        resolver.package_version('[workspace]\nmembers = ["a"]\n')


def test_main_publishes_step_outputs(repository: Path, tmp_path: Path) -> None:
    """The command line writes the commit, version, and rule for later jobs."""
    head = git(repository, "rev-parse", "HEAD")
    output = tmp_path / "github-output"

    status = resolver.main(["--workflow-sha", head, "--output", str(output)])

    assert status == 0, "resolution should succeed"
    assert output.read_text(encoding="utf-8") == (
        f"commit={head}\nversion=0.1.0-beta3\nsource=workflow\n"
    ), "outputs should carry the commit, version, and rule"


@pytest.mark.usefixtures("repository")
def test_main_reports_an_unresolvable_candidate(tmp_path: Path) -> None:
    """A failed resolution exits non-zero and publishes no outputs."""
    output = tmp_path / "github-output"

    status = resolver.main([
        "--requested",
        "no-such-ref",
        "--workflow-sha",
        "0" * 40,
        "--output",
        str(output),
    ])

    assert status == 1, "resolution should fail"
    assert not output.exists(), "a failure must publish no outputs"

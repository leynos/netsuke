"""Resolve the exact Netsuke commit that the downstream canaries exercise.

The release workflow runs the three downstream migration canaries against one
candidate commit. By default that is the commit the workflow itself runs on:
the tag commit of a release, the pull request's merge commit on a dry run, or
the ref chosen in the Actions UI for a manual rehearsal. A manual run may
instead name another ref, or ``auto``, which selects the newest ``-rcN`` tag
when it is also the newest version tag and otherwise the ``main`` branch.

The resolved commit, the package version its ``Cargo.toml`` declares, and the
rule that selected it are written to ``GITHUB_OUTPUT`` so the canary jobs can
build and verify that exact candidate.
"""

import argparse
import collections.abc as cabc
import dataclasses
import re
import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - Git is invoked with fixed argument vectors.
import sys
import tomllib
from pathlib import Path

#: The requested-ref value that applies the release-candidate selection rule.
AUTO_REF = "auto"
#: The remote-tracking ref that ``auto`` falls back to.
MAIN_REF = "refs/remotes/origin/main"
#: A release tag: ``v`` followed by a SemVer 2.0 version without build metadata.
VERSION_TAG = re.compile(
    r"^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$"
)
#: The pre-release component of a release-candidate tag, such as ``rc1``.
RELEASE_CANDIDATE = re.compile(r"^rc\.?\d+$")
#: A full hexadecimal commit identifier.
COMMIT_ID = re.compile(r"^[0-9a-f]{40}$")

type GitRunner = cabc.Callable[[cabc.Sequence[str]], str]


@dataclasses.dataclass(frozen=True, slots=True)
class Candidate:
    """Describe the resolved release candidate.

    Attributes
    ----------
    commit
        The full commit identifier the canaries build.
    version
        The package version declared by that commit's ``Cargo.toml``.
    source
        The rule that selected the commit: ``workflow``, ``explicit``,
        ``rc-tag``, or ``main``.
    """

    commit: str
    version: str
    source: str


def version_key(tag: str) -> tuple[object, ...] | None:
    """Return a SemVer precedence key for a version tag, if it is one.

    Parameters
    ----------
    tag
        A Git tag name such as ``v0.1.0-rc1``.

    Returns
    -------
    tuple[object, ...] | None
        A key whose ordering matches SemVer 2.0 precedence, or ``None`` when
        the tag is not a version tag.

    Examples
    --------
    >>> version_key("v0.1.0-beta10") > version_key("v0.1.0-beta3")
    False
    >>> version_key("v0.1.0") > version_key("v0.1.0-rc2")
    True
    >>> version_key("vTEST") is None
    True
    """
    match = VERSION_TAG.match(tag)
    if match is None:
        return None
    major, minor, patch, prerelease = match.groups()
    core = (int(major), int(minor), int(patch))
    if prerelease is None:
        # A release outranks every pre-release of the same version.
        return (*core, 1, ())
    identifiers = tuple(
        (0, int(part), "") if part.isdigit() else (1, 0, part)
        for part in prerelease.split(".")
    )
    return (*core, 0, identifiers)


def latest_version_tag(tags: cabc.Iterable[str]) -> str | None:
    """Return the version tag with the highest SemVer precedence.

    Parameters
    ----------
    tags
        Tag names; non-version tags are ignored.

    Returns
    -------
    str | None
        The highest version tag, or ``None`` when there is none.

    Examples
    --------
    >>> latest_version_tag(["vTEST", "v0.1.0-beta3", "v0.1.0-rc1"])
    'v0.1.0-rc1'
    """
    keyed = [(key, tag) for tag in tags if (key := version_key(tag)) is not None]
    if not keyed:
        return None
    return max(keyed)[1]


def is_release_candidate_tag(tag: str) -> bool:
    """Return whether a version tag names a release candidate.

    Parameters
    ----------
    tag
        A version tag.

    Returns
    -------
    bool
        ``True`` when the pre-release component is ``rcN`` or ``rc.N``.

    Examples
    --------
    >>> is_release_candidate_tag("v0.1.0-rc2")
    True
    >>> is_release_candidate_tag("v0.1.0-beta3")
    False
    """
    match = VERSION_TAG.match(tag)
    prerelease = match.group(4) if match else None
    return prerelease is not None and RELEASE_CANDIDATE.match(prerelease) is not None


def select_auto_ref(tags: cabc.Iterable[str]) -> tuple[str, str]:
    """Apply the ``auto`` rule to the repository's tags.

    The newest release candidate is preferred only while it is also the newest
    version tag; once a later release or pre-release exists, ``main`` is the
    candidate instead.

    Parameters
    ----------
    tags
        Every tag name in the repository.

    Returns
    -------
    tuple[str, str]
        The ref to resolve and the name of the rule that selected it.

    Examples
    --------
    >>> select_auto_ref(["v0.1.0-beta3", "v0.1.0-rc1"])
    ('refs/tags/v0.1.0-rc1', 'rc-tag')
    >>> select_auto_ref(["v0.1.0-rc1", "v0.1.0"])
    ('refs/remotes/origin/main', 'main')
    """
    latest = latest_version_tag(tags)
    if latest is not None and is_release_candidate_tag(latest):
        return f"refs/tags/{latest}", "rc-tag"
    return MAIN_REF, "main"


def package_version(cargo_toml: str) -> str:
    r"""Return the ``[package]`` version declared by a Cargo manifest.

    Parameters
    ----------
    cargo_toml
        The manifest's text.

    Returns
    -------
    str
        The declared package version.

    Raises
    ------
    ValueError
        When the manifest declares no package version.
    TypeError
        When the declared version is not a string.

    Examples
    --------
    >>> package_version('[package]\nname = "netsuke"\nversion = "0.1.0-rc1"\n')
    '0.1.0-rc1'
    """
    version = tomllib.loads(cargo_toml).get("package", {}).get("version")
    if version is None:
        msg = "Cargo.toml declares no [package] version"
        raise ValueError(msg)
    if not isinstance(version, str):
        msg = "Cargo.toml declares a non-string [package] version"
        raise TypeError(msg)
    return version


def requested_ref(requested: str, workflow_sha: str, git: GitRunner) -> tuple[str, str]:
    """Return the ref named by the request and the rule that chose it.

    Parameters
    ----------
    requested
        The requested ref: empty for the workflow's own commit, ``auto`` for
        the release-candidate rule, or any other ref name.
    workflow_sha
        The commit the workflow runs on.
    git
        Runs Git with an argument vector and returns its standard output.

    Returns
    -------
    tuple[str, str]
        The ref to resolve and the name of the selecting rule.
    """
    if not requested:
        return workflow_sha, "workflow"
    if requested == AUTO_REF:
        return select_auto_ref(git(["tag", "--list"]).split())
    return requested, "explicit"


def peel_to_commit(ref: str, git: GitRunner) -> str:
    """Return Git's resolution of ``ref`` to a commit.

    A ref that does not resolve raises ``subprocess.CalledProcessError``.

    Parameters
    ----------
    ref
        The ref to resolve.
    git
        Runs Git with an argument vector and returns its standard output.

    Returns
    -------
    str
        Git's output, stripped.
    """
    # `--end-of-options` stops a caller-supplied ref being read as an option.
    return git([
        "rev-parse",
        "--verify",
        "--end-of-options",
        f"{ref}^{{commit}}",
    ]).strip()


def resolve_commit(ref: str, source: str, git: GitRunner) -> str:
    """Return the commit ``ref`` names, trying its remote branch as well.

    The resolver's checkout has every branch only as a remote-tracking ref,
    so an explicitly requested name that does not resolve as given is tried
    under ``refs/remotes/origin/``. A name that resolves as given, such as a
    tag, always wins. When neither resolves, the remote spelling's
    ``subprocess.CalledProcessError`` propagates.

    Parameters
    ----------
    ref
        The ref to resolve.
    source
        The rule that selected it.
    git
        Runs Git with an argument vector and returns its standard output.

    Returns
    -------
    str
        Git's resolution of the first spelling that resolves.
    """
    if source != "explicit" or ref.startswith("refs/"):
        return peel_to_commit(ref, git)
    try:
        return peel_to_commit(ref, git)
    except subprocess.CalledProcessError:
        return peel_to_commit(f"refs/remotes/origin/{ref}", git)


def resolve(requested: str, workflow_sha: str, git: GitRunner) -> Candidate:
    """Resolve the candidate commit and the version it declares.

    Parameters
    ----------
    requested
        The requested ref, as accepted by :func:`requested_ref`.
    workflow_sha
        The commit the workflow runs on.
    git
        Runs Git with an argument vector and returns its standard output.

    Returns
    -------
    Candidate
        The resolved commit, its declared version, and the selecting rule.

    Raises
    ------
    ValueError
        When the ref does not resolve to a full commit, or its manifest
        declares no version. A non-string version propagates the
        ``TypeError`` raised by :func:`package_version`.
    """
    ref, source = requested_ref(requested, workflow_sha, git)
    commit = resolve_commit(ref, source, git)
    if COMMIT_ID.match(commit) is None:
        msg = f"candidate ref did not resolve to a commit: {ref}"
        raise ValueError(msg)
    version = package_version(git(["show", f"{commit}:Cargo.toml"]))
    return Candidate(commit=commit, version=version, source=source)


def run_git(arguments: cabc.Sequence[str]) -> str:
    """Run Git in the current directory and return its standard output.

    A failing Git command raises ``subprocess.CalledProcessError``.

    Parameters
    ----------
    arguments
        Git arguments, excluding the ``git`` program name.

    Returns
    -------
    str
        The command's standard output.

    Raises
    ------
    FileNotFoundError
        When no ``git`` executable is on ``PATH``.
    """
    program = shutil.which("git")
    if program is None:
        msg = "git is not on PATH"
        raise FileNotFoundError(msg)
    completed = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - resolved program, list arguments.
        [program, *arguments], check=True, capture_output=True, text=True
    )
    return completed.stdout


def main(argv: cabc.Sequence[str] | None = None) -> int:
    """Resolve the candidate and append it to the ``GITHUB_OUTPUT`` file.

    Parameters
    ----------
    argv
        Arguments excluding the program name, or ``None`` for ``sys.argv``.

    Returns
    -------
    int
        ``0`` on success, ``1`` when the candidate cannot be resolved.
    """
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--requested", default="", help="ref, 'auto', or empty")
    parser.add_argument("--workflow-sha", required=True, help="the run's commit")
    parser.add_argument("--output", required=True, type=Path, help="GITHUB_OUTPUT")
    arguments = parser.parse_args(argv)
    try:
        candidate = resolve(arguments.requested, arguments.workflow_sha, run_git)
    except (ValueError, TypeError, OSError, subprocess.CalledProcessError) as error:
        print(f"release candidate resolution failed: {error}", file=sys.stderr)
        return 1
    with arguments.output.open("a", encoding="utf-8") as output:
        output.write(
            f"commit={candidate.commit}\nversion={candidate.version}\n"
            f"source={candidate.source}\n"
        )
    print(
        f"release candidate {candidate.commit} {candidate.version} ({candidate.source})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())

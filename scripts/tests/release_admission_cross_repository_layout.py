r"""Build the two-repository layout a cross-repository admission case runs on.

The admission job checks out the caller's candidate at the workspace root and
the pinned verifier in a subdirectory, and the gate fetches the candidate
revision from ``origin``. No command double can decide which repository that
is: a cmd-mox invocation records its arguments, its standard input, and its
environment, but its working directory -- the input that chooses ``origin`` --
is invisible to it. These helpers therefore build bare remotes and clones in
the case's own temporary tree, so the host's own ``git`` can serve as the
adapter and the working directory is the only difference between two runs.

The layout is asserted as it is built: the caller's repository holds the
candidate and the verifier's repository does not, so a passing fetch cannot be
vacuous.

Example (from a runtime test module)::

    layout = build_layout(tmp_path)
    checkout = layout.working_directory(at_verifier=False)
"""

import dataclasses
import os
import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - real Git is the boundary under test.
import typing as typ

if typ.TYPE_CHECKING:
    import pathlib


@dataclasses.dataclass(frozen=True, slots=True)
class Repositories:
    """Name the checkouts one case runs the gate against.

    Attributes
    ----------
    workspace
        The candidate checkout. Its ``origin`` is the caller's repository,
        which holds the candidate commit.
    verifier
        The pinned verifier checkout inside the workspace. Its ``origin`` is
        the verifier's repository, which does not hold the candidate.
    candidate
        The caller's commit, present in the caller's repository only.
    home
        A private home directory, so no host Git configuration reaches a run.
    """

    workspace: pathlib.Path
    verifier: pathlib.Path
    candidate: str
    home: pathlib.Path

    def working_directory(self, *, at_verifier: bool) -> pathlib.Path:
        """Return the directory a gate run starts in.

        Returns
        -------
        pathlib.Path
            The candidate checkout, or the verifier checkout inside it when a
            case models the regression.
        """
        return self.verifier if at_verifier else self.workspace


def git_binary() -> str:
    """Return the host's own ``git``, which these cases run for real."""
    binary = shutil.which("git")
    assert binary is not None, "these cases need a real git on PATH"
    return binary


def _git_environment(home: pathlib.Path) -> dict[str, str]:
    """Return an environment that isolates Git from the host's configuration.

    Returns
    -------
    dict[str, str]
        The parent environment with every ``GIT_*`` variable removed, a private
        home, no system configuration, and a fixed commit identity.

    Notes
    -----
    Removing the inherited ``GIT_*`` names matters because these cases may run
    under a Git hook, whose ``GIT_DIR`` and ``GIT_INDEX_FILE`` would otherwise
    redirect the commands below at the repository that started the test run.
    """
    environment = {
        name: value for name, value in os.environ.items() if not name.startswith("GIT_")
    }
    environment.update({
        "HOME": str(home),
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_AUTHOR_NAME": "admission test",
        "GIT_AUTHOR_EMAIL": "admission@example.invalid",
        "GIT_COMMITTER_NAME": "admission test",
        "GIT_COMMITTER_EMAIL": "admission@example.invalid",
        "GIT_TERMINAL_PROMPT": "0",
    })
    return environment


def _git(*arguments: str, cwd: pathlib.Path, home: pathlib.Path) -> str:
    """Run one Git command in a repository these cases created.

    Returns
    -------
    str
        The command's standard output, stripped.
    """
    completed = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed program; shell is disabled.
        [git_binary(), *arguments],
        capture_output=True,
        check=False,
        cwd=cwd,
        env=_git_environment(home),
        text=True,
    )
    assert completed.returncode == 0, (
        f"git {' '.join(arguments)} failed in {cwd}: {completed.stderr}"
    )
    return completed.stdout.strip()


def _init_bare(path: pathlib.Path, home: pathlib.Path) -> None:
    """Create a bare repository that accepts a fetch of any reachable commit.

    A plain fetch of a commit id is refused unless the server allows it, and
    the gate fetches exactly that form, so the allowance is part of modelling
    the remote faithfully rather than a concession to the test.
    """
    path.mkdir(parents=True)
    _git("init", "--bare", "--initial-branch=main", str(path), cwd=path, home=home)
    _git("config", "uploadpack.allowAnySHA1InWant", "true", cwd=path, home=home)


def _commit(repository: pathlib.Path, message: str, home: pathlib.Path) -> str:
    """Commit one file and return the new commit's id."""
    (repository / f"{message}.txt").write_text(message, encoding="utf-8")
    _git("add", "--all", cwd=repository, home=home)
    _git("commit", "--message", message, cwd=repository, home=home)
    return _git("rev-parse", "HEAD", cwd=repository, home=home)


def _push(repository: pathlib.Path, remote: pathlib.Path, home: pathlib.Path) -> None:
    """Publish the repository's main branch to a bare remote over ``file://``.

    The URL form matters: a plain path makes Git use its local transport, while
    ``file://`` runs the server side the way a network remote does, so the
    upload-pack allowance above is what decides the fetch.
    """
    _git("remote", "add", "origin", f"file://{remote}", cwd=repository, home=home)
    _git("push", "--quiet", "origin", "main", cwd=repository, home=home)


def _clone(remote: pathlib.Path, destination: pathlib.Path, home: pathlib.Path) -> None:
    """Clone a bare remote into a working checkout."""
    _git(
        "clone",
        "--quiet",
        f"file://{remote}",
        str(destination),
        cwd=destination.parent,
        home=home,
    )


def _holds_commit(repository: pathlib.Path, revision: str, home: pathlib.Path) -> bool:
    """Return whether a repository contains the named commit."""
    completed = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed program; shell is disabled.
        [git_binary(), "cat-file", "-e", f"{revision}^{{commit}}"],
        capture_output=True,
        check=False,
        cwd=repository,
        env=_git_environment(home),
        text=True,
    )
    return completed.returncode == 0


def build_layout(tmp_path: pathlib.Path) -> Repositories:
    """Build the two-repository layout the admission job produces.

    The caller's repository holds two commits, and its checkout sits on the
    later one, so the candidate is an earlier commit that the workspace does
    not have as its head. The verifier's repository holds a different commit
    entirely, which is what makes the fetch's working directory decisive.

    Returns
    -------
    Repositories
        The two checkouts, the caller's candidate commit, and the private home
        directory they run under. The layout is asserted as it is built, so a
        verifier repository that started holding the candidate would fail here
        rather than let a passing fetch prove nothing.
    """
    home = tmp_path / "home"
    home.mkdir()
    trees = tmp_path / "repositories"
    trees.mkdir()

    caller_source = trees / "caller-source"
    caller_source.mkdir()
    _git("init", "--initial-branch=main", cwd=caller_source, home=home)
    candidate = _commit(caller_source, "candidate", home)
    later = _commit(caller_source, "later", home)
    caller_remote = trees / "caller.git"
    _init_bare(caller_remote, home)
    _push(caller_source, caller_remote, home)

    verifier_source = trees / "verifier-source"
    verifier_source.mkdir()
    _git("init", "--initial-branch=main", cwd=verifier_source, home=home)
    _commit(verifier_source, "verifier", home)
    verifier_remote = trees / "verifier.git"
    _init_bare(verifier_remote, home)
    _push(verifier_source, verifier_remote, home)

    workspace = trees / "workspace"
    _clone(caller_remote, workspace, home)
    verifier = workspace / "gate"
    _clone(verifier_remote, verifier, home)

    assert _git("rev-parse", "HEAD", cwd=workspace, home=home) == later, (
        "the candidate checkout must not sit on the candidate itself"
    )
    assert not _holds_commit(verifier, candidate, home), (
        "the verifier repository must not contain the caller's candidate, or "
        "the refusal this module measures could pass for the wrong reason"
    )
    assert _holds_commit(caller_remote, candidate, home), (
        "the caller's repository must contain the candidate the gate fetches"
    )
    return Repositories(
        workspace=workspace, verifier=verifier, candidate=candidate, home=home
    )

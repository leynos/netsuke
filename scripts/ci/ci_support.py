"""Shared helpers for the CI helper scripts under ``scripts/ci``.

The scripts replace inline shell in ``.github/workflows/ci.yml``. Each one is
a Cyclopts application that reads its parameters from ``INPUT_*`` environment
variables (and the ambient GitHub Actions variables it needs), runs external
programs through cuprum's allowlisted catalogue, and uses ``pathlib`` for the
filesystem. This module holds the pieces they share: the catalogue, a thin
synchronous runner, ``GITHUB_PATH`` publication, and checksum-verified
download and extraction of release archives.

Every function here is exercised by ``scripts/tests/test_ci_support.py``.
"""

import dataclasses
import hashlib
import os
import pathlib
import shutil
import tarfile
import urllib.parse
import urllib.request

from cuprum import (
    CommandResult,
    ExecutionContext,
    Program,
    ProgramCatalogue,
    ProjectSettings,
    sh,
)

PROJECT_NAME = "netsuke-ci"
DOCUMENTATION = ("docs/developers-guide.md",)
#: Release archives come from GitHub over HTTPS; tests serve fixtures from
#: ``file://`` URLs so the real download path is exercised without a network.
ALLOWED_URL_SCHEMES = frozenset({"https", "file"})
DOWNLOAD_TIMEOUT_SECONDS = 120.0
EXECUTABLE_MODE = 0o755


class CiScriptError(RuntimeError):
    """An actionable failure whose message names the remediation."""


def catalogue(*programs: str) -> ProgramCatalogue:
    """Return a cuprum catalogue allowing exactly ``programs``.

    Parameters
    ----------
    programs
        Program names, or absolute paths for executables the script staged
        itself, that the calling script is permitted to run.

    Returns
    -------
    ProgramCatalogue
        A catalogue whose single project lists ``programs``.
    """
    return ProgramCatalogue(
        projects=(
            ProjectSettings(
                name=PROJECT_NAME,
                programs=tuple(Program(program) for program in programs),
                documentation_locations=DOCUMENTATION,
                noise_rules=(),
            ),
        )
    )


@dataclasses.dataclass(frozen=True, slots=True)
class RunSpec:
    """How one command invocation should behave, separate from what it runs.

    The program and its arguments are positional, so they read as the command
    being run; these are the two things about the *invocation* a caller
    sometimes needs to change. Grouping them keeps a call site from growing a
    parameter per concern, and a script that never changes one simply omits
    the group.
    """

    allowed: ProgramCatalogue
    echo: bool = True
    env: dict[str, str] | None = None


def run(program: str, *args: str, spec: RunSpec) -> CommandResult:
    """Run one allowlisted program synchronously and return its result.

    Parameters
    ----------
    program
        The program to run; it must be in ``spec.allowed``.
    args
        Arguments passed verbatim, never through a shell.
    spec
        The catalogue permitting ``program``, whether to mirror the child's
        output to this process's streams as well as capturing it, and any
        environment variables overlaid on the live environment for this run
        only.

    Returns
    -------
    CommandResult
        The exit code and captured output; a non-zero exit is returned, not
        raised, so callers decide what it means.
    """
    command = sh.make(Program(program), catalogue=spec.allowed)(*args)
    return command.run_sync(
        echo=spec.echo,
        context=ExecutionContext(env=spec.env),
    )


def describe_failure(result: CommandResult) -> str:
    """Return a one-line description of a failed command for an error message."""
    argv = " ".join((str(result.program), *result.argv))
    detail = (result.stderr or "").strip().splitlines()
    suffix = f": {detail[-1]}" if detail else ""
    return f"`{argv}` exited {result.exit_code}{suffix}"


def append_github_path(directory: pathlib.Path, github_path: pathlib.Path) -> None:
    """Publish ``directory`` to later workflow steps through ``GITHUB_PATH``.

    Parameters
    ----------
    directory
        The directory to prepend to ``PATH`` for the rest of the job.
    github_path
        The file GitHub Actions names in ``GITHUB_PATH``.
    """
    with github_path.open("a", encoding="utf-8") as handle:
        handle.write(f"{directory}\n")


def prepend_path(directory: pathlib.Path) -> None:
    """Prepend ``directory`` to this process's ``PATH`` for later commands."""
    current = os.environ.get("PATH", "")
    os.environ["PATH"] = (
        f"{directory}{os.pathsep}{current}" if current else str(directory)
    )


def is_executable_file(path: pathlib.Path) -> bool:
    """Return whether ``path`` is a regular file this process may execute."""
    return path.is_file() and os.access(path, os.X_OK)


def download(url: str, destination: pathlib.Path) -> None:
    """Download ``url`` to ``destination``.

    Parameters
    ----------
    url
        An ``https://`` release URL, or a ``file://`` URL under test.
    destination
        The file to write; its parent must exist.

    Raises
    ------
    CiScriptError
        If the URL scheme is not allowed or the download fails.
    """
    scheme = urllib.parse.urlsplit(url).scheme
    if scheme not in ALLOWED_URL_SCHEMES:
        message = (
            f"refusing to download {url!r}: only "
            f"{sorted(ALLOWED_URL_SCHEMES)} URLs are permitted"
        )
        raise CiScriptError(message)
    try:
        # The scheme is allowlisted just above; `file://` serves test fixtures.
        with (
            urllib.request.urlopen(url, timeout=DOWNLOAD_TIMEOUT_SECONDS) as response,  # ruff: ignore[suspicious-url-open-usage] - scheme allowlisted above
            destination.open("wb") as handle,
        ):
            shutil.copyfileobj(response, handle)
    except OSError as error:
        message = f"downloading {url} failed: {error}"
        raise CiScriptError(message) from error


def sha256_of(path: pathlib.Path) -> str:
    """Return the lower-case hex SHA-256 digest of ``path``."""
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify_sha256(path: pathlib.Path, expected: str) -> None:
    """Fail unless ``path`` hashes to ``expected``.

    Parameters
    ----------
    path
        The downloaded archive.
    expected
        The pinned hex digest from the workflow.

    Raises
    ------
    CiScriptError
        If the digest differs; the message names both digests so a pin bump
        and a tampered download are told apart in the log.
    """
    actual = sha256_of(path)
    if actual != expected.lower():
        message = (
            f"{path.name} does not match its pinned SHA-256: expected "
            f"{expected}, got {actual}; the release archive changed or the "
            f"pin in .github/workflows/ci.yml is stale"
        )
        raise CiScriptError(message)


@dataclasses.dataclass(frozen=True, slots=True)
class OpenArchive:
    """A checksum-verified tarball, held open with its members indexed.

    The index is built once so each named member is a dictionary lookup rather
    than a scan, and so a per-member check needs the archive's name and stream
    without carrying both through the call separately.
    """

    path: pathlib.Path
    tar: tarfile.TarFile
    members: dict[str, tarfile.TarInfo]


def extract_members(
    archive: pathlib.Path,
    destination: pathlib.Path,
    members: tuple[str, ...],
) -> list[pathlib.Path]:
    """Extract named regular-file members of a gzip tarball as executables.

    Parameters
    ----------
    archive
        A ``.tar.gz`` archive that has already been checksum-verified.
    destination
        The directory the members are written into, flat, under their own
        base names.
    members
        The exact member names to extract; every one must be present and be
        a regular file. Directories and links are refused so an archive can
        never write outside ``destination``.

    Returns
    -------
    list[pathlib.Path]
        The extracted executables, in ``members`` order.

    Raises
    ------
    CiScriptError
        If a member is missing or is not a regular file.
    """
    with tarfile.open(archive, "r:gz") as tar:
        opened = OpenArchive(
            archive, tar, {member.name: member for member in tar.getmembers()}
        )
        extracted = []
        for name in members:
            member = opened.members.get(name)
            if member is None:
                message = f"{opened.path.name} has no member {name!r}"
                raise CiScriptError(message)
            if not member.isfile():
                message = f"{opened.path.name} member {name!r} is not a regular file"
                raise CiScriptError(message)
            extracted.append(_write_member(opened, destination, name, member))
        return extracted


def _write_member(
    opened: OpenArchive,
    destination: pathlib.Path,
    name: str,
    member: tarfile.TarInfo,
) -> pathlib.Path:
    """Write the already-validated member ``name`` into ``destination``.

    Validation happens in :func:`extract_members`, which refuses a member
    naming a directory or a link before anything is written, so a hostile
    archive cannot use one to escape ``destination``.

    Returns
    -------
    pathlib.Path
        The extracted executable, under its own base name in ``destination``.

    Raises
    ------
    CiScriptError
        If the member's stream cannot be read.
    """
    source = opened.tar.extractfile(member)
    if source is None:  # pragma: no cover - isfile() guarantees a stream
        message = f"{opened.path.name} member {name!r} could not be read"
        raise CiScriptError(message)
    target = destination / pathlib.PurePosixPath(name).name
    with source, target.open("wb") as handle:
        shutil.copyfileobj(source, handle)
    target.chmod(EXECUTABLE_MODE)
    return target

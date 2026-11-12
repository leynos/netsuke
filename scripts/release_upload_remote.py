"""Read and write a release's attached assets at the ``gh`` boundary.

:mod:`release_upload_run` decides which names must be uploaded; this module is
the only place that talks to GitHub. Every call goes through one injected
``SubprocessRunner``, so a test replaces the whole boundary with a stub and no
test reaches the network.

Two behaviours here exist to keep a reconciliation honest rather than merely
successful:

1. A listing is taken with ``--json assets`` and each entry's vendor digest is
   normalised by :func:`release_github_assets_adapter.normalise_github_digest`.
   An entry GitHub reports without a usable digest becomes ``None``, which the
   caller reads as "unknown", never as "matches".
2. An unknown digest may still be established by downloading the asset and
   hashing it. The download is *not* reported as a failure when it does not
   happen: the name simply keeps its ``None``, and reconciliation classifies it
   as ``UNVERIFIABLE`` rather than as a match on no evidence. That is
   deliberately not the same outcome as a conflicting digest: a conflict says
   the release holds demonstrably wrong bytes, while an unverifiable name says
   the run could not tell, so the fault may lie in the listing or the download
   rather than in the release.

Uploads are sent without ``--clobber``. The flag deletes the existing asset
before transferring, so a failed upload would lose the original; and a release
that replaces its own bytes on a re-run could publish a second commit's build
under the first commit's tag.

Run via ``.github/actions/upload-declared-assets``; behavioural coverage lives
in ``scripts/tests/test_upload_declared_assets_cli.py``.
"""

import json
import subprocess  # ruff: ignore[suspicious-subprocess-import] - upload boundary.
import typing as typ

import release_github_assets_adapter

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

__all__ = [
    "ASSETS_MEMBER",
    "PRODUCTION_RUNNER",
    "GhCommandFailedError",
    "NoNetworkInDoctestError",
    "RemoteListingError",
    "StaticRunner",
    "SubprocessRunner",
    "UploadError",
    "attached_digests",
    "upload",
]

#: The shape of ``subprocess.run`` this module needs. Declared so a test can
#: supply its own runner instead of patching the global ``subprocess`` module,
#: which keeps the failure modes of *launching* a command reachable without
#: actually launching one.
type SubprocessRunner = cabc.Callable[..., subprocess.CompletedProcess[str]]

#: Member of ``gh release view --json assets`` holding the attached assets.
ASSETS_MEMBER = "assets"

#: The real subprocess seam. Named here so the CLI imports no subprocess
#: module of its own and the boundary has exactly one production entry point.
PRODUCTION_RUNNER: SubprocessRunner = subprocess.run


class UploadError(RuntimeError):
    """Report a release operation that could not be completed."""


class RemoteListingError(UploadError):
    """Report a release listing that cannot support a reconciliation."""


class GhCommandFailedError(UploadError):
    """Report a ``gh`` invocation that exited non-zero when it had to succeed."""


def attached_digests(tag: str, *, runner: SubprocessRunner) -> dict[str, str | None]:
    """Read the assets the release already attaches and their digests.

    Parameters
    ----------
    tag
        The validated release tag to list.
    runner
        The injected subprocess seam.

    Returns
    -------
    dict[str, str | None]
        Attached asset name mapped to its bare digest, or ``None`` where
        GitHub reported no digest this release can compare against.

    Notes
    -----
    A listing that cannot be taken surfaces ``GhCommandFailedError`` from
    :func:`gh`, and output that does not carry an asset list surfaces
    ``RemoteListingError`` from :func:`_decoded_assets`.

    Examples
    --------
    >>> attached_digests("v1", runner=StaticRunner('{"assets": []}'))
    {}
    >>> attached_digests(
    ...     "v1", runner=StaticRunner('{"assets": [{"name": "a", "digest": null}]}')
    ... )
    {'a': None}
    """
    completed = gh(("release", "view", tag, "--json", ASSETS_MEMBER), runner=runner)
    assets = _decoded_assets(completed.stdout)
    return {
        entry["name"]: release_github_assets_adapter.normalise_github_digest(
            entry.get("digest")
        )
        for entry in assets
        if isinstance(entry, dict) and isinstance(entry.get("name"), str)
    }


def _decoded_assets(stdout: str | None) -> list[object]:
    """Return the release's asset list from ``gh``'s JSON output.

    Parameters
    ----------
    stdout
        The command's standard output, or ``None`` when it produced none.

    Returns
    -------
    list[object]
        The decoded entries, left unchecked so the caller can skip any entry
        that is not an object carrying a string name.

    Raises
    ------
    RemoteListingError
        If the output is not JSON carrying an ``assets`` list.

    Examples
    --------
    >>> _decoded_assets('{"assets": [{"name": "a"}, 7]}')
    [{'name': 'a'}, 7]
    >>> _decoded_assets("not json")
    Traceback (most recent call last):
        ...
    release_upload_remote.RemoteListingError: the release listing is not JSON: ...
    """
    try:
        decoded = json.loads(stdout or "")
    except json.JSONDecodeError as error:
        msg = f"the release listing is not JSON: {error}"
        raise RemoteListingError(msg) from error
    assets = decoded.get(ASSETS_MEMBER) if isinstance(decoded, dict) else None
    if not isinstance(assets, list):
        msg = "the release listing carries no asset list"
        raise RemoteListingError(msg)
    return assets


def upload(
    tag: str,
    names: cabc.Iterable[str],
    payload_dir: pathlib.Path,
    *,
    runner: SubprocessRunner,
) -> None:
    """Upload the named files, without ``--clobber`` and in one call.

    Parameters
    ----------
    tag
        The validated release tag to attach the files to.
    names
        The declared asset names the release is missing.
    payload_dir
        Directory holding the verified payload under its published names.
    runner
        The injected subprocess seam.

    Notes
    -----
    ``--clobber`` is deliberately absent: it deletes the existing asset before
    uploading, so a failed transfer loses the original; and a release that
    replaced its own bytes on a re-run could publish a second commit's build
    under the first commit's tag. ``gh`` exiting non-zero surfaces
    ``GhCommandFailedError`` from :func:`gh`.

    Examples
    --------
    An empty ``names`` transfers nothing, so it returns without invoking
    ``gh``. ``gh release upload`` given no file is not a no-op: it is a
    request the release endpoint may reject, and a caller misreading that
    rejection as a refused transfer would report a failure for a run that had
    every declared asset already attached.

    >>> import pathlib
    >>> upload("v1", [], pathlib.Path("/payload"), runner=_never_called)
    >>> upload("v1", ["a"], pathlib.Path("/payload"), runner=_never_called)
    Traceback (most recent call last):
        ...
    release_upload_remote.NoNetworkInDoctestError: this path must not invoke gh
    """
    files = [str(payload_dir / name) for name in sorted(names)]
    if not files:
        return
    gh(("release", "upload", tag, *files), runner=runner)


def gh(
    arguments: cabc.Sequence[str],
    *,
    runner: SubprocessRunner,
    required: bool = True,
) -> subprocess.CompletedProcess[str]:
    """Run one ``gh`` subcommand through the injected seam.

    Parameters
    ----------
    arguments
        The subcommand and its arguments, without the executable.
    runner
        The injected subprocess seam.
    required
        Whether a non-zero exit is an error.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The completed invocation, whatever its status.

    Raises
    ------
    _GhExitError
        If ``required`` and the command exits non-zero.

    Notes
    -----
    The runner is the injected seam, not the ``subprocess`` module, so the
    command launch happens in exactly one place and a test replaces it whole.
    """
    completed = runner(["gh", *arguments], check=False, capture_output=True, text=True)
    if required and completed.returncode != 0:
        raise _GhExitError(arguments, completed)
    return completed


class _GhExitError(GhCommandFailedError):
    """Report the exact ``gh`` invocation and status that failed."""

    def __init__(
        self, arguments: cabc.Sequence[str], completed: subprocess.CompletedProcess[str]
    ) -> None:
        """Fold the arguments and the captured standard error into the message."""
        detail = (completed.stderr or "").strip()
        super().__init__(
            f"gh {' '.join(arguments)} exited {completed.returncode}: {detail}"
        )


class NoNetworkInDoctestError(AssertionError):
    """Report a doctest that reached a path which must not touch the network.

    Named and exported so a doctest in another module can assert that the
    path it exercises never reaches ``gh``.
    """

    def __init__(self) -> None:
        """Carry the fixed message the doctest's assertion matches on."""
        super().__init__("this path must not invoke gh")


class StaticRunner:
    """Stand in for a subprocess runner that always succeeds with fixed output.

    A doctest may name it to exercise a path that reads a listing without
    reaching ``gh``. It is not a production seam; the exchange is recorded so a
    test can assert what a real caller would have run.
    """

    def __init__(self, stdout: str) -> None:
        """Hold the fixed standard output and start an empty record of calls."""
        self.stdout = stdout
        self.calls: list[list[str]] = []

    def __call__(
        self, arguments: cabc.Sequence[str], **_keywords: object
    ) -> subprocess.CompletedProcess[str]:
        """Record the invocation and return a successful, empty-stderr result."""
        self.calls.append(list(arguments))
        return subprocess.CompletedProcess(
            args=list(arguments), returncode=0, stdout=self.stdout, stderr=""
        )


def _never_called(*_arguments: object, **_keywords: object) -> typ.NoReturn:
    """Fail the doctest that reaches the network, which none may do."""
    raise NoNetworkInDoctestError

"""Run the extracted binary once to observe the version it reports.

This is the smoke's execution boundary and the only place a downloaded file is
launched. Nothing here decides whether the reported version is the candidate's
— that is :func:`release_consumer_payload.require_probe` — and nothing here
touches the archive; this module passes an absolute path, ``--version`` as an
argument, a wall-clock limit, an output bound, and a fixed environment, then
reports what the child did.

The environment is fixed rather than inherited. The job holds a read-only
token, but the binary came from a public URL and has no business reading any
credential the runner happens to export. The runner is injected so a test can
observe the launch without executing a downloaded file.

Standard error is discarded rather than captured. Only the banner decides
acceptance, and decoding a stray byte on standard error would let a diagnostic
the smoke never reads turn a correct banner into a failed run.

Run through ``scripts/consumer_smoke_cli.py``; behavioural coverage lives in
``scripts/tests/test_release_binstall_archive.py``.
"""

import dataclasses
import subprocess  # ruff: ignore[suspicious-subprocess-import] - probe boundary.
import typing as typ

from release_binstall_archive import ArchiveRefusedError

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

#: The shape of ``subprocess.run`` this module needs, injected so a test can
#: observe the probe without launching the extracted binary.
type SubprocessRunner = cabc.Callable[..., subprocess.CompletedProcess[str]]

#: Wall-clock seconds the version probe may run before it is killed.
PROBE_TIMEOUT_SECONDS = 60

#: Environment the probe receives. It inherits nothing, so no exported
#: credential reaches a file this workflow downloaded.
PROBE_ENVIRONMENT = {"PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C"}


class ProbeLaunchError(ArchiveRefusedError):
    """Report a shipped binary that could not be launched at all."""


class ProbeTimeoutError(ArchiveRefusedError):
    """Report a shipped binary that did not answer within the time budget."""


class ProbeOutputError(ArchiveRefusedError):
    """Report standard output that is not UTF-8 text.

    The banner comparison needs text, and a replaced or ignored byte would
    hide the fact that the binary wrote something this smoke cannot read. The
    refusal names that fact instead of presenting a mangled banner as if the
    binary had printed it.
    """


@dataclasses.dataclass(frozen=True, slots=True)
class ProbeResult:
    """Hold the exit status and standard output of one ``--version`` probe."""

    returncode: int
    stdout: str


def probe_version(
    binary: pathlib.Path, *, runner: SubprocessRunner | None = None
) -> ProbeResult:
    r"""Run ``binary --version`` and return its status and standard output.

    Parameters
    ----------
    binary
        Absolute path of the extracted binary.
    runner
        The callable that launches it. ``None`` uses :func:`subprocess.run`;
        a test supplies its own so the probe boundary is exercised without
        executing a downloaded file.

    Returns
    -------
    ProbeResult
        The exit status and standard output.

    Raises
    ------
    ProbeTimeoutError
        If the probe exceeds :data:`PROBE_TIMEOUT_SECONDS`.
    ProbeLaunchError
        If the binary could not be started.
    ProbeOutputError
        If standard output is not UTF-8 text, so no banner can be read from it.

    Notes
    -----
    A non-zero exit status is not a failure here. It is reported in the
    result so the banner decision can name the exit status in its diagnosis,
    which is a different operational fact from a binary that never ran.

    Examples
    --------
    >>> import pathlib
    >>> def fake(command, **_kwargs):
    ...     return subprocess.CompletedProcess(command, 0, "netsuke 1.2.3\\n", "")
    >>> probe_version(pathlib.Path("/tmp/netsuke"), runner=fake).stdout
    'netsuke 1.2.3\\n'
    """
    launch = subprocess.run if runner is None else runner
    try:
        completed = launch(
            [str(binary), "--version"],
            check=False,
            # Standard error goes nowhere rather than into a decoded pipe: a
            # child that writes a non-UTF-8 diagnostic would otherwise fail the
            # launch before the banner that decides acceptance is ever read.
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            timeout=PROBE_TIMEOUT_SECONDS,
            env=PROBE_ENVIRONMENT,
        )
    except subprocess.TimeoutExpired as error:
        msg = (
            f"the shipped binary did not report its version within "
            f"{PROBE_TIMEOUT_SECONDS}s and was killed"
        )
        raise ProbeTimeoutError(msg) from error
    except UnicodeDecodeError as error:
        msg = "the shipped binary wrote bytes that are not UTF-8 to standard output"
        raise ProbeOutputError(msg) from error
    except OSError as error:
        msg = f"the shipped binary could not be launched: {error}"
        raise ProbeLaunchError(msg) from error
    return ProbeResult(returncode=completed.returncode, stdout=completed.stdout)

"""Test launching the extracted binary and reading its version banner.

The probe is driven through an injected launcher, so no case executes a
downloaded file. Some cases do launch a shell script the test itself writes,
because the reading, decoding, and stopping of a child's streams belong to
``subprocess`` and an injected launcher cannot reproduce them.

Run via ``make test-release-publication``.
"""

import io
import sys
import threading
import time
import typing as typ

import pytest
import release_binstall_child as child_adapter
import release_binstall_probe as probe
import release_consumer_archive_policy as policy
from consumer_smoke_harness import FakeChild

if typ.TYPE_CHECKING:
    import pathlib


class _StalledStream(io.TextIOBase):
    """Standard output that yields nothing until the child is stopped."""

    def __init__(self) -> None:
        """Start with no output released."""
        self.released = threading.Event()

    def read(self, size: int | None = -1, /) -> str:
        """Block until the child is stopped, then report end of stream."""
        _ = size
        self.released.wait()
        return ""


class _StalledChild(FakeChild):
    """A child that never answers until the probe stops it."""

    def stop(self) -> int:
        """Release the waiting read, then stop and reap the child.

        A real child's stop ends the read blocked on its pipe, which is the
        whole reason the probe's watchdog has a thread of its own. Releasing
        first reproduces that order, so the case proves the probe is waiting on
        a stopped child rather than returning on its own.

        Returns
        -------
        int
            The status the child exited with.
        """
        typ.cast("_StalledStream", self.stream).released.set()
        return super().stop()


class _UndecodableStream(io.TextIOBase):
    """Standard output that cannot be decoded as text."""

    def read(self, size: int | None = -1, /) -> str:
        """Fail the way text-mode capture fails on an undecodable byte."""
        _ = size
        raise UnicodeDecodeError("utf-8", b"\xff", 0, 1, "invalid start byte")


def _child(*, returncode: int = 0, stdout: str = "netsuke 1.2.3\n") -> FakeChild:
    """Return a child that answers with one fixed status and banner."""
    return FakeChild(returncode=returncode, stream=io.StringIO(stdout))


def test_probe_version_launches_the_binary_with_its_version_flag(
    tmp_path: pathlib.Path,
) -> None:
    """Pass the absolute path and a minimal environment, and report the answer."""
    observed: dict[str, object] = {}

    def launcher(command: list[str], *, env: dict[str, str]) -> FakeChild:
        """Record the launch, then answer as a healthy binary would."""
        observed["command"] = command
        observed["env"] = env
        return _child()

    binary = tmp_path / "netsuke"
    result = probe.probe_version(binary, launcher=launcher)

    assert observed["command"] == [str(binary), "--version"], (
        "the probe must pass the binary path and its version flag as argv"
    )
    assert observed["env"] == probe.PROBE_ENVIRONMENT, (
        "the probe must run with its own fixed environment"
    )
    assert "GITHUB_TOKEN" not in typ.cast("dict[str, str]", observed["env"]), (
        "the probe must not inherit a workflow credential"
    )
    assert (result.returncode, result.stdout) == (0, "netsuke 1.2.3\n"), (
        "the probe must report the status and standard output it observed"
    )


def test_probe_version_stops_a_child_that_never_answers(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Translate an expired wall-clock budget into its own refusal."""
    monkeypatch.setattr(probe, "PROBE_TIMEOUT_SECONDS", 0.05)
    child = _StalledChild(returncode=0, stream=_StalledStream())

    with pytest.raises(probe.ProbeTimeoutError):
        probe.probe_version(tmp_path / "netsuke", launcher=lambda *_, **__: child)

    assert child.stopped, "a probe that ran too long must stop its child"


def test_probe_version_stops_a_child_that_floods_its_output(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse, and stop, a child that writes past the ceiling.

    The child is stopped before the refusal is raised, so a binary that floods
    its standard output cannot outlive the run that rejected it.
    """
    flooded = _child(stdout="x" * (probe.PROBE_OUTPUT_LIMIT + 1))

    with pytest.raises(probe.ProbeOutputLimitError):
        probe.probe_version(tmp_path / "netsuke", launcher=lambda *_, **__: flooded)

    assert flooded.reads == [probe.PROBE_OUTPUT_LIMIT + 1], (
        "the probe must ask for one character past the ceiling and no more"
    )
    assert flooded.stopped, "a child that floods its output must be stopped"
    assert flooded.reaped, "a child that floods its output must be reaped"


def test_probe_version_accepts_a_banner_exactly_at_the_ceiling(
    tmp_path: pathlib.Path,
) -> None:
    """Keep a banner the ceiling only just admits."""
    banner = "x" * probe.PROBE_OUTPUT_LIMIT
    child = _child(stdout=banner)

    result = probe.probe_version(tmp_path / "netsuke", launcher=lambda *_, **__: child)

    assert result.stdout == banner, (
        "the ceiling bounds the read; it must not refuse what fits inside it"
    )
    assert child.reads == [probe.PROBE_OUTPUT_LIMIT + 1], (
        "the probe must still ask for one character past the ceiling"
    )
    assert child.reaped, "a probe that finished inside its budget must reap its child"
    assert not child.stopped, (
        "a child that answered must be waited for, not killed for how long it took"
    )
    assert child.closed, "the captured standard output must be released on every path"


def test_probe_version_stops_a_child_that_answered_then_hung(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a child that closes its output and then stays alive.

    The banner arrives at once, so only the wait can refuse this child. Waiting
    without a deadline would hang the smoke on a binary that printed the right
    line and never exited.
    """

    class _LingeringChild(FakeChild):
        """A child that answers, then never exits."""

        exit_status: int | None = None

        def wait_until(self, deadline: float) -> int | None:
            """Report the status the child had reached by ``deadline``.

            Returns
            -------
            int | None
                ``None`` for this child: it never exits, whatever it is asked,
                so the probe has to stop it once the budget is spent.
            """
            _ = deadline
            return self.exit_status

    child = _LingeringChild(returncode=0, stream=io.StringIO("netsuke 1.2.3\n"))

    with pytest.raises(probe.ProbeTimeoutError):
        probe.probe_version(tmp_path / "netsuke", launcher=lambda *_, **__: child)

    assert child.stopped, "a child that outlived the budget must be stopped"
    assert child.closed, "the captured standard output must be released on every path"


def test_probe_version_reports_a_launch_failure(tmp_path: pathlib.Path) -> None:
    """Translate a binary that never started into its own refusal."""

    def launcher(command: list[str], **_kwargs: object) -> typ.NoReturn:
        """Fail the launch the way a missing binary would."""
        raise FileNotFoundError(command[0])

    with pytest.raises(probe.ProbeLaunchError):
        probe.probe_version(tmp_path / "netsuke", launcher=launcher)


def test_probe_version_reports_output_that_is_not_utf8(
    tmp_path: pathlib.Path,
) -> None:
    """Translate undecodable standard output into its own refusal."""
    child = FakeChild(returncode=0, stream=_UndecodableStream())

    with pytest.raises(probe.ProbeOutputError):
        probe.probe_version(tmp_path / "netsuke", launcher=lambda *_, **__: child)


@pytest.mark.skipif(sys.platform == "win32", reason="needs a POSIX shell script")
def test_probe_version_ignores_a_non_utf8_diagnostic(tmp_path: pathlib.Path) -> None:
    """Accept a correct banner whatever the child writes to standard error.

    The child is a script this test writes, not a downloaded file. It is
    launched for real because the defect is in ``subprocess``: text-mode
    capture decodes both streams, so one stray byte on standard error would
    otherwise refuse a run whose banner is exactly right.
    """
    binary = tmp_path / policy.BINARY_NAME
    _ = binary.write_text(
        "#!/bin/sh\nprintf 'netsuke 1.2.3\\n'\nprintf '\\377' >&2\n",
        encoding="utf-8",
    )
    binary.chmod(0o755)

    result = probe.probe_version(binary, launcher=child_adapter.launch)

    assert result.stdout == "netsuke 1.2.3\n", (
        "a diagnostic on standard error must not decide the banner's verdict"
    )


@pytest.mark.skipif(sys.platform == "win32", reason="needs a POSIX shell script")
def test_probe_version_stops_a_real_child_that_floods_its_output(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a real child's flood without reading all of it.

    The ceiling is enforced against a child this test launches, so the bound
    is proved against ``subprocess`` rather than only against a fake. The child
    writes a bounded flood and finishes on its own, so nothing is left running
    whichever way the case ends.
    """
    binary = tmp_path / policy.BINARY_NAME
    _ = binary.write_text(
        "#!/bin/sh\n"
        "printf 'netsuke 1.2.3\\n'\n"
        "dd if=/dev/zero bs=1000 count=20 2>/dev/null | tr '\\0' 'x'\n",
        encoding="utf-8",
    )
    binary.chmod(0o755)

    with pytest.raises(probe.ProbeOutputLimitError):
        probe.probe_version(binary, launcher=child_adapter.launch)


@pytest.mark.skipif(sys.platform == "win32", reason="needs a POSIX shell script")
def test_probe_version_releases_a_pipe_a_descendant_holds(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Refuse promptly when a descendant keeps the captured pipe open.

    The child prints the right banner and exits, so the only thing keeping the
    read blocked is the ``sleep`` it started, which inherited standard output.
    Ending the child alone would leave that pipe open and the read waiting for
    an end of file that never comes. The case bounds the whole call by the
    drain budget: a probe that released the pipe returns as soon as the group
    is killed, while one that waited for the reader would spend the entire
    drain before reporting the same refusal.
    """
    monkeypatch.setattr(probe, "PROBE_TIMEOUT_SECONDS", 0.5)
    binary = tmp_path / policy.BINARY_NAME
    _ = binary.write_text(
        "#!/bin/sh\nprintf 'netsuke 1.2.3\\n'\nsleep 5 &\nexit 0\n",
        encoding="utf-8",
    )
    binary.chmod(0o755)

    started = time.monotonic()
    with pytest.raises(probe.ProbeTimeoutError):
        probe.probe_version(binary, launcher=child_adapter.launch)
    elapsed = time.monotonic() - started

    assert elapsed < probe.PROBE_DRAIN_SECONDS, (
        "the read must be released by stopping the process group; waiting for "
        "the drain budget means the descendant kept the pipe open"
    )

"""Test launching the extracted binary and reading its version banner.

The probe is driven through an injected runner, so no case executes a
downloaded file. One case does launch a shell script the test itself writes,
because decoding of a child's streams belongs to ``subprocess`` and an
injected runner cannot reproduce it.

Run via ``make test-release-publication``.
"""

import subprocess  # ruff: ignore[suspicious-subprocess-import] - the seam under test.
import sys
import typing as typ

import pytest
import release_binstall_probe as probe
import release_consumer_archive_policy as policy

if typ.TYPE_CHECKING:
    import pathlib


def test_probe_version_launches_the_binary_with_its_version_flag(
    tmp_path: pathlib.Path,
) -> None:
    """Pass the absolute path, ``--version``, and a minimal environment."""
    observed: dict[str, object] = {}

    def runner(
        command: list[str], **kwargs: object
    ) -> subprocess.CompletedProcess[str]:
        """Record the launch, then answer as a healthy binary would."""
        observed["command"] = command
        observed["env"] = kwargs["env"]
        return subprocess.CompletedProcess(command, 0, "netsuke 1.2.3\n", "")

    binary = tmp_path / "netsuke"
    result = probe.probe_version(binary, runner=runner)

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


def test_probe_version_reports_a_timeout(tmp_path: pathlib.Path) -> None:
    """Translate a killed probe into its own refusal."""

    def runner(
        command: list[str], **_kwargs: object
    ) -> subprocess.CompletedProcess[str]:
        """Fail the launch the way a probe that ran too long would."""
        raise subprocess.TimeoutExpired(command, probe.PROBE_TIMEOUT_SECONDS)

    with pytest.raises(probe.ProbeTimeoutError):
        probe.probe_version(tmp_path / "netsuke", runner=runner)


def test_probe_version_reports_a_launch_failure(tmp_path: pathlib.Path) -> None:
    """Translate a binary that never started into its own refusal."""

    def runner(
        command: list[str], **_kwargs: object
    ) -> subprocess.CompletedProcess[str]:
        """Fail the launch the way a missing binary would."""
        raise FileNotFoundError(command[0])

    with pytest.raises(probe.ProbeLaunchError):
        probe.probe_version(tmp_path / "netsuke", runner=runner)


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

    result = probe.probe_version(binary)

    assert result.stdout == "netsuke 1.2.3\n", (
        "a diagnostic on standard error must not decide the banner's verdict"
    )


def test_probe_version_reports_output_that_is_not_utf8(
    tmp_path: pathlib.Path,
) -> None:
    """Translate undecodable standard output into its own refusal."""

    def runner(
        command: list[str], **_kwargs: object
    ) -> subprocess.CompletedProcess[str]:
        """Fail the way text-mode capture does on undecodable bytes."""
        raise UnicodeDecodeError("utf-8", b"\xff", 0, 1, "invalid start byte")

    with pytest.raises(probe.ProbeOutputError):
        probe.probe_version(tmp_path / "netsuke", runner=runner)

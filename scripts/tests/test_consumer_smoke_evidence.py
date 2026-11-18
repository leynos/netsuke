"""Hold the smoke's evidence reader to its three distinct answers.

A recorded download status is either a number ``curl`` wrote, something that is
not a status at all, or a file the host could not read. Only the last is an
operator-visible failure: the first two both mean the smoke has no status to
judge the download by. The undecodable case is easy to miss, because the read
raises ``UnicodeDecodeError`` — a ``ValueError`` — before any check of the
content runs, and an escape there would end the run with a traceback instead of
a refusal.

Every fixture is a real file in ``tmp_path``, so the cases exercise the code
that reads it rather than a patched reader, and nothing here reaches the
network.

Run via ``make test-release-publication``.
"""

import typing as typ

import consumer_smoke_evidence as evidence
import pytest

if typ.TYPE_CHECKING:
    import pathlib


def test_recorded_status_reads_the_number_curl_wrote(tmp_path: pathlib.Path) -> None:
    """Return the status a completed transfer recorded."""
    recorded = tmp_path / "crate.status"
    _ = recorded.write_text("200\n", encoding="utf-8")

    assert evidence.recorded_status(recorded) == 200, (
        "a recorded status must be read back as the integer curl wrote"
    )


def test_recorded_status_treats_an_absent_file_as_no_status(
    tmp_path: pathlib.Path,
) -> None:
    """Report no status when the download left no status file behind."""
    assert evidence.recorded_status(tmp_path / "absent.status") is None, (
        "a status file that was never written must read as no status at all"
    )


def test_recorded_status_treats_a_non_number_as_no_status(
    tmp_path: pathlib.Path,
) -> None:
    """Report no status when the file holds something other than a number."""
    recorded = tmp_path / "crate.status"
    _ = recorded.write_text("not a status\n", encoding="utf-8")

    assert evidence.recorded_status(recorded) is None, (
        "a file that is not a decimal status must not be reported as one"
    )


def test_recorded_status_treats_undecodable_bytes_as_no_status(
    tmp_path: pathlib.Path,
) -> None:
    """Report no status when the file's bytes are not UTF-8.

    A decimal status is ASCII, so bytes the reader cannot decode are not a
    status either. ``read_text`` raises ``UnicodeDecodeError``, which is a
    ``ValueError`` raised by the read rather than by the conversion, so a
    reader that only guards the conversion lets it escape as a traceback.
    """
    recorded = tmp_path / "crate.status"
    _ = recorded.write_bytes(b"\xff")

    assert evidence.recorded_status(recorded) is None, (
        "undecodable bytes must read as no status, not raise out of the reader"
    )


def test_recorded_status_keeps_an_unreadable_file_visible(
    tmp_path: pathlib.Path,
) -> None:
    """Propagate a host failure to read a status file the run listed.

    An unreadable file is a different diagnosis from a status that was never
    written, and the caller reports it differently, so the reader must not
    swallow it as absence.
    """
    unreadable = tmp_path / "crate.status"
    unreadable.mkdir()

    with pytest.raises(IsADirectoryError):
        _ = evidence.recorded_status(unreadable)

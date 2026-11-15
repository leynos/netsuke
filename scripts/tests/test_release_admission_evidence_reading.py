"""Pin the byte bound and the object kind the evidence read enforces.

:func:`~_release_admission.filesystem.read_bytes` is the first thing the gate
does with the manifest path, and it runs before anything is decoded. Two
properties have to hold there for a refusal to be possible at all: the bound is
applied to the bytes actually read, and a name that is not a regular file is
refused before a read begins.

The second property needs a name that exists but is not a document. Opening a
FIFO without a writer waits, so a reader that opens first never reaches its own
refusal and the gate can neither refuse nor finish; a device can block inside
the read for the same reason. The FIFO case therefore runs the read on a daemon
thread with a bounded wait, so a regression fails the assertion rather than
hanging the suite.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with 'cuprum==0.1.0' \
        python -m pytest scripts/tests/test_release_admission_evidence_reading.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import dataclasses
import os
import sys
import threading
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / ".github" / "scripts"))
from _release_admission import filesystem

#: How long a read that must be refused may take before it is called a wait.
#: The refusal itself is immediate; the allowance is for thread start-up only.
REFUSAL_DEADLINE_SECONDS = 5.0


@dataclasses.dataclass(slots=True)
class ReadAttempt:
    """Hold what one read attempt raised, and whether it finished at all.

    An attempt that is still blocked when the deadline passes has neither
    raised nor returned. Keeping that apart from ``error is None`` lets the case
    report the wait it is really testing for, instead of a missing exception.
    """

    error: BaseException | None = None
    is_settled: bool = False


def test_the_byte_bound_admits_exactly_the_bound_and_refuses_one_more(
    tmp_path: Path,
) -> None:
    """Read a document of exactly the bound and refuse one byte more."""
    target = tmp_path / "evidence.json"
    target.write_bytes(b"x" * 5)

    assert filesystem.read_bytes(str(target), 5) == b"x" * 5, (
        "the bound is inclusive: a document of exactly the bound is admitted"
    )
    with pytest.raises(filesystem.EvidenceTooLargeError):
        filesystem.read_bytes(str(target), 4)


def test_an_absent_document_propagates_the_absence(tmp_path: Path) -> None:
    """Let the caller tell an absent document from one that cannot be read."""
    with pytest.raises(FileNotFoundError):
        filesystem.read_bytes(str(tmp_path / "absent.json"), 64)


def test_a_directory_is_refused_as_not_a_document(tmp_path: Path) -> None:
    """Refuse a directory before reading, rather than reading it as nothing."""
    with pytest.raises(filesystem.NotRegularFileError):
        filesystem.read_bytes(str(tmp_path), 64)


@pytest.mark.skipif(
    not hasattr(os, "mkfifo"), reason="a FIFO is a POSIX filesystem object"
)
def test_a_fifo_without_a_writer_is_refused_without_waiting(tmp_path: Path) -> None:
    """Refuse a FIFO rather than wait for a writer that is not coming.

    Notes
    -----
    The precondition is the case: opening the FIFO for reading is what waits,
    so a reader that opens before it inspects the handle never reaches its own
    refusal. The bounded attempt turns that wait into a failed assertion rather
    than a hung suite.
    """
    fifo = tmp_path / "evidence.json"
    os.mkfifo(fifo)

    attempt = _read_with_deadline(str(fifo))

    assert attempt.is_settled, (
        "reading a FIFO must return; waiting for a writer never does"
    )
    assert isinstance(attempt.error, filesystem.NotRegularFileError), (
        f"expected a non-regular-file refusal, got {attempt.error!r}"
    )


def _read_with_deadline(configured: str) -> ReadAttempt:
    """Read *configured* on a daemon thread, waiting no longer than the deadline.

    A read that never returns is the defect this module exists to catch, so it
    must not hang the suite. The thread is a daemon and the wait is bounded; an
    attempt still blocked when the wait ends is reported as unsettled.

    Returns
    -------
    ReadAttempt
        What the read raised, and whether it finished before the deadline.
    """
    attempt = ReadAttempt()

    def read() -> None:
        """Perform the read and record its outcome for the waiting thread."""
        try:
            filesystem.read_bytes(configured, 64)
        except Exception as error:  # ruff: ignore[blind-except] - thread hand-off.
            attempt.error = error
        finally:
            attempt.is_settled = True

    thread = threading.Thread(target=read, daemon=True)
    thread.start()
    thread.join(REFUSAL_DEADLINE_SECONDS)
    return attempt

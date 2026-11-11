"""Map each refusal the declared-asset uploader can encounter to its status.

The uploader's exit status is a contract, not a diagnostic detail: a workflow
branches on it, and an operator retries one status and escalates another. The
table below is what keeps that contract honest. Every entry names an exception
the upload step can raise at its boundary, the status a shell should see, and
the sentence its log line leads with, so a reader can check the promise the
CLI's docstring makes against the code that keeps it.

Two properties are load-bearing, and both are tested rather than assumed:

- A subclass must precede its base, or the base's label would describe the
  wrong failure.
- Every status the CLI documents must be reachable. A status with no entry is
  a promise the workflow can never observe.

Keeping the table here rather than inside
:mod:`upload_declared_assets_cli` is what lets the CLI stay within the
repository's module-length ceiling while the table grows with the number of
distinguishable failures.

Run via ``make test-release-publication``; exercised through its only caller,
:func:`scripts.upload_declared_assets_cli.main`.
"""

import release_digest_record
import release_payload_errors
import release_payload_handoff
import release_upload_remote

EXIT_OK = 0
EXIT_CONFLICT = 1
EXIT_INPUT_UNREADABLE = 2
EXIT_UPLOAD_FAILED = 3
EXIT_OUTPUT_UNWRITABLE = 4

__all__ = [
    "EXIT_CONFLICT",
    "EXIT_INPUT_UNREADABLE",
    "EXIT_OK",
    "EXIT_OUTPUT_UNWRITABLE",
    "EXIT_UPLOAD_FAILED",
    "REFUSALS",
    "REFUSAL_LABELS",
    "refusal",
]


class _UnclassifiedRefusalError(AssertionError):
    """Report a refusal the exit-code table does not name.

    Unreachable while the caller's ``except`` tuple and the table are derived
    from one another; it guards against a future edit that breaks that tie. An
    ``AssertionError`` rather than a ``RuntimeError`` because reaching it is a
    defect in this module, not a condition of the release being published.
    """


#: Every refusal the upload step reports, most specific first, with the exit
#: code and the label its log line uses. Order matters: a subclass must precede
#: its base, or the base's label would describe the wrong failure.
REFUSAL_LABELS: tuple[tuple[type[Exception], int, str], ...] = (
    (
        release_digest_record.RecordedDigestMismatchError,
        EXIT_CONFLICT,
        "the payload disagrees with the trusted record",
    ),
    (
        release_digest_record.DigestRecordError,
        EXIT_INPUT_UNREADABLE,
        "the trusted record is unusable",
    ),
    (
        release_payload_handoff.PayloadHandoffError,
        EXIT_CONFLICT,
        "the selected payload is not the declared one",
    ),
    (
        release_payload_errors.StagedFileUnreadableError,
        EXIT_INPUT_UNREADABLE,
        "a staged path could not be inspected",
    ),
    (
        release_upload_remote.UploadError,
        EXIT_UPLOAD_FAILED,
        "the upload could not be completed",
    ),
    (OSError, EXIT_INPUT_UNREADABLE, "the payload could not be read"),
)

#: The classes above, for ``except``. Kept as a tuple literal so the handler
#: never swallows an exception it cannot classify.
REFUSALS: tuple[type[Exception], ...] = tuple(entry[0] for entry in REFUSAL_LABELS)


def refusal(error: Exception) -> tuple[int, str]:
    """Classify one refusal into its exit code and the label it logs under.

    Parameters
    ----------
    error
        The exception the upload step caught at its boundary.

    Returns
    -------
    tuple[int, str]
        The exit code and log label of the first entry that names the error's
        own class or one of its bases.

    Raises
    ------
    _UnclassifiedRefusalError
        If no entry names the error. Unreachable while :data:`REFUSALS` and
        :data:`REFUSAL_LABELS` come from one another; it turns a future break
        of that tie into a bug report rather than a mislabelled release
        failure.

    Examples
    --------
    The most specific entry wins, so the mismatch subclass reports as a
    conflict rather than as an unreadable record:

    >>> import release_digest_record
    >>> refusal(release_digest_record.RecordedDigestMismatchError("x"))
    (1, 'the payload disagrees with the trusted record')
    >>> refusal(OSError("gone"))
    (2, 'the payload could not be read')
    """
    for named, exit_code, label in REFUSAL_LABELS:
        if isinstance(error, named):
            return exit_code, label
    raise _UnclassifiedRefusalError(type(error).__name__) from error

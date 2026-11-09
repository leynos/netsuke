"""Name why an attached asset's bytes could not be established.

An asset the listing reported without a digest is downloaded and hashed, and
that second look can fail for reasons that call for different responses: a
transient transfer fault is worth a retry, a name that resolved to a symlink is
worth an investigation, and a file that could not be read at all may be a
permissions problem on the runner. Collapsing them into the single word
"unverifiable" told the operator only that the run had refused.

The categories here are deliberately few and fixed. A diagnostic that quoted
the underlying error would carry whatever a failing command wrote to standard
error -- which can include the release URL, a temporary path, or a token a
tool chose to echo -- and a plan file outlives the run that wrote it. A bounded
vocabulary keeps the diagnostic safe to publish and cheap to assert on, and
losing the free text costs little: the same categories are what an operator
branches on.

The categories describe *evidence*, not the release. None of them says the
attached bytes disagree with the trusted record; that finding is
:data:`release_reconcile_github.AssetAction.CONFLICT` and stays separate, with
its own plan key and its own remedy.

Run via ``.github/actions/upload-declared-assets``; behavioural coverage lives
in ``scripts/tests/test_release_upload_remote.py`` and
``scripts/tests/test_upload_declared_assets_status.py``.
"""

import enum

__all__ = ["DigestFailure"]


class DigestFailure(enum.Enum):
    """Name the bounded reason one attached asset's digest is unknown.

    The set is closed on purpose: a value a caller cannot anticipate is a value
    it cannot branch on, and an unbounded string would put command output into
    a retained artefact.
    """

    #: ``gh release download`` exited non-zero, so no bytes arrived.
    DOWNLOAD_FAILED = "download-failed"

    #: The download reported success but left no file at the requested name.
    MISSING_FILE = "missing-file"

    #: Something is at the name, but it is not a regular file. A symlink is
    #: refused here rather than followed, so a download cannot hand the hash
    #: the contents of a file outside the directory it wrote into.
    NOT_A_REGULAR_FILE = "not-a-regular-file"

    #: The file is a regular file and hashing it raised an ``OSError``.
    READ_FAILED = "read-failed"

    @property
    def remedy(self) -> str:
        """The one-line action this category suggests.

        Returns
        -------
        str
            What an operator should try, in the same bounded vocabulary the
            category value uses.

        Examples
        --------
        >>> DigestFailure.DOWNLOAD_FAILED.remedy
        'retry the download'
        >>> DigestFailure.NOT_A_REGULAR_FILE.remedy
        'inspect what the release attached under that name'
        """
        remedies = {
            DigestFailure.DOWNLOAD_FAILED: "retry the download",
            DigestFailure.MISSING_FILE: "check the asset is attached and complete",
            DigestFailure.NOT_A_REGULAR_FILE: (
                "inspect what the release attached under that name"
            ),
            DigestFailure.READ_FAILED: "check the download directory is readable",
        }
        return remedies[self]

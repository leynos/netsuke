"""Establish digests for attached assets GitHub would not report.

A release listing normally carries each asset's vendor digest, and the uploader
compares those against the trusted record. Some listings report an asset with no
usable digest at all, and the only remaining evidence is the bytes: fetch the
asset and hash it. That work is here rather than in
:mod:`release_upload_remote`, which keeps the listing and upload sides of the
same boundary small enough to read.

A failed download is not an error here. The name keeps ``None``, and
reconciliation reads that as unverifiable: the run still refuses to upload,
but it reports "could not tell" rather than "the release holds wrong bytes",
because those two call for different operator responses. Which *kind* of
"could not tell" it was is recorded beside the digest as a
:class:`release_upload_failures.DigestFailure`, so a refusal can say whether a
retry is worth trying without quoting anything a failing command wrote.

``names`` is an iterable read twice, so it is materialized before either pass.
A generator would be exhausted by the first and nothing would be downloaded or
hashed -- a silent no-op that reconciliation would read as every asset
differing.

Run via ``.github/actions/upload-declared-assets``; behavioural coverage lives
in ``scripts/tests/test_release_upload_remote.py``.
"""

import dataclasses
import pathlib
import stat
import tempfile
import typing as typ

import release_digest_files
from release_upload_failures import DigestFailure

if typ.TYPE_CHECKING:
    import collections.abc as cabc

    from release_upload_remote import SubprocessRunner

__all__ = ["DownloadEvidence", "downloaded_digests"]


@dataclasses.dataclass(frozen=True, slots=True)
class DownloadEvidence:
    """Hold what a digest round established and what it could not.

    Attributes
    ----------
    digests
        Asset name mapped to the digest of its downloaded bytes, or ``None``
        where none could be established.
    failures
        Asset name mapped to the bounded category naming *why* its digest is
        ``None``. Never names an asset that has a digest.

    Notes
    -----
    The two travel together because they answer one question: an asset with no
    digest is unverifiable, and an asset with no digest *and* no category is a
    gap in this module rather than a property of the release. Splitting them
    into two return values would let a caller keep one and drop the other.
    """

    digests: dict[str, str | None]
    failures: dict[str, DigestFailure]


def downloaded_digests(
    tag: str, names: cabc.Iterable[str], *, runner: SubprocessRunner
) -> DownloadEvidence:
    """Establish digests for attached assets GitHub would not report.

    Parameters
    ----------
    tag
        The validated release tag to download from.
    names
        The attached asset names whose listing carried no usable digest.
    runner
        The injected subprocess seam.

    Returns
    -------
    DownloadEvidence
        Each name's digest, or ``None`` with the category that explains it.

    Notes
    -----
    A download that fails is not an error here: the asset stays ``None``, and
    reconciliation reads that as unverifiable rather than as a match. A whole
    directory is downloaded rather than a single ``--output`` file so a name
    that also matches a glob cannot hand back another asset's bytes -- the
    exact name is what gets hashed, and its absence yields ``None``.

    ``names`` is an iterable read twice, once to drive the downloads and once
    to hash what arrived, so it is materialized first: a generator would be
    exhausted by the first pass and nothing would be downloaded or hashed,
    which reconciliation would read as every asset differing.

    Examples
    --------
    >>> from release_upload_remote import _never_called
    >>> downloaded_digests("v1", [], runner=_never_called)
    DownloadEvidence(digests={}, failures={})

    A generator names the assets as well as a list does. The recorder below
    counts what was asked for, which is the thing an exhausted generator
    changes -- and a failing download leaves the digest ``None``, so the
    mapping alone would not show it:

    >>> import subprocess
    >>> asked = []
    >>> def _record(arguments, **_keywords):
    ...     asked.append(arguments[arguments.index("--pattern") + 1])
    ...     return subprocess.CompletedProcess(arguments, 1, "", "404")
    >>> evidence = downloaded_digests("v1", (n for n in ["a.deb"]), runner=_record)
    >>> evidence.digests, evidence.failures["a.deb"].value
    ({'a.deb': None}, 'download-failed')
    >>> asked
    ['a.deb']
    """
    ordered = sorted(names)
    with tempfile.TemporaryDirectory() as temporary:
        directory = pathlib.Path(temporary)
        digests: dict[str, str | None] = {}
        failures: dict[str, DigestFailure] = {}
        for name in ordered:
            returncode = _download(tag, name, directory, runner=runner)
            digest, failure = _established(directory / name, returncode=returncode)
            digests[name] = digest
            if failure is not None:
                failures[name] = failure
        return DownloadEvidence(digests=digests, failures=failures)


def _download(
    tag: str, name: str, directory: pathlib.Path, *, runner: SubprocessRunner
) -> int:
    """Fetch one attached asset into ``directory``, tolerating failure.

    Parameters
    ----------
    tag
        The validated release tag to download from.
    name
        The exact attached asset name to fetch.
    directory
        The directory the asset is written into.
    runner
        The injected subprocess seam.

    Returns
    -------
    int
        The status ``gh`` exited with.

    Notes
    -----
    ``required=False`` keeps a failed download from raising: the status is
    returned instead, so the caller can record *that* the transfer failed
    separately from what the directory then contains. A download that exits
    zero but writes nothing and one that exits non-zero are different findings
    -- the first is a broken transfer or a name that resolved to nothing, the
    second is a refused one.
    """
    from release_upload_remote import gh

    completed = gh(
        ("release", "download", tag, "--pattern", name, "--dir", str(directory)),
        runner=runner,
        required=False,
    )
    return completed.returncode


def _established(
    path: pathlib.Path, *, returncode: int
) -> tuple[str | None, DigestFailure | None]:
    """Hash ``path`` when it is a readable regular file, or explain why not.

    Parameters
    ----------
    path
        The downloaded asset, under the exact name that was asked for.
    returncode
        The status the download exited with.

    Returns
    -------
    tuple[str | None, DigestFailure | None]
        The digest and ``None``, or ``None`` and the category that explains it.

    Notes
    -----
    The download's own status is checked first, so a refused transfer is
    reported as refused even when a stale file of that name happens to be
    present -- hashing it would answer a question about the release with bytes
    the release never sent.

    The remaining checks are ordered from the least to the most specific
    observation. An absence is a missing file; something that exists but is
    not a regular file is refused rather than followed, because following a
    symlink would hash whatever it points at; and a regular file that cannot
    be read is a local fault with no bearing on the release.

    Examples
    --------
    >>> from pathlib import Path
    >>> _established(Path("/nonexistent/asset"), returncode=0)
    (None, <DigestFailure.MISSING_FILE: 'missing-file'>)
    >>> _established(Path("/nonexistent/asset"), returncode=1)[1].value
    'download-failed'
    """
    if returncode != 0:
        return None, DigestFailure.DOWNLOAD_FAILED
    try:
        status = path.lstat().st_mode
    except FileNotFoundError:
        return None, DigestFailure.MISSING_FILE
    except OSError:
        return None, DigestFailure.READ_FAILED
    if not stat.S_ISREG(status):
        return None, DigestFailure.NOT_A_REGULAR_FILE
    try:
        return release_digest_files.digest_of_file(path), None
    except OSError:
        return None, DigestFailure.READ_FAILED

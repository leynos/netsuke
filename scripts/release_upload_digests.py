"""Establish digests for attached assets GitHub would not report.

A release listing normally carries each asset's vendor digest, and the uploader
compares those against the trusted record. Some listings report an asset with no
usable digest at all, and the only remaining evidence is the bytes: fetch the
asset and hash it. That work is here rather than in
:mod:`release_upload_remote`, which keeps the listing and upload sides of the
same boundary small enough to read.

A failed download is not an error. The name keeps ``None``, and reconciliation
reads that as a conflict rather than as a match on no evidence.

``names`` is an iterable read twice, so it is materialized before either pass.
A generator would be exhausted by the first and nothing would be downloaded or
hashed -- a silent no-op that reconciliation would read as every asset
differing.

Run via ``.github/actions/upload-declared-assets``; behavioural coverage lives
in ``scripts/tests/test_release_upload_remote.py``.
"""

import pathlib
import stat
import tempfile
import typing as typ

import release_digest_files

if typ.TYPE_CHECKING:
    import collections.abc as cabc

    from release_upload_remote import SubprocessRunner

__all__ = ["downloaded_digests"]


def downloaded_digests(
    tag: str, names: cabc.Iterable[str], *, runner: SubprocessRunner
) -> dict[str, str | None]:
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
    dict[str, str | None]
        Name mapped to the digest of the downloaded bytes, or ``None`` when
        the download or the hash failed.

    Notes
    -----
    A download that fails is not an error here: the asset stays ``None``, and
    reconciliation reads that as a conflict rather than as a match. A whole
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
    {}

    A generator names the assets as well as a list does. The recorder below
    counts what was asked for, which is the thing an exhausted generator
    changes -- and a failing download leaves the digest ``None``, so the
    mapping alone would not show it:

    >>> import subprocess
    >>> asked = []
    >>> def _record(arguments, **_keywords):
    ...     asked.append(arguments[arguments.index("--pattern") + 1])
    ...     return subprocess.CompletedProcess(arguments, 1, "", "404")
    >>> downloaded_digests("v1", (name for name in ["a.deb"]), runner=_record)
    {'a.deb': None}
    >>> asked
    ['a.deb']
    """
    ordered = sorted(names)
    with tempfile.TemporaryDirectory() as temporary:
        directory = pathlib.Path(temporary)
        for name in ordered:
            _download(tag, name, directory, runner=runner)
        return {name: _digest_if_present(directory / name) for name in ordered}


def _download(
    tag: str, name: str, directory: pathlib.Path, *, runner: SubprocessRunner
) -> None:
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

    Notes
    -----
    ``required=False`` keeps a failed download from raising: it leaves the name
    without a digest, and reconciliation turns that into a conflict rather than
    into a match on no evidence.
    """
    from release_upload_remote import gh

    gh(
        ("release", "download", tag, "--pattern", name, "--dir", str(directory)),
        runner=runner,
        required=False,
    )


def _digest_if_present(path: pathlib.Path) -> str | None:
    """Hash ``path`` when it is a regular file, or answer ``None``.

    Parameters
    ----------
    path
        A candidate downloaded asset.

    Returns
    -------
    str | None
        The lowercase SHA-256 of the bytes, or ``None`` when the path is not a
        readable regular file.

    Examples
    --------
    >>> _digest_if_present(pathlib.Path("/nonexistent/asset"))
    """
    try:
        if not stat.S_ISREG(path.lstat().st_mode):
            return None
        return release_digest_files.digest_of_file(path)
    except OSError:
        return None

"""Observe and copy the staged release tree at the filesystem boundary.

:mod:`release_payload_selection` decides from names alone; this module turns a
real directory into the facts that decision needs and, once it has approved a
selection, copies exactly those files into a fresh directory. Every read and
every write the payload selection performs lives here.

Three properties make the copy trustworthy:

1. Nothing is copied until selection and digest verification have both
   finished. A refusal therefore leaves no partial payload for a later step to
   mistake for a complete one.
2. The output directory is created fresh and must not already exist, so a
   stale directory from an earlier attempt cannot be mistaken for this run's
   payload.
3. Every copy is made from a verified regular file under a verified regular
   ancestor. A symlinked leaf or a symlinked producer directory is refused
   rather than followed, because a link is not a claim this run can make.

Run via ``scripts/select_release_payload_cli.py``; behavioural coverage lives
in ``scripts/tests/test_select_release_payload_cli.py``.
"""

import json
import pathlib
import shutil
import stat
import typing as typ

import hoist_binstall_discovery
import release_digest_files
import release_digest_record
import release_digest_record_files
import release_github_assets_adapter
import release_producer_provenance
from record_producer_provenance_record import RECORD_NAME
from release_payload_errors import (
    PayloadError,
    StagedFileMissingError,
    StagedFileUnreadableError,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

    import release_payload_selection

__all__ = [
    "PAYLOAD_DIRECTORY",
    "RECORD_NAME",
    "PayloadHandoffError",
    "copy_verified",
    "install_payload",
    "observe_payload",
    "read_provenances",
    "record_locations",
    "require_payload_matches",
    "require_regular",
]

#: Directory name the selected payload is copied into, below the output root.
PAYLOAD_DIRECTORY = "payload"


class PayloadHandoffError(PayloadError):
    """Report a payload directory that is not the set the record declares.

    The selection stage is the only writer of this directory, so a mismatch
    means the hand-off -- not the release -- is wrong, and uploading any part
    of it would publish bytes nobody selected.
    """


class _HandoffDisagreementError(PayloadHandoffError):
    """Report exactly which declared names the payload omits and adds."""

    def __init__(self, missing: list[str], extra: list[str]) -> None:
        """Name both sides of the disagreement in one operator-readable line."""
        super().__init__(f"payload omits {missing!r} and adds {extra!r}")


def observe_payload(dist_dir: pathlib.Path) -> dict[str, list[pathlib.Path]]:
    """Index the staged tree by the name each regular file publishes under.

    Returns
    -------
    dict[str, list[pathlib.Path]]
        Published name mapped to every regular file claiming it, in walk order.

    Notes
    -----
    The producer records are *not* indexed: they are not release assets, and a
    record left in a producer directory must never be uploaded as one.

    Symlinks and other non-regular leaves are excluded here and detected again
    by :func:`require_regular` when they are copied. Excluding them keeps a
    dangling link from being indexed as a candidate; refusing them at copy
    time makes a symlinked *ancestor* visible.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     root = Path(tmp)
    ...     _ = (root / "netsuke.deb").write_bytes(b"deb")
    ...     sorted(observe_payload(root))
    ['netsuke.deb']
    """
    indexed: dict[str, list[pathlib.Path]] = {}
    for path in hoist_binstall_discovery.walk_files(dist_dir):
        if path.name == RECORD_NAME or not _has_mode(path, stat.S_ISREG):
            continue
        published = release_github_assets_adapter.published_asset_name(path.name)
        indexed.setdefault(published, []).append(path)
    return indexed


def read_provenances(
    dist_dir: pathlib.Path,
) -> list[release_producer_provenance.ProducerProvenance]:
    """Read and validate every producer record found below the staged tree.

    Returns
    -------
    list[release_producer_provenance.ProducerProvenance]
        Each validated record, sorted by target so refusals are stable.

    Notes
    -----
    Every record is read and validated before any is judged against the
    release. A malformed record is reported with the record reader's own
    diagnosis rather than as a stale candidate, because those send an operator
    to different files.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     read_provenances(Path(tmp))
    []
    """
    records = [
        _read_provenance(path)
        for path in hoist_binstall_discovery.walk_files(dist_dir)
        if path.name == RECORD_NAME
    ]
    return sorted(records, key=lambda record: record.target)


def record_locations(dist_dir: pathlib.Path) -> dict[pathlib.Path, str]:
    """Map every producer record's path to the target that record names.

    Returns
    -------
    dict[pathlib.Path, str]
        Record path mapped to its validated target key.

    Notes
    -----
    The record is read here rather than by the caller because a record's
    *position* is what attributes the artefacts beside it, and only the reader
    knows which subtree a given record came from. A record that cannot be read
    or validated surfaces exactly what :func:`read_provenances` does, so a
    caller that needs both can call this first and lose no diagnosis.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     record_locations(Path(tmp))
    {}
    """
    return {
        path: _read_provenance(path).target
        for path in hoist_binstall_discovery.walk_files(dist_dir)
        if path.name == RECORD_NAME
    }


def _read_provenance(
    path: pathlib.Path,
) -> release_producer_provenance.ProducerProvenance:
    """Read and validate one producer record, or propagate its diagnosis.

    Returns
    -------
    release_producer_provenance.ProducerProvenance
        The validated record.

    Notes
    -----
    A record that cannot be read surfaces ``OSError``; unusable JSON surfaces
    ``ValueError``; and a record of the wrong shape surfaces the reader's own
    ``ProvenanceError``.
    """
    decoded = json.loads(path.read_text(encoding="utf-8"))
    return release_producer_provenance.validate_provenance(decoded)


def install_payload[Source: pathlib.Path](
    selection: release_payload_selection.PayloadSelection[Source],
    output_dir: pathlib.Path,
    digest_record: pathlib.Path,
    computed: cabc.Mapping[str, str],
) -> None:
    """Copy the payload into a fresh directory and write the trusted record.

    Parameters
    ----------
    selection
        The approved selection, holding one source path per declared name.
    output_dir
        Directory to create; it must not already exist.
    digest_record
        Where to write the record of the bytes just copied.
    computed
        Published name mapped to the digest of the staged bytes.

    Raises
    ------
    OSError
        If a copy or the record write fails.
    PayloadError
        If the copy is refused before it starts, re-raised after cleanup.

    Notes
    -----
    A selected source that is not a regular file below regular directories
    surfaces ``StagedFileMissingError`` from :func:`copy_verified`. Both
    failure families remove the partial directory before the error
    propagates, so a caller that catches and retries cannot mistake what is
    left for a complete payload.

    An existing output directory surfaces ``FileExistsError`` from
    :func:`pathlib.Path.mkdir` rather than being merged into, so a stale
    payload cannot be mistaken for this run's.
    """
    output_dir.mkdir(parents=True)
    payload = output_dir / PAYLOAD_DIRECTORY
    try:
        payload.mkdir()
        for name in selection.declared:
            copy_verified(selection.sources[name], payload / name)
        release_digest_record_files.write_digest_record(computed, digest_record)
    except OSError, PayloadError:
        # A refused copy leaves exactly as much of a payload behind as a failed
        # one, so both are cleaned up: a partial directory that survives would
        # be published as though it were the whole release.
        shutil.rmtree(output_dir, ignore_errors=True)
        raise


def require_payload_matches(
    payload_dir: pathlib.Path, expected: cabc.Mapping[str, str]
) -> None:
    """Require the payload directory to hold exactly the declared bytes.

    Parameters
    ----------
    payload_dir
        The directory the selection stage installed.
    expected
        Published name mapped to the digest the trusted record demands.

    Raises
    ------
    _HandoffDisagreementError
        If the directory names any other set of files.

    Notes
    -----
    A stored file whose bytes differ surfaces
    ``release_digest_record.RecordedDigestMismatchError`` from
    :func:`release_digest_record.require_downloads_match_record`.

    Selection verified these bytes before copying them; this checks the
    *hand-off*, which is a different claim. A job that restored the wrong
    artefact, or a step that appended a file, is caught here rather than after
    the upload.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     root = Path(tmp)
    ...     _ = (root / "a.deb").write_bytes(b"deb")
    ...     digest = release_digest_files.digest_of_file(root / "a.deb")
    ...     require_payload_matches(root, {"a.deb": digest})
    ...     try:
    ...         require_payload_matches(root, {})
    ...     except PayloadHandoffError as refusal:
    ...         print("adds" in str(refusal))
    True
    """
    present = observe_payload(payload_dir)
    missing = sorted(set(expected) - set(present))
    extra = sorted(set(present) - set(expected))
    if missing or extra:
        raise _HandoffDisagreementError(missing, extra)
    computed = {
        name: release_digest_files.digest_of_file(present[name][0])
        for name in sorted(expected)
    }
    release_digest_record.require_downloads_match_record(
        computed,
        trusted=release_digest_record.DigestRecord.from_mapping(expected),
    )


def copy_verified(source: pathlib.Path, destination: pathlib.Path) -> None:
    """Copy ``source`` after proving it and its ancestors are regular files.

    Notes
    -----
    A path that is not a regular file below regular directories surfaces
    :class:`release_payload_errors.StagedFileMissingError` from
    :func:`require_regular`, and a copy failure propagates ``OSError``.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     root = Path(tmp)
    ...     source = root / "asset.deb"
    ...     _ = source.write_bytes(b"deb")
    ...     copy_verified(source, root / "out.deb")
    ...     (root / "out.deb").read_bytes()
    b'deb'
    """
    require_regular(source)
    _ = shutil.copyfile(source, destination)


def require_regular(path: pathlib.Path) -> pathlib.Path:
    """Return ``path`` when it and every ancestor below the root are regular.

    Returns
    -------
    pathlib.Path
        The same path, once proven regular.

    Raises
    ------
    StagedFileMissingError
        If the leaf is absent or not a regular file, or any ancestor is not a
        directory.

    Notes
    -----
    ``StagedFileUnreadableError`` reaches the caller from :func:`_has_mode`
    when ``lstat`` refuses to describe the leaf or an ancestor, so it is
    described here rather than in ``Raises``.

    ``lstat`` is used throughout so a link is judged as itself: following a
    symlinked producer directory would let the payload hold bytes from outside
    the downloaded tree, which is the substitution the trusted record exists to
    catch. A vanished source is ``StagedFileMissingError`` rather than
    ``FileNotFoundError``, because from the caller's side a source that
    disappeared between observation and copy is the same refusal as one that
    was never regular, and the CLI maps the typed family to one exit code. A
    source the operating system will not describe keeps its own refusal: that
    is an environment fault, not a staging one.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     root = Path(tmp)
    ...     real = root / "real.deb"
    ...     _ = real.write_bytes(b"deb")
    ...     (root / "link.deb").symlink_to(real)
    ...     try:
    ...         require_regular(root / "link.deb")
    ...     except StagedFileMissingError as refusal:
    ...         print("regular file" in str(refusal))
    True
    """
    if not _has_mode(path, stat.S_ISREG):
        raise StagedFileMissingError(path, expected="a regular file")
    for ancestor in path.parents:
        if not _has_mode(ancestor, stat.S_ISDIR):
            raise StagedFileMissingError(ancestor, expected="a directory")
    return path


def _has_mode(path: pathlib.Path, predicate: cabc.Callable[[int], bool]) -> bool:
    """Report whether ``path``'s own mode satisfies ``predicate``.

    Returns
    -------
    bool
        ``True`` when the entry exists and its ``lstat`` mode satisfies
        ``predicate``; ``False`` when it is genuinely absent or of another
        type.

    Raises
    ------
    StagedFileUnreadableError
        If ``lstat`` fails for any reason other than absence, such as a
        permission refusal or an I/O fault.

    Notes
    -----
    Absence and unreadability are answered differently on purpose: a path the
    operating system will not describe is not evidence the file is not there,
    and calling it absent would send an operator to fix a staging step that did
    the right thing. Only ``ENOENT`` and ``ENOTDIR`` mean "no such entry"; the
    rest is an environment fault, and ``walk_files`` reaches this predicate
    too, so an unreadable entry stops selection rather than being skipped.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     _ = (Path(tmp) / "a").write_bytes(b"x")
    ...     _has_mode(Path(tmp) / "a", stat.S_ISREG)
    True
    >>> _has_mode(pathlib.Path("/nonexistent/a"), stat.S_ISREG)
    False
    """
    try:
        return predicate(path.lstat().st_mode)
    except FileNotFoundError, NotADirectoryError:
        return False
    except OSError as error:
        raise StagedFileUnreadableError(path, error) from error

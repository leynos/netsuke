"""Observe and copy the staged release tree at the filesystem boundary.

:mod:`release_payload_selection` decides from names alone; this module turns a
real directory into the facts that decision needs and, once it has approved a
selection, copies exactly those files into a fresh directory. Every read and
every write the payload selection performs is reached from here.

Three properties make the copy trustworthy:

1. Nothing is copied until selection and digest verification have both
   finished. A refusal therefore leaves no partial payload for a later step to
   mistake for a complete one.
2. The output directory is created fresh and must not already exist, so a
   stale directory from an earlier attempt cannot be mistaken for this run's
   payload.
3. Every copy is made from a verified regular file under a verified regular
   ancestor. A symlinked leaf or a symlinked producer directory is refused
   rather than followed, because a link is not a claim this run can make. The
   proof itself lives in :mod:`release_payload_regular_files`; this module
   re-exports it so a caller keeps importing one name.

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
from release_payload_errors import PayloadError

# Re-exported because the tree adapter is the one name a caller imports; the
# regularity proof it copies through is an implementation detail of that
# adapter, not a second entry point.
from release_payload_regular_files import (
    _has_mode,
    copy_verified,
    require_regular,
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

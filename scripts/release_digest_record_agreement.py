"""Require the two records publication writes to describe the same bytes.

Publication writes two digest records, and the difference between them is the
point. The selection record is written from the staged sources selection
verified, before those sources are copied; the declaration record is written
independently from the staged tree. Neither alone proves the copy. A record
compared only with a later hash of the copied tree would accept bytes that were
swapped between the two hashes, so :func:`require_records_agree` makes the two
records answer for one another before either is used against the payload.

The record type, its validation, and its comparisons against a declaration or
against downloaded bytes belong to :mod:`release_digest_record`. This module
holds the one comparison that is about neither: whether the two records the
publication stages wrote agree with each other. Splitting it keeps each file
below the repository's 400-line cap and gives the cross-record claim its own
place to state what a disagreement means.

Nothing here reads a file, reaches the network, or holds state.

Run via :mod:`release_upload_run`; behavioural coverage lives in
``scripts/tests/test_release_digest_record_agreement.py``.
"""

import typing as typ

import release_digest
import release_digest_record

if typ.TYPE_CHECKING:
    import collections.abc as cabc

__all__ = ["require_records_agree"]


def require_records_agree(
    selection: cabc.Mapping[str, str], *, declaration: cabc.Mapping[str, str]
) -> None:
    """Require the selection and declaration records to name the same bytes.

    Parameters
    ----------
    selection
        The record the selection stage wrote from the staged sources it
        verified, before copying them.
    declaration
        The record the declaration stage wrote independently from the staged
        tree.

    Raises
    ------
    release_digest_record.RecordedDigestMismatchError
        If either record omits an asset the other names, or the two disagree
        about any asset's digest.

    Notes
    -----
    Both directions are checked, so a record that names an extra asset is as
    much a refusal as one that omits a declared asset. Each is computed from
    the same tree by a different step, and a disagreement means at least one
    of them does not describe the release.

    Examples
    --------
    >>> require_records_agree({"a": "ab"}, declaration={"a": "ab"})
    >>> require_records_agree({"a": "ab"}, declaration={"a": "cd"})
    ... # doctest: +ELLIPSIS
    Traceback (most recent call last):
        ...
    release_digest_record.RecordedDigestMismatchError: the selection and ...
    """
    disagreeing = sorted(
        name
        for name in set(selection) | set(declaration)
        if name not in selection
        or name not in declaration
        or not release_digest.constant_time_equal(selection[name], declaration[name])
    )
    if not disagreeing:
        return
    msg = (
        "the selection and declaration records describe different bytes: "
        f"{disagreeing!r}"
    )
    raise release_digest_record.RecordedDigestMismatchError(msg)

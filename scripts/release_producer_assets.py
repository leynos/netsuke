"""Judge the asset map one producer record claims for its target.

A producer record names artefacts by the name a release publishes them under,
so every entry carries two independent risks: the name may describe something
other than one bare file in the staging directory, and the digest may not be a
SHA-256 of the bytes at all. Both are refused here, and neither is inferred
from the other.

This module owns only that map. The record around it -- schema, commit, run
identity, target, and platform -- is :mod:`release_producer_provenance`'s, and
the cross-record checks belong to
:mod:`release_producer_provenance_checks`. Keeping the map separate is what
lets the record reader stay under the repository's file-size ceiling without
either module reaching into the other's concerns.

Names are judged before digests, because the digest refusal quotes the name and
a value that is not a string cannot be quoted as one.

Nothing here reads a file, parses JSON, or hashes bytes: the values arrive
already decoded, and the refusals live in
:mod:`release_producer_provenance_errors`. Nor does anything here test the
map's *type*: that the record's ``assets`` member is an object rather than a
list or a string is a wire-shape question, and
:func:`release_producer_provenance_decode.decode_provenance` answers it before
this module is reached.

Run via :func:`release_producer_provenance.validated_provenance`; behavioural
coverage lives in ``scripts/tests/test_release_producer_provenance.py``.
"""

import re
import typing as typ

import release_asset_names
import release_digest
from release_producer_provenance_errors import (
    EmptyAssetsError,
    InvalidDigestError,
    UnsafeAssetNameError,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

__all__ = ["validated_assets"]

#: One SHA-256 digest, lowercase hexadecimal. The producer writes the digest
#: itself rather than reading it from a sidecar, so no case folding applies.
_DIGEST = re.compile(rf"\A[0-9a-f]{{{release_digest.DIGEST_HEX_LENGTH}}}\Z")


def validated_assets(
    declared: cabc.Mapping[str, object], *, target: str
) -> dict[str, str]:
    """Return a validated name-to-digest mapping for one producer.

    Returns
    -------
    dict[str, str]
        Published asset name mapped to its lowercase digest.

    Raises
    ------
    EmptyAssetsError
        If the mapping holds no entry.

    Notes
    -----
    The mapping's *shape* -- that the record's ``assets`` member is an object
    rather than a list or a string -- is :func:`decode_provenance`'s, so this
    function is never handed a value of another type and does not test for
    one. Every refusal here is about a claim: a name that is not a bare file
    name, a digest that is not hexadecimal, or an empty set.

    ``UnsafeAssetNameError`` and ``InvalidDigestError`` surface from the two
    helpers below.

    Examples
    --------
    >>> validated_assets({"a.deb": "b" * 64}, target="linux-x86_64")
    {'a.deb': 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'}
    """
    if not declared:
        raise EmptyAssetsError(target)
    return {
        _validated_name(name, target=target): _validated_digest(
            name, digest, target=target
        )
        for name, digest in declared.items()
    }


def _validated_name(name: object, *, target: str) -> str:
    """Return one claimed asset name, refusing anything but a bare file name.

    Returns
    -------
    str
        The name, once it is a string a release may publish.

    Raises
    ------
    UnsafeAssetNameError
        If the name is not a string, or names anything but one bare file.

    Notes
    -----
    The name is judged first because the digest's refusal quotes it, and a
    value that is not a string cannot be quoted as one.
    """
    if not isinstance(name, str) or not release_asset_names.is_bare_file_name(name):
        raise UnsafeAssetNameError(target, str(name))
    return name


def _validated_digest(name: str, digest: object, *, target: str) -> str:
    """Return the digest claimed for ``name``, refusing a malformed one.

    Returns
    -------
    str
        The lowercase SHA-256 digest the producer recorded.

    Raises
    ------
    InvalidDigestError
        If the digest is not a string of the expected hexadecimal length.
    """
    if not isinstance(digest, str) or not _DIGEST.match(digest):
        raise InvalidDigestError(target, name)
    return digest

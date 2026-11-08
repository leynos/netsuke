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
:mod:`release_producer_provenance_errors`.

Run via :func:`release_producer_provenance.validate_provenance`; behavioural
coverage lives in ``scripts/tests/test_release_producer_provenance.py``.
"""

import re

import release_asset_names
import release_digest
from release_producer_provenance_errors import (
    EmptyAssetsError,
    InvalidDigestError,
    MalformedRecordError,
    UnsafeAssetNameError,
)

__all__ = ["validated_assets"]

#: One SHA-256 digest, lowercase hexadecimal. The producer writes the digest
#: itself rather than reading it from a sidecar, so no case folding applies.
_DIGEST = re.compile(rf"\A[0-9a-f]{{{release_digest.DIGEST_HEX_LENGTH}}}\Z")


def validated_assets(declared: object, *, target: str) -> dict[str, str]:
    """Return a validated name-to-digest mapping for one producer.

    Returns
    -------
    dict[str, str]
        Published asset name mapped to its lowercase digest.

    Raises
    ------
    MalformedRecordError
        If ``declared`` is not a mapping.
    EmptyAssetsError
        If it holds no entry.

    Notes
    -----
    ``UnsafeAssetNameError`` and ``InvalidDigestError`` surface from the two
    helpers below.

    Examples
    --------
    >>> validated_assets({"a.deb": "b" * 64}, target="linux-x86_64")
    {'a.deb': 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'}
    """
    if not isinstance(declared, dict):
        msg = (
            f"the producer record assets must be a mapping, got "
            f"{type(declared).__name__}"
        )
        raise MalformedRecordError(msg)
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

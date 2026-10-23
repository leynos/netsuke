"""Read, write and enforce the trusted digest record for a release.

A release has to be able to say, before anything is uploaded, which bytes it
intends to publish. That answer is a record mapping each declared asset name to
the SHA-256 of the bytes the build produced. This module owns the record's
shape and the comparisons made against it, so the producer and both consumers
cannot disagree about what a usable record is.

The record is ``{"assets": {name: digest}}``. It is written by
``release_declaration_cli --digest-record`` in the same run that declares the
names, read by ``verify_release_inventory_cli`` and
``finalize_github_release_cli``, and never derived from downloaded bytes.

That last property is the whole point, and it is what an earlier flow lacked. A
record derived from the downloaded assets proves only that GitHub is
self-consistent: replace an operating-system package, or replace an archive
together with the sidecar beside it, and the download still agrees with itself
perfectly. Comparing each downloaded asset against this record is what makes
either substitution visible.

Nothing here reaches the network, and nothing here decides. A record that
cannot be read, does not map names to digests, or does not cover the
declaration is refused rather than interpreted, because every consumer reads
this record as permission to act on a release.

Run via ``scripts/release_declaration_cli.py`` in the release workflow;
behavioural coverage lives in ``scripts/tests/test_release_digest_record.py``.
"""

import json
import re
import typing as typ

import release_digest

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

#: One hexadecimal SHA-256 digest, in either case. Matched rather than parsed
#: with :func:`int` because ``int`` accepts signs and underscores, so a value
#: it parses is not necessarily a digest.
_DIGEST_TEXT = re.compile(rf"\A[0-9a-fA-F]{{{release_digest.DIGEST_HEX_LENGTH}}}\Z")

#: The member of the record's JSON object holding the name-to-digest mapping.
ASSETS_MEMBER = "assets"


class DigestRecordError(ValueError):
    """Report a digest record that cannot support a verification decision."""


class RecordedDigestMismatchError(
    DigestRecordError, release_digest.DigestMismatchError
):
    """Report a downloaded asset whose bytes differ from the trusted record.

    Derived from :class:`release_digest.DigestMismatchError` as well, so a
    caller guarding either the record hierarchy or the digest hierarchy catches
    it without a second clause. :mod:`scripts.verify_crates_io_bytes` builds
    its mismatch error the same way, for the same reason.
    """


def write_digest_record(
    digests: cabc.Mapping[str, str], destination: pathlib.Path
) -> None:
    """Write the digest record in the shape every consumer reads.

    Parameters
    ----------
    digests
        Published name to the digest its staged bytes hash to.
    destination
        File the record is written to.

    Notes
    -----
    Sorted and indented, so two runs over the same tree produce the same file
    and a reader comparing records compares digests rather than ordering. The
    trailing newline matches every other record this branch writes.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     out = Path(tmp) / "record.json"
    ...     write_digest_record({"b": "cd", "a": "ab"}, out)
    ...     print(out.read_text(encoding="utf-8"), end="")
    {
      "assets": {
        "a": "ab",
        "b": "cd"
      }
    }
    """
    destination.write_text(
        json.dumps({ASSETS_MEMBER: dict(digests)}, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def read_digest_record(source: pathlib.Path) -> dict[str, str]:
    """Read a digest record and return its name-to-digest mapping.

    Parameters
    ----------
    source
        Path to a record written by :func:`write_digest_record`.

    Returns
    -------
    dict[str, str]
        Published name to lowercase hexadecimal digest.

    Notes
    -----
    Every refusal is a :class:`DigestRecordError`, raised by the three helpers
    below and propagated unchanged: the file cannot be read, is not a JSON
    object, carries no ``assets`` member, carries an empty or non-mapping one,
    or maps something that is not an asset name to something that is not a
    hexadecimal SHA-256 digest. The hierarchy is named here rather than in a
    ``Raises`` section because nothing in this function raises it; Ruff's
    DOC501 reserves that section for exceptions the function itself raises.

    Every refusal here exists because the alternative is a vacuous success. A
    ``.get("assets", {})`` default turns a missing field into an empty mapping,
    and an empty mapping matches nothing and so refuses nothing: every consumer
    of this record reads it as permission to act on a release. Absence of
    evidence therefore has to be an error.

    A digest that is not hexadecimal is refused rather than carried forward.
    Compared as a string it could never equal a computed digest, so the
    consumer would report the download as wrong when the record is what is
    malformed — sending an operator to the upload for a fault in a file the
    build wrote.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     record = Path(tmp) / "record.json"
    ...     write_digest_record({"netsuke.tar.gz": "AB" * 32}, record)
    ...     read_digest_record(record)["netsuke.tar.gz"] == "ab" * 32
    True
    """
    record = _record_object(source)
    declared = _declared_assets(record, source)
    return _validated_entries(declared, source)


def _record_object(source: pathlib.Path) -> dict[str, object]:
    """Return the record's top-level JSON object.

    Returns
    -------
    dict[str, object]
        The object the record holds.

    Raises
    ------
    DigestRecordError
        If the file cannot be read or does not hold a JSON object.
    """
    try:
        record = json.loads(source.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        msg = f"the digest record {source} could not be read: {error}"
        raise DigestRecordError(msg) from error
    if not isinstance(record, dict):
        msg = f"the digest record must be a JSON object, got {type(record).__name__}"
        raise DigestRecordError(msg)
    return record


def _declared_assets(
    record: dict[str, object], source: pathlib.Path
) -> dict[object, object]:
    """Return the record's non-empty ``assets`` mapping.

    Returns
    -------
    dict[object, object]
        The mapping as parsed, before its entries are validated.

    Raises
    ------
    DigestRecordError
        If ``assets`` is absent, is not a mapping, or is empty.
    """
    declared = record.get(ASSETS_MEMBER)
    if not isinstance(declared, dict):
        msg = (
            f"the digest record {source} must carry an '{ASSETS_MEMBER}' "
            f"mapping, got {type(declared).__name__}"
        )
        raise DigestRecordError(msg)
    if not declared:
        msg = (
            f"the digest record {source} declares no assets, "
            "so there is nothing to verify"
        )
        raise DigestRecordError(msg)
    return declared


def _validated_entries(
    declared: cabc.Mapping[object, object], source: pathlib.Path
) -> dict[str, str]:
    """Return the record's entries as name-to-digest strings.

    Returns
    -------
    dict[str, str]
        The mapping with each digest lowercased.

    Raises
    ------
    DigestRecordError
        If any name or digest is not a string, or any digest is not one
        hexadecimal SHA-256.
    """
    entries: dict[str, str] = {}
    for name, digest in declared.items():
        if not isinstance(name, str) or not isinstance(digest, str):
            msg = (
                f"the digest record {source} must map asset names to digests as strings"
            )
            raise DigestRecordError(msg)
        if not _DIGEST_TEXT.match(digest):
            msg = (
                f"the digest record {source} holds a non-digest for {name!r}: "
                f"{digest!r}"
            )
            raise DigestRecordError(msg)
        entries[name] = digest.lower()
    return entries


def require_record_matches_declaration(
    digests: cabc.Mapping[str, str], *, expected: cabc.Iterable[str]
) -> None:
    """Require the record to cover exactly the assets the release declares.

    Parameters
    ----------
    digests
        Published name to recorded digest.
    expected
        Asset names the release declares.

    Raises
    ------
    DigestRecordError
        If the declaration names no asset, or the two sets differ in either
        direction.

    Notes
    -----
    Both directions are refused. A record that omits a declared asset would let
    that asset through uncompared, which is the failure the record exists to
    prevent. A record that adds one names an asset this release never declared,
    so its digest describes bytes nobody vouched for.

    An empty declaration is refused here rather than left to the inventory
    check. Set equality holds vacuously over two empty sets, so an empty
    declaration paired with an empty record would pass every comparison and
    report a release with nothing in it as verified.

    Examples
    --------
    >>> require_record_matches_declaration({"a": "ab"}, expected=["a"])
    >>> require_record_matches_declaration({"a": "ab"}, expected=["a", "b"])
    Traceback (most recent call last):
        ...
    release_digest_record.DigestRecordError: the digest record and the \
declaration disagree: record omits ['b'], record adds []
    """
    declared = set(expected)
    if not declared:
        msg = "the declaration names no asset, so there is nothing to verify"
        raise DigestRecordError(msg)
    recorded = set(digests)
    missing = sorted(declared - recorded)
    extra = sorted(recorded - declared)
    if not missing and not extra:
        return
    msg = (
        "the digest record and the declaration disagree: "
        f"record omits {missing!r}, record adds {extra!r}"
    )
    raise DigestRecordError(msg)


def require_downloads_match_record(
    computed: cabc.Mapping[str, str], *, trusted: cabc.Mapping[str, str]
) -> None:
    """Require every downloaded asset's bytes to hash to its recorded digest.

    Parameters
    ----------
    computed
        Published name to the digest actually computed over the download.
    trusted
        Published name to the digest the build recorded before upload.

    Raises
    ------
    RecordedDigestMismatchError
        If any name is recorded but absent from ``computed``, or any digest
        differs from its recorded value.

    Notes
    -----
    The comparison runs over the *trusted* names, not the computed ones. That
    direction matters: it reports a recorded asset that never arrived, which
    iterating the downloads cannot see, and the exact-set check has already
    refused any download the record does not name.

    The digests are compared with :func:`hmac.compare_digest` inside
    :mod:`scripts.release_digest`, so a mismatch is reported without leaking
    how many leading characters happened to agree.

    Examples
    --------
    >>> require_downloads_match_record({"a": "ab"}, trusted={"a": "ab"})
    >>> require_downloads_match_record({"a": "cd"}, trusted={"a": "ab"})
    Traceback (most recent call last):
        ...
    release_digest_record.RecordedDigestMismatchError: uploaded assets do not \
match the staged candidate: ['a']
    """
    absent = sorted(name for name in trusted if name not in computed)
    differing = sorted(
        name
        for name, recorded in trusted.items()
        if name in computed
        and not release_digest.constant_time_equal(computed[name], recorded)
    )
    wrong = sorted({*absent, *differing})
    if not wrong:
        return
    msg = f"uploaded assets do not match the staged candidate: {wrong!r}"
    raise RecordedDigestMismatchError(msg)

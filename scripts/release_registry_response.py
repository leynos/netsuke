"""Read what crates.io answered, without deciding what it means.

The registry replies in two shapes, and both carry the fields a publication
decision needs. Reading them is mechanical: parse the JSON, find the version
records, pick out the number, the checksum, and the yank flag. Deciding
whether those facts permit a publish is the caller's job, and lives in
:mod:`release_reconcile_crates`.

Keeping the two apart is what lets each be reviewed on its own terms. This
module is where a schema surprise belongs — a record without a checksum, a
body that is neither shape, a history that disagrees with its own stated
total. The policy module never has to guess how the registry spells things,
and this one never has to know what a release intends to do.

The shapes, both taken from live responses rather than documentation:

- ``GET /api/v1/crates/<crate>/<version>`` answers ``{"version": {...}}`` —
  one record, for the version asked about.
- ``GET /api/v1/crates/<crate>`` answers ``{"versions": [{...}, ...]}``
  alongside ``crate`` — the crate's **entire publication history**. Probed on
  2026-10-23, ``serde`` returned all 316 of its versions and
  ``crate.num_versions`` said 316.

Nothing here performs network I/O; the caller records the response and passes
it in. Coverage lives in ``scripts/tests/test_release_reconcile_crates.py``.

"""

import json
import typing as typ

#: Top-level key in a *crate* response: an array of every version.
VERSIONS_KEY = "versions"

#: Top-level key in a *version* response: one version's record.
VERSION_KEY = "version"

#: Top-level key naming the crate metadata object in a crate response.
CRATE_KEY = "crate"

#: Count of versions the crate endpoint states the history holds.
NUM_VERSIONS_KEY = "num_versions"

#: The per-version key carrying the version number the record describes.
NUM_KEY = "num"

#: The per-version key holding the authoritative SHA-256 of the ``.crate``.
CHECKSUM_KEY = "checksum"

#: The per-version key marking a version its maintainers withdrew.
YANKED_KEY = "yanked"


class RegistryResponseError(ValueError):
    """Report a response body that is not the JSON shape the registry sends."""


class VersionRecord(typ.NamedTuple):
    """Carry the fields of one registry version record this code reads."""

    #: The version the record describes, as the registry spells it, or ``None``
    #: when the record carried no usable number.
    num: str | None

    #: The authoritative SHA-256 of that version's ``.crate``.
    checksum: str

    #: Whether the version's maintainers withdrew it.
    is_yanked: bool


class RegistryAnswer(typ.NamedTuple):
    """Carry a readable answer's records, and what kind of answer it was."""

    #: Whether the body listed a crate's whole history rather than one version.
    is_history: bool

    #: How many versions the body says the history holds, if it says at all.
    stated_total: int | None

    #: The version records the body carried.
    records: list[VersionRecord]


def _raw_records(parsed: object) -> list[object]:
    """Return the version records from a parsed crates.io response.

    Parameters
    ----------
    parsed
        A decoded ``crates.io`` response.

    Returns
    -------
    list[object]
        Either the one record from a version response, or every record from a
        crate response.

    Raises
    ------
    RegistryResponseError
        If ``parsed`` is neither shape.

    Notes
    -----
    A reader written for one shape rejects the other, so both are accepted
    here and folded to one list. This is not defensive over-acceptance: each
    branch is a documented response from an endpoint the release calls, and
    the checksum lives in the same field in both.
    """
    if not isinstance(parsed, dict):
        msg = f"registry response is not a JSON object: {parsed!r}"
        raise RegistryResponseError(msg)
    single = parsed.get(VERSION_KEY)
    if isinstance(single, dict):
        return [single]
    many = parsed.get(VERSIONS_KEY)
    if isinstance(many, list):
        return many
    msg = (
        f"registry response carries neither a {VERSION_KEY!r} object nor a "
        f"{VERSIONS_KEY!r} list: {parsed!r}"
    )
    raise RegistryResponseError(msg)


def _record_fields(versions: list[object]) -> list[VersionRecord]:
    """Return the fields this code reads from each version record.

    Parameters
    ----------
    versions
        The version records from a decoded crates.io response.

    Returns
    -------
    list[VersionRecord]
        One record per entry, in the order the registry listed them.

    Raises
    ------
    RegistryResponseError
        If a version record is not an object or carries no usable checksum.

    Notes
    -----
    A missing ``num`` is not an error here. The registry does send it — every
    record on both shapes carried it at the 2026-10-23 probe — but a body that
    omits it is still a body whose checksum and yank flag can be read, and
    refusing it would widen this module's authority over the caller's
    vocabulary. The record is kept with ``num=None``, and what that means for
    a version filter is stated where the filter is applied.
    """
    records: list[VersionRecord] = []
    for version in versions:
        if not isinstance(version, dict):
            msg = f"registry version record is not an object: {version!r}"
            raise RegistryResponseError(msg)
        checksum = version.get(CHECKSUM_KEY)
        if not isinstance(checksum, str) or not checksum:
            msg = f"registry version record carries no checksum: {version!r}"
            raise RegistryResponseError(msg)
        num = version.get(NUM_KEY)
        records.append(
            VersionRecord(
                num=num if isinstance(num, str) else None,
                checksum=checksum,
                is_yanked=bool(version.get(YANKED_KEY, False)),
            )
        )
    return records


def _stated_total(parsed: object) -> int | None:
    """Return the version count a crate response claims to be listing."""
    if not isinstance(parsed, dict):
        return None
    crate = parsed.get(CRATE_KEY)
    if not isinstance(crate, dict):
        return None
    stated = crate.get(NUM_VERSIONS_KEY)
    return stated if isinstance(stated, int) else None


def read_registry_response(body: str | None) -> RegistryAnswer | None:
    """Return the version records a readable response body carries.

    Parameters
    ----------
    body
        Response body as recorded, or ``None`` when the response had none.

    Returns
    -------
    RegistryAnswer | None
        The records the body carried and what kind of answer it was, or
        ``None`` when the body is not a readable answer at all.

    Notes
    -----
    ``None`` and "no records" are deliberately different answers, so the
    return is optional rather than an empty answer. An absent or unparseable
    body is a transient or proxied reply the caller may retry; a body that
    parses and does not fit is a schema surprise, which is why the error is
    raised rather than folded into ``None``. Collapsing the two would let a
    wrong assumption about the registry's schema look like a bad connection.

    A body that is JSON but neither crates.io shape, or one carrying a version
    record this module cannot read, raises :class:`RegistryResponseError`.
    Neither condition is detected here: :func:`_raw_records` and
    :func:`_record_fields` raise it while the parse is being unpacked, which
    is where the shape is actually read.

    Examples
    --------
    >>> read_registry_response(None) is None
    True
    >>> read_registry_response("not json") is None
    True
    >>> answer = read_registry_response(
    ...     '{"version": {"num": "1.0.0", "checksum": "ab"}}'
    ... )
    >>> answer.is_history, answer.stated_total
    (False, None)
    >>> answer.records
    [VersionRecord(num='1.0.0', checksum='ab', is_yanked=False)]
    >>> history = read_registry_response(
    ...     '{"crate": {"num_versions": 1}, '
    ...     '"versions": [{"num": "1.0.0", "checksum": "ab"}]}'
    ... )
    >>> history.is_history, history.stated_total
    (True, 1)
    """
    if body is None:
        # A 200 with no body is not a readable answer, whatever it claims.
        return None
    try:
        parsed = json.loads(body)
    except json.JSONDecodeError:
        return None
    return RegistryAnswer(
        is_history=isinstance(parsed, dict)
        and isinstance(parsed.get(VERSIONS_KEY), list),
        stated_total=_stated_total(parsed),
        records=_record_fields(_raw_records(parsed)),
    )

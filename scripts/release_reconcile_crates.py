"""Decide what a release run may do about a crates.io version already present.

Re-running a tagged release is ordinary: a job fails, someone re-runs it. The
run must then decide, from what the registry reports, whether to publish, to
stand down, or to stop. This module makes that decision a total function over a
recorded response, so it can be tested without a network and reviewed without
guessing what the registry said.

The decision is state-based, never name-based. "The version already exists" is
not one outcome but several: the published bytes may be the ones this run would
upload, or different bytes under the same version, or a version its maintainers
withdrew. Publishing over the last two is impossible anyway -- crates.io is
immutable -- but the failure must be *reported* rather than mistaken for the
benign case, or a re-run would report success while shipping nothing.

Two registry behaviours shape the classification, both taken from observed
responses rather than from documentation:

- A crate that has never been published and a version that does not exist yet
  both answer **HTTP 404**. The prose differs; the status does not. So absence
  is read from the status alone, and the body is never parsed to distinguish
  the two, because it cannot.
- Absence at the *download* host (``static.crates.io``, which Amazon S3 serves)
  answers **HTTP 403**, not 404. A verifier that treats 404 as "nothing there"
  and 403 as an error will report a healthy crate as broken, so
  :mod:`verify_crates_io_bytes` accepts both as absent.

Anything the registry reports that is not a clean 200 with a usable checksum --
a rate limit, a proxy error, a truncated body -- is `INDETERMINATE`, which
refuses to act. Guessing "absent" from an unrecognized response would publish
over a version that exists; guessing "present" would skip a version that does
not.

Nothing here performs network I/O; the caller records the response and passes
it in.

Run via the release workflow's registry probe; behavioural coverage lives in
``scripts/tests/test_release_reconcile_crates.py``.
"""

import enum
import json

#: The status crates.io returns for a crate or a version that does not exist.
NOT_FOUND = 404

#: The status of a version that is published and readable.
OK = 200

#: Top-level key in a *crate* response: an array of every version.
VERSIONS_KEY = "versions"

#: Top-level key in a *version* response: one version's record.
VERSION_KEY = "version"

#: The per-version key holding the authoritative SHA-256 of the ``.crate``.
CHECKSUM_KEY = "checksum"

#: The per-version key marking a version its maintainers withdrew.
YANKED_KEY = "yanked"


class RegistryState(enum.Enum):
    """What the registry reports about the version this run intends to publish."""

    #: No such version. Publishing is the only way forward.
    ABSENT = "absent"

    #: The published version's checksum equals the one this run would upload.
    IDENTICAL = "identical"

    #: A version exists under this number with different bytes.
    CONFLICT = "conflict"

    #: The version exists and has been yanked.
    YANKED = "yanked"

    #: The response cannot be interpreted, so nothing may be concluded.
    INDETERMINATE = "indeterminate"


class RegistryResponseError(ValueError):
    """Report a response body that is not the JSON shape the registry sends."""


def _version_records(parsed: object) -> list[object]:
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
    The API answers the two queries this release makes in two different
    shapes, both confirmed against the live registry on 2026-10-15:

    - ``GET /api/v1/crates/<crate>/<version>`` answers
      ``{"version": {...}}`` — singular.
    - ``GET /api/v1/crates/<crate>`` answers
      ``{"versions": [{...}, ...]}`` — plural, alongside ``crate``.

    A classifier written for one shape rejects the other, so both are
    accepted here and folded to one list. This is not defensive
    over-acceptance: each branch is a documented response from the endpoint
    the release actually calls, and the checksum lives in the same field in
    both.
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


def _checksums(versions: list[object]) -> list[tuple[str, bool]]:
    """Return every version's ``(checksum, is_yanked)`` pair.

    Raises
    ------
    RegistryResponseError
        If a version record carries no usable checksum.
    """
    records: list[tuple[str, bool]] = []
    for version in versions:
        if not isinstance(version, dict):
            msg = f"registry version record is not an object: {version!r}"
            raise RegistryResponseError(msg)
        checksum = version.get(CHECKSUM_KEY)
        if not isinstance(checksum, str) or not checksum:
            msg = f"registry version record carries no checksum: {version!r}"
            raise RegistryResponseError(msg)
        records.append((checksum, bool(version.get(YANKED_KEY, False))))
    return records


def classify_registry_state(
    body: str | None, *, intended_sha256: str, http_status: int
) -> RegistryState:
    """Classify what the registry reports about the intended version.

    Parameters
    ----------
    body
        Response body as recorded, or ``None`` when the response had none.
    intended_sha256
        SHA-256 this run would publish, lowercase hexadecimal.
    http_status
        HTTP status the registry answered with.

    Returns
    -------
    RegistryState
        The state, from :class:`RegistryState`.

    Raises
    ------
    RegistryResponseError
        If a 200 response body is not the shape crates.io sends.

    Notes
    -----
    Absence is read from ``404`` alone. A crate never published and a version
    not yet published both answer 404, so the body cannot separate them and is
    not consulted. Every other status that is not a readable 200 is
    ``INDETERMINATE``: a 429 from the registry, a 502 from a proxy, or a 401
    from a misconfigured credential all mean the answer is unknown, and acting
    on an unknown answer is how a re-run publishes over an existing version.

    Yank is judged before identity. A yanked version whose checksum happens to
    match is still withdrawn, and reporting ``IDENTICAL`` would let a release
    skip publication while pointing consumers at a version they cannot
    resolve.

    Examples
    --------
    >>> classify_registry_state(None, http_status=404, intended_sha256="ab")
    <RegistryState.ABSENT: 'absent'>
    >>> body = '{"version": {"checksum": "ab", "yanked": false}}'
    >>> classify_registry_state(body, http_status=200, intended_sha256="ab")
    <RegistryState.IDENTICAL: 'identical'>
    >>> classify_registry_state(body, http_status=200, intended_sha256="cd")
    <RegistryState.CONFLICT: 'conflict'>
    >>> yanked = '{"versions": [{"checksum": "ab", "yanked": true}]}'
    >>> classify_registry_state(yanked, http_status=200, intended_sha256="ab")
    <RegistryState.YANKED: 'yanked'>
    >>> classify_registry_state("{}", http_status=429, intended_sha256="ab")
    <RegistryState.INDETERMINATE: 'indeterminate'>
    """
    if http_status == NOT_FOUND:
        return RegistryState.ABSENT
    if http_status != OK:
        return RegistryState.INDETERMINATE
    if body is None:
        # A 200 with no body is not a readable answer, whatever it claims.
        return RegistryState.INDETERMINATE
    try:
        parsed = json.loads(body)
    except json.JSONDecodeError:
        return RegistryState.INDETERMINATE
    records = _checksums(_version_records(parsed))
    if any(is_yanked for _, is_yanked in records):
        return RegistryState.YANKED
    if intended_sha256 in {checksum for checksum, _ in records}:
        return RegistryState.IDENTICAL
    return RegistryState.CONFLICT


def plan_crates_action(state: RegistryState) -> str:
    """Return the action to take for a classified registry state.

    Parameters
    ----------
    state
        A state from :func:`classify_registry_state`.

    Returns
    -------
    str
        One of ``"publish"``, ``"skip"``, or ``"fail"``.

    Notes
    -----
    Only ``IDENTICAL`` skips. That is the whole point of the classification:
    every other state either acts or stops, and the single state that stands
    down does so because the bytes are already the ones this run would send.

    ``INDETERMINATE`` fails rather than retries here. A retry policy belongs to
    the caller, which knows its attempt budget; a state planner that answered
    "retry" would make the caller's `if` on the result a second decision point
    with no state to decide on.

    Examples
    --------
    >>> plan_crates_action(RegistryState.ABSENT)
    'publish'
    >>> plan_crates_action(RegistryState.IDENTICAL)
    'skip'
    >>> plan_crates_action(RegistryState.CONFLICT)
    'fail'
    >>> plan_crates_action(RegistryState.YANKED)
    'fail'
    >>> plan_crates_action(RegistryState.INDETERMINATE)
    'fail'
    """
    if state is RegistryState.ABSENT:
        return "publish"
    if state is RegistryState.IDENTICAL:
        return "skip"
    return "fail"

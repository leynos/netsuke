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

Three registry behaviours shape the classification, all taken from observed
responses rather than from documentation:

- A crate that has never been published and a version that does not exist yet
  both answer **HTTP 404**. The prose differs; the status does not. So absence
  is read from the status alone, and the body is never parsed to distinguish
  the two, because it cannot.
- Absence at the *download* host (``static.crates.io``, which Amazon S3 serves)
  answers **HTTP 403**, not 404. A verifier that treats 404 as "nothing there"
  and 403 as an error will report a healthy crate as broken, so
  :mod:`verify_crates_io_bytes` accepts both as absent.
- The crate endpoint answers with the crate's **entire publication history**,
  not with one record. Probed live on 2026-10-23,
  ``GET /api/v1/crates/serde`` returned all 316 of its versions, matching
  ``crate.num_versions``, and 3 of them are yanked. Classification is
  therefore per version: a yank anywhere in the history says nothing about the
  version this run would publish.

Reading the response is :mod:`release_registry_response`'s job; this module
decides, and it is the only one of the two that knows what a release intends.

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

from release_registry_response import (
    RegistryAnswer,
    RegistryResponseError,
    VersionRecord,
    read_registry_response,
)

#: The status crates.io returns for a crate or a version that does not exist.
NOT_FOUND = 404

#: The status of a version that is published and readable.
OK = 200


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


def _records_for_version(
    records: list[VersionRecord], version: str
) -> list[VersionRecord]:
    """Return the records describing ``version``.

    Parameters
    ----------
    records
        Every record the response carried.
    version
        The version this run intends to publish.

    Returns
    -------
    list[VersionRecord]
        The records whose ``num`` equals ``version``. Empty when none does,
        which is what a crate endpoint's history looks like before the
        intended version has ever been published.

    Notes
    -----
    The crate endpoint answers with the crate's whole publication history --
    316 records for ``serde`` at the 2026-10-23 probe -- so the classification
    questions ("is *this* version yanked?", "does *this* version carry the
    intended bytes?") are only meaningful per record. Asking them of the whole
    list is wrong in both directions: one yanked old release would make every
    later release classify ``YANKED``, and an old release whose bytes happened
    to match would make a genuinely new one classify ``IDENTICAL``.

    A record whose number could not be read never matches. It cannot be
    evidence about the intended version, and treating it as a match would let
    an unidentifiable record decide the release.

    Examples
    --------
    >>> from release_registry_response import VersionRecord as R
    >>> records = [R("0.9.0", "ab", True), R("1.0.0", "cd", False)]
    >>> _records_for_version(records, "1.0.0")
    [VersionRecord(num='1.0.0', checksum='cd', is_yanked=False)]
    >>> _records_for_version(records, "2.0.0")
    []
    >>> _records_for_version([R(None, "ab", True)], "1.0.0")
    []
    """
    return [record for record in records if record.num == version]


def _classify_answer(
    answer: RegistryAnswer, *, version: str, intended_sha256: str
) -> RegistryState:
    """Classify a readable answer against the intended version and digest."""
    if answer.is_history and answer.stated_total not in {None, len(answer.records)}:
        # A history that disagrees with its own count is a truncated one. The
        # check is made before the version filter, because the filter's empty
        # result is the dangerous case: a truncated body could contain no
        # record for the intended version, and a missing record is
        # indistinguishable from a version never published -- which plans a
        # publish.
        return RegistryState.INDETERMINATE
    records = _records_for_version(answer.records, version)
    if not records:
        # A history that does not list this version says the version is not
        # published, which is the same fact the version endpoint's 404 states.
        # A single-version answer about another number is different: the caller
        # asked about one version and was told about another.
        return RegistryState.ABSENT if answer.is_history else RegistryState.CONFLICT
    if any(record.is_yanked for record in records):
        return RegistryState.YANKED
    if any(record.checksum == intended_sha256 for record in records):
        return RegistryState.IDENTICAL
    return RegistryState.CONFLICT


def classify_registry_state(
    body: str | None, *, version: str, intended_sha256: str, http_status: int
) -> RegistryState:
    """Classify what the registry reports about the intended version.

    Parameters
    ----------
    body
        Response body as recorded, or ``None`` when the response had none.
    version
        The version this run intends to publish, spelled as the registry
        spells it.
    intended_sha256
        SHA-256 this run would publish, lowercase hexadecimal.
    http_status
        HTTP status the registry answered with.

    Returns
    -------
    RegistryState
        The state, from :class:`RegistryState`.

    Notes
    -----
    Absence is read from ``404`` alone, for the reason the module docstring
    gives; the body is not consulted. Every other status that is not a
    readable 200 is ``INDETERMINATE``, and acting on an unknown answer is how
    a re-run publishes over an existing version.

    ``version`` selects which records the answer is read from, so a body
    describing the crate's whole history is never asked a question the release
    did not ask. See :func:`_records_for_version`. Yank is judged before
    identity: a yanked version whose checksum matches is still withdrawn, and
    ``IDENTICAL`` would send consumers to a version they cannot resolve.

    A well-formed body in a shape crates.io does not send propagates
    :class:`release_registry_response.RegistryResponseError` rather than
    becoming a state: that would mean this code's schema assumption is wrong,
    and a state would swallow it.

    Examples
    --------
    >>> classify_registry_state(
    ...     None, version="1.0.0", http_status=404, intended_sha256="ab"
    ... )
    <RegistryState.ABSENT: 'absent'>
    >>> classify_registry_state(
    ...     '{"version": {"num": "1.0.0", "checksum": "ab", "yanked": false}}',
    ...     version="1.0.0", http_status=200, intended_sha256="cd",
    ... )
    <RegistryState.CONFLICT: 'conflict'>
    >>> classify_registry_state(
    ...     "{}", version="1.0.0", http_status=429, intended_sha256="ab"
    ... )
    <RegistryState.INDETERMINATE: 'indeterminate'>
    """
    if http_status == NOT_FOUND:
        return RegistryState.ABSENT
    if http_status != OK:
        return RegistryState.INDETERMINATE
    answer = read_registry_response(body)
    if answer is None:
        return RegistryState.INDETERMINATE
    return _classify_answer(answer, version=version, intended_sha256=intended_sha256)


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


__all__ = [
    "NOT_FOUND",
    "OK",
    "RegistryResponseError",
    "RegistryState",
    "classify_registry_state",
    "plan_crates_action",
]

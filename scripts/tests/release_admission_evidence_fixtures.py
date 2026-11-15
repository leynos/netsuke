"""Build the candidate-bound evidence manifests the runtime tests hand the gate.

The gate reads a manifest rather than an environment variable, so every case
needs a real file whose fields name the candidate the child is running for.
These builders keep that construction in one place: a case names the *state* it
wants -- fresh, stale, malformed -- and the mapping from state to document lives
here, beside the schema it must satisfy.

Timestamps are measured from the real clock immediately before the child runs,
because the gate compares them with its own reading of the same clock. The
offsets below are an hour or more either side of the age bound, so scheduling
delay between writing the document and running the gate cannot decide a result.
A state that is meant to be accepted is stamped *now*; one meant to be refused
is stamped a full hour beyond the bound it violates.

Nothing here imports the gate's own parser or its schema constants. A fixture
that built its documents from the verifier, or from the verifier's idea of the
schema, would agree with a broken verifier: dropping a required result would
drop it from the fixture too, and the case that omits it would still pass. The
schema is therefore written out independently, here, and compared by the gate.
"""

from __future__ import annotations

import datetime as dt
import json
import typing as typ

from release_admission_test_doubles import (
    GITHUB_REPOSITORY,
    RELEASE_TAG,
    REVISION,
    WORKFLOW_RUN_ID,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

#: The manifest schema version this module writes.
SCHEMA_VERSION = 1
#: The results every manifest must report. Spelled out rather than imported, so
#: a gate that stopped requiring one of them fails the case that omits it.
REQUIRED_RESULT_NAMES = ("history_scan", "release_candidate")
#: The outcome a result reports when the canary it names passed.
SUCCESS = "success"
#: The outcome a result reports when the canary it names failed.
FAILURE = "failure"
#: The byte bound the gate reads a document under. A document one byte past it
#: must be refused without being parsed.
MAX_DOCUMENT_BYTES = 64 * 1024
#: The age bound the gate applies when none is configured, used only to place a
#: stale document a full hour beyond it.
DEFAULT_MAX_AGE_SECONDS = 86_400


class Candidate(typ.NamedTuple):
    """Name the release identity the manifest handed to one run must describe.

    The child's environment decides all four values, so a case that overrides
    ``GITHUB_SHA`` or ``GITHUB_RUN_ID`` receives a manifest about the candidate
    it actually runs for rather than about the module's defaults.
    """

    repository: str
    commit: str
    tag: str
    run_id: str


def candidate_for(environment: cabc.Mapping[str, str]) -> Candidate:
    """Return the candidate named by the environment one child will inherit.

    Returns
    -------
    Candidate
        The repository, commit, tag and run identifier the child resolves.
    """
    return Candidate(
        repository=environment.get("GITHUB_REPOSITORY", GITHUB_REPOSITORY),
        commit=environment.get("GITHUB_SHA", REVISION),
        tag=environment.get("NETSUKE_RELEASE_ADMISSION_TAG") or RELEASE_TAG,
        run_id=environment.get("GITHUB_RUN_ID", WORKFLOW_RUN_ID),
    )


#: The state naming an absent file: nothing is written at the path.
MISSING = "missing"
#: A document that admits the candidate.
FRESH = "fresh"
#: A document that admits the candidate and reports one result this gate does
#: not require, which is how a future producer extends the manifest.
FORWARD_COMPATIBLE = "forward-compatible"
#: A document issued beyond the maximum age.
STALE = "stale"
#: A document issued further ahead than the clock-skew allowance.
FUTURE = "future"
#: Bytes that are not JSON at all.
MALFORMED = "malformed"
#: A document larger than the gate's byte bound.
OVERSIZED = "oversized"
#: A document declaring a schema version the verifier does not know.
UNKNOWN_SCHEMA = "unknown-schema"
#: A document naming another commit.
MISMATCHED = "mismatched"
#: A document naming another workflow run.
WRONG_RUN = "wrong-run"
#: A document naming another tag.
WRONG_TAG = "wrong-tag"
#: A document naming another repository.
WRONG_REPOSITORY = "wrong-repository"
#: A document reporting a required canary as unsuccessful.
UNSUCCESSFUL = "unsuccessful"
#: A document omitting one required canary.
INCOMPLETE = "incomplete"

#: How far beyond each bound a refused timestamp sits. An hour is far more than
#: the seconds a test run can slip, and far less than the day-long default age.
OFFSET_SECONDS: typ.Final[float] = 3_600.0

#: Padding that pushes a document one byte past :data:`MAX_DOCUMENT_BYTES` once
#: the encoded manifest is appended to it.
_OVERSIZE_BYTES: typ.Final[int] = MAX_DOCUMENT_BYTES + 1

#: A result name no version of this gate requires, reporting a canary a future
#: producer may add.
_EXTRA_RESULT_NAME = "licence_scan"


def write_evidence(path: pathlib.Path, state: str, subject: Candidate) -> None:
    """Write the document *state* names, or nothing at all for an absent file.

    Parameters
    ----------
    path
        The path the gate will be told to read.
    state
        One of this module's state names.
    subject
        The candidate the child runs for, which the document must describe
        unless the state exists to describe something else.
    """
    document = _document(state, subject)
    if document is None:
        return
    # A case may name a directory the gate itself would create, so the fixture
    # must not depend on the gate having run before the document is written.
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(document)


def _document(state: str, subject: Candidate) -> bytes | None:
    """Return the bytes one state names, or ``None`` for an absent file.

    Returns
    -------
    bytes | None
        The document to write, or ``None`` when the state names no file.

    Raises
    ------
    ValueError
        If the state is not one this module defines, which is a test fault
        rather than a state the gate has to handle.
    """
    if state == MISSING:
        return None
    builder = _BUILDERS.get(state)
    if builder is None:
        message = f"{state!r} is not an evidence state this module defines"
        raise ValueError(message)
    return builder(subject)


def _extended_results() -> dict[str, object]:
    """Return every required result plus one no version of the gate requires."""
    extended = dict(_results(SUCCESS))
    extended[_EXTRA_RESULT_NAME] = _result(SUCCESS)
    return extended


def _incomplete_results() -> dict[str, object]:
    """Return every required result except the history scan."""
    results = _results(SUCCESS)
    del results["history_scan"]
    return results


def _stale_offset() -> float:
    """Return an age one hour beyond the maximum the gate defaults to."""
    return DEFAULT_MAX_AGE_SECONDS + OFFSET_SECONDS


def _moment(offset: float) -> str:
    """Render the instant *offset* seconds from now as an RFC 3339 UTC string."""
    moment = dt.datetime.now(dt.UTC) + dt.timedelta(seconds=offset)
    return moment.isoformat().replace("+00:00", "Z")


def _result(outcome: str) -> dict[str, object]:
    """Return one result object reporting *outcome*, observed now."""
    return {"outcome": outcome, "observed_at": _moment(0)}


def _results(outcome: str) -> dict[str, object]:
    """Return one result per required canary, all reporting *outcome*."""
    return {name: _result(outcome) for name in REQUIRED_RESULT_NAMES}


def _encoded(subject: Candidate, **overrides: object) -> bytes:
    """Encode the manifest describing *subject*, with the named fields replaced."""
    document: dict[str, object] = {
        "schema_version": SCHEMA_VERSION,
        "repository": subject.repository,
        "commit": subject.commit,
        "tag": subject.tag,
        "workflow_run_id": subject.run_id,
        "issued_at": _moment(0),
        "results": _results(SUCCESS),
    }
    document.update(overrides)
    return json.dumps(document).encode("utf-8")


#: How each state that writes a file builds it. An absent file is not here:
#: nothing is written for :data:`MISSING`, so the gate observes the same
#: ``FileNotFoundError`` an unpublished manifest produces.
_BUILDERS: cabc.Mapping[str, cabc.Callable[[Candidate], bytes]] = {
    FRESH: _encoded,
    FORWARD_COMPATIBLE: lambda subject: _encoded(subject, results=_extended_results()),
    STALE: lambda subject: _encoded(subject, issued_at=_moment(-_stale_offset())),
    FUTURE: lambda subject: _encoded(subject, issued_at=_moment(OFFSET_SECONDS)),
    MALFORMED: lambda _: b'{"schema_version": 1,',
    OVERSIZED: lambda subject: _encoded(subject) + b" " * _OVERSIZE_BYTES,
    UNKNOWN_SCHEMA: lambda subject: _encoded(subject, schema_version=2),
    MISMATCHED: lambda subject: _encoded(subject, commit="b" * 40),
    WRONG_RUN: lambda subject: _encoded(subject, workflow_run_id="9999"),
    WRONG_TAG: lambda subject: _encoded(subject, tag="v0.0.0"),
    WRONG_REPOSITORY: lambda subject: _encoded(subject, repository="someone/else"),
    UNSUCCESSFUL: lambda subject: _encoded(subject, results=_results(FAILURE)),
    INCOMPLETE: lambda subject: _encoded(subject, results=_incomplete_results()),
}


__all__ = (
    "FORWARD_COMPATIBLE",
    "FRESH",
    "FUTURE",
    "INCOMPLETE",
    "MALFORMED",
    "MISMATCHED",
    "MISSING",
    "OVERSIZED",
    "STALE",
    "UNKNOWN_SCHEMA",
    "UNSUCCESSFUL",
    "WRONG_REPOSITORY",
    "WRONG_RUN",
    "WRONG_TAG",
    "Candidate",
    "candidate_for",
    "write_evidence",
)

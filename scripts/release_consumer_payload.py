"""Decide whether a saved download and a probe result are the candidate's.

The smoke asks two questions of what a consumer would actually receive. First,
did the public host deliver the asset, and are the delivered bytes the ones the
trusted record names? Second, does the binary the archive ships report exactly
the candidate version?

Both are decided here from values already in memory: a
:class:`PayloadFacts` describing one saved file, and the exit status and
standard output of one ``--version`` run. Reading the file, hashing it, and
launching the binary belong to :mod:`release_binstall_archive`,
:mod:`release_binstall_probe`, and :mod:`consumer_smoke_cli`; keeping the
judgements separate is what lets a unit test exercise every refusal without a
download or a child process.

The verdict record is bounded here too. It carries fixed phase labels and
registered outcomes only, so an error string, an asset name, or a workflow
input can never reach the uploaded artefact through it.

Run through ``scripts/consumer_smoke_cli.py``; behavioural coverage lives in
``scripts/tests/test_release_consumer_smoke.py``.
"""

import dataclasses
import typing as typ

import release_digest
from release_consumer_smoke import ConsumerSmokeError, expected_banner

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: The fixed phase labels a verdict record may carry. Bounded on purpose: the
#: record is uploaded as telemetry, so it must not be able to grow a field
#: named by a workflow input, an asset name, or an error string.
SMOKE_PHASES = (
    "crate_download",
    "crate_verification",
    "archive_download",
    "archive_verification",
    "executable_probe",
)

#: The outcomes a phase may report.
PHASE_OUTCOMES = frozenset({"success", "failure", "skipped"})


class PayloadUnavailableError(ConsumerSmokeError):
    """Report a download the public host did not deliver."""


class PayloadEmptyError(ConsumerSmokeError):
    """Report a delivered download that carries no bytes."""


class PayloadDigestMissingError(ConsumerSmokeError):
    """Report a present download whose digest could not be computed."""


class PayloadMismatchError(ConsumerSmokeError, release_digest.DigestMismatchError):
    """Report downloaded bytes that differ from the trusted digest."""


class ProbeExitError(ConsumerSmokeError):
    """Report a shipped binary that exited non-zero."""


class EmptyBannerError(ConsumerSmokeError):
    """Report a shipped binary that printed nothing on its standard output."""


class BannerMismatchError(ConsumerSmokeError):
    """Report output that is not exactly the candidate's version banner."""


@dataclasses.dataclass(frozen=True, slots=True)
class PayloadFacts:
    """Describe one saved download without reading it.

    Parameters
    ----------
    delivered
        Whether the host returned the body this run asked for.
    byte_count
        Number of bytes saved.
    sha256
        Lowercase digest of the saved bytes, or ``None`` when unreadable.
    """

    delivered: bool
    byte_count: int
    sha256: str | None


def require_delivered(*, facts: PayloadFacts, asset_name: str) -> int:
    """Return the byte count of a download the host actually delivered.

    Parameters
    ----------
    facts
        What the workflow observed after saving the response body.
    asset_name
        The bare asset name the download was requested under.

    Returns
    -------
    int
        The number of bytes saved, which is positive.

    Raises
    ------
    PayloadUnavailableError
        If the host did not deliver the body.
    PayloadEmptyError
        If the delivered body is empty.

    Notes
    -----
    Separated from :func:`require_payload` so the smoke can report a missing
    download and a mismatched digest under different phase labels: "not
    delivered" is a different operational fact from "delivered, but not the
    bytes we published".

    Examples
    --------
    >>> require_delivered(
    ...     facts=PayloadFacts(delivered=True, byte_count=7, sha256=None),
    ...     asset_name="a.crate",
    ... )
    7
    """
    if not facts.delivered:
        msg = f"{asset_name} was not downloaded from its public URL"
        raise PayloadUnavailableError(msg)
    if facts.byte_count <= 0:
        msg = f"{asset_name} was downloaded but carries no bytes"
        raise PayloadEmptyError(msg)
    return facts.byte_count


def require_payload(
    *, facts: PayloadFacts, asset_name: str, expected_sha256: str
) -> str:
    """Return the verified digest of one saved download.

    Parameters
    ----------
    facts
        What the workflow observed after saving the response body.
    asset_name
        The bare asset name the download was requested under.
    expected_sha256
        The digest the trusted record holds for that name.

    Returns
    -------
    str
        The observed digest, once it matches the trusted record.

    Raises
    ------
    PayloadDigestMissingError
        If no digest accompanies a present body.
    PayloadMismatchError
        If the observed and trusted digests differ.

    Notes
    -----
    An undelivered or empty body fails first, in :func:`require_delivered`,
    which the smoke runs under its own phase label.

    Examples
    --------
    >>> facts = PayloadFacts(delivered=True, byte_count=7, sha256="ab" * 32)
    >>> require_payload(facts=facts, asset_name="a.crate", expected_sha256="ab" * 32)
    'abababababababababababababababababababababababababababababababab'
    """
    _ = require_delivered(facts=facts, asset_name=asset_name)
    if facts.sha256 is None:
        msg = f"the download of {asset_name} could not be hashed"
        raise PayloadDigestMissingError(msg)
    if not release_digest.constant_time_equal(facts.sha256, expected_sha256):
        msg = (
            f"{asset_name} hashes to {facts.sha256}, but the trusted record "
            f"expects {expected_sha256}"
        )
        raise PayloadMismatchError(msg)
    return facts.sha256


def require_probe(*, returncode: int, stdout: str, version: str) -> str:
    r"""Return the banner when the probe reported exactly the candidate version.

    Parameters
    ----------
    returncode
        The probe's exit status.
    stdout
        Everything the probe wrote to standard output.
    version
        The candidate version the banner must spell.

    Returns
    -------
    str
        The accepted banner.

    Raises
    ------
    ProbeExitError
        If the binary exited non-zero.
    EmptyBannerError
        If standard output held only whitespace.
    BannerMismatchError
        If standard output is not the banner plus at most one trailing
        newline.

    Notes
    -----
    Stderr is deliberately not consulted. A binary that warns while reporting
    its version is still the candidate, and copying raw stderr into a summary
    would risk publishing text this workflow does not control.

    Examples
    --------
    >>> require_probe(returncode=0, stdout="netsuke 1.2.3\n", version="1.2.3")
    'netsuke 1.2.3'
    """
    if returncode != 0:
        msg = f"the shipped binary exited {returncode} when asked for its version"
        raise ProbeExitError(msg)
    if not stdout.strip():
        msg = "the shipped binary printed nothing on standard output"
        raise EmptyBannerError(msg)
    banner = expected_banner(version)
    if stdout not in {banner, f"{banner}\n"}:
        msg = f"the shipped binary printed {stdout!r}, not exactly {banner!r}"
        raise BannerMismatchError(msg)
    return banner


def verdict_record(phases: cabc.Mapping[str, str]) -> dict[str, object]:
    """Render a bounded verdict record from the phase outcomes observed.

    Parameters
    ----------
    phases
        Outcome per :data:`SMOKE_PHASES` label. An absent label is recorded
        as ``skipped``.

    Returns
    -------
    dict[str, object]
        A mapping with exactly ``phases`` and ``verdict`` members, suitable
        for the telemetry artefact.

    Raises
    ------
    ConsumerSmokeError
        If a supplied outcome is not one of :data:`PHASE_OUTCOMES`.

    Examples
    --------
    >>> record = verdict_record({"crate_download": "failure"})
    >>> record["verdict"]
    'failed'
    >>> record["phases"]["executable_probe"]
    'skipped'
    """
    outcomes: dict[str, str] = {}
    for label in SMOKE_PHASES:
        outcome = phases.get(label, "skipped")
        if outcome not in PHASE_OUTCOMES:
            msg = f"phase {label!r} reported an unregistered outcome {outcome!r}"
            raise ConsumerSmokeError(msg)
        outcomes[label] = outcome
    verified = all(outcome == "success" for outcome in outcomes.values())
    return {
        "phases": outcomes,
        "verdict": "verified" if verified else "failed",
    }

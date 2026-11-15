"""Check the saved downloads and the shipped executable against a smoke plan.

``verify`` is the half of the smoke that touches what the workflow downloaded.
It re-derives the plan from the trusted evidence, refuses a saved plan that
disagrees, and then walks the five phases: the crate's arrival, the crate's
digest, the archive's and sidecar's arrival, the archive's own layout and
sidecar, and finally the extracted binary's version banner.

Two records come out of it, both kept current by
:class:`consumer_smoke_ledger.PhaseLedger`: the bounded verdict artefact a
consumer can read afterwards, and the bounded per-phase log the workflow turns
into job outputs.

Run through ``scripts/consumer_smoke_cli.py``; behavioural coverage lives in
``scripts/tests/test_consumer_smoke_cli.py``.
"""

import dataclasses
import sys
import typing as typ

import consumer_smoke_evidence as evidence
import release_binstall_archive as archive_adapter
import release_binstall_extract as extract_adapter
import release_binstall_probe as probe_adapter
import release_binstall_sidecar as sidecar_adapter
import release_consumer_archive_policy as archive_policy
import release_consumer_payload as payload
import release_consumer_smoke as smoke
from consumer_smoke_ledger import PhaseLedger
from release_consumer_smoke import ConsumerSmokeError, EvidenceRejectedError

if typ.TYPE_CHECKING:
    import argparse
    import pathlib

#: The exit status of a run in which every phase verified.
EXIT_OK = 0

#: The exit status of a run whose evidence, saved plan, or verdict is unusable.
EXIT_EVIDENCE_REJECTED = 1

#: The exit status of a run whose public download was missing, empty, or wrong.
EXIT_DOWNLOAD_REJECTED = 2

#: The exit status of a run whose archive is corrupt, unsafe, or unwritable.
EXIT_ARCHIVE_REJECTED = 3

#: The exit status of a run whose executable did not report the candidate version.
EXIT_PROBE_REJECTED = 4

#: Every refusal the phases can report, caught so one table can diagnose them.
#:
#: The probe's own refusals subclass :class:`ArchiveRefusedError`, so the
#: clauses here are grouped rather than ordered; the ordering that matters is
#: in :data:`_REFUSALS`, where a probe refusal is matched before the archive
#: refusal it subclasses.
_REFUSAL_TYPES = (
    ConsumerSmokeError,
    archive_adapter.ArchiveRefusedError,
    archive_policy.ArchiveLayoutError,
)

#: How each refusal becomes a diagnosis and an exit status.
#:
#: Ordered so a probe that never ran is reported as the executable phase's
#: failure rather than the archive's: both subclass
#: :class:`ArchiveRefusedError`, and the page a reader needs names the phase
#: that failed. The last row is the residual for anything else, which is
#: reached only by a ledger that could not write its own record.
_REFUSALS: tuple[tuple[tuple[type[Exception], ...], int, str], ...] = (
    (
        (
            probe_adapter.ProbeLaunchError,
            probe_adapter.ProbeTimeoutError,
            probe_adapter.ProbeOutputError,
        ),
        EXIT_PROBE_REJECTED,
        "executable rejected",
    ),
    (
        (
            payload.PayloadUnavailableError,
            payload.PayloadEmptyError,
            payload.PayloadUnreadableError,
            payload.PayloadDigestMissingError,
            payload.PayloadMismatchError,
        ),
        EXIT_DOWNLOAD_REJECTED,
        "download rejected",
    ),
    (
        (archive_adapter.ArchiveRefusedError, archive_policy.ArchiveLayoutError),
        EXIT_ARCHIVE_REJECTED,
        "archive rejected",
    ),
    (
        (
            payload.ProbeExitError,
            payload.EmptyBannerError,
            payload.BannerMismatchError,
        ),
        EXIT_PROBE_REJECTED,
        "executable rejected",
    ),
    ((ConsumerSmokeError,), EXIT_EVIDENCE_REJECTED, "could not record its verdict"),
)


@dataclasses.dataclass(frozen=True, slots=True)
class SavedDownload:
    """Pair one saved download's facts with the path they describe.

    Parameters
    ----------
    facts
        What the host returned, and the digest of what it saved.
    path
        The file the body was saved into.
    """

    facts: payload.PayloadFacts
    path: pathlib.Path


@dataclasses.dataclass(frozen=True, slots=True)
class Extraction:
    """Name the archive, its accepted layout, and where to extract it.

    Parameters
    ----------
    archive
        The verified archive.
    layout
        The layout the verification scan accepted.
    sha256
        The trusted digest the archive was verified against, which extraction
        re-checks so a replacement archive matching the layout is still
        refused.
    destination
        The caller-created, empty directory the binary is written into.
    """

    archive: pathlib.Path
    layout: archive_policy.ArchiveLayout
    sha256: str
    destination: pathlib.Path


def verify_archive(
    *, plan: smoke.SmokePlan, archive: SavedDownload, sidecar: SavedDownload
) -> archive_policy.ArchiveLayout:
    """Check both release downloads and the archive's own member layout.

    Parameters
    ----------
    plan
        The candidate this run is bound to.
    archive
        The saved cargo-binstall archive and what the host returned for it.
    sidecar
        The saved checksum sidecar and what the host returned for it.

    Returns
    -------
    release_consumer_archive_policy.ArchiveLayout
        The single root binary and the member totals the scan accepted.

    Notes
    -----
    A download that is missing, empty, unhashable, or not the trusted bytes
    fails as the matching :class:`release_consumer_payload.ConsumerSmokeError`
    raised by :func:`release_consumer_payload.require_payload`.

    The archive's own refusals — too large, corrupt, or describing different
    bytes than its sidecar — are raised by
    :func:`release_binstall_archive.scan_archive` and
    :func:`release_binstall_sidecar.verify_sidecar`; an unsafe member layout
    by :func:`release_consumer_archive_policy.require_safe_layout`.
    """
    for download, name, digest in (
        (archive, plan.archive_name, plan.archive_sha256),
        (sidecar, plan.sidecar_name, plan.sidecar_sha256),
    ):
        _ = payload.require_payload(
            facts=download.facts, asset_name=name, expected_sha256=digest
        )
    layout = archive_adapter.scan_archive(
        archive.path, expected_sha256=plan.archive_sha256
    )
    _ = sidecar_adapter.verify_sidecar(archive.path, sidecar.path)
    return layout


def probe(
    *,
    plan: smoke.SmokePlan,
    extraction: Extraction,
    runner: probe_adapter.SubprocessRunner | None,
) -> None:
    """Extract the shipped binary and require its exact version banner.

    Notes
    -----
    A banner that does not report the candidate version fails as the matching
    :class:`release_consumer_payload.ConsumerSmokeError` raised by
    :func:`release_consumer_payload.require_probe`.

    Extraction and launch failures are raised by
    :func:`release_binstall_extract.extract_binary` and
    :func:`release_binstall_probe.probe_version`. The extractor checks the
    archive's digest before it writes, so an archive replaced since the scan
    is refused even when its members still match the scanned layout.
    """
    binary = extract_adapter.extract_binary(
        extraction.archive,
        extraction.layout,
        extraction.destination,
        expected_sha256=extraction.sha256,
    )
    result = probe_adapter.probe_version(binary, runner=runner)
    _ = payload.require_probe(
        returncode=result.returncode, stdout=result.stdout, version=plan.version
    )


def run_verify(
    arguments: argparse.Namespace,
    *,
    runner: probe_adapter.SubprocessRunner | None,
) -> int:
    """Check the saved downloads and the shipped executable against the plan.

    Returns
    -------
    int
        The exit status matching the first refusal the run reported, or
        :data:`EXIT_OK` when every phase verified.
    """
    ledger = PhaseLedger(destination=arguments.verdict, log_path=arguments.phase_log)
    try:
        plan = smoke.plan_smoke(evidence.gather_evidence(arguments))
        evidence.require_saved_plan(arguments.plan, plan)
    except (EvidenceRejectedError, OSError) as error:
        print(f"consumer smoke evidence rejected: {error}", file=sys.stderr)
        return EXIT_EVIDENCE_REJECTED
    try:
        _run_phases(arguments, plan, ledger, runner)
    except _REFUSAL_TYPES as error:
        return _report_refusal(error)
    return EXIT_OK


def _run_phases(
    arguments: argparse.Namespace,
    plan: smoke.SmokePlan,
    ledger: PhaseLedger,
    runner: probe_adapter.SubprocessRunner | None,
) -> None:
    """Run every smoke phase in order, recording each outcome as it completes.

    Notes
    -----
    Each phase raises its own refusal, which the caller diagnoses through
    :data:`_REFUSALS`: a :class:`release_consumer_payload.ConsumerSmokeError`
    for a download, sidecar, or banner, a
    :class:`release_binstall_archive.ArchiveRefusedError` for an archive that
    is corrupt or unwritable, and a
    :class:`release_consumer_archive_policy.ArchiveLayoutError` for members
    that cannot be extracted safely.
    """
    crate = ledger.run(
        "crate_download",
        lambda: _delivered(arguments.crate, arguments.crate_status, plan.crate_name),
    )
    _ = ledger.run(
        "crate_verification",
        lambda: payload.require_payload(
            facts=crate.facts,
            asset_name=plan.crate_name,
            expected_sha256=plan.crate_sha256,
        ),
    )
    archive, sidecar = ledger.run(
        "archive_download",
        lambda: _delivered_assets(arguments, plan),
    )
    layout = ledger.run(
        "archive_verification",
        lambda: verify_archive(plan=plan, archive=archive, sidecar=sidecar),
    )
    _ = ledger.run(
        "executable_probe",
        lambda: probe(
            plan=plan,
            extraction=Extraction(
                archive=archive.path,
                layout=layout,
                sha256=plan.archive_sha256,
                destination=arguments.extract_dir,
            ),
            runner=runner,
        ),
    )


def _delivered(
    body: pathlib.Path, status: pathlib.Path, asset_name: str
) -> SavedDownload:
    """Require one download to have arrived and return the facts observed for it.

    Returns
    -------
    SavedDownload
        The facts the host's response established, and the saved path.

    Notes
    -----
    The facts are gathered here, inside the phase, so a read that fails is
    recorded against the download that needed it; reading them up front would
    let an ``OSError`` escape the ledger and end the run without a verdict.

    An undelivered download fails as
    :class:`release_consumer_payload.PayloadUnavailableError`, an empty one as
    :class:`release_consumer_payload.PayloadEmptyError`, and an unreadable one
    as :class:`release_consumer_payload.PayloadUnreadableError`.
    """
    saved = _saved(body, status)
    _ = payload.require_delivered(facts=saved.facts, asset_name=asset_name)
    return saved


def _delivered_assets(
    arguments: argparse.Namespace, plan: smoke.SmokePlan
) -> tuple[SavedDownload, SavedDownload]:
    """Require both release downloads to have arrived.

    Returns
    -------
    tuple[SavedDownload, SavedDownload]
        The archive and its sidecar, each with the facts gathered for it.

    Notes
    -----
    The refusals are the ones :func:`_delivered` raises, under the archive's
    and then the sidecar's asset name.
    """
    return (
        _delivered(arguments.archive, arguments.archive_status, plan.archive_name),
        _delivered(arguments.sidecar, arguments.sidecar_status, plan.sidecar_name),
    )


def _saved(body: pathlib.Path, status: pathlib.Path) -> SavedDownload:
    """Pair a saved body with the HTTP status recorded beside it.

    Returns
    -------
    SavedDownload
        The facts the host's response established, and the saved path.

    Raises
    ------
    PayloadUnreadableError
        If the saved file could not be stat'ed or hashed. The host's
        ``OSError`` is translated here so the download phase records a refusal
        rather than the run escaping with an untranslated failure.
    """
    try:
        facts = evidence.payload_facts(body=body, status=status)
    except OSError as error:
        msg = f"the saved download {body} could not be read: {error}"
        raise payload.PayloadUnreadableError(msg) from error
    return SavedDownload(facts=facts, path=body)


def _report_refusal(error: Exception) -> int:
    """Report one refusal under the phase it belongs to.

    Returns
    -------
    int
        The exit status the matching :data:`_REFUSALS` row names.

    Examples
    --------
    The diagnosis is written to standard error, which a doctest does not
    compare, so only the status is shown here:

    >>> from release_consumer_payload import PayloadEmptyError
    >>> _report_refusal(PayloadEmptyError("a.crate carries no bytes"))
    2
    """
    _, status, label = next(
        (row for row in _REFUSALS if isinstance(error, row[0])), _REFUSALS[-1]
    )
    print(f"consumer smoke {label}: {error}", file=sys.stderr)
    return status

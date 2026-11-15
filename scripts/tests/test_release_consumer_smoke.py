"""Test the pure decisions a public-consumer smoke makes.

Every case here is decided from in-memory values: no download, tarball, or
child process is involved. Two modules carry those decisions:
:mod:`release_consumer_smoke` binds the plan to one candidate, and
:mod:`release_consumer_payload` judges a saved download and a probe result.
The archive and probe boundaries are exercised in
``test_release_binstall_archive.py``, and the composition in
``test_consumer_smoke_cli.py``.

Run via ``make test-release-publication``.
"""

import dataclasses

import pytest
import release_candidate
import release_consumer_payload as payload
import release_consumer_smoke as smoke
import release_digest_record

COMMIT = "0" * 40
TARGET = "x86_64-unknown-linux-gnu"
CRATE = "netsuke-build-1.2.3.crate"
ARCHIVE = f"netsuke-build-1.2.3-{TARGET}.tar.gz"
SIDECAR = f"{ARCHIVE}.sha256"
ARCHIVE_DIGEST = "ab" * 32
SIDECAR_DIGEST = "cd" * 32
CRATE_DIGEST = "ef" * 32

IDENTITY = release_candidate.ReleaseIdentity(
    tag="v1.2.3",
    commit=COMMIT,
    package_name="netsuke-build",
    version="1.2.3",
    bin_name="netsuke",
    is_prerelease=False,
)


@dataclasses.dataclass(frozen=True, slots=True)
class EvidenceChanges:
    """Name the evidence fields a case varies from the shared candidate.

    Attributes
    ----------
    identity
        The candidate the evidence claims to describe.
    crate_name
        The crate asset the download was requested under.
    digests
        The trusted record, or ``None`` for the record every case shares.
    expected_archives
        The archive names the release declared.
    github_sha
        The commit the workflow run reports, which must match the identity.
    """

    identity: release_candidate.ReleaseIdentity = IDENTITY
    crate_name: str = CRATE
    digests: release_digest_record.DigestRecord | None = None
    expected_archives: tuple[str, ...] = (ARCHIVE,)
    github_sha: str = COMMIT


def _record(
    *, archive: bool = True, sidecar: bool = True
) -> release_digest_record.DigestRecord:
    """Build a trusted record covering the archive and sidecar.

    Returns
    -------
    release_digest_record.DigestRecord
        A record holding whichever of the two names were asked for.
    """
    entries: dict[str, str] = {}
    if archive:
        entries[ARCHIVE] = ARCHIVE_DIGEST
    if sidecar:
        entries[SIDECAR] = SIDECAR_DIGEST
    return release_digest_record.DigestRecord.from_mapping(entries)


def _evidence(changes: EvidenceChanges | None = None) -> smoke.SmokeEvidence:
    """Build evidence for one candidate, with per-test overrides.

    Returns
    -------
    smoke.SmokeEvidence
        The shared candidate's evidence, with ``changes`` applied. Only the
        fields a case varies are named there; the rest describe the archive
        and sidecar every case shares.
    """
    chosen = EvidenceChanges() if changes is None else changes
    return smoke.SmokeEvidence(
        identity=chosen.identity,
        crate_name=chosen.crate_name,
        crate_sha256=CRATE_DIGEST,
        digests=_record() if chosen.digests is None else chosen.digests,
        expected_archives=chosen.expected_archives,
        repository="leynos/netsuke",
        target=TARGET,
        github_sha=chosen.github_sha,
    )


def test_plan_binds_the_published_names_and_banner() -> None:
    """Bind the plan to this candidate's names, digests, and banner."""
    plan = smoke.plan_smoke(_evidence())

    assert plan.archive_name == ARCHIVE, (
        "the plan must name the archive the release publishes"
    )
    assert plan.sidecar_name == SIDECAR, (
        "the sidecar is the archive name with the checksum suffix"
    )
    assert (plan.archive_sha256, plan.sidecar_sha256) == (
        ARCHIVE_DIGEST,
        SIDECAR_DIGEST,
    ), "each download must carry the digest its trusted record entry names"
    assert plan.expected_banner == "netsuke 1.2.3", (
        "the banner must spell the candidate version"
    )


@pytest.mark.parametrize(
    "evidence",
    [
        _evidence(EvidenceChanges(identity=dataclasses.replace(IDENTITY, tag=None))),
        _evidence(EvidenceChanges(github_sha="1" * 40)),
        _evidence(
            EvidenceChanges(identity=dataclasses.replace(IDENTITY, tag="v9.9.9"))
        ),
        _evidence(
            EvidenceChanges(
                identity=dataclasses.replace(IDENTITY, package_name="other")
            )
        ),
        _evidence(
            EvidenceChanges(identity=dataclasses.replace(IDENTITY, bin_name="other"))
        ),
        _evidence(EvidenceChanges(crate_name="netsuke-build-1.2.2.crate")),
        _evidence(EvidenceChanges(expected_archives=())),
        _evidence(EvidenceChanges(digests=_record(sidecar=False))),
    ],
    ids=[
        "rehearsal",
        "foreign-commit",
        "tag-mismatch",
        "foreign-package",
        "foreign-binary",
        "crate-mismatch",
        "undeclared-archive",
        "missing-digest",
    ],
)
def test_plan_refuses_evidence_that_names_another_candidate(
    evidence: smoke.SmokeEvidence,
) -> None:
    """Refuse evidence that does not name exactly one publishable candidate."""
    with pytest.raises(smoke.EvidenceRejectedError):
        smoke.plan_smoke(evidence)


@pytest.mark.parametrize(
    "repository",
    ["", "netsuke", "leynos/netsuke/extra", "/netsuke", "../netsuke", "leynos/.."],
)
def test_require_repository_refuses_a_value_no_url_can_use(
    repository: str,
) -> None:
    """Refuse a slug that is not exactly one owner and one repository name."""
    with pytest.raises(smoke.EvidenceRejectedError):
        smoke.require_repository(repository)


def test_require_repository_accepts_a_github_slug() -> None:
    """Accept the slug shape every release URL is built from."""
    assert smoke.require_repository("leynos/netsuke") == "leynos/netsuke", (
        "a valid slug must be returned unchanged"
    )


def test_require_delivered_reports_an_absent_download() -> None:
    """Refuse a download the host did not deliver."""
    facts = payload.PayloadFacts(delivered=False, byte_count=0, sha256=None)

    with pytest.raises(payload.PayloadUnavailableError):
        payload.require_delivered(facts=facts, asset_name=ARCHIVE)


def test_require_delivered_reports_an_empty_download() -> None:
    """Refuse a delivered body with no bytes."""
    facts = payload.PayloadFacts(delivered=True, byte_count=0, sha256=None)

    with pytest.raises(payload.PayloadEmptyError):
        payload.require_delivered(facts=facts, asset_name=ARCHIVE)


def test_require_payload_accepts_the_trusted_digest() -> None:
    """Accept bytes whose digest is the trusted one."""
    facts = payload.PayloadFacts(
        delivered=True, byte_count=7, sha256=ARCHIVE_DIGEST.upper()
    )

    assert (
        payload.require_payload(
            facts=facts, asset_name=ARCHIVE, expected_sha256=ARCHIVE_DIGEST
        )
        == ARCHIVE_DIGEST.upper()
    ), "the observed digest is returned once it matches, ignoring case"


def test_require_payload_refuses_bytes_the_record_does_not_name() -> None:
    """Refuse a body that is not the bytes the trusted record names."""
    facts = payload.PayloadFacts(delivered=True, byte_count=7, sha256="9" * 64)

    with pytest.raises(payload.PayloadMismatchError):
        payload.require_payload(
            facts=facts, asset_name=ARCHIVE, expected_sha256=ARCHIVE_DIGEST
        )


def test_require_payload_refuses_a_download_that_could_not_be_hashed() -> None:
    """Refuse a present body whose digest is unknown."""
    facts = payload.PayloadFacts(delivered=True, byte_count=7, sha256=None)

    with pytest.raises(payload.PayloadDigestMissingError):
        payload.require_payload(
            facts=facts, asset_name=ARCHIVE, expected_sha256=ARCHIVE_DIGEST
        )


@pytest.mark.parametrize("stdout", ["netsuke 1.2.3", "netsuke 1.2.3\n"])
def test_require_probe_accepts_the_banner_with_at_most_one_newline(
    stdout: str,
) -> None:
    """Accept the banner in either accepted trailing-newline shape."""
    assert (
        payload.require_probe(returncode=0, stdout=stdout, version="1.2.3")
        == "netsuke 1.2.3"
    ), "both accepted shapes must return the bare banner"


def test_require_probe_preserves_a_prerelease_suffix() -> None:
    """Require the prerelease suffix to be reported exactly."""
    assert (
        payload.require_probe(
            returncode=0, stdout="netsuke 1.2.3-beta4\n", version="1.2.3-beta4"
        )
        == "netsuke 1.2.3-beta4"
    ), "a prerelease version must be matched in full"


@pytest.mark.parametrize(
    ("returncode", "stdout", "error"),
    [
        (1, "netsuke 1.2.3\n", payload.ProbeExitError),
        (0, "", payload.EmptyBannerError),
        (0, "  \n", payload.EmptyBannerError),
        (0, "netsuke 1.2.3\nextra\n", payload.BannerMismatchError),
        (0, "prefix netsuke 1.2.3\n", payload.BannerMismatchError),
        (0, "netsuke 1.2.3-beta4\n", payload.BannerMismatchError),
        (0, "netsuke 1.2.4\n", payload.BannerMismatchError),
    ],
    ids=[
        "exit",
        "blank",
        "whitespace",
        "extra-line",
        "substring",
        "prerelease",
        "other",
    ],
)
def test_require_probe_refuses_anything_but_the_exact_banner(
    returncode: int, stdout: str, error: type[Exception]
) -> None:
    """Refuse a probe that did not report exactly the candidate version."""
    with pytest.raises(error):
        payload.require_probe(returncode=returncode, stdout=stdout, version="1.2.3")


def test_verdict_reports_verified_only_when_every_phase_succeeded() -> None:
    """Report ``verified`` only for five successful phases."""
    phases = dict.fromkeys(payload.SMOKE_PHASES, "success")

    assert payload.verdict_record(phases) == {
        "phases": phases,
        "verdict": "verified",
    }, "every phase succeeding must be the only path to a verified verdict"


def test_verdict_records_absent_phases_as_skipped() -> None:
    """Fill an absent label with ``skipped`` and report a failure."""
    record = payload.verdict_record({"crate_download": "success"})

    assert record["phases"]["archive_download"] == "skipped", (
        "a phase that never ran must be recorded as skipped"
    )
    assert record["verdict"] == "failed", (
        "a partially completed run is not a verified run"
    )


def test_verdict_refuses_an_unregistered_outcome() -> None:
    """Refuse an outcome the artefact's consumers do not know."""
    with pytest.raises(smoke.ConsumerSmokeError):
        payload.verdict_record({"crate_download": "maybe"})

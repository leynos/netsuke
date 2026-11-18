"""Decide which public downloads one consumer smoke must exercise.

The release smoke fetches the crate and the cargo-binstall archive from the
public URLs every consumer uses, then runs the binary the archive ships. This
module owns the decisions that need no I/O: whether the trusted evidence names
one publishable candidate, and which names and digests that candidate must be
fetched under.

File reads, archive parsing, extraction, and process launch belong to
:mod:`release_binstall_archive`, :mod:`release_binstall_extract`,
:mod:`release_binstall_probe`, and :mod:`consumer_smoke_cli`. Deciding the plan
from in-memory values is what lets a unit test exercise every refusal without a
network, a tarball, or a child process — the same split
:mod:`verify_crates_io_bytes` draws from its registry adapter.

The published asset spelling comes from
:func:`release_github_assets_adapter.published_asset_name` rather than being
restated: the trusted record is keyed by the name GitHub publishes, so a second
copy of the ``~``-to-``.`` rule would let the planner and uploader disagree.

What the smoke then decides about a *saved* download — whether the host
delivered it, whether its bytes are the published ones, and whether the shipped
binary reports the candidate version — lives in :mod:`release_consumer_payload`.

Run through ``scripts/consumer_smoke_cli.py``; behavioural coverage lives in
``scripts/tests/test_release_consumer_smoke.py``.
"""

import dataclasses
import re
import typing as typ

import release_github_assets_adapter
from release_asset_names import escapes_its_directory, is_bare_file_name

if typ.TYPE_CHECKING:
    import release_candidate
    import release_digest_record

#: The Cargo package the public consumer installs. A release of any other
#: package is not what this smoke exists to prove.
EXPECTED_PACKAGE = "netsuke-build"

#: The installed binary name, which differs from the package name.
EXPECTED_BIN_NAME = "netsuke"

#: Suffix of the crate archive the registry serves.
CRATE_SUFFIX = ".crate"

#: Suffix of the cargo-binstall archive the release publishes.
ARCHIVE_SUFFIX = ".tar.gz"

#: Suffix of the checksum sidecar published beside each archive.
SIDECAR_SUFFIX = ".sha256"

#: One GitHub repository slug component. A dot is permitted because repository
#: names may carry one, but the component must start alphanumerically so a
#: value such as ``..`` can never pass.
_REPOSITORY_PART = re.compile(r"\A[A-Za-z0-9][A-Za-z0-9._-]*\Z")


class ConsumerSmokeError(ValueError):
    """Report a smoke check that cannot vouch for the published candidate."""


class EvidenceRejectedError(ConsumerSmokeError):
    """Report trusted evidence that does not name one publishable candidate."""


@dataclasses.dataclass(frozen=True, slots=True)
class SmokeEvidence:
    """Hold the same-run facts a smoke plan is bound to.

    Parameters
    ----------
    identity
        The candidate the packaging stage recorded and re-validated on read.
    crate_name
        The archive name the package record spells.
    crate_sha256
        Lowercase digest of the packaged crate, from the package record.
    digests
        The trusted pre-upload digest record, keyed by published asset name.
    expected_archives
        Every archive name the configuration declares for this version.
    repository
        The ``owner/name`` GitHub slug the release was published under.
    target
        The Rust target triple the archive is built for.
    github_sha
        The commit this workflow run checked out.
    """

    identity: release_candidate.ReleaseIdentity
    crate_name: str
    crate_sha256: str
    digests: release_digest_record.DigestRecord
    expected_archives: tuple[str, ...]
    repository: str
    target: str
    github_sha: str


@dataclasses.dataclass(frozen=True, slots=True)
class SmokePlan:
    """Record the exact bytes and names one smoke run must exercise."""

    repository: str
    tag: str
    commit: str
    package_name: str
    version: str
    target: str
    bin_name: str
    crate_name: str
    crate_sha256: str
    archive_name: str
    sidecar_name: str
    archive_sha256: str
    sidecar_sha256: str
    expected_banner: str


def require_repository(repository: str) -> str:
    """Return ``repository`` when it is one ``owner/name`` GitHub slug.

    Returns
    -------
    str
        The validated slug.

    Raises
    ------
    EvidenceRejectedError
        If the value is not exactly one owner, a slash, and one name.

    Examples
    --------
    >>> require_repository("leynos/netsuke")
    'leynos/netsuke'
    >>> require_repository("../netsuke")  # doctest: +ELLIPSIS
    Traceback (most recent call last):
        ...
    release_consumer_smoke.EvidenceRejectedError: repository ...
    """
    if not _is_owner_name_slug(repository):
        msg = (
            f"repository {repository!r} is not an 'owner/name' GitHub slug; "
            "the public release URL cannot be built from it"
        )
        raise EvidenceRejectedError(msg)
    return repository


def _is_owner_name_slug(repository: str) -> bool:
    """Return whether ``repository`` is exactly one owner and one name.

    Returns
    -------
    bool
        Whether both components are single safe slug parts.

    Notes
    -----
    An extra slash is refused because it would put a path segment between the
    owner and the asset that the download host does not serve.
    """
    owner, separator, name = repository.partition("/")
    if not separator or "/" in name:
        return False
    return all(
        _REPOSITORY_PART.match(component) is not None for component in (owner, name)
    )


def expected_banner(version: str) -> str:
    """Return the exact standard output the shipped binary must print.

    Returns
    -------
    str
        The binary's name and the candidate version, separated by one space.

    Examples
    --------
    >>> expected_banner("1.2.3-beta4")
    'netsuke 1.2.3-beta4'
    """
    return f"{EXPECTED_BIN_NAME} {version}"


def plan_smoke(evidence: SmokeEvidence) -> SmokePlan:
    """Bind the trusted evidence to one exact set of public downloads.

    Parameters
    ----------
    evidence
        The identity, crate record, digest record, declaration, and run facts
        gathered by the workflow.

    Returns
    -------
    SmokePlan
        The published names, digests, and expected banner for this run.

    Notes
    -----
    Nothing here compares bytes. The plan says which bytes must be compared,
    and :func:`release_consumer_payload.require_payload` decides whether the
    saved downloads are them.

    Every refusal is an :class:`EvidenceRejectedError`: a rehearsal identity, a
    commit other than this run's, a package or binary this smoke does not
    exercise, a crate name that spells another version, an archive the release
    configuration never declares, or a missing trusted digest.

    Examples
    --------
    >>> import release_candidate, release_digest_record
    >>> identity = release_candidate.ReleaseIdentity(
    ...     "v1.2.3", "0" * 40, "netsuke-build", "1.2.3", "netsuke", False
    ... )
    >>> record = release_digest_record.DigestRecord.from_mapping({
    ...     "netsuke-build-1.2.3-x86_64-unknown-linux-gnu.tar.gz": "ab" * 32,
    ...     "netsuke-build-1.2.3-x86_64-unknown-linux-gnu.tar.gz.sha256": "cd" * 32,
    ... })
    >>> evidence = SmokeEvidence(
    ...     identity=identity,
    ...     crate_name="netsuke-build-1.2.3.crate",
    ...     crate_sha256="ef" * 32,
    ...     digests=record,
    ...     expected_archives=(
    ...         "netsuke-build-1.2.3-x86_64-unknown-linux-gnu.tar.gz",
    ...     ),
    ...     repository="leynos/netsuke",
    ...     target="x86_64-unknown-linux-gnu",
    ...     github_sha="0" * 40,
    ... )
    >>> plan = plan_smoke(evidence)
    >>> plan.expected_banner
    'netsuke 1.2.3'
    """
    identity = evidence.identity
    _ = require_repository(evidence.repository)
    _require_candidate(identity, evidence.github_sha)
    _require_crate_name(evidence.crate_name, identity)
    archive = _published_archive(evidence, identity)
    sidecar = f"{archive}{SIDECAR_SUFFIX}"
    published = evidence.digests.as_mapping()
    return SmokePlan(
        repository=evidence.repository,
        tag=_required_tag(identity),
        commit=identity.commit,
        package_name=identity.package_name,
        version=identity.version,
        target=evidence.target,
        bin_name=identity.bin_name,
        crate_name=evidence.crate_name,
        crate_sha256=evidence.crate_sha256,
        archive_name=archive,
        sidecar_name=sidecar,
        archive_sha256=_require_digest(published, archive),
        sidecar_sha256=_require_digest(published, sidecar),
        expected_banner=expected_banner(identity.version),
    )


def _required_tag(identity: release_candidate.ReleaseIdentity) -> str:
    """Return the release tag, refusing a rehearsal identity.

    Returns
    -------
    str
        The tag the identity records.

    Raises
    ------
    EvidenceRejectedError
        If the identity records no tag.
    """
    if identity.tag is None:
        msg = (
            "the recorded identity names no tag, so it is a rehearsal record; "
            "a public-consumer smoke runs only for a published candidate"
        )
        raise EvidenceRejectedError(msg)
    return identity.tag


def _require_candidate(
    identity: release_candidate.ReleaseIdentity, github_sha: str
) -> None:
    """Refuse an identity that is not this run's public candidate.

    Raises
    ------
    EvidenceRejectedError
        If the tag does not spell the version, the commit differs from the
        run's, or the package or binary is not the one this smoke exercises.
    """
    tag = _required_tag(identity)
    if tag != f"v{identity.version}":
        msg = f"tag {tag!r} does not spell version {identity.version!r}"
        raise EvidenceRejectedError(msg)
    if identity.commit != github_sha:
        msg = (
            f"the identity records commit {identity.commit}, but this run "
            f"checked out {github_sha}"
        )
        raise EvidenceRejectedError(msg)
    _require_identity_fields(identity)


def _require_identity_fields(identity: release_candidate.ReleaseIdentity) -> None:
    """Refuse an identity naming a package or binary this smoke does not use.

    Raises
    ------
    EvidenceRejectedError
        If the package is not :data:`EXPECTED_PACKAGE`, or the binary is not
        :data:`EXPECTED_BIN_NAME`.
    """
    named = (
        (identity.package_name, EXPECTED_PACKAGE, "package"),
        (identity.bin_name, EXPECTED_BIN_NAME, "binary"),
    )
    for observed, expected, role in named:
        if observed != expected:
            msg = f"the identity names {role} {observed!r}, not {expected!r}"
            raise EvidenceRejectedError(msg)


def _require_crate_name(
    crate_name: str, identity: release_candidate.ReleaseIdentity
) -> None:
    """Require the packaged crate name to spell the candidate's version.

    Raises
    ------
    EvidenceRejectedError
        If the crate name is not ``{package}-{version}.crate``.
    """
    expected = f"{identity.package_name}-{identity.version}{CRATE_SUFFIX}"
    if crate_name != expected:
        msg = f"the package record names crate {crate_name!r}, not {expected!r}"
        raise EvidenceRejectedError(msg)


def _published_archive(
    evidence: SmokeEvidence, identity: release_candidate.ReleaseIdentity
) -> str:
    """Derive the published archive name and refuse an undeclared one.

    Returns
    -------
    str
        The archive name as GitHub publishes it.

    Raises
    ------
    EvidenceRejectedError
        If the configured targets do not declare this archive, or the
        published spelling is not one safe bare file name.
    """
    staged = (
        f"{identity.package_name}-{identity.version}-{evidence.target}{ARCHIVE_SUFFIX}"
    )
    if staged not in evidence.expected_archives:
        msg = (
            f"the release configuration does not declare {staged!r}; the "
            "smoke would download a URL no producer stages"
        )
        raise EvidenceRejectedError(msg)
    published = release_github_assets_adapter.published_asset_name(staged)
    if escapes_its_directory(published) or not is_bare_file_name(published):
        msg = f"the published archive name {published!r} is not a bare file name"
        raise EvidenceRejectedError(msg)
    return published


def _require_digest(published: dict[str, str], name: str) -> str:
    """Return the trusted digest for ``name``, refusing an absent entry.

    Returns
    -------
    str
        The digest the trusted record holds under ``name``.

    Raises
    ------
    EvidenceRejectedError
        If the trusted record holds no digest under ``name``.
    """
    digest = published.get(name)
    if digest is None:
        msg = f"the trusted digest record holds no entry for {name!r}"
        raise EvidenceRejectedError(msg)
    return digest

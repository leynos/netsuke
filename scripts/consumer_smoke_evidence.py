"""Read the trusted records and saved downloads a smoke run is judged against.

The smoke is only as trustworthy as the evidence it starts from. This module
reads that evidence — the package record the packaging stage wrote, the
trusted pre-upload digest record, the release declaration, and the HTTP status
each public download was saved under — and turns it into the records the plan
and the payload decisions consume. It also renders and re-checks the plan
document the two commands exchange.

Nothing here decides whether a candidate is publishable or whether bytes are
the right ones; those live in :mod:`release_consumer_smoke` and
:mod:`release_consumer_payload`. Nor does anything here fetch: the workflow
downloads, and this module reads what was saved, so the same public URLs are
exercised while the Python side stays network-free.

Run through ``scripts/consumer_smoke_cli.py``; behavioural coverage lives in
``scripts/tests/test_consumer_smoke_cli.py``.
"""

import contextlib
import json
import pathlib
import tempfile
import typing as typ

import hoist_binstall_discovery
import release_candidate
import release_consumer_payload as payload
import release_consumer_smoke as smoke
import release_crates_io_adapter
import release_digest_files
import release_digest_record
import release_digest_record_files
import release_github_assets_adapter
import release_identity_record
from release_consumer_smoke import EvidenceRejectedError

if typ.TYPE_CHECKING:
    from consumer_smoke_options import EvidenceOptions


def read_json(source: pathlib.Path) -> object:
    """Read one JSON document, refusing anything that is not valid JSON.

    Returns
    -------
    object
        The decoded document, which the caller narrows to the shape it needs.

    Raises
    ------
    EvidenceRejectedError
        If the bytes are not UTF-8 JSON text.

    Notes
    -----
    A file that cannot be read at all is an :class:`OSError`, which callers
    report as unreadable evidence without this function having to guess why.
    """
    try:
        return json.loads(source.read_bytes())
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        msg = f"{source} is not usable JSON text: {error}"
        raise EvidenceRejectedError(msg) from error


def write_json(destination: pathlib.Path, value: object) -> None:
    """Atomically write ``value`` as sorted, newline-terminated JSON.

    The temporary file is a sibling of ``destination``, so the replacement
    stays on one filesystem and a reader never observes a partial document.

    Raises
    ------
    OSError
        If the temporary file cannot be written, removed, or replaced.
    """
    temporary_path: pathlib.Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            dir=destination.parent,
            prefix=f".{destination.name}.",
            suffix=".tmp",
            delete=False,
        ) as temporary:
            temporary_path = pathlib.Path(temporary.name)
            _ = temporary.write(json.dumps(value, indent=2, sort_keys=True) + "\n")
        temporary_path.replace(destination)
    except OSError:
        if temporary_path is not None:
            with contextlib.suppress(OSError):
                temporary_path.unlink(missing_ok=True)
        raise


def package_record(
    source: pathlib.Path,
) -> tuple[release_candidate.ReleaseIdentity, str, str]:
    """Decode the package record's identity, crate name, and crate digest.

    Returns
    -------
    tuple[release_candidate.ReleaseIdentity, str, str]
        The re-validated identity, the crate's file name, and its lowercase
        digest.

    Raises
    ------
    EvidenceRejectedError
        If the record is not a package record, or names an identity or digest
        the release policy refuses.

    Notes
    -----
    A record that cannot be read fails as an :class:`OSError`.
    """
    envelope = read_json(source)
    recorded, crate, digest = _record_fields(source, envelope)
    try:
        identity = release_identity_record.identity_from_mapping(recorded)
        validated = release_digest_record.DigestRecord.from_mapping({crate: digest})
    except (
        release_candidate.IdentityError,
        release_digest_record.DigestRecordError,
    ) as error:
        msg = f"{source} is not a usable package record: {error}"
        raise EvidenceRejectedError(msg) from error
    return identity, crate, validated.as_mapping()[crate]


def _record_fields(
    source: pathlib.Path, envelope: object
) -> tuple[dict[str, object], str, str]:
    """Return the identity mapping, crate name, and digest a record declares.

    Returns
    -------
    tuple[dict[str, object], str, str]
        The raw identity mapping, the crate's file name, and its digest as the
        record spelled it, all still to be validated.

    Raises
    ------
    EvidenceRejectedError
        If the record is not a JSON object, or does not name a release
        identity and both a crate archive and its digest.
    """
    if not isinstance(envelope, dict):
        msg = f"{source} does not hold a JSON object"
        raise EvidenceRejectedError(msg)
    recorded = envelope.get("identity")
    crate = envelope.get("crate")
    digest = envelope.get("sha256")
    if not isinstance(recorded, dict):
        msg = f"{source} holds no release identity"
        raise EvidenceRejectedError(msg)
    if not isinstance(crate, str) or not isinstance(digest, str):
        msg = f"{source} does not name both a crate archive and its digest"
        raise EvidenceRejectedError(msg)
    return recorded, crate, digest


def declared_archives(
    staging_config: pathlib.Path, manifest: pathlib.Path, version: str
) -> tuple[str, ...]:
    """Return the archive names the release configuration declares.

    Returns
    -------
    tuple[str, ...]
        Every archive name the configuration declares for this version, in
        staged spelling.

    Raises
    ------
    EvidenceRejectedError
        If either file does not declare a usable target set or package name.

    Notes
    -----
    A configuration that cannot be read fails as an :class:`OSError`.
    """
    try:
        return tuple(
            hoist_binstall_discovery.expected_archive_names(
                staging_config, manifest, version
            )
        )
    except (ValueError, KeyError) as error:
        msg = (
            f"the release configuration could not declare the archives for "
            f"version {version!r}: {error}"
        )
        raise EvidenceRejectedError(msg) from error


def gather_evidence(arguments: EvidenceOptions) -> smoke.SmokeEvidence:
    """Gather the trusted evidence one plan is bound to.

    Returns
    -------
    release_consumer_smoke.SmokeEvidence
        The identity, crate record, digest record, declared archives, and run
        facts the plan is derived from.

    Raises
    ------
    EvidenceRejectedError
        If the trusted digest record is unusable.

    Notes
    -----
    The package record and the release declaration refuse their own contents,
    and a file that cannot be read fails as an :class:`OSError`; both travel
    through this function unchanged.
    """
    identity, crate, digest = package_record(arguments.identity)
    try:
        digests = release_digest_record_files.read_digest_record(arguments.trusted)
    except release_digest_record.DigestRecordError as error:
        msg = f"the trusted digest record {arguments.trusted} is unusable: {error}"
        raise EvidenceRejectedError(msg) from error
    return smoke.SmokeEvidence(
        identity=identity,
        crate_name=crate,
        crate_sha256=digest,
        digests=digests,
        expected_archives=declared_archives(
            arguments.staging_config, arguments.manifest, identity.version
        ),
        repository=arguments.repository,
        target=arguments.target,
        github_sha=arguments.commit,
    )


def download_document(name: str, url: str, sha256: str) -> dict[str, str]:
    """Describe one download the smoke must perform.

    Returns
    -------
    dict[str, str]
        The asset name, the public URL it is served from, and its trusted
        digest.

    Examples
    --------
    >>> download_document("a.crate", "https://example.invalid/a.crate", "ab")["name"]
    'a.crate'
    """
    return {"name": name, "url": url, "sha256": sha256}


def plan_document(plan: smoke.SmokePlan) -> dict[str, object]:
    """Render the plan as the JSON the workflow downloads and re-checks.

    Returns
    -------
    dict[str, object]
        The candidate facts, one entry per download with its public URL and
        trusted digest, and the request's User-Agent so the workflow curl does
        not restate the header policy.
    """
    release_url = release_github_assets_adapter.public_download_url
    return {
        "repository": plan.repository,
        "tag": plan.tag,
        "commit": plan.commit,
        "package_name": plan.package_name,
        "version": plan.version,
        "target": plan.target,
        "bin_name": plan.bin_name,
        "expected_banner": plan.expected_banner,
        "user_agent": release_crates_io_adapter.required_headers()["User-Agent"],
        "crate": download_document(
            plan.crate_name,
            release_crates_io_adapter.download_url(plan.package_name, plan.version),
            plan.crate_sha256,
        ),
        "archive": download_document(
            plan.archive_name,
            release_url(plan.repository, plan.tag, plan.archive_name),
            plan.archive_sha256,
        ),
        "sidecar": download_document(
            plan.sidecar_name,
            release_url(plan.repository, plan.tag, plan.sidecar_name),
            plan.sidecar_sha256,
        ),
    }


def require_saved_plan(source: pathlib.Path, plan: smoke.SmokePlan) -> None:
    """Require the saved plan to be exactly the one this evidence derives.

    Raises
    ------
    EvidenceRejectedError
        If the saved plan is not the derived one. Verifying bytes another
        run's plan named would check the wrong download under the right name.

    Notes
    -----
    A saved plan that cannot be read fails as an :class:`OSError`.
    """
    saved = read_json(source)
    expected = plan_document(plan)
    if saved != expected:
        msg = (
            f"the saved plan {source} is not the plan this evidence derives; "
            "refusing to verify bytes a different plan named"
        )
        raise EvidenceRejectedError(msg)


def recorded_status(source: pathlib.Path) -> int | None:
    """Return the HTTP status recorded beside a download, or ``None``.

    Returns
    -------
    int | None
        The status curl wrote, or ``None`` when no status was recorded or the
        file holds something other than an integer.

    Notes
    -----
    Only an absent file means no status at all. A file present but holding a
    non-number is also no status: ``curl`` writes ``000`` after a transport
    failure and removes the body, so a status that is not a number cannot come
    from a transfer that completed. Bytes that are not UTF-8 are no status for
    the same reason — a decimal status is ASCII — and they are caught here
    because :class:`UnicodeDecodeError` is a :class:`ValueError` raised by the
    read itself, so it would otherwise escape the check below.

    A file that exists but cannot be read propagates its :class:`OSError`. A
    host failure is kept visible so the caller reports an unreadable download
    rather than a download the host never delivered, which are different
    diagnoses an operator acts on differently.
    """
    try:
        content = source.read_text(encoding="utf-8")
    except FileNotFoundError, UnicodeDecodeError:
        return None
    try:
        return int(content.strip())
    except ValueError:
        return None


def payload_facts(*, body: pathlib.Path, status: pathlib.Path) -> payload.PayloadFacts:
    """Describe one saved download from its body and recorded status.

    Returns
    -------
    release_consumer_payload.PayloadFacts
        Whether the host delivered the body, how many bytes it holds, and its
        digest when there are bytes to hash.

    Notes
    -----
    An unreadable recorded status propagates its :class:`OSError` from
    :func:`recorded_status`; the caller translates it into a download refusal
    instead of reporting the download as absent.
    """
    delivered = (
        recorded_status(status) == release_crates_io_adapter.SUCCESS_STATUS
        and body.is_file()
    )
    if not delivered:
        return payload.PayloadFacts(delivered=False, byte_count=0, sha256=None)
    byte_count = body.stat().st_size
    digest = release_digest_files.digest_of_file(body) if byte_count else None
    return payload.PayloadFacts(delivered=True, byte_count=byte_count, sha256=digest)

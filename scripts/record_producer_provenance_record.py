"""Record what one producer built, from the bytes it is about to hand on.

Each release producer runs after packaging and before upload. At that moment
it alone holds the archive, its sidecar, and the operating-system packages it
just built, and it is the only process that can say which commit produced
them. This module turns that into a small JSON record: the candidate commit,
the run identity, the configured target and platform, and the SHA-256 digest
of every artefact, keyed by the name the release will publish. "Publish" is
GitHub's spelling, not the staged one, and :mod:`release_github_assets_adapter`
owns that translation: a record keyed by the staged spelling would name an
asset the declaration never declares whenever a prerelease package has ``~``.

Only files named explicitly are hashed. A tree walk would put retained
workspace material and unrelated files into the record, which is the opposite
of what it exists to prove: this producer answers for these bytes and no more.

The record is written beside the artefacts and travels to publication in the
same GitHub artefact. Publication compares the downloaded bytes against it, so
a substituted package, a replaced archive, or an artefact from an older commit
is visible even when a matching replacement sidecar was supplied.

Parsing arguments and choosing exit codes belong to
:mod:`record_producer_provenance_cli`.

Run by the build workflow's producer step; behavioural coverage lives in
``scripts/tests/test_record_producer_provenance_cli.py``.
"""

import dataclasses
import json
import pathlib
import stat
import tempfile
import typing as typ

import release_asset_names
import release_digest_files
import release_github_assets_adapter
import release_package_formats
import release_producer_provenance
import release_producer_provenance_decode
from record_producer_provenance_errors import (
    DoublyPublishedNameError,
    DuplicatePackageFormatError,
    IrregularArtifactError,
    MissingPackageFormatError,
    ProvenanceRefusedError,
    UnreadableArtifactError,
    UnrecordedPlatformError,
    UnsafePublishedNameError,
)
from release_producer_provenance_errors import ProvenanceError

if typ.TYPE_CHECKING:
    import collections.abc as cabc

__all__ = [
    "RECORD_NAME",
    "ProducerIdentity",
    "record",
    "recorded_assets",
    "write_record",
]

#: The file name every producer writes its record under.
RECORD_NAME = "producer-provenance.json"


@dataclasses.dataclass(frozen=True, slots=True)
class ProducerIdentity:
    """Name the candidate, run, target, and platform a producer claims.

    The five values travel together everywhere: the record carries them, the
    policy validates them, and the producer step passes them from the workflow
    environment. Grouping them keeps a caller from transposing two adjacent
    strings that mean very different things.

    Attributes
    ----------
    commit
        The full lowercase candidate commit the producer built.
    run_id
        The workflow run that produced the artefacts.
    run_attempt
        The attempt within that run.
    target
        The configured staging target the producer served.
    platform
        The platform whose package formats the target stages.
    """

    commit: str
    run_id: str
    run_attempt: str
    target: str
    platform: str


def record(
    identity: ProducerIdentity, assets: cabc.Mapping[str, str]
) -> dict[str, object]:
    """Assemble the record mapping from a claimed identity and digests.

    Parameters
    ----------
    identity
        The candidate, run, target, and platform the producer claims.
    assets
        Published name mapped to the digest the producer measured.

    Returns
    -------
    dict[str, object]
        The mapping the JSON writer serializes.

    Raises
    ------
    ProvenanceRefusedError
        If the values do not describe a record publication can read back.

    Examples
    --------
    >>> identity = ProducerIdentity(
    ...     commit="a" * 40, run_id="7", run_attempt="1",
    ...     target="linux-x86_64", platform="linux",
    ... )
    >>> record(identity, {"asset.deb": "b" * 64})["commit"]
    'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
    """
    mapping = {
        release_producer_provenance.SCHEMA_MEMBER: (
            release_producer_provenance_decode.SCHEMA_VERSION
        ),
        release_producer_provenance.COMMIT_MEMBER: identity.commit,
        release_producer_provenance.RUN_ID_MEMBER: identity.run_id,
        release_producer_provenance.RUN_ATTEMPT_MEMBER: identity.run_attempt,
        release_producer_provenance.TARGET_MEMBER: identity.target,
        release_producer_provenance.PLATFORM_MEMBER: identity.platform,
        release_producer_provenance.ASSETS_MEMBER: dict(assets),
    }
    # The producer validates its own output through the same decoder
    # publication uses, so a malformed claim fails here rather than after the
    # artefacts are uploaded.
    try:
        _ = release_producer_provenance_decode.decode_provenance(mapping)
    except ProvenanceError as error:
        raise ProvenanceRefusedError(str(error)) from error
    return mapping


def recorded_assets(
    paths: cabc.Iterable[pathlib.Path], *, platform: str, target: str
) -> dict[str, str]:
    """Hash each named artefact, keyed by the name it will publish under.

    Returns
    -------
    dict[str, str]
        Published name mapped to its lowercase SHA-256 digest.

    Raises
    ------
    UnrecordedPlatformError
        If the platform stages no package format this release can name.
    DoublyPublishedNameError
        If two artefacts publish under one name.

    Notes
    -----
    A path that is not a regular file surfaces ``IrregularArtifactError`` or
    ``UnreadableArtifactError`` from :func:`_require_regular`; an unsafe name
    surfaces ``UnsafePublishedNameError`` from :func:`_published_name`; an
    unreadable digest surfaces ``UnreadableArtifactError`` from
    :func:`_digest`; and an incomplete or ambiguous package set surfaces
    ``MissingPackageFormatError`` or ``DuplicatePackageFormatError`` from
    :func:`_require_every_format`.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     deb = Path(tmp) / "netsuke_1_amd64.deb"
    ...     rpm = Path(tmp) / "netsuke-1.x86_64.rpm"
    ...     _ = deb.write_bytes(b"deb")
    ...     _ = rpm.write_bytes(b"rpm")
    ...     sorted(recorded_assets([deb, rpm], platform="linux", target="linux"))
    ['netsuke-1.x86_64.rpm', 'netsuke_1_amd64.deb']
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     try:
    ...         recorded_assets(
    ...             [Path(tmp) / "absent.deb"], platform="linux", target="linux"
    ...         )
    ...     except UnreadableArtifactError as refusal:
    ...         print("absent" in str(refusal))
    True
    """
    if platform not in release_package_formats.PLATFORM_FORMATS:
        raise UnrecordedPlatformError(platform)
    assets: dict[str, str] = {}
    for path in paths:
        name = _published_name(path)
        if name in assets:
            raise DoublyPublishedNameError(name)
        assets[name] = _digest(_require_regular(path))
    _require_every_format(assets, platform=platform, target=target)
    return dict(sorted(assets.items()))


def _published_name(path: pathlib.Path) -> str:
    """Return the name an artefact publishes under, refusing an unsafe one.

    The name is translated the way the release declaration translates it, so
    the record and the declaration key the same bytes under one name. A
    prerelease RPM staged as ``netsuke-1.0.0~beta4-1.x86_64.rpm`` publishes as
    ``netsuke-1.0.0.beta4-1.x86_64.rpm``; a record holding the staged spelling
    would describe an asset the release never declares.

    Returns
    -------
    str
        The bare published name of the artefact.

    Raises
    ------
    UnsafePublishedNameError
        If the staged name is not one bare file name under either path flavour.

    Examples
    --------
    >>> _published_name(pathlib.Path("/tmp/netsuke.tar.gz"))
    'netsuke.tar.gz'
    >>> _published_name(pathlib.Path("/tmp/netsuke-1~beta-1.x86_64.rpm"))
    'netsuke-1.beta-1.x86_64.rpm'
    >>> try:
    ...     _published_name(pathlib.Path(".."))
    ... except UnsafePublishedNameError as refusal:
    ...     print("unsafe" in str(refusal))
    True
    """
    staged = path.name
    # Safety is judged on the staged spelling: the translation only rewrites
    # characters, so a name that was one bare component stays one.
    if not release_asset_names.is_bare_file_name(staged):
        raise UnsafePublishedNameError(staged)
    return release_github_assets_adapter.published_asset_name(staged)


def _digest(path: pathlib.Path) -> str:
    """Return the SHA-256 digest of the bytes at ``path``.

    Returns
    -------
    str
        The lowercase hexadecimal digest.

    Raises
    ------
    UnreadableArtifactError
        If the bytes cannot be read, so a read failure is not reported as an
        unwritable output.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     asset = Path(tmp) / "asset.bin"
    ...     _ = asset.write_bytes(b"")
    ...     _digest(asset)
    'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'
    """
    try:
        return release_digest_files.digest_of_file(path)
    except OSError as failure:
        raise UnreadableArtifactError(path, failure) from failure


def _require_regular(path: pathlib.Path) -> pathlib.Path:
    """Return ``path`` when it is a regular file, refusing anything else.

    Returns
    -------
    pathlib.Path
        The same path, once proven regular.

    Raises
    ------
    UnreadableArtifactError
        If the path cannot be inspected at all.
    IrregularArtifactError
        If the path is a symlink, a directory, or another special file.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     regular = Path(tmp) / "asset.bin"
    ...     _ = regular.write_bytes(b"bytes")
    ...     _require_regular(regular) == regular
    True
    """
    try:
        mode = path.lstat().st_mode
    except OSError as failure:
        raise UnreadableArtifactError(path, failure) from failure
    # A symlink to a regular file would let this producer vouch for bytes that
    # are not in the directory it is handing on; the lstat above means the mode
    # describes the link itself, which is not a regular file.
    if not stat.S_ISREG(mode):
        raise IrregularArtifactError(path)
    return path


def _require_every_format(
    assets: dict[str, str], *, platform: str, target: str
) -> None:
    """Refuse a producer that did not record every package format it owes.

    Raises
    ------
    MissingPackageFormatError
        If a format the platform must stage is absent from the record.

    Notes
    -----
    A format recorded more than once surfaces
    ``DuplicatePackageFormatError`` from :func:`_require_one_per_format`.

    Examples
    --------
    >>> try:
    ...     _require_every_format(
    ...         {"netsuke_1_amd64.deb": "a" * 64}, platform="linux", target="t"
    ...     )
    ... except MissingPackageFormatError as refusal:
    ...     print(".rpm" in str(refusal))
    True
    >>> _require_every_format(
    ...     {"a.deb": "a" * 64, "b.rpm": "b" * 64}, platform="linux", target="t"
    ... )
    """
    names = list(assets)
    missing = [
        package_format
        for package_format in release_package_formats.PLATFORM_FORMATS[platform]
        if not release_package_formats.match_format(names, package_format)
    ]
    if missing:
        raise MissingPackageFormatError(target, missing)
    for package_format in release_package_formats.PLATFORM_FORMATS[platform]:
        _require_one_per_format(names, package_format, target=target)


def _require_one_per_format(
    names: list[str], package_format: str, *, target: str
) -> None:
    """Refuse a producer that recorded two packages of one format.

    Raises
    ------
    DuplicatePackageFormatError
        If more than one name carries ``package_format``.

    Examples
    --------
    >>> _require_one_per_format(["a.deb"], ".deb", target="t")
    >>> try:
    ...     _require_one_per_format(["a.deb", "b.deb"], ".deb", target="t")
    ... except DuplicatePackageFormatError as refusal:
    ...     print("one" in str(refusal))
    True
    """
    matches = release_package_formats.match_format(names, package_format)
    if len(matches) > 1:
        raise DuplicatePackageFormatError(target, package_format, len(matches))


def write_record(record: cabc.Mapping[str, object], output: pathlib.Path) -> None:
    """Install the record atomically, leaving no partial file behind.

    The temporary directory is a sibling of the destination, so the final
    rename stays within one filesystem and cannot expose a half-written
    record to a concurrent reader.

    Examples
    --------
    >>> with tempfile.TemporaryDirectory() as directory:
    ...     destination = pathlib.Path(directory) / "record.json"
    ...     write_record({"schema": 1}, destination)
    ...     json.loads(destination.read_text(encoding="utf-8"))
    {'schema': 1}
    """
    with tempfile.TemporaryDirectory(
        dir=output.parent, prefix=f".{output.name}."
    ) as temporary:
        staged = pathlib.Path(temporary) / RECORD_NAME
        _ = staged.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
        _ = staged.replace(output)

"""Read the retained crate archive and the manifest it packages.

This adapter owns the tar and TOML boundary of candidate validation. It
selects the single archived ``.crate`` recorded by the identity artifact and
reads the ``Cargo.toml`` that archive packages, so the command-line module can
compare both against the release workflow without opening an archive itself.
"""

import io
import tarfile
import tomllib
import typing as typ

import release_asset_names
from crates_io_candidate_errors import CandidateError, CandidateIssue

if typ.TYPE_CHECKING:
    import pathlib


def _crate_basename(record: dict[str, object]) -> str:
    """Require the recorded crate name to be a flat archive basename."""
    crate = record.get("crate")
    if not isinstance(crate, str) or not release_asset_names.is_bare_file_name(crate):
        raise CandidateError(CandidateIssue.CRATE_BASENAME)
    if not crate.endswith(".crate"):
        raise CandidateError(CandidateIssue.CRATE_SUFFIX, (crate,))
    return crate


def archive_and_crate(
    record: dict[str, object], directory: pathlib.Path
) -> tuple[str, pathlib.Path]:
    """Select the sole archive after validating its recorded basename."""
    crate = _crate_basename(record)
    archives = sorted(directory.glob("*.crate"))
    found = [archive.name for archive in archives]
    if found != [crate]:
        raise CandidateError(CandidateIssue.ARCHIVE_COUNT, (crate, found))
    archive = directory / crate
    if archive.is_symlink() or not archive.is_file():
        raise CandidateError(CandidateIssue.ARCHIVE_FILE, (crate,))
    return crate, archive


def _manifest_contents(package: tarfile.TarFile, member_name: str) -> str:
    """Read the one regular manifest member with the expected archive identity."""
    matches = [member for member in package.getmembers() if member.name == member_name]
    if len(matches) != 1 or not matches[0].isfile():
        raise CandidateError(CandidateIssue.MANIFEST_MEMBER, (member_name,))
    manifest_file = package.extractfile(matches[0])
    if manifest_file is None:
        raise CandidateError(CandidateIssue.MANIFEST_READ, (member_name,))
    return manifest_file.read().decode("utf-8")


def _check_manifest_identity(
    manifest: dict[str, object], package_name: str, version: str
) -> None:
    """Require the packaged manifest identity to match the validated workflow."""
    package_table = manifest.get("package")
    if not isinstance(package_table, dict):
        raise CandidateError(CandidateIssue.PACKAGE_TABLE)
    if (
        package_table.get("name") != package_name
        or package_table.get("version") != version
    ):
        raise CandidateError(CandidateIssue.MANIFEST_IDENTITY)


def read_packaged_manifest(
    archive_path: pathlib.Path, package_name: str, version: str
) -> dict[str, object]:
    """Read and validate the Cargo manifest stored in the retained archive."""
    member_name = f"{package_name}-{version}/Cargo.toml"
    try:
        archive_bytes = archive_path.read_bytes()
        with tarfile.open(fileobj=io.BytesIO(archive_bytes), mode="r:gz") as package:
            manifest = tomllib.loads(_manifest_contents(package, member_name))
    except (OSError, UnicodeError, tarfile.TarError, tomllib.TOMLDecodeError) as error:
        raise CandidateError(CandidateIssue.MANIFEST_PARSE, (error,)) from error
    _check_manifest_identity(manifest, package_name, version)
    return manifest

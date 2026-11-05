"""Read the retained crate archive and the manifest it packages.

This adapter owns the tar and TOML boundary of candidate validation. It
selects the single archived ``.crate`` recorded by the identity artifact,
parses the ``Cargo.toml`` that archive packages, and reports the archive root
holding it, so the command-line module can compare all three against the
release workflow without opening an archive itself. It reports what the archive
contains and leaves acceptance to :mod:`crates_io_candidate_policy`.
"""

import io
import pathlib
import tarfile
import tomllib

import release_asset_names
from crates_io_candidate_errors import CandidateError, CandidateIssue

#: A packaged manifest sits one directory below the archive root, so its POSIX
#: path has exactly the ``<root>/Cargo.toml`` shape: two components.
_MANIFEST_DEPTH = 2


def _crate_basename(record: dict[str, object]) -> str:
    """Require the recorded crate name to be a flat archive basename."""
    match record.get("crate"):
        case str() as crate if release_asset_names.is_bare_file_name(crate):
            if not crate.endswith(".crate"):
                raise CandidateError(CandidateIssue.CRATE_SUFFIX, (crate,))
            return crate
        case _:
            raise CandidateError(CandidateIssue.CRATE_BASENAME)


def archive_and_crate(
    record: dict[str, object], directory: pathlib.Path
) -> tuple[str, pathlib.Path]:
    """Select the sole archive after validating its recorded basename.

    Parameters
    ----------
    record : dict[str, object]
        The identity record whose ``crate`` entry names the expected archive.
    directory : pathlib.Path
        The directory holding the packaged ``.crate`` files.

    Returns
    -------
    tuple[str, pathlib.Path]
        The validated archive basename and its path.

    Raises
    ------
    CandidateError
        If the recorded name is not a flat ``.crate`` basename, the directory
        does not hold exactly that archive, or the archive is not a regular
        file.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as directory:
    ...     root = Path(directory)
    ...     _ = (root / "netsuke-1.2.3.crate").write_bytes(b"archive")
    ...     crate, path = archive_and_crate({"crate": "netsuke-1.2.3.crate"}, root)
    ...     print(crate, path.name, path.is_file())
    netsuke-1.2.3.crate netsuke-1.2.3.crate True
    """
    crate = _crate_basename(record)
    archives = sorted(directory.glob("*.crate"))
    found = [archive.name for archive in archives]
    if found != [crate]:
        raise CandidateError(CandidateIssue.ARCHIVE_COUNT, (crate, found))
    archive = directory / crate
    if archive.is_symlink() or not archive.is_file():
        raise CandidateError(CandidateIssue.ARCHIVE_FILE, (crate,))
    return crate, archive


def _is_packaged_manifest(member: tarfile.TarInfo) -> bool:
    """Whether the member is a Cargo manifest one directory below the root."""
    parts = pathlib.PurePosixPath(member.name).parts
    return len(parts) == _MANIFEST_DEPTH and parts[-1] == "Cargo.toml"


def _manifest_member(package: tarfile.TarFile) -> tarfile.TarInfo:
    """Require the archive to hold exactly one regular packaged Cargo manifest."""
    # A packaged crate has one top-level ``<package>-<version>`` directory, so
    # its manifest sits exactly one path segment down. Matching the shape rather
    # than a name built from the expected identity keeps this reader free of the
    # identity comparison the policy module performs; the root is reported so
    # the policy can bind it to the release.
    manifests = [
        member for member in package.getmembers() if _is_packaged_manifest(member)
    ]
    found = [member.name for member in manifests]
    if len(manifests) != 1 or not manifests[0].isfile():
        raise CandidateError(CandidateIssue.MANIFEST_MEMBER, (found,))
    return manifests[0]


def _packaged_manifest(package: tarfile.TarFile) -> tuple[str, str]:
    """Read the sole packaged manifest and the directory root holding it."""
    member = _manifest_member(package)
    manifest_file = package.extractfile(member)
    if manifest_file is None:
        raise CandidateError(CandidateIssue.MANIFEST_READ, (member.name,))
    root = pathlib.PurePosixPath(member.name).parent.name
    return root, manifest_file.read().decode("utf-8")


def read_packaged_manifest(archive_path: pathlib.Path) -> tuple[str, dict[str, object]]:
    """Read the Cargo manifest stored in the retained archive.

    Parameters
    ----------
    archive_path : pathlib.Path
        The retained ``.crate`` archive to read.

    Returns
    -------
    tuple[str, dict[str, object]]
        The single directory segment holding the manifest, and the parsed
        packaged manifest. Comparing both against the release workflow belongs
        to :func:`crates_io_candidate_policy.require_manifest_identity`.

    Raises
    ------
    CandidateError
        If the archive cannot be read, or holds no single regular packaged
        manifest.

    Examples
    --------
    >>> read_packaged_manifest.__name__
    'read_packaged_manifest'
    """
    try:
        archive_bytes = archive_path.read_bytes()
        with tarfile.open(fileobj=io.BytesIO(archive_bytes), mode="r:gz") as package:
            root, contents = _packaged_manifest(package)
            return root, tomllib.loads(contents)
    except (OSError, UnicodeError, tarfile.TarError, tomllib.TOMLDecodeError) as error:
        raise CandidateError(CandidateIssue.MANIFEST_PARSE, (error,)) from error

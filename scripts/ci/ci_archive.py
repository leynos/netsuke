"""Checksum-verified download and extraction for the CI helper scripts.

The installers under ``scripts/ci`` fetch a pinned release archive, confirm it
against the digest the workflow declares, and write named members out as
executables. That is one concern with one failure mode -- a hostile or stale
artifact -- so it lives here rather than beside the command runner.

Every URL is allowlisted by scheme, and the allowlist is applied to the
outgoing request rather than to the caller's string, so a redirect target is
checked by the same guard as the URL that named it.
"""

import dataclasses
import hashlib
import pathlib
import shutil
import tarfile
import urllib.parse
import urllib.request

from ci_support import CiScriptError

#: Release archives come from GitHub over HTTPS; tests serve fixtures from
#: ``file://`` URLs so the real download path is exercised without a network.
ALLOWED_URL_SCHEMES = frozenset({"https", "file"})
DOWNLOAD_TIMEOUT_SECONDS = 120.0
EXECUTABLE_MODE = 0o755


class _SchemeCheckedRequestHandler(urllib.request.BaseHandler):
    """Refuse to send any request whose scheme leaves ``ALLOWED_URL_SCHEMES``.

    ``urllib`` follows a redirect itself, by opening the target as a fresh
    request, so the redirect target reaches this guard by the same route as the
    URL the caller asked for. Validating here therefore covers both, and does so
    before anything is sent: CPython's own guard in ``http_error_302`` only
    rejects schemes outside ``http``, ``https``, ``ftp`` and the empty string,
    which would let an ``https://`` release URL be redirected to plain ``http``
    or ``ftp``. It also reads either a ``Location`` or a ``URI`` header, so a
    check that inspected only ``Location`` would be bypassable; inspecting the
    outgoing request avoids having to know which header named the target.

    ``file_request`` is what carries the ``file://`` test fixtures, so the
    allowlist is applied to every scheme rather than only the network ones.

    The schemes arrive through the constructor rather than being read from
    ``ALLOWED_URL_SCHEMES`` directly, which keeps the guard a function of its
    own configuration and gives the hook a genuine use of ``self``. No
    ``@override`` marks the hook: typeshed declares ``BaseHandler`` without
    these methods, so it would override nothing statically; each class that
    does implement them -- ``HTTPCookieProcessor``, ``HTTPHandler`` -- declares
    its own, which is the shape ``OpenerDirector`` discovers at run time
    through the ``process_request`` table.
    """

    def __init__(self, allowed_schemes: frozenset[str]) -> None:
        """Record the schemes this handler will let through."""
        self._allowed_schemes = allowed_schemes

    def http_request(self, request: urllib.request.Request) -> urllib.request.Request:
        """Reject an outgoing request outside the allowed schemes.

        Returns
        -------
        urllib.request.Request
            ``request`` itself, which ``urllib`` then sends unchanged.

        Raises
        ------
        CiScriptError
            If the request's scheme is not one the downloader permits.
        """
        scheme = urllib.parse.urlsplit(request.full_url).scheme
        if scheme not in self._allowed_schemes:
            message = (
                f"refusing to open {request.full_url!r}: scheme {scheme!r} "
                f"is not one of {sorted(self._allowed_schemes)}"
            )
            raise CiScriptError(message)
        return request

    https_request = http_request
    ftp_request = http_request
    file_request = http_request


def download(url: str, destination: pathlib.Path) -> None:
    """Download ``url`` to ``destination``.

    Parameters
    ----------
    url
        An ``https://`` release URL, or a ``file://`` URL under test.
    destination
        The file to write; its parent must exist.

    Raises
    ------
    CiScriptError
        If the URL scheme is not allowed, a redirect leaves the allowlist, or
        the download fails.
    """
    scheme = urllib.parse.urlsplit(url).scheme
    if scheme not in ALLOWED_URL_SCHEMES:
        message = (
            f"refusing to download {url!r}: only "
            f"{sorted(ALLOWED_URL_SCHEMES)} URLs are permitted"
        )
        raise CiScriptError(message)
    try:
        # The scheme is allowlisted just above; the handler applies the same
        # check to every request `urllib` sends afterwards, including redirect
        # targets, so a redirect cannot leave the allowlist. `file://` serves
        # test fixtures.
        opener = urllib.request.build_opener(
            _SchemeCheckedRequestHandler(ALLOWED_URL_SCHEMES)
        )
        with (
            opener.open(url, timeout=DOWNLOAD_TIMEOUT_SECONDS) as response,
            destination.open("wb") as handle,
        ):
            shutil.copyfileobj(response, handle)
    except OSError as error:
        message = f"downloading {url} failed: {error}"
        raise CiScriptError(message) from error


def sha256_of(path: pathlib.Path) -> str:
    """Return the lower-case hex SHA-256 digest of ``path``."""
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify_sha256(path: pathlib.Path, expected: str) -> None:
    """Fail unless ``path`` hashes to ``expected``.

    Parameters
    ----------
    path
        The downloaded archive.
    expected
        The pinned hex digest from the workflow.

    Raises
    ------
    CiScriptError
        If the digest differs; the message names both digests so a pin bump
        and a tampered download are told apart in the log.
    """
    actual = sha256_of(path)
    if actual != expected.lower():
        message = (
            f"{path.name} does not match its pinned SHA-256: expected "
            f"{expected}, got {actual}; the release archive changed or the "
            f"pin in .github/workflows/ci.yml is stale"
        )
        raise CiScriptError(message)


@dataclasses.dataclass(frozen=True, slots=True)
class OpenArchive:
    """A checksum-verified tarball, held open with its members indexed.

    The index is built once so each named member is a dictionary lookup rather
    than a scan, and so a per-member check needs the archive's name and stream
    without carrying both through the call separately.
    """

    path: pathlib.Path
    tar: tarfile.TarFile
    members: dict[str, tarfile.TarInfo]


def extract_members(
    archive: pathlib.Path,
    destination: pathlib.Path,
    members: tuple[str, ...],
) -> list[pathlib.Path]:
    """Extract named regular-file members of a gzip tarball as executables.

    Parameters
    ----------
    archive
        A ``.tar.gz`` archive that has already been checksum-verified.
    destination
        The directory the members are written into, flat, under their own
        base names.
    members
        The exact member names to extract; every one must be present and be
        a regular file. Directories and links are refused so an archive can
        never write outside ``destination``.

    Returns
    -------
    list[pathlib.Path]
        The extracted executables, in ``members`` order.

    Raises
    ------
    CiScriptError
        If a member is missing or is not a regular file.
    """
    with tarfile.open(archive, "r:gz") as tar:
        opened = OpenArchive(
            archive, tar, {member.name: member for member in tar.getmembers()}
        )
        extracted = []
        for name in members:
            member = opened.members.get(name)
            if member is None:
                message = f"{opened.path.name} has no member {name!r}"
                raise CiScriptError(message)
            if not member.isfile():
                message = f"{opened.path.name} member {name!r} is not a regular file"
                raise CiScriptError(message)
            extracted.append(_write_member(opened, destination, name, member))
        return extracted


def _base_name(opened: OpenArchive, name: str) -> str:
    """Return the plain file name ``name`` is written under in ``destination``.

    A member is written under its own base name, never under its archive path,
    so a name carrying directory components is flattened rather than followed.
    Flattening is not on its own enough: ``..`` and ``.`` are their own base
    names, so they would resolve to ``destination``'s parent and to
    ``destination`` itself. Both are refused here rather than left to fail
    later as an ``IsADirectoryError``, which would name neither the archive nor
    the member and so read as a broken filesystem rather than a bad archive.

    Returns
    -------
    str
        The base name, which is never empty, ``.`` or ``..``.

    Raises
    ------
    CiScriptError
        If ``name`` has no base name that can name a file.
    """
    base = pathlib.PurePosixPath(name).name
    if base in {"", ".", ".."}:
        message = f"{opened.path.name} member {name!r} does not name a file"
        raise CiScriptError(message)
    return base


def _write_member(
    opened: OpenArchive,
    destination: pathlib.Path,
    name: str,
    member: tarfile.TarInfo,
) -> pathlib.Path:
    """Write the already-validated member ``name`` into ``destination``.

    Validation happens in :func:`extract_members` and :func:`_base_name`, which
    refuse a member naming a directory or a link, and a member whose base name
    cannot name a file, before anything is written, so a hostile archive cannot
    use one to escape ``destination``.

    Returns
    -------
    pathlib.Path
        The extracted executable, under its own base name in ``destination``.

    Raises
    ------
    CiScriptError
        If the member's stream cannot be read.
    """
    source = opened.tar.extractfile(member)
    if source is None:  # pragma: no cover - isfile() guarantees a stream
        message = f"{opened.path.name} member {name!r} could not be read"
        raise CiScriptError(message)
    target = destination / _base_name(opened, name)
    with source, target.open("wb") as handle:
        shutil.copyfileobj(source, handle)
    target.chmod(EXECUTABLE_MODE)
    return target

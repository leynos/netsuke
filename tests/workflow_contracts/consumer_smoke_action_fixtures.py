"""Publish the release fixtures a consumer-smoke action case runs over.

An execution case needs a candidate the action can treat as published: a package
record naming its version and commit, the pre-upload digest record the verifier
compares a download against, and the bytes the host serves for each published
asset. This module builds those fixtures in memory and writes them under a
case's temporary directory. It also owns the ``curl`` stand-in the bytes are
served through, so a case can refuse a transfer, answer it with a 404, or
substitute other bytes without any request leaving the host.

The harness that executes the action's steps lives in
``consumer_smoke_action_harness.py``; the cases that name a candidate live in
``consumer_smoke_action_execution_test.py``.

Run via ``make test-workflow-contracts``.
"""

import dataclasses
import hashlib
import io
import json
import tarfile
import typing as typ

from release_publish_path_consumer_shape import SMOKE_ACTION_TARGET

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

    type BeforeStep = cabc.Callable[[int, Path], None]

#: The candidate version every fixture describes unless a case says otherwise.
VERSION = "1.2.3"
#: The crate whose package record is the smoke's identity evidence.
PACKAGE = "netsuke-build"
#: The published names the plan must derive for this candidate. The target is
#: read from the job's contract rather than restated, so a workflow that
#: changes its matrix target changes this too.
CRATE_NAME = f"{PACKAGE}-{VERSION}.crate"
ARCHIVE_NAME = f"{PACKAGE}-{VERSION}-{SMOKE_ACTION_TARGET}.tar.gz"
SIDECAR_NAME = f"{ARCHIVE_NAME}.sha256"
#: The banner the extracted binary prints when it is the candidate's.
BANNER = f"netsuke {VERSION}"
#: The commit the package record and the workflow both name.
COMMIT = "0" * 40
#: The shipped binary, as the archive carries it. A shell script rather than an
#: ELF file because the probe only asks it to answer ``--version``.
BINARY = f"#!/bin/sh\nprintf '%s\\n' '{BANNER}'\n".encode()

#: Stands in for ``curl``. It serves the fixture named by the URL's last path
#: segment, and refuses the way a host does: an ``.absent`` marker is a
#: transport failure with no status, an ``.http404`` marker is a delivered 404
#: with an empty body. A name with neither marker is a fixture a case forgot to
#: publish, so it fails as a transport error rather than serving other bytes.
CURL_STUB = """\
#!/bin/sh
set -eu
url=""
output=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --url) url="$2"; shift 2 ;;
    --output) output="$2"; shift 2 ;;
    --write-out) shift 2 ;;
    --connect-timeout|--max-time|--max-filesize|--max-redirs|--proto) shift 2 ;;
    --proto-redir|--retry|--retry-delay|--retry-max-time|--user-agent) shift 2 ;;
    *) shift ;;
  esac
done
printf '%s\\n' "$url" >> "$CURL_LOG"
name="${url##*/}"
if [ -f "$CURL_FIXTURES/$name" ]; then
  cp "$CURL_FIXTURES/$name" "$output"
  printf '200\\n'
  exit 0
fi
if [ -f "$CURL_FIXTURES/$name.http404" ]; then
  : > "$output"
  printf '404\\n'
  exit 0
fi
rm -f "$output"
exit 7
"""

#: The file suffix a case uses to have one asset answered with a 404.
HTTP404 = ".http404"
#: The file suffix a case uses to have one asset never reach the host.
ABSENT = ".absent"


def _digest(data: bytes) -> str:
    """Return the lowercase SHA-256 of ``data``."""
    return hashlib.sha256(data).hexdigest()


def archive_bytes(
    binary: bytes = BINARY, members: cabc.Sequence[str] = ("netsuke",)
) -> bytes:
    """Build a cargo-binstall archive holding ``binary`` at its first member.

    Returns
    -------
    bytes
        The gzipped tar archive.

    Notes
    -----
    Every member is written executable, so a case that varies the layout varies
    the property under test and not the file mode as well.
    """
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w:gz") as archive:
        for index, name in enumerate(members):
            member = tarfile.TarInfo(name)
            payload = binary if index == 0 else b""
            member.size = len(payload)
            member.mode = 0o755
            archive.addfile(member, io.BytesIO(payload))
    return buffer.getvalue()


def sidecar_bytes(archive: bytes, name: str = ARCHIVE_NAME) -> bytes:
    """Build the checksum sidecar the release publishes beside ``archive``."""
    return f"{_digest(archive)}  {name}\n".encode()


@dataclasses.dataclass(frozen=True, slots=True)
class Smoke:
    """Describe what a case hands the action and what the host publishes.

    Attributes
    ----------
    version
        The candidate version the package record and the archive name carry.
    binary
        The bytes the archive publishes as its first member.
    members
        The member names the published archive carries.
    served
        Asset names the host replaces, mapped to the bytes it serves instead.
        A replacement digest is not what the trusted record names, which is how
        a case models a substituted download.
    refused
        Asset names the host never delivers at all. Each gets an ``.absent``
        marker, so ``curl`` fails as a transport error with no status.
    not_found
        Asset names the host answers with a 404 and an empty body.
    inputs
        Action inputs a case overrides, keyed by the name the manifest declares.
    before_step
        Called with the next step's index and the work directory before that
        step runs, so a case can populate the directory an earlier attempt left
        behind.
    """

    version: str = VERSION
    binary: bytes = BINARY
    members: tuple[str, ...] = ("netsuke",)
    served: dict[str, bytes] = dataclasses.field(default_factory=dict)
    refused: frozenset[str] = frozenset()
    not_found: frozenset[str] = frozenset()
    inputs: dict[str, str] = dataclasses.field(default_factory=dict)
    before_step: BeforeStep | None = None


#: The candidate a case that names none publishes.
VERIFIED = Smoke()


@dataclasses.dataclass(frozen=True, slots=True)
class Published:
    """Hold the records a case wrote and the bytes the host serves.

    Attributes
    ----------
    identity
        The package record the packaging stage retained.
    trusted
        The pre-upload digest record, keyed by published asset name.
    fixtures
        The directory the ``curl`` stand-in serves from.
    work_dir
        The directory the action is told to keep its records in.
    """

    identity: Path
    trusted: Path
    fixtures: Path
    work_dir: Path


def publish(root: Path, smoke: Smoke) -> Published:
    """Write the evidence records and the bytes a run publishes.

    Returns
    -------
    Published
        The two record paths, the fixture directory, and the work directory.

    Notes
    -----
    The crate body is arbitrary bytes: the smoke compares the download with the
    digest the package record names, so the content only has to stay the same
    between the record and the fixture.
    """
    archive = archive_bytes(smoke.binary, smoke.members)
    sidecar = sidecar_bytes(archive)
    crate = f"the {PACKAGE} {smoke.version} crate\n".encode()
    fixtures = root / "fixtures"
    fixtures.mkdir()
    for name, body in (
        (CRATE_NAME, smoke.served.get(CRATE_NAME, crate)),
        (ARCHIVE_NAME, smoke.served.get(ARCHIVE_NAME, archive)),
        (SIDECAR_NAME, smoke.served.get(SIDECAR_NAME, sidecar)),
    ):
        _stage_fixture(fixtures, name, body, smoke)
    identity = root / "identity.json"
    identity.write_text(
        json.dumps({
            "crate": CRATE_NAME,
            "identity": {
                "tag": f"v{smoke.version}",
                "commit": COMMIT,
                "package_name": PACKAGE,
                "version": smoke.version,
                "bin_name": "netsuke",
                "is_prerelease": False,
            },
            "sha256": _digest(crate),
        }),
        encoding="utf-8",
    )
    trusted = root / "trusted.json"
    trusted.write_text(
        json.dumps({
            "assets": {ARCHIVE_NAME: _digest(archive), SIDECAR_NAME: _digest(sidecar)}
        }),
        encoding="utf-8",
    )
    return Published(
        identity=identity,
        trusted=trusted,
        fixtures=fixtures,
        work_dir=root / "runner" / "work",
    )


def _stage_fixture(fixtures: Path, name: str, body: bytes, smoke: Smoke) -> None:
    """Write one asset's bytes, or the marker that stands in for a refusal."""
    if name in smoke.refused:
        (fixtures / f"{name}{ABSENT}").touch()
    elif name in smoke.not_found:
        (fixtures / f"{name}{HTTP404}").touch()
    else:
        (fixtures / name).write_bytes(body)


__all__ = [
    "ABSENT",
    "ARCHIVE_NAME",
    "BANNER",
    "BINARY",
    "COMMIT",
    "CRATE_NAME",
    "CURL_STUB",
    "HTTP404",
    "PACKAGE",
    "SIDECAR_NAME",
    "VERIFIED",
    "VERSION",
    "Published",
    "Smoke",
    "archive_bytes",
    "publish",
    "sidecar_bytes",
]

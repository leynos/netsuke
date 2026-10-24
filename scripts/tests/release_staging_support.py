"""Stage release fixtures alongside the trusted record that describes them.

Both release-staging test modules need the same three things: an archive with
the sidecar beside it, an operating-system package, and a digest record
computed from the staged bytes rather than from the code under test. Keeping
one copy here is what lets the two suites disagree about *behaviour* while
agreeing about *fixtures*; two copies would let a format change quietly make
one suite assert against a release shape the other no longer produces.

The record helper is the load-bearing one. It hashes the staged files directly
with :func:`release_digest.digest_of_file` instead of reading back what the
producer wrote, so a test that substitutes a download is comparing it against
an independent account of the build's bytes. A record obtained from the module
under test would let a fault in that module vouch for itself.

Nothing here reaches the network.
"""

import typing as typ

import release_digest
import release_digest_record
import release_inventory

if typ.TYPE_CHECKING:
    import pathlib

#: The archive a staged release publishes, named as the hoist leaves it.
ARCHIVE = "netsuke-build-0.1.0-x86_64-unknown-linux-gnu.tar.gz"

#: The sidecar staged immediately after :data:`ARCHIVE`.
SIDECAR = f"{ARCHIVE}.sha256"

#: One operating-system package, which travels without a sidecar of its own.
PACKAGE = "netsuke_0.1.0-1_amd64.deb"

#: The bytes every staged archive holds, before a test replaces them.
BODY = b"netsuke archive bytes"


def stage_archive(
    directory: pathlib.Path, *, sidecar_body: bytes | None = None
) -> None:
    """Write the archive and a sidecar describing it.

    Parameters
    ----------
    directory
        Directory to stage into. Created, with parents, if absent.
    sidecar_body
        Digest text to write into the sidecar. Defaults to the archive's own
        digest, which is the well-formed case; a test passing anything else is
        staging a sidecar that disagrees with the file beside it.

    Notes
    -----
    The sidecar names ``dist/<archive>`` rather than the bare file name,
    because that is the shape the release workflow produces: the digest is
    computed before the hoist relocates the archive out of ``dist``.
    """
    directory.mkdir(parents=True, exist_ok=True)
    archive = directory / ARCHIVE
    _ = archive.write_bytes(BODY)
    digest = release_digest.digest_of_file(archive).encode()
    recorded = sidecar_body if sidecar_body is not None else digest
    _ = (directory / SIDECAR).write_text(
        f"{recorded.decode()}  dist/{ARCHIVE}\n", encoding="utf-8"
    )


def stage_package(directory: pathlib.Path, body: bytes) -> None:
    """Write an operating-system package, which carries no sidecar of its own.

    Parameters
    ----------
    directory
        Directory to stage into. Created, with parents, if absent.
    body
        Bytes to write. Two calls with different bodies are how a test stages
        a package and then a substituted one.
    """
    directory.mkdir(parents=True, exist_ok=True)
    _ = (directory / PACKAGE).write_bytes(body)


def write_declaration(destination: pathlib.Path, names: list[str]) -> pathlib.Path:
    """Write the declared asset list in the format the verifier reads.

    Parameters
    ----------
    destination
        File to write.
    names
        Asset names to declare, one per line.

    Returns
    -------
    pathlib.Path
        ``destination``, so a caller can pass the result straight to an
        invocation's argument list.
    """
    _ = destination.write_text("\n".join(names) + "\n", encoding="utf-8")
    return destination


def trusted_digests(staging: pathlib.Path) -> dict[str, str]:
    """Record the staged tree's own digests, as the build would before upload.

    Parameters
    ----------
    staging
        Directory holding the staged files.

    Returns
    -------
    dict[str, str]
        Asset name to the digest of that staged file's own bytes.

    Notes
    -----
    Computed from the staged files rather than through the verifier, so a later
    substitution is compared against an independent record and not against the
    module under test's idea of one.
    """
    return {
        asset.name: release_digest.digest_of_file(staging / asset.name)
        for asset in release_inventory.read_inventory(staging)
    }


def write_trusted_record(
    destination: pathlib.Path, digests: dict[str, str]
) -> pathlib.Path:
    """Write a trusted digest record the CLIs can read.

    Parameters
    ----------
    destination
        File to write.
    digests
        Asset name to recorded digest.

    Returns
    -------
    pathlib.Path
        ``destination``, so a caller can pass the result straight to an
        invocation's argument list.
    """
    release_digest_record.write_digest_record(digests, destination)
    return destination

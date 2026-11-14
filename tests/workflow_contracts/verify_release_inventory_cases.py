"""Hold the builds, releases, and scenarios the verifier cases share.

Every case needs a publisher tree, a release GitHub holds, and a set of action
inputs, and most cases need the default of all three so that the one thing they
vary stands alone. Keeping the defaults here means a case reads as the single
difference it is making -- a substituted archive, a release that drops an asset,
a download directory that is already occupied -- rather than as another
restatement of a build tree.

Nothing here executes anything. The harness that turns a scenario into stubs, a
staged workspace, and a run of real shell bodies is
``verify_release_inventory_harness.py``, and the cases are the two
``verify_release_inventory_*_test`` modules.

Run via ``make test-workflow-contracts``.
"""

import hashlib
import typing as typ

from verify_release_inventory_scenario import (
    Inputs,
    Publisher,
    ReleaseEntry,
    Remote,
    RemoteAsset,
    Scenario,
    prepare_nothing,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

    from verify_release_inventory_harness import ActionRun

#: The tag the one draft release carries.
TAG = "v0.1.0"

#: The archive a staged release publishes, and the sidecar beside it.
ARCHIVE = "netsuke-0.1.0-x86_64-unknown-linux-gnu.tar.gz"
SIDECAR = f"{ARCHIVE}.sha256"

#: One operating-system package, which travels without a sidecar of its own.
PACKAGE = "netsuke_0.1.0-1_amd64.deb"

#: The identifier the asset listing and each download address.
RELEASE_ID = 7

#: The identifier the first planned asset carries.
FIRST_ASSET_ID = 11

#: The values a case that names none is handed: the job's own wiring.
JOB_INPUTS = Inputs()


def sidecar(archive: bytes) -> bytes:
    """Return the sidecar text a well-formed release carries beside ``archive``.

    Returns
    -------
    bytes
        The digest line, naming the archive as the build wrote it.
    """
    return f"{hashlib.sha256(archive).hexdigest()}  dist/{ARCHIVE}\n".encode()


def build_assets() -> dict[str, bytes]:
    """Return the assets the publisher built, keyed by published name.

    Returns
    -------
    dict[str, bytes]
        Each asset name mapped to the bytes the build produced.
    """
    archive = b"netsuke archive bytes"
    return {
        ARCHIVE: archive,
        SIDECAR: sidecar(archive),
        PACKAGE: b"netsuke package bytes",
    }


def default_publisher() -> Publisher:
    """Return a publisher whose declaration and record cover its whole build.

    Returns
    -------
    Publisher
        The default build, with no name omitted from either record.
    """
    return Publisher(files=build_assets())


def release_assets(bodies: cabc.Mapping[str, bytes]) -> tuple[RemoteAsset, ...]:
    """Return the attachments a release of ``bodies`` holds.

    Returns
    -------
    tuple[RemoteAsset, ...]
        One asset per name, numbered from :data:`FIRST_ASSET_ID` in order.
    """
    return tuple(
        RemoteAsset(identifier=FIRST_ASSET_ID + offset, name=name, body=body)
        for offset, (name, body) in enumerate(bodies.items())
    )


def draft_release(assets: tuple[RemoteAsset, ...]) -> Remote:
    """Return the release GitHub holds for a case.

    Returns
    -------
    Remote
        One draft release carrying :data:`TAG`, holding ``assets``.
    """
    return Remote(releases=(ReleaseEntry(RELEASE_ID, TAG),), assets=assets)


def default_scenario(
    *,
    inputs: Inputs = JOB_INPUTS,
    prepare: cabc.Callable[[pathlib.Path], None] = prepare_nothing,
) -> Scenario:
    """Return a scenario whose release matches the default publisher build.

    Returns
    -------
    Scenario
        The default build, a matching release, and the values a case chose.
    """
    publisher = default_publisher()
    return Scenario(
        publisher=publisher,
        remote=draft_release(release_assets(publisher.files)),
        inputs=inputs,
        prepare=prepare,
    )


def final_diagnostic(run: ActionRun) -> str:
    """Return the last completed step's combined diagnostic.

    Returns
    -------
    str
        The final step's standard error, where a refusal is reported.
    """
    return run.completed[-1].stderr


__all__ = [
    "ARCHIVE",
    "FIRST_ASSET_ID",
    "JOB_INPUTS",
    "PACKAGE",
    "RELEASE_ID",
    "SIDECAR",
    "TAG",
    "build_assets",
    "default_publisher",
    "default_scenario",
    "draft_release",
    "final_diagnostic",
    "release_assets",
    "sidecar",
]

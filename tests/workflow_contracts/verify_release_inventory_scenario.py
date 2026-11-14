"""Describe a build, a release, and the inputs one verifier run is handed.

A case that runs the inventory verifier has to say three things: what the
publisher built and recorded, what GitHub holds, and which values the caller
passes to the action. Keeping those in one place lets a case vary exactly one of
them -- a release that drops an asset, a record that misses one, a download
directory that already holds a file -- and leaves the other two at a shape a
different case has already proved verifies.

Nothing here executes anything or touches the filesystem. The harness that turns
a :class:`Scenario` into stubs, a staged workspace, and a run of real shell
bodies is ``verify_release_inventory_harness.py``.

Run via ``make test-workflow-contracts``.
"""

import dataclasses
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

#: The directory the action keeps its listings and download plan in, relative to
#: the runner's temporary root.
INVENTORY_DIRECTORY = "release-inventory"

#: The value the listing and download steps receive in place of a credential.
#: The stand-in never reads it: no request leaves the machine, so it only has to
#: be present for the steps that forward one.
STAND_IN_VALUE = "unused-by-the-stand-in"


@dataclasses.dataclass(frozen=True, slots=True)
class Inputs:
    """Hold the values the caller hands the action.

    The defaults are the job's own wiring: a relative trusted tree, a relative
    download directory, and a relative output path, all resolved from the
    workspace the steps run in. A case that is about a refusal overrides the one
    input it needs rather than restating the rest.

    Attributes
    ----------
    tag
        The release tag the draft must carry.
    repository
        The ``OWNER/REPOSITORY`` the listings address.
    trusted_directory
        The directory holding the publisher's declaration and digest record.
    declaration_file
        The declared asset names, resolving inside ``trusted_directory``.
    trusted_record_file
        The publisher's digest record, resolving inside ``trusted_directory``.
    download_directory
        The directory the attachments are downloaded into.
    output_path
        The verified record the verification step writes.
    github_token
        The token the listing and download steps receive.
    """

    tag: str = "v0.1.0"
    repository: str = "leynos/netsuke"
    trusted_directory: str = "trusted"
    declaration_file: str = "trusted/declaration.txt"
    trusted_record_file: str = "trusted/trusted.json"
    download_directory: str = "release-download"
    output_path: str = "verified/verified-digests.json"
    github_token: str = STAND_IN_VALUE

    def values(self) -> dict[str, str]:
        """Return the inputs keyed by the names the manifest declares.

        Returns
        -------
        dict[str, str]
            Each action input name mapped to its value.
        """
        return {
            "tag": self.tag,
            "repository": self.repository,
            "trusted-directory": self.trusted_directory,
            "declaration-file": self.declaration_file,
            "trusted-record-file": self.trusted_record_file,
            "download-directory": self.download_directory,
            "output-path": self.output_path,
            "github-token": self.github_token,
        }


@dataclasses.dataclass(frozen=True, slots=True)
class Publisher:
    """Describe what the publisher built, declared, and recorded.

    Attributes
    ----------
    files
        Asset name to the bytes the build produced. These are the bytes a
        correct release carries, so every digest the trusted record holds is
        taken from them rather than from anything downloaded.
    declared
        The names the declaration lists. Defaults to every built file. A case
        that shortens or lengthens this one list stages a declaration that
        disagrees with the build without having to restage the files.
    recorded
        The names the trusted record covers. Defaults to every built file; a
        case that omits one stages a record that does not cover the release.
    """

    files: dict[str, bytes]
    declared: tuple[str, ...] | None = None
    recorded: tuple[str, ...] | None = None

    @property
    def declared_names(self) -> tuple[str, ...]:
        """The names the declaration lists."""
        return tuple(self.files) if self.declared is None else self.declared

    @property
    def recorded_names(self) -> tuple[str, ...]:
        """The names the trusted digest record covers."""
        return tuple(self.files) if self.recorded is None else self.recorded


@dataclasses.dataclass(frozen=True, slots=True)
class RemoteAsset:
    """Describe one attachment GitHub reports for the release.

    Attributes
    ----------
    identifier
        The asset number the download step addresses.
    name
        The name the listing reports, which the plan writes into the download
        directory.
    body
        The bytes a download of this asset yields.
    state
        The attachment state the listing reports.
    size
        The byte size the listing reports. Defaults to the length of ``body``;
        a case that sets it separately stages an entry whose claim about its own
        size disagrees with the bytes behind it.
    download_fails
        Whether a download of this asset is refused rather than served.
    """

    identifier: int
    name: str
    body: bytes
    state: str = "uploaded"
    size: int | None = None
    download_fails: bool = False


@dataclasses.dataclass(frozen=True, slots=True)
class ReleaseEntry:
    """Describe one release the listing reports.

    Attributes
    ----------
    identifier
        The release number, which the asset listing addresses.
    tag
        The tag the release carries.
    draft
        Whether the release is still a draft.
    """

    identifier: int
    tag: str
    draft: bool = True


@dataclasses.dataclass(frozen=True, slots=True)
class Remote:
    """Describe the release GitHub holds during one run.

    Attributes
    ----------
    releases
        Every release the listing reports, in listing order.
    assets
        Every attachment of the first release, in listing order.
    """

    releases: tuple[ReleaseEntry, ...]
    assets: tuple[RemoteAsset, ...]


def prepare_nothing(_workspace: Path) -> None:
    """Leave the staged workspace untouched."""


@dataclasses.dataclass(frozen=True, slots=True)
class Scenario:
    """Describe the build, the release, and the inputs one run is handed.

    Attributes
    ----------
    publisher
        The build tree, its declaration, and its trusted digest record.
    remote
        The release GitHub holds.
    inputs
        The values the caller hands the action.
    prepare
        A hook run after the workspace is staged and before the first step, for
        a case whose refusal needs the filesystem in a state the inputs alone
        cannot express, such as a download directory that already holds a file.
    """

    publisher: Publisher
    remote: Remote
    inputs: Inputs = Inputs()
    prepare: cabc.Callable[[Path], None] = prepare_nothing

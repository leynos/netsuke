"""Name the options each consumer-smoke command is handed.

Cyclopts flattens a dataclass annotated ``Parameter(name="*")`` into one flag
per field, so these dataclasses are simultaneously the command line the
composite action invokes and the values the evidence and verify stages read. A
field name is therefore both a flag name and an attribute those stages use, and
the two cannot drift apart without changing both in one place.

The fields are keyword-only because a derived command adds its own required
fields: a subclass cannot follow a base with a field that has a default unless
the order stops mattering, which ``kw_only`` is what removes.

Run through ``scripts/consumer_smoke_cli.py``.

Examples
--------
Both commands read the same evidence, so only the defaults live on the shared
half; a command that omits an optional flag still names the repository default:

>>> options = EvidenceOptions(
...     identity=Path("identity.json"),
...     trusted=Path("trusted.json"),
...     repository="leynos/netsuke",
...     target="x86_64-unknown-linux-gnu",
...     commit="0" * 40,
... )
>>> str(options.staging_config), str(options.manifest)
('.github/release-staging.toml', 'Cargo.toml')
"""

import dataclasses
from pathlib import Path

#: The default release staging configuration, relative to the repository root.
DEFAULT_STAGING_CONFIG = Path(".github/release-staging.toml")
#: The default Cargo manifest naming the package.
DEFAULT_MANIFEST = Path("Cargo.toml")


@dataclasses.dataclass(frozen=True, slots=True, kw_only=True)
class EvidenceOptions:
    """Name the trusted evidence both commands are judged against.

    Attributes
    ----------
    identity
        The package record the packaging stage wrote.
    trusted
        The pre-upload digest record, keyed by published asset name.
    repository
        The ``OWNER/NAME`` GitHub slug whose release is downloaded.
    target
        The Rust target triple whose cargo-binstall archive is exercised.
    commit
        The commit this run checked out.
    staging_config
        The release staging configuration naming every target.
    manifest
        The Cargo manifest naming the package.
    """

    identity: Path
    trusted: Path
    repository: str
    target: str
    commit: str
    staging_config: Path = DEFAULT_STAGING_CONFIG
    manifest: Path = DEFAULT_MANIFEST


@dataclasses.dataclass(frozen=True, slots=True, kw_only=True)
class PlanOptions(EvidenceOptions):
    """Name where ``plan`` writes the downloads this run must perform.

    Attributes
    ----------
    output
        Where the plan document is written.
    """

    output: Path


@dataclasses.dataclass(frozen=True, slots=True, kw_only=True)
class VerifyOptions(EvidenceOptions):
    """Name the saved downloads and the records ``verify`` writes.

    Attributes
    ----------
    plan
        The plan document this run re-derives and enforces.
    crate
        The saved crate archive.
    crate_status
        The HTTP status curl recorded for the crate.
    archive
        The saved cargo-binstall archive.
    archive_status
        The HTTP status curl recorded for the archive.
    sidecar
        The saved checksum sidecar.
    sidecar_status
        The HTTP status curl recorded for the sidecar.
    extract_dir
        A fresh, empty directory the binary is extracted into.
    verdict
        Where to write the bounded phase-and-verdict record.
    phase_log
        Where to write the bounded per-phase outcome and timing record.
    """

    plan: Path
    crate: Path
    crate_status: Path
    archive: Path
    archive_status: Path
    sidecar: Path
    sidecar_status: Path
    extract_dir: Path
    verdict: Path
    phase_log: Path


__all__ = [
    "DEFAULT_MANIFEST",
    "DEFAULT_STAGING_CONFIG",
    "EvidenceOptions",
    "PlanOptions",
    "VerifyOptions",
]

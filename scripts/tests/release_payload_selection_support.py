"""Stage two same-platform targets so a swapped claim has somewhere to hide.

The cases in ``test_release_payload_selection.py`` mostly vary one
contradiction on a single target. A claim exchanged between two records is not
expressible that way: it needs two targets that share a platform, stage
different archives, and claim the same *number* of packages, because that is
the shape in which the union of their claims stays identical while each
record vouches for the other's bytes.

Everything here is a plain value or a constructor over names. Nothing decides
anything, so a case still states its own contradiction and its own assertion.
"""

import typing as typ

from release_producer_provenance import ProducerProvenance

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: The x86_64 Linux target the single-target cases use.
X86_TARGET = "linux-x86_64"
X86_ARCHIVE = "netsuke-1.0.0-x86_64-unknown-linux-gnu.tar.gz"
X86_DEB = "netsuke_1.0.0_amd64.deb"
X86_RPM = "netsuke-1.0.0-1.x86_64.rpm"

#: A second Linux target. Same platform, different triple and architecture, so
#: the two records are interchangeable by count and distinct by name.
ARM_TARGET = "linux-aarch64"
ARM_ARCHIVE = "netsuke-1.0.0-aarch64-unknown-linux-gnu.tar.gz"
ARM_DEB = "netsuke_1.0.0_arm64.deb"
ARM_RPM = "netsuke-1.0.0-1.aarch64.rpm"

#: Both targets, configured the way ``release-staging.toml`` configures them.
TWO_TARGET_CONFIG: dict[str, object] = {
    "targets": {
        X86_TARGET: {"platform": "linux"},
        ARM_TARGET: {"platform": "linux"},
    },
}

#: Each target key mapped to the archive that target stages.
TWO_TARGET_ARCHIVES = {X86_TARGET: X86_ARCHIVE, ARM_TARGET: ARM_ARCHIVE}

#: The identity every staged record in these cases claims. The x86_64 target
#: keeps the one-target names the cases are written in.
CANDIDATE = "a" * 40
RUN_ID = "1234"
TARGET = X86_TARGET
ARCHIVE = X86_ARCHIVE
SIDECAR = f"{X86_ARCHIVE}.sha256"
DEB = X86_DEB
RPM = X86_RPM


def record(
    target: str, assets: cabc.Mapping[str, str], *, platform: str = "linux"
) -> ProducerProvenance:
    """Build one producer record for ``target`` over ``assets``.

    Returns
    -------
    ProducerProvenance
        A record for the candidate and run the cases declare.
    """
    return ProducerProvenance(
        commit=CANDIDATE,
        run_id=RUN_ID,
        run_attempt="1",
        target=target,
        platform=platform,
        assets=dict(assets),
    )


def staged_assets() -> dict[str, str]:
    """Return the x86_64 target's own claims, one distinct digest per name.

    Returns
    -------
    dict[str, str]
        The archive, its sidecar, and two packages.
    """
    return {
        ARCHIVE: "1" * 64,
        SIDECAR: "2" * 64,
        DEB: "3" * 64,
        RPM: "4" * 64,
    }


def x86_record(**overrides: object) -> ProducerProvenance:
    """Build the x86_64 record over :func:`staged_assets`, field by field.

    Returns
    -------
    ProducerProvenance
        The record, with any field the case chose replaced.

    Notes
    -----
    The override surface is a keyword bag rather than a parameter list because
    a case varies exactly one field at a time -- the commit, the run, the
    platform, or the claims -- and naming all four would invite a caller to
    vary two and read the wrong refusal.
    """
    fields: dict[str, object] = {
        "commit": CANDIDATE,
        "run_id": RUN_ID,
        "run_attempt": "1",
        "target": TARGET,
        "platform": "linux",
        "assets": staged_assets(),
    }
    fields.update(overrides)
    return ProducerProvenance(**fields)


def arm_record(
    *,
    archive: str = ARM_ARCHIVE,
    packages: cabc.Sequence[str] = (ARM_DEB, ARM_RPM),
) -> ProducerProvenance:
    """Build the ARM target's record over the names a case chooses.

    Parameters
    ----------
    archive
        The archive this record claims, with its sidecar derived from it.
    packages
        The package names this record claims.

    Returns
    -------
    ProducerProvenance
        The record, with a distinct digest per claimed name.
    """
    names = (archive, f"{archive}.sha256", *packages)
    return record(
        ARM_TARGET, {name: _digest(index) for index, name in enumerate(names)}
    )


def two_target_staged() -> dict[str, list[str]]:
    """Return a staged tree holding both targets' declared names.

    Returns
    -------
    dict[str, list[str]]
        One source per declared name, for both targets.
    """
    return {
        X86_ARCHIVE: ["x86-archive"],
        f"{X86_ARCHIVE}.sha256": ["x86-sidecar"],
        X86_DEB: ["x86-deb"],
        X86_RPM: ["x86-rpm"],
        ARM_ARCHIVE: ["arm-archive"],
        f"{ARM_ARCHIVE}.sha256": ["arm-sidecar"],
        ARM_DEB: ["arm-deb"],
        ARM_RPM: ["arm-rpm"],
    }


def two_target_origins() -> dict[str, str]:
    """Return each target's own packages attributed to that target.

    Returns
    -------
    dict[str, str]
        Published package name mapped to the target whose downloaded subtree
        staged it.

    Notes
    -----
    Archives are deliberately absent. They are hoisted to the ``dist`` root
    and bound to a target by *name*, so an origin for them would be a second,
    weaker account of a fact the configuration already states.
    """
    return {
        X86_DEB: X86_TARGET,
        X86_RPM: X86_TARGET,
        ARM_DEB: ARM_TARGET,
        ARM_RPM: ARM_TARGET,
    }


def _digest(index: int) -> str:
    """Return a distinct, well-formed digest for the ``index``th name."""
    return f"{index + 1:x}" * 64


__all__ = [
    "ARCHIVE",
    "ARM_ARCHIVE",
    "ARM_DEB",
    "ARM_RPM",
    "ARM_TARGET",
    "CANDIDATE",
    "DEB",
    "RPM",
    "RUN_ID",
    "SIDECAR",
    "TARGET",
    "TWO_TARGET_ARCHIVES",
    "TWO_TARGET_CONFIG",
    "X86_ARCHIVE",
    "X86_DEB",
    "X86_RPM",
    "X86_TARGET",
    "arm_record",
    "record",
    "staged_assets",
    "two_target_origins",
    "two_target_staged",
    "x86_record",
]

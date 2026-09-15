"""Derive Windows Installer ordering metadata from Netsuke release versions."""

import argparse
import re
import sys
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

FINAL_RELEASE_RANK = 65_535
"""Rank assigned to a final release after every supported beta release."""

_RELEASE_VERSION = re.compile(
    r"^(?P<major>0|[1-9]\d*)\.(?P<minor>0|[1-9]\d*)\."
    r"(?P<patch>0|[1-9]\d*)(?:-beta(?P<sequence>0|[1-9]\d*))?$"
)


class ReleaseRankError(ValueError):
    """Represent a release version that Windows MSI packaging cannot order."""


def release_rank(version: str) -> int:
    """Return the Windows Installer release rank for a supported version.

    Final releases follow every beta from the same numeric release line. WiX
    stores only the numeric version, so the rank records the otherwise-lost
    prerelease ordering in the installed product.

    Returns
    -------
    int
        The beta sequence or `65535` for a final release.

    Raises
    ------
    ReleaseRankError
        If ``version`` is not `MAJOR.MINOR.PATCH` or
        `MAJOR.MINOR.PATCH-betaN`, or its beta sequence is outside
        ``1..=65534``.
    """
    match = _RELEASE_VERSION.fullmatch(version)
    if match is None:
        message = (
            "Windows MSI releases must use MAJOR.MINOR.PATCH or "
            "MAJOR.MINOR.PATCH-betaN."
        )
        raise ReleaseRankError(message)

    sequence = match.group("sequence")
    if sequence is None:
        return FINAL_RELEASE_RANK

    rank = int(sequence)
    if not 1 <= rank < FINAL_RELEASE_RANK:
        message = "Windows MSI beta sequence must be an integer from 1 through 65534."
        raise ReleaseRankError(message)
    return rank


def parse_arguments(arguments: cabc.Sequence[str] | None = None) -> argparse.Namespace:
    """Parse the one release-version argument accepted by the helper CLI."""
    parser = argparse.ArgumentParser(
        description="derive the Windows MSI release rank for a Netsuke version"
    )
    parser.add_argument("version", help="final or beta SemVer release version")
    return parser.parse_args(arguments)


def main(arguments: cabc.Sequence[str] | None = None) -> int:
    """Print a release rank and report invalid versions on standard error."""
    version = parse_arguments(arguments).version
    try:
        print(release_rank(version))
    except ReleaseRankError as error:
        print(error, file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

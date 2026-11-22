"""Name the downstream migration canaries a release must report.

Issue #598 gates a release on real downstream repositories driven through their
own Netsukefiles by the exact candidate. Each matrix row in
``.github/workflows/release.yml`` is one canary, and each canary is one result
the evidence manifest must declare. This module is the single list of those
rows, so the producer that writes the manifest, the verifier that requires the
results, and the workflow contract that compares both with the matrix all read
the same names.

The names are result names, not metric labels: they never reach a metric or
trace series, so adding a canary changes the required evidence without
widening the bounded vocabularies in :mod:`policy`.
"""

import typing as typ

#: Every downstream canary matrix row, Linux and Windows, in workflow order.
CANARIES: typ.Final[tuple[str, ...]] = (
    "repovec-appliance",
    "mxd-postgres",
    "mxd-sqlite",
    "mxd-wireframe-only",
    "ortho-config",
    "ortho-config-windows",
)


#: The canary rows the Windows job runs; every other row runs on Linux.
WINDOWS_CANARIES: typ.Final[frozenset[str]] = frozenset({"ortho-config-windows"})


def result_name(canary: str) -> str:
    """Return the evidence-manifest result name for one canary row.

    The prefix keeps a downstream result apart from the RFC 0005 results such
    as ``history_scan``, and the hyphens become underscores to match them.

    Returns
    -------
    str
        The result name the producer declares and the verifier requires.

    Examples
    --------
    >>> result_name("mxd-wireframe-only")
    'downstream_mxd_wireframe_only'
    """
    return "downstream_" + canary.replace("-", "_")


#: The result name for every canary, in :data:`CANARIES` order.
RESULTS: typ.Final[tuple[str, ...]] = tuple(result_name(name) for name in CANARIES)

__all__ = ("CANARIES", "RESULTS", "WINDOWS_CANARIES", "result_name")

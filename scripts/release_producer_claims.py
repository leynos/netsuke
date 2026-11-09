"""Bind each producer record's claims to the target that record names.

A producer record says which target it served, so the archive, that archive's
sidecar, and the package formats its platform stages are all knowable before
any claim is compared with another record's. Publication needs that binding:
comparing only the *union* of every record's claims with the release
declaration lets two same-platform records exchange their archive, sidecar,
and packages while the union stays identical, and the release would then
attach one target's bytes under another target's name.

The archive is bound by *name*, because a target's archive interpolates its
Rust triple and the staging configuration is the only authority for that. A
package name cannot be derived: three external shared actions own those names,
so only the format suffix and the per-target count are knowable.

That is exactly what the package half of this check enforces, and no more. A
record must claim one package per format its platform stages, and a claim that
matches no format at all is refused. It does **not** establish that a package
belongs to the record's own target: two same-platform targets that trade one
package each leave every count unchanged and the union identical, and this
check accepts the trade. `test_release_payload_selection` records that
acceptance so no reader mistakes the count for an identity proof. Establishing
package-to-target identity would need a boundary that could read the package's
own metadata, which nothing here does.

Nothing here reads a file, parses JSON, or hashes bytes. Claims arrive
validated from :mod:`release_producer_provenance`, and the refusals live in
:mod:`release_producer_provenance_errors`, so this module decides and nothing
else.

Run via ``scripts/release_payload_selection.py``; behavioural coverage lives in
``scripts/tests/test_release_payload_selection.py``.
"""

import typing as typ

import release_declaration
import release_package_formats
from release_producer_provenance_errors import TargetClaimMismatchError

if typ.TYPE_CHECKING:
    import collections.abc as cabc

    from release_producer_provenance import ProducerProvenance

__all__ = ["require_target_claims"]


def require_target_claims(
    provenances: cabc.Iterable[ProducerProvenance],
    *,
    archives: cabc.Mapping[str, str],
    targets: cabc.Mapping[str, str],
) -> None:
    """Refuse a record that claims artefacts its own target cannot have staged.

    Raises
    ------
    TargetClaimMismatchError
        If any record omits its own archive or sidecar, claims a name that is
        neither, or does not claim exactly one package per format its platform
        stages.

    Notes
    -----
    ``UnknownPlatformError`` reaches the caller from
    :func:`_package_claim_offenders` when a record's configured platform
    stages no package format at all, and so stays out of the ``Raises``
    section.

    The module docstring states which half of this check is an identity proof
    and which is only a count, and why. A target with no entry in ``archives``
    or ``targets`` is skipped here rather than reported: coverage calls it an
    uncovered target, which sends the operator to the configuration rather
    than to a producer record.

    Examples
    --------
    >>> from release_producer_provenance import ProducerProvenance
    >>> provenance = ProducerProvenance(
    ...     schema=1, commit="a" * 40, run_id="7", run_attempt="1",
    ...     target="linux-x86_64", platform="linux",
    ...     assets={"pkg-gnu.tar.gz": "b" * 64, "pkg-gnu.tar.gz.sha256": "c" * 64,
    ...             "pkg.deb": "d" * 64, "pkg.rpm": "e" * 64},
    ... )
    >>> require_target_claims(
    ...     [provenance],
    ...     archives={"linux-x86_64": "pkg-gnu.tar.gz"},
    ...     targets={"linux-x86_64": "linux"},
    ... )
    >>> try:
    ...     require_target_claims(
    ...         [provenance],
    ...         archives={"linux-x86_64": "pkg-musl.tar.gz"},
    ...         targets={"linux-x86_64": "linux"},
    ...     )
    ... except TargetClaimMismatchError as refusal:
    ...     print("omits its own" in str(refusal))
    True
    """
    offenders = [
        offender
        for provenance in provenances
        for offender in _target_claim_offenders(
            provenance, archives=archives, targets=targets
        )
    ]
    if offenders:
        raise TargetClaimMismatchError(offenders)


def _target_claim_offenders(
    provenance: ProducerProvenance,
    *,
    archives: cabc.Mapping[str, str],
    targets: cabc.Mapping[str, str],
) -> list[str]:
    """Describe every way one record's claims are not its target's own.

    Returns
    -------
    list[str]
        Human-readable descriptions; empty when the record is bound to its
        target.

    Notes
    -----
    ``UnknownPlatformError`` reaches the caller from
    :func:`_package_claim_offenders` rather than being raised here.
    """
    archive = archives.get(provenance.target)
    platform = targets.get(provenance.target)
    if archive is None or platform is None:
        return []
    expected = {archive, f"{archive}{release_declaration.SIDECAR_SUFFIX}"}
    claimed = set(provenance.assets)
    missing = sorted(expected - claimed)
    omissions = (
        [f"target {provenance.target!r} omits its own {missing!r}"] if missing else []
    )
    return [
        *omissions,
        *_package_claim_offenders(provenance, claimed - expected, platform=platform),
    ]


def _package_claim_offenders(
    provenance: ProducerProvenance, names: cabc.Iterable[str], *, platform: str
) -> list[str]:
    """Describe claims that are not one package per format ``platform`` stages.

    Parameters
    ----------
    provenance
        The record whose extra claims are being judged.
    names
        The claims left once the record's own archive and sidecar are removed.
    platform
        The platform whose package formats the target stages.

    Returns
    -------
    list[str]
        Human-readable descriptions, sorted so a re-run reports the same list.

    Raises
    ------
    release_package_formats.UnknownPlatformError
        If ``platform`` stages no package format at all.
    """
    formats = release_package_formats.PLATFORM_FORMATS.get(platform)
    if formats is None:
        raise release_package_formats.UnknownPlatformError(platform)
    per_format: dict[str, int] = dict.fromkeys(formats, 0)
    offenders: list[str] = []
    for name in sorted(names):
        matched = next(
            (
                package_format
                for package_format in formats
                if name.endswith(package_format)
            ),
            None,
        )
        if matched is None:
            offenders.append(
                f"target {provenance.target!r} claims {name!r}, which is neither "
                f"its archive, its sidecar, nor a {platform} package"
            )
        else:
            per_format[matched] += 1
    offenders.extend(
        f"target {provenance.target!r} must claim 1 {package_format} package, "
        f"claims {count}"
        for package_format, count in sorted(per_format.items())
        if count != 1
    )
    return offenders

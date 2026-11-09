"""Bind each producer record's claims to the target that record names.

A producer record says which target it served, so the archive, that archive's
sidecar, and the package formats its platform stages are all knowable before
any claim is compared with another record's. Publication needs that binding:
comparing only the *union* of every record's claims with the release
declaration lets two same-platform records exchange their archive, sidecar,
and packages while the union stays identical, and the release would then
attach one target's bytes under another target's name.

A package name cannot be *derived*: three external shared actions own those
names, so the format suffix and the per-target count are all the name itself
says. Identity comes from elsewhere. Every producer uploads one workflow
artefact, the release downloads each into its own subtree of ``dist``, and the
record is written inside the subtree its own producer uploaded. A package's
position therefore names the target that staged it, which is an observation a
count can never supply: two same-platform targets that trade one package each
leave every count unchanged and the union identical, and only the position
refuses the trade.

Position is not proof either, and this module does not claim it is. It says
where the download put a file, not who built it; a producer that wrote into
another target's subtree would be believed. What it does establish is that the
claims are consistent with the tree the run actually has, which is the window
a count leaves open.

So this check enforces three things about the package half, and the caller
supplies the third. A record must claim one package per format its platform
stages, a claim matching no format at all is refused, and -- given
``package_origins``, the target keys whose downloaded subtree each published
name was found in -- every package it claims must have been staged under its
own target. The origins are the adapter's observation of a real tree, so this
module still reads no file and parses nothing.

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
    package_origins: cabc.Mapping[str, str] | None = None,
) -> None:
    """Refuse a record that claims artefacts its own target cannot have staged.

    Parameters
    ----------
    provenances
        Every validated producer record found in the downloaded tree.
    archives
        Configured target key mapped to the archive name that target stages.
    targets
        Configured target key mapped to the platform it serves.
    package_origins
        Published package name mapped to the target key whose downloaded
        subtree it was staged in. Omitted by a caller with no tree to observe,
        which leaves the per-format count as the only package guarantee.

    Raises
    ------
    TargetClaimMismatchError
        If any record omits its own archive or sidecar, claims a name that is
        neither, does not claim exactly one package per format its platform
        stages, or claims a package another target's subtree staged.

    Notes
    -----
    ``UnknownPlatformError`` reaches the caller from
    :func:`_package_claim_offenders` when a record's configured platform
    stages no package format at all, and so stays out of the ``Raises``
    section.

    A target with no entry in ``archives`` or ``targets`` is skipped here
    rather than reported: coverage calls it an uncovered target, which sends
    the operator to the configuration rather than to a producer record.

    ``package_origins`` is what turns the package half from a count into an
    observation of which target actually staged each name, so a caller that has
    a tree should always pass it. It is corroboration, not proof: the adapter
    reported where the download put each file, and a walked tree is only as
    trustworthy as the producer that wrote into it. A name absent from the
    mapping is not reported: the staged-tree check owns "this declared name was
    never staged", and reporting it here would blame the producer for an
    absence no record can cause.

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
    origins = {} if package_origins is None else package_origins
    offenders = [
        offender
        for provenance in provenances
        for offender in _target_claim_offenders(
            provenance, archives=archives, targets=targets, origins=origins
        )
    ]
    if offenders:
        raise TargetClaimMismatchError(offenders)


def _target_claim_offenders(
    provenance: ProducerProvenance,
    *,
    archives: cabc.Mapping[str, str],
    targets: cabc.Mapping[str, str],
    origins: cabc.Mapping[str, str],
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
    packages = claimed - expected
    missing = sorted(expected - claimed)
    omissions = (
        [f"target {provenance.target!r} omits its own {missing!r}"] if missing else []
    )
    return [
        *omissions,
        *_package_claim_offenders(provenance, packages, platform=platform),
        *_foreign_package_offenders(provenance, packages, origins=origins),
    ]


def _foreign_package_offenders(
    provenance: ProducerProvenance,
    names: cabc.Iterable[str],
    *,
    origins: cabc.Mapping[str, str],
) -> list[str]:
    """Describe packages a record claims from another target's subtree.

    Parameters
    ----------
    provenance
        The record whose package claims are being judged.
    names
        The claims left once the record's own archive and sidecar are removed.
    origins
        Published package name mapped to the target key that staged it.

    Returns
    -------
    list[str]
        One description per claim whose staged position names another target,
        sorted so a re-run reports the same list.

    Notes
    -----
    A name with no recorded origin is passed over rather than refused. The
    staged-tree check owns an absent name, and blaming the producer for an
    absence it cannot cause would send the operator to the wrong job.
    """
    return [
        f"target {provenance.target!r} claims {name!r}, which "
        f"target {origins[name]!r} staged"
        for name in sorted(names)
        if name in origins and origins[name] != provenance.target
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

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
own target. A package no subtree staged has no origin at all, and that is
refused rather than passed over: the name-based staged check cannot see a
package moved to an unattributed directory, because it still carries its own
name. The origins are the adapter's observation of a real tree, so this module
still reads no file and parses nothing.

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
        subtree staged it, or ``None`` when no tree was walked. Supplied,
        every claimed package must have an origin and it must be the record's
        own target; omitted, the per-format count is the whole guarantee. An
        observation, not a proof, as the module docstring sets out.

    Raises
    ------
    TargetClaimMismatchError
        If any record omits its own archive or sidecar, claims a name that is
        neither, misses or doubles a package format its platform stages, or --
        when origins were supplied -- claims a package its own target's
        subtree did not stage, including one no subtree staged.

    Notes
    -----
    ``UnknownPlatformError`` reaches the caller from
    :func:`_package_claim_offenders` rather than being raised here, so it stays
    out of the ``Raises`` section. A target with no entry in ``archives`` or
    ``targets`` is skipped rather than reported: coverage calls it an
    uncovered target, which sends the operator to the configuration.

    Examples
    --------
    >>> from release_producer_provenance import ProducerProvenance
    >>> provenance = ProducerProvenance(
    ...     schema=1, commit="a" * 40, run_id="7", run_attempt="1",
    ...     target="linux-x86_64", platform="linux",
    ...     assets={"pkg-gnu.tar.gz": "b" * 64, "pkg-gnu.tar.gz.sha256": "c" * 64,
    ...             "pkg.deb": "d" * 64, "pkg.rpm": "e" * 64},
    ... )
    >>> archives = {"linux-x86_64": "pkg-gnu.tar.gz"}
    >>> targets = {"linux-x86_64": "linux"}
    >>> require_target_claims([provenance], archives=archives, targets=targets)
    >>> try:
    ...     require_target_claims(
    ...         [provenance],
    ...         archives={"linux-x86_64": "pkg-musl.tar.gz"},
    ...         targets=targets,
    ...     )
    ... except TargetClaimMismatchError as refusal:
    ...     print("omits its own" in str(refusal))
    True
    """
    offenders = list(
        _offenders(
            provenances,
            archives=archives,
            targets=targets,
            origins=package_origins,
        )
    )
    if offenders:
        raise TargetClaimMismatchError(offenders)


def _offenders(
    provenances: cabc.Iterable[ProducerProvenance],
    *,
    archives: cabc.Mapping[str, str],
    targets: cabc.Mapping[str, str],
    origins: cabc.Mapping[str, str] | None,
) -> cabc.Iterator[str]:
    """Yield every way any record's claims are not its target's own.

    Yields
    ------
    str
        One description per finding, record by record.

    Notes
    -----
    A generator rather than a flat list so one record's refusals are produced
    and the next record's begin, which keeps a long release's refusal from
    being assembled twice: once per record and once for the whole run.

    ``origins`` is ``None`` when the caller supplied no observation, which is
    distinct from an empty one: an empty mapping is a tree that was walked and
    attributed nothing, and every claimed package is then refused.
    """
    for provenance in provenances:
        yield from _target_claim_offenders(
            provenance, archives=archives, targets=targets, origins=origins
        )


def _target_claim_offenders(
    provenance: ProducerProvenance,
    *,
    archives: cabc.Mapping[str, str],
    targets: cabc.Mapping[str, str],
    origins: cabc.Mapping[str, str] | None,
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
    origins: cabc.Mapping[str, str] | None,
) -> list[str]:
    """Describe packages a record claims without its own subtree staging them.

    Parameters
    ----------
    provenance
        The record whose package claims are being judged.
    names
        The claims left once the record's own archive and sidecar are removed.
    origins
        Published package name mapped to the target key that staged it. Only
        consulted when the caller supplied an observation at all; see
        :func:`require_target_claims`.

    Returns
    -------
    list[str]
        One description per claim whose staged position names another target
        or no target at all, sorted so a re-run reports the same list.

    Notes
    -----
    A name with no recorded origin is refused here, not passed over. The
    adapter attributes a package only when exactly one producer subtree holds
    it, so an absent origin means the download put the file somewhere no
    producer owns -- the ``dist`` root, or a subtree holding no record. The
    staged-tree check cannot see that: it matches published *names*, and a
    package moved to an unattributed location keeps its name, so a swapped
    package with an intact digest would otherwise select cleanly.

    The refusal is confined to a call that supplied origins. A caller with no
    observation has not claimed the packages are unattributed, only that it
    did not look, and the count checks are then all the guarantee there is.
    """
    if origins is None:
        return []
    return [
        _unattributed(provenance, name, origins)
        if name not in origins
        else f"target {provenance.target!r} claims {name!r}, which "
        f"target {origins[name]!r} staged"
        for name in sorted(names)
        if origins.get(name) != provenance.target
    ]


def _unattributed(
    provenance: ProducerProvenance, name: str, origins: cabc.Mapping[str, str]
) -> str:
    """Describe a claimed package no producer's subtree was found to hold.

    Returns
    -------
    str
        The refusal naming the record's target, the package, and the stale
        observation it was judged against, so an operator can tell a moved
        file from a subtree whose record is missing.
    """
    return (
        f"target {provenance.target!r} claims {name!r}, which no producer "
        f"subtree staged; the observation covers {sorted(origins)!r}"
    )


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

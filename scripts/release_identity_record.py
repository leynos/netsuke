"""Rebuild a recorded release identity, re-validating it on the way in.

:mod:`release_candidate` decides which candidate a run may publish, and
:meth:`release_candidate.ReleaseIdentity.as_record` exposes that decision as a
mapping. The CLI adapter serializes it, and this adapter owns the record's
field names and decodes them back into a validated identity on read. The two
are separate because their inputs differ: a resolver sees facts the workflow
has just gathered, while a reader sees a value that may have been hand-edited,
truncated by a failed write, or left over from an earlier run.

That difference is why later stages call this rather than handing a mapping to
:func:`release_candidate.resolve_identity`. A stored ``is_prerelease`` flag is
checked against the version instead of being trusted, because a stale record
must not be able to reclassify a prerelease as a final release. A ``null`` tag
is preserved as absence, because ``str(None)`` would read back as the
four-character tag ``"None"`` — a value no tag could have and one the validator
would refuse with a misleading diagnosis.

Every other field is re-validated in full, so no stage can act on a candidate
the policy module would have refused.
"""

import typing as typ

import release_candidate

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: Every field an identity record must carry. Listed rather than derived from
#: the dataclass, so a record written by an older or hand-edited producer is
#: refused here instead of raising ``KeyError`` from somewhere further down.
_IDENTITY_FIELDS = (
    "tag",
    "commit",
    "package_name",
    "version",
    "bin_name",
    "is_prerelease",
)


class MissingFieldError(release_candidate.IdentityError):
    """Report a record that does not carry every field an identity needs."""

    def __init__(self, missing: list[str]) -> None:
        """Initialise the error naming the absent fields.

        Parameters
        ----------
        missing
            The field names the record did not carry.
        """
        super().__init__(f"identity record is missing {missing!r}")


class RecordedFlagError(release_candidate.IdentityError):
    """Report a record whose stored prerelease flag contradicts its version."""

    def __init__(self, recorded: object, version: str, *, derived: bool) -> None:
        """Initialise the error naming the contradiction.

        Parameters
        ----------
        recorded
            The flag the record carried.
        version
            The version the record declared.
        derived
            The flag that version implies.
        """
        super().__init__(
            f"identity record claims is_prerelease={recorded!r} for version "
            f"{version!r}, which derives {derived!r}"
        )


def _recorded_tag(recorded: object) -> str | None:
    """Rebuild a stored tag, preserving a rehearsal's recorded absence.

    A rehearsal wrote ``null`` rather than a tag, and ``str(None)`` would read
    back as the four-character tag ``"None"`` — a value no tag could have and
    one the validator would refuse with a misleading diagnosis. Absence is
    therefore passed through as absence; any other non-string is coerced, so
    the validators, not this reader, decide whether it is acceptable.

    Returns
    -------
    str | None
        ``None`` when the record carried no tag, otherwise the stored value
        coerced to a string.

    Examples
    --------
    >>> _recorded_tag(None) is None
    True
    >>> _recorded_tag("v1.2.3")
    'v1.2.3'
    """
    return None if recorded is None else str(recorded)


def identity_from_mapping(
    record: cabc.Mapping[str, object],
) -> release_candidate.ReleaseIdentity:
    """Rebuild an identity from a previously recorded mapping.

    Later stages read the recorded identity rather than resolving it again, so
    that every stage compares against the candidate one process resolved.

    Parameters
    ----------
    record
        A mapping produced by
        :meth:`release_candidate.ReleaseIdentity.as_record`.

    Returns
    -------
    release_candidate.ReleaseIdentity
        The identity the record describes, re-validated on the way in.

    Raises
    ------
    MissingFieldError
        If the record does not carry every field an identity needs.
    RecordedFlagError
        If the stored ``is_prerelease`` flag contradicts the version.

    Notes
    -----
    A field that fails validation surfaces as ``IdentityError`` from
    :func:`release_candidate.resolve_identity`, which this function delegates
    to and therefore does not raise itself.

    Examples
    --------
    >>> record = {"tag": "v1.2.3", "commit": "0" * 40,
    ...           "package_name": "netsuke-build", "version": "1.2.3",
    ...           "bin_name": "netsuke", "is_prerelease": False}
    >>> identity_from_mapping(record).version
    '1.2.3'
    """
    missing = [name for name in _IDENTITY_FIELDS if name not in record]
    if missing:
        raise MissingFieldError(missing)
    identity = release_candidate.resolve_identity(
        release_candidate.CandidateFacts(
            tag=_recorded_tag(record["tag"]),
            commit=str(record["commit"]),
            package_name=str(record["package_name"]),
            version=str(record["version"]),
            bin_name=str(record["bin_name"]),
        )
    )
    # Reject a record whose stored flag contradicts its own version rather
    # than trusting the field: a hand-edited or stale record must not be able
    # to silently reclassify a prerelease as a final release.
    recorded = record["is_prerelease"]
    if not isinstance(recorded, bool) or recorded is not identity.is_prerelease:
        raise RecordedFlagError(
            recorded, identity.version, derived=identity.is_prerelease
        )
    return identity

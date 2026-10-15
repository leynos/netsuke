"""Decide whether a string is a well-formed semantic version.

Split from ``release_candidate`` because the version grammar and the release
identity are separate concerns: the grammar is a closed, testable predicate
over strings, while the identity is a record built from validated facts. A
module holding both would put the leading-zero rule and the tag-agreement rule
in one place, where a reader cannot tell which one refused a candidate.

The rule that matters is the leading-zero rule, and its exact shape is easy to
state too broadly. Cargo parses versions with the `semver` crate, whose
identifier rule refuses a leading zero only when an identifier is *entirely*
numeric (the `!segment_has_nondigit` guard in its `parse.rs`). So ``0.1.0-01``
is invalid but ``0.1.0-beta04`` is valid: the padding sits inside an
alphanumeric identifier, which the grammar permits. ``0.1.0-beta04`` is still
not interchangeable with ``0.1.0-beta4`` — they are different identifiers, and
no ordering comparison makes them equal — so the tag must spell whichever one
the manifest declares. Confusing those two facts would either refuse a
publishable version or accept a tag naming a different one.
"""

import re

#: One dot-separated SemVer identifier that is entirely numeric. A leading
#: zero is permitted only for the single digit ``0``.
_NUMERIC_IDENTIFIER = re.compile(r"\A(?:0|[1-9][0-9]*)\Z")

#: One dot-separated SemVer identifier that carries at least one non-digit.
#: Hyphens are permitted; underscores are not, which is the rule that refuses
#: ``0.1.0-beta_4``.
_ALPHANUMERIC_IDENTIFIER = re.compile(r"\A[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*\Z")

_SEMVER = re.compile(
    r"\A(?P<core>[0-9]+(?:\.[0-9]+){2})"
    r"(?:-(?P<prerelease>[0-9A-Za-z.-]+))?"
    r"(?:\+(?P<metadata>[0-9A-Za-z.-]+))?\Z"
)

#: A semantic version's core is exactly three dot-separated identifiers.
_CORE_IDENTIFIERS = 3


def _has_well_formed_identifiers(text: str) -> bool:
    """Return whether every dot-separated identifier in ``text`` is well formed.

    Parameters
    ----------
    text
        A prerelease or build-metadata field, without its leading separator.

    Returns
    -------
    bool
        `True` when the field is non-empty and every identifier is either a
        canonical numeric identifier or an alphanumeric one carrying at least
        one non-digit. The split between those two branches is where the
        leading-zero rule lives: a padded identifier is refused only when it
        has no non-digit to make it alphanumeric, so ``01`` fails while
        ``beta04`` passes.

    Examples
    --------
    >>> _has_well_formed_identifiers("beta.4")
    True
    >>> _has_well_formed_identifiers("01")
    False
    """
    identifiers = text.split(".")
    if not identifiers or any(not part for part in identifiers):
        return False
    return all(
        _NUMERIC_IDENTIFIER.match(part) or _ALPHANUMERIC_IDENTIFIER.match(part)
        for part in identifiers
    )


def is_valid_semver(text: str) -> bool:
    """Return whether ``text`` is a well-formed semantic version.

    The core is three dot-separated numeric identifiers with no leading zeros;
    an optional prerelease and build metadata follow. A prerelease identifier
    that is entirely numeric must likewise not carry a leading zero, which is
    what makes ``0.1.0-01`` invalid while ``0.1.0-beta04`` remains valid.

    Parameters
    ----------
    text
        The candidate version string.

    Returns
    -------
    bool
        `True` when the whole string is one well-formed semantic version.

    Examples
    --------
    >>> is_valid_semver("0.1.0-beta4")
    True
    >>> is_valid_semver("0.1.0-01")
    False
    """
    match = _SEMVER.match(text)
    if match is None:
        return False
    core = match.group("core").split(".")
    if len(core) != _CORE_IDENTIFIERS or not all(
        _NUMERIC_IDENTIFIER.match(part) for part in core
    ):
        return False
    prerelease = match.group("prerelease")
    if prerelease is not None and not _has_well_formed_identifiers(prerelease):
        return False
    metadata = match.group("metadata")
    return metadata is None or _has_well_formed_identifiers(metadata)


def is_prerelease(version: str) -> bool:
    """Return whether a well-formed version carries a prerelease field.

    Parameters
    ----------
    version
        A version already accepted by :func:`is_valid_semver`.

    Returns
    -------
    bool
        `True` when a prerelease field is present. Build metadata alone does
        not make a version a prerelease, which is why this reads the
        prerelease group rather than testing for the presence of any suffix.

    Examples
    --------
    >>> is_prerelease("0.1.0-beta4")
    True
    >>> is_prerelease("0.1.0+meta")
    False
    """
    match = _SEMVER.match(version)
    return match is not None and match.group("prerelease") is not None

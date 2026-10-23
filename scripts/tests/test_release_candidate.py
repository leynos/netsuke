"""Specify the release-candidate identity and tag/version agreement.

These are the LEM-1 cases. The tag accepts exactly the package version it
spells, prerelease included, with no lossy normalization: the assertion is
byte equality against the Cargo version, not a re-rendering of a parsed
version. Both an accept row and several reject rows are present, so a
validator that always raises and one that never raises each fail.
"""

import dataclasses
import json

import pytest
import release_candidate
import release_identity_record
from hypothesis import given, settings
from hypothesis import strategies as st

TAG = "v0.1.0-beta4"
VERSION = "0.1.0-beta4"
COMMIT = "f537009f0c1b0e6a1d2c3b4a5968778695a4b3c2"
PACKAGE = "netsuke-build"
BIN_NAME = "netsuke"


#: A fully tagged candidate the refusal rows vary one field at a time. It is a
#: value rather than a dict of overrides because :func:`dataclasses.replace`
#: checks each replacement against the field it replaces; unpacking a mapping
#: would widen `tag` to `str | None` and lose that per-field agreement.
TAGGED_FACTS = release_candidate.CandidateFacts(
    tag=TAG,
    commit=COMMIT,
    package_name=PACKAGE,
    version=VERSION,
    bin_name=BIN_NAME,
)


def _facts(**changes: str | None) -> release_candidate.CandidateFacts:
    """Return candidate facts with the named fields replaced."""
    return dataclasses.replace(TAGGED_FACTS, **changes)


@pytest.mark.parametrize(
    ("tag", "version"),
    [
        pytest.param("v0.1.0-beta4", "0.1.0-beta4", id="the-beta4-prerelease"),
        pytest.param("v1.2.3", "1.2.3", id="a-plain-three-part-release"),
        pytest.param("v0.1.0-beta.4", "0.1.0-beta.4", id="a-dotted-prerelease"),
        pytest.param("v0.1.0-beta4+meta", "0.1.0-beta4+meta", id="build-metadata"),
        pytest.param("v0.0.0", "0.0.0", id="the-zero-version"),
        pytest.param("v1.0.0-0", "1.0.0-0", id="a-numeric-prerelease"),
        pytest.param("v1.2.3-x.7.z.92", "1.2.3-x.7.z.92", id="a-multi-part-one"),
    ],
)
def test_matching_tag_and_version_are_accepted(tag: str, version: str) -> None:
    """Accept a tag whose spelling is exactly the package version."""
    identity = release_candidate.resolve_identity(_facts(tag=tag, version=version))
    assert identity.version == version, "the accepted version must be the one asked for"
    assert identity.tag == tag, "the accepted tag must be the one asked for"


@pytest.mark.parametrize(
    "version",
    [
        pytest.param("0.1.0-beta3", id="an-earlier-prerelease"),
        pytest.param("0.1.0", id="the-release-the-prerelease-leads-to"),
        pytest.param("0.1.0-beta5", id="a-later-prerelease"),
        pytest.param("0.1.0-beta04", id="a-padded-prerelease-identifier"),
        pytest.param("0.1.0-rc4", id="a-different-prerelease-word"),
        pytest.param("0.1.1-beta4", id="a-different-patch"),
        pytest.param("0.2.0-beta4", id="a-different-minor"),
        pytest.param("1.0.0-beta4", id="a-different-major"),
    ],
)
def test_version_that_disagrees_with_the_tag_is_rejected(version: str) -> None:
    """Reject a package version the tag does not spell, whatever the reason."""
    with pytest.raises(release_candidate.TagVersionMismatchError) as caught:
        release_candidate.resolve_identity(_facts(version=version))
    # The diagnostic must name both readings, or a maintainer cannot tell a
    # stale tag from a version bump that was never tagged.
    assert version in str(caught.value), "the diagnostic must name the version"
    assert TAG in str(caught.value), "the diagnostic must name the tag"


@pytest.mark.parametrize(
    "version",
    [
        pytest.param("0.1.0-beta4+meta", id="build-metadata-added"),
        pytest.param("v0.1.0-beta4", id="the-v-prefix-carried-into-the-version"),
        pytest.param(" 0.1.0-beta4", id="a-leading-space"),
        pytest.param("0.1.0-beta4 ", id="a-trailing-space"),
        pytest.param("0.1.0-beta4\n", id="a-trailing-newline"),
        pytest.param("0.01.0-beta4", id="a-leading-zero-in-the-core"),
        pytest.param("0.1.0-beta4+build.1", id="a-longer-build-metadata"),
    ],
)
def test_lossy_normalization_does_not_accept_a_near_match(version: str) -> None:
    """Accept only byte equality, never an agreed-after-parsing spelling.

    Every row here would be accepted by an implementation that parsed both
    sides into a normalized form and compared those, so they are what
    separates equality from a lossy comparison.
    """
    with pytest.raises(release_candidate.IdentityError):
        release_candidate.resolve_identity(_facts(version=version))


@pytest.mark.parametrize(
    "tag",
    [
        pytest.param("0.1.0-beta4", id="a-missing-v-prefix"),
        pytest.param("V0.1.0-beta4", id="a-capital-v"),
        pytest.param("release-0.1.0-beta4", id="another-prefix"),
        pytest.param("v", id="a-bare-v"),
        pytest.param("", id="an-empty-tag"),
        pytest.param("v0.1.0-beta4 ", id="a-trailing-space"),
    ],
)
def test_tag_that_is_not_v_prefixed_semver_is_rejected(tag: str) -> None:
    """Require a `v`-prefixed tag and a well-formed version after it."""
    with pytest.raises(release_candidate.MalformedTagError) as caught:
        release_candidate.resolve_identity(_facts(tag=tag))
    assert repr(tag) in str(caught.value), "the diagnostic must name the tag"


@pytest.mark.parametrize(
    "commit",
    [
        pytest.param("", id="empty"),
        pytest.param("abc1234", id="an-abbreviated-sha"),
        pytest.param("F537009F0C1B0E6A1D2C3B4A5968778695A4B3C2", id="uppercase"),
        pytest.param("f537009f0c1b0e6a1d2c3b4a5968778695a4b3c", id="thirty-nine"),
        pytest.param("f537009f0c1b0e6a1d2c3b4a5968778695a4b3c2d", id="forty-one"),
        pytest.param("g537009f0c1b0e6a1d2c3b4a5968778695a4b3c2", id="a-non-hex-digit"),
        pytest.param("HEAD", id="a-symbolic-revision"),
    ],
)
def test_unpeeled_commit_is_rejected(commit: str) -> None:
    """Require a full lowercase commit SHA, never an abbreviated or symbolic ref."""
    with pytest.raises(release_candidate.IdentityError) as caught:
        release_candidate.resolve_identity(_facts(commit=commit))
    assert "commit" in str(caught.value), "the diagnostic must name the field"


@pytest.mark.parametrize(
    ("version", "expected"),
    [
        pytest.param("0.1.0-beta4", True, id="a-prerelease"),
        pytest.param("0.1.0-rc.1", True, id="a-dotted-prerelease"),
        pytest.param("0.1.0", False, id="a-plain-release"),
        pytest.param("0.1.0+meta", False, id="build-metadata-is-not-a-prerelease"),
    ],
)
def test_prerelease_flag_tracks_the_cargo_version(
    version: str, *, expected: bool
) -> None:
    """Derive the prerelease flag from the version, not from the tag spelling."""
    identity = release_candidate.resolve_identity(
        _facts(tag=f"v{version}", version=version)
    )
    assert identity.is_prerelease is expected, (
        "the prerelease flag must be derived from the version"
    )


def test_identity_record_has_a_stable_key_order() -> None:
    """Serialize to JSON with sorted keys, so two records diff by content only."""
    identity = release_candidate.resolve_identity(_facts())
    record = identity.as_record()

    assert list(record) == sorted(record), (
        "the identity record's keys must already be in sorted order"
    )
    rendered = identity.to_json()
    assert rendered == json.dumps(record, sort_keys=True, indent=2) + "\n", (
        "the rendered record must be sorted, indented, and newline-terminated"
    )
    assert identity.to_json() == rendered, "serialization must be deterministic"
    assert json.loads(rendered) == record, "the record must survive a round trip"


def test_identity_record_names_the_candidate_and_its_distribution() -> None:
    """Carry the facts a later stage needs to prove it published one candidate."""
    record = release_candidate.resolve_identity(_facts()).as_record()

    assert record["commit"] == COMMIT, "the record must name the candidate commit"
    assert record["tag"] == TAG, "the record must name the release tag"
    assert record["version"] == VERSION, "the record must name the package version"
    assert record["package_name"] == PACKAGE, "the record must name the package"
    assert record["bin_name"] == BIN_NAME, "the record must name the binary"
    assert record["is_prerelease"] is True, "beta4 is a prerelease"


def test_identity_record_is_immutable() -> None:
    """Reject mutation, so a stage cannot rewrite the candidate mid-sequence.

    The assignment is written through ``setattr`` on purpose. A literal
    ``identity.commit = ...`` is refused statically by the typechecker, so the
    only way to reach the runtime guard is to bypass its static twin; going
    through ``setattr`` states that intent rather than suppressing a
    diagnostic, and it fails here for the frozen-dataclass reason rather than
    a lint reason.
    """
    identity = release_candidate.resolve_identity(_facts())
    rewritten = "commit"
    with pytest.raises(dataclasses.FrozenInstanceError):
        setattr(identity, rewritten, "0" * 40)


@pytest.mark.parametrize(
    ("package_name", "bin_name"),
    [
        pytest.param("", BIN_NAME, id="an-empty-package-name"),
        pytest.param(PACKAGE, "", id="an-empty-binary-name"),
        pytest.param("netsuke build", BIN_NAME, id="a-spaced-package-name"),
        pytest.param("netsuke.build", BIN_NAME, id="a-dotted-package-name"),
        pytest.param(PACKAGE, "netsuke ", id="a-padded-binary-name"),
        pytest.param(PACKAGE, "../netsuke", id="a-traversing-binary-name"),
        pytest.param(PACKAGE, "netsuke/evil", id="a-slashing-binary-name"),
        pytest.param("netsukeé", BIN_NAME, id="a-non-ascii-package-name"),
    ],
)
def test_malformed_distribution_names_are_rejected(
    package_name: str, bin_name: str
) -> None:
    """Reject names that could not appear in a Cargo manifest or on disk.

    `netsuke_build` is deliberately absent: Cargo accepts an underscore in a
    package name, so refusing it would be this validator inventing a rule the
    registry does not have.
    """
    with pytest.raises(release_candidate.IdentityError):
        release_candidate.resolve_identity(
            _facts(package_name=package_name, bin_name=bin_name)
        )


@pytest.mark.parametrize(
    "package_name",
    [
        pytest.param("netsuke-build", id="the-netsuke-package"),
        pytest.param("netsuke", id="the-binary-name-too"),
        pytest.param("netsuke_build", id="an-underscore"),
        pytest.param("2captcha", id="a-leading-digit"),
        pytest.param("a", id="a-single-character"),
    ],
)
def test_ordinary_cargo_names_are_accepted(package_name: str) -> None:
    """Accept the names Cargo itself accepts, so the rule invents nothing.

    A validator that refused a second one of these would be stricter than the
    registry and would fail a legitimate release for the wrong reason.
    """
    identity = release_candidate.resolve_identity(
        _facts(package_name=package_name, bin_name=package_name)
    )
    assert identity.package_name == package_name, (
        "a Cargo-legal name must be accepted unchanged"
    )


_NUMERIC = st.integers(min_value=0, max_value=999).map(str)
#: A prerelease identifier that satisfies the leading-zero rule: either a
#: canonical numeric identifier, or an alphanumeric one carrying a letter.
#: Generating `00` here would produce an invalid version and make the round
#: trip assert a rule the contract does not claim.
_PRERELEASE_IDENTIFIER = st.one_of(
    _NUMERIC,
    st.from_regex(r"[A-Za-z][A-Za-z0-9-]*", fullmatch=True),
)
_CORE = st.lists(_NUMERIC, min_size=3, max_size=3).map(".".join)
_PRERELEASE = st.one_of(
    st.none(),
    st.lists(_PRERELEASE_IDENTIFIER, min_size=1, max_size=3).map(".".join),
)
_METADATA = st.lists(
    st.one_of(_NUMERIC, st.from_regex(r"[A-Za-z][A-Za-z0-9-]*", fullmatch=True)),
    min_size=1,
    max_size=2,
).map(".".join)


@settings(max_examples=200, deadline=None, derandomize=True)
@given(core=_CORE, prerelease=_PRERELEASE, metadata=st.one_of(st.none(), _METADATA))
def test_any_well_formed_version_round_trips_through_its_own_tag(
    core: str, prerelease: str | None, metadata: str | None
) -> None:
    """Accept every well-formed version under the tag that spells it.

    The generator reaches all three material classes named by LEM-1 — a bare
    release, a prerelease, and a version with build metadata — so a validator
    that handled only one spelling would fail here rather than pass vacuously.
    Build metadata is generated separately from the prerelease because the two
    are independently optional: without the independent draw, the only
    metadata-bearing cases would also be prereleases, and a validator that
    rejected metadata on a placed release would pass.
    """
    suffix = "" if metadata is None else f"+{metadata}"
    version = core if prerelease is None else f"{core}-{prerelease}{suffix}"
    if prerelease is None and metadata is not None:
        version = f"{core}+{metadata}"

    identity = release_candidate.resolve_identity(
        _facts(tag=f"v{version}", version=version)
    )

    assert identity.version == version, "the round trip must preserve the version"
    assert identity.is_prerelease is (prerelease is not None), (
        "the prerelease flag must match whether a prerelease field was generated"
    )
    assert json.loads(identity.to_json())["version"] == version, (
        "the serialized record must carry the same version"
    )


@settings(max_examples=200, deadline=None, derandomize=True)
@given(
    left=_CORE,
    right=_CORE,
    prerelease=st.from_regex(r"\b[a-z][a-z0-9]*\b", fullmatch=True),
)
def test_only_the_exact_spelling_is_accepted(
    left: str, right: str, prerelease: str
) -> None:
    """Reject a version the tag does not spell, over generated pairs.

    A near-match is built from the same generator as an accepted value, so the
    rejection is not an artifact of malformed input: the only difference is the
    prerelease suffix the tag omits.
    """
    version = f"{left}-{prerelease}"
    with pytest.raises(release_candidate.TagVersionMismatchError):
        release_candidate.resolve_identity(_facts(tag=f"v{left}", version=version))

    if left != right:
        with pytest.raises(release_candidate.TagVersionMismatchError):
            release_candidate.resolve_identity(_facts(tag=f"v{left}", version=right))


def test_a_tagless_candidate_is_accepted_as_a_rehearsal() -> None:
    """Accept an identity that records no tag, because a rehearsal has none.

    A pull-request run is triggered by an event whose ref is a merge ref, so
    there is no release tag whose spelling could agree with the manifest. The
    alternative — inventing the version-shaped tag for the package — would
    record a tag that does not point at the commit the bytes were built from,
    which is the near-match this module exists to refuse. Absence is therefore
    the rehearsal's marker.
    """
    identity = release_candidate.resolve_identity(_facts(tag=None))

    assert identity.tag is None, "a rehearsal must record the absence of a tag"
    assert identity.version == VERSION, "a rehearsal still records the version"
    assert identity.is_prerelease is True, "the version still decides prerelease"


def test_a_tagless_identity_round_trips_through_its_record() -> None:
    """Read a rehearsal's ``tag: null`` back as absence, not as the text `None`.

    ``str(None)`` would make the tag ``"None"``, which the tag validator would
    then refuse — so a record written by this module could not be read by it.
    """
    identity = release_candidate.resolve_identity(_facts(tag=None))

    rebuilt = release_identity_record.identity_from_mapping(
        json.loads(identity.to_json())
    )

    assert rebuilt.tag is None, "absence must survive the round trip as absence"


def test_a_malformed_tag_is_refused_as_malformed_not_merely_unmatched() -> None:
    """Refuse a bad version as malformed, which is a different diagnosis.

    This is the row that exposed the weak witness. `0.01.0-beta4` was refused
    by the tag/version agreement — the tag `v0.1.0-beta4` does not spell it —
    so a grammar that permitted leading zeros still refused it under the same
    exception, and the rule looked verified while nothing tested it. The tag
    here carries the malformed version, so the agreement *would* hold if the
    grammar allowed it; only the grammar can refuse this input, and the
    diagnosis it raises is the point. ``test_release_semver`` owns the
    exhaustive rows for the grammar itself.
    """
    with pytest.raises(release_candidate.MalformedTagError):
        release_candidate.resolve_identity(
            _facts(tag="v0.01.0-beta4", version="0.01.0-beta4")
        )

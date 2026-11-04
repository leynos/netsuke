"""Specify the candidate acceptance rules over parsed identity and manifest facts.

The command-line suite exercises these rules through the whole check, where a
rejection also proves the readers ran in order. This module calls them
directly, so each rule's accept and reject outcomes are named where they are
decided.
"""

import dataclasses

import crates_io_candidate_archive
import pytest
import release_candidate
from crates_io_candidate_errors import CandidateError, CandidateIssue
from crates_io_candidate_policy import (
    CandidateExpectation,
    require_identity_matches,
    require_manifest_identity,
    require_release_mode,
)
from crates_io_candidate_support import COMMIT, PACKAGE_NAME, TAG, VERSION


def _identity(*, tag: str | None = TAG) -> release_candidate.ReleaseIdentity:
    """Build the accepted release identity, replacing only the tag."""
    return release_candidate.ReleaseIdentity(
        tag, COMMIT, PACKAGE_NAME, VERSION, "netsuke", is_prerelease=False
    )


@dataclasses.dataclass(frozen=True, slots=True)
class ExpectationOverrides:
    """Carry the workflow values a rule test substitutes.

    Every field defaults to the accepted candidate's value, so a case names only
    the one fact under test. ``tag`` distinguishes the two release modes, where
    ``None`` is a rehearsal.
    """

    tag: str | None = TAG
    commit: str = COMMIT
    package_name: str = PACKAGE_NAME
    version: str = VERSION


def _expectation(overrides: ExpectationOverrides | None = None) -> CandidateExpectation:
    """Build the workflow expectation from the substituted values."""
    chosen = overrides or ExpectationOverrides()
    return CandidateExpectation(
        chosen.tag, chosen.commit, chosen.package_name, chosen.version
    )


@pytest.mark.parametrize(
    ("identity_tag", "expectation_tag"),
    [
        pytest.param(TAG, TAG, id="publishing"),
        pytest.param(None, None, id="rehearsal"),
    ],
)
def test_require_release_mode_accepts_each_matching_mode(
    identity_tag: str | None, expectation_tag: str | None
) -> None:
    """Accept a tagless rehearsal and a tagged publication that agree."""
    require_release_mode(
        _identity(tag=identity_tag),
        _expectation(ExpectationOverrides(tag=expectation_tag)),
    )


@pytest.mark.parametrize(
    ("identity_tag", "expectation_tag", "expected"),
    [
        pytest.param(TAG, None, CandidateIssue.UNEXPECTED_TAG, id="rehearsal-tagged"),
        pytest.param(None, TAG, CandidateIssue.REHEARSAL, id="publishing-tagless"),
    ],
)
def test_require_release_mode_rejects_a_mode_disagreement(
    identity_tag: str | None, expectation_tag: str | None, expected: CandidateIssue
) -> None:
    """Classify each mode disagreement by the run that cannot accept it."""
    with pytest.raises(CandidateError) as error:
        require_release_mode(
            _identity(tag=identity_tag),
            _expectation(ExpectationOverrides(tag=expectation_tag)),
        )

    assert error.value.issue is expected, "name the release-mode disagreement"


def test_require_identity_matches_accepts_the_recorded_identity() -> None:
    """Accept an identity whose tag, commit, package, and version all agree."""
    require_identity_matches(_identity(), _expectation())


def test_require_identity_matches_names_every_disagreeing_field() -> None:
    """Report all mismatched fields at once rather than the first one found."""
    expectation = _expectation(ExpectationOverrides(commit="b" * 40, version="2.0.0"))

    with pytest.raises(CandidateError) as error:
        require_identity_matches(_identity(), expectation)

    assert error.value.issue is CandidateIssue.IDENTITY_MISMATCH, (
        "classify the identity disagreement"
    )
    rendered = str(error.value)
    assert f"commit={COMMIT!r} (expected {'b' * 40!r})" in rendered, (
        "name the disagreeing commit"
    )
    assert "version='1.2.3' (expected '2.0.0')" in rendered, (
        "name the disagreeing version"
    )
    assert "package_name" not in rendered, "omit fields that agreed"
    assert "tag" not in rendered, "omit the tag when the identity recorded it"


def test_require_identity_matches_checks_release_mode_first() -> None:
    """Prefer the release-mode diagnosis when the mode already disagrees."""
    expectation = _expectation(ExpectationOverrides(tag=None, version="2.0.0"))

    with pytest.raises(CandidateError) as error:
        require_identity_matches(_identity(), expectation)

    assert error.value.issue is CandidateIssue.UNEXPECTED_TAG, (
        "a tagged rehearsal reports its tag, not a secondary field"
    )


def test_require_manifest_identity_accepts_the_packaged_package() -> None:
    """Accept a manifest that names the validated package and version."""
    manifest: dict[str, object] = {
        "package": {"name": PACKAGE_NAME, "version": VERSION}
    }

    require_manifest_identity(manifest, PACKAGE_NAME, VERSION)


@pytest.mark.parametrize(
    ("package", "expected"),
    [
        pytest.param(
            {"name": PACKAGE_NAME, "version": "2.0.0"},
            CandidateIssue.MANIFEST_IDENTITY,
            id="other-version",
        ),
        pytest.param(
            {"name": "another-crate", "version": VERSION},
            CandidateIssue.MANIFEST_IDENTITY,
            id="other-package",
        ),
        pytest.param({}, CandidateIssue.MANIFEST_IDENTITY, id="identity-absent"),
        pytest.param("netsuke", CandidateIssue.PACKAGE_TABLE, id="not-a-table"),
        pytest.param(None, CandidateIssue.PACKAGE_TABLE, id="no-package-table"),
    ],
)
def test_require_manifest_identity_classifies_rejections(
    package: object, expected: CandidateIssue
) -> None:
    """Separate a disagreeing identity from a structurally absent package table."""
    with pytest.raises(CandidateError) as error:
        require_manifest_identity({"package": package}, PACKAGE_NAME, VERSION)

    assert error.value.issue is expected, "classify the manifest rejection"


def test_archive_adapter_exposes_its_manifest_reader() -> None:
    """Prove the archive adapter imported and exposes the reader policy composes."""
    assert callable(crates_io_candidate_archive.read_packaged_manifest), (
        "the adapter imports and exposes the manifest reader"
    )

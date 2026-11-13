"""Hold producer provenance to the claims publication is allowed to trust.

A producer record is the only evidence that the bytes a job uploaded were the
bytes that job built. These tests pin the *claim* a record may make and the
comparisons publication performs before trusting it: what a commit and a run
identity look like, which platforms can be packaged, and which asset names and
digests a producer may declare.

Reading the document itself -- that it is an object, that it carries its
members, that its schema version is understood, and that each member is the
type the format defines -- is a different boundary with different diagnoses,
and is covered by
``scripts/tests/test_release_producer_provenance_decode.py``. Keeping the two
apart is what lets a failure here name a bad *claim* rather than a bad file.

They are deliberately separate from the CLI that writes the record: the policy
is proved without touching a filesystem, and the adapter is proved against real
bytes.

Run via ``make test-release-publication``.
"""

import pytest
from release_producer_provenance import (
    ProducerProvenance,
    RecordedMembers,
    validated_provenance,
)
from release_producer_provenance_checks import (
    declared_names,
    require_candidate,
    require_target_coverage,
    trusted_digests,
)
from release_producer_provenance_errors import (
    DuplicateAssetError,
    DuplicateTargetError,
    EmptyAssetsError,
    ForeignRunError,
    InvalidDigestError,
    MalformedCommitError,
    MalformedRunIdentityError,
    MissingTargetError,
    StaleCandidateError,
    UnknownPlatformError,
    UnsafeAssetNameError,
)

COMMIT = "0123456789abcdef0123456789abcdef01234567"
OTHER_COMMIT = "fedcba9876543210fedcba9876543210fedcba98"
DIGEST = "b" * 64
OTHER_DIGEST = "c" * 64


def _members(**overrides: object) -> RecordedMembers:
    """Return well-typed decoded members, with the requested ones overridden.

    The values are what :func:`release_producer_provenance_decode.decode_provenance`
    hands the policy once it has checked the document's types, so a case below
    is about what the policy decides and never about what the wire carried.

    Returns
    -------
    RecordedMembers
        The decoded members, with the requested ones replaced.
    """
    members: dict[str, object] = {
        "commit": COMMIT,
        "run_id": "1234",
        "run_attempt": "1",
        "target": "linux-x86_64",
        "platform": "linux",
        "assets": {"netsuke_1_amd64.deb": DIGEST},
    }
    members.update(overrides)
    # The overrides are deliberately unvalidated: a case supplies the wrong
    # type on purpose so the policy can be seen refusing it.
    return RecordedMembers(**members)


def _provenance(
    *, target: str = "linux-x86_64", assets: dict[str, str] | None = None
) -> ProducerProvenance:
    """Return a validated producer claim for the coverage helpers."""
    return ProducerProvenance(
        commit=COMMIT,
        run_id="1234",
        run_attempt="1",
        target=target,
        platform="linux",
        assets={"netsuke_1_amd64.deb": DIGEST} if assets is None else assets,
    )


def test_a_complete_record_is_projected_onto_the_claim() -> None:
    """Read every member publication compares, without inventing any."""
    claim = _provenance()

    assert claim.commit == COMMIT, "the claim must name the built commit"
    assert claim.run_id == "1234", "the claim must name the producing run"
    assert claim.run_attempt == "1", "the attempt must be recorded"
    assert claim.target == "linux-x86_64", "the claim must name its target"
    assert claim.platform == "linux", "the claim must name its platform"
    assert claim.assets == {"netsuke_1_amd64.deb": DIGEST}, (
        "the claim must carry the digests verbatim"
    )


@pytest.mark.parametrize(
    "commit",
    [
        "0123456",
        COMMIT.upper(),
        f"{COMMIT[:39]}g",
        "",
    ],
    ids=["short", "uppercase", "not-hex", "empty"],
)
def test_a_commit_that_is_not_a_full_lowercase_commit_is_refused(
    commit: object,
) -> None:
    """Refuse a commit whose shape cannot name one commit unambiguously."""
    with pytest.raises(MalformedCommitError):
        validated_provenance(_members(commit=commit))


@pytest.mark.parametrize("member", ["run_id", "run_attempt"])
@pytest.mark.parametrize("value", ["0", "-1", "run", ""], ids=repr)
def test_a_run_identity_that_is_not_positive_integer_text_is_refused(
    member: str, value: str
) -> None:
    """Refuse a run identity publication cannot compare as a number."""
    with pytest.raises(MalformedRunIdentityError) as refusal:
        validated_provenance(_members(**{member: value}))

    assert member in str(refusal.value), "the diagnosis must name the member"


def test_a_platform_with_no_package_format_is_refused() -> None:
    """Refuse a platform whose packages this release cannot name."""
    with pytest.raises(UnknownPlatformError) as refusal:
        validated_provenance(_members(platform="solaris"))

    assert "solaris" in str(refusal.value), "the diagnosis must name the platform"


def test_a_producer_that_claims_no_assets_is_refused() -> None:
    """Refuse a producer that says nothing about the bytes it uploaded."""
    with pytest.raises(EmptyAssetsError) as refusal:
        validated_provenance(_members(assets={}))

    assert "linux-x86_64" in str(refusal.value), "the diagnosis must name the target"


@pytest.mark.parametrize(
    "name", ["", ".", "..", "dir/asset.deb", "..\\asset.deb"], ids=repr
)
def test_an_unsafe_claimed_name_is_refused(name: str) -> None:
    """Refuse a name that could escape the directory it is written into."""
    with pytest.raises(UnsafeAssetNameError):
        validated_provenance(_members(assets={name: DIGEST}))


@pytest.mark.parametrize(
    "digest",
    ["b" * 63, "b" * 65, DIGEST.upper(), "g" * 64, "", None],
    ids=["short", "long", "uppercase", "not-hex", "empty", "none"],
)
def test_a_malformed_digest_is_refused(digest: object) -> None:
    """Refuse a digest that cannot be compared against real bytes."""
    with pytest.raises(InvalidDigestError):
        validated_provenance(_members(assets={"netsuke.deb": digest}))


def test_a_record_from_another_commit_is_refused_as_stale() -> None:
    """Separate a stale candidate from a malformed record."""
    with pytest.raises(StaleCandidateError) as refusal:
        require_candidate(_provenance(), commit=OTHER_COMMIT, run_id="1234")

    assert COMMIT in str(refusal.value), "the diagnosis must name the built commit"
    assert OTHER_COMMIT in str(refusal.value), "the diagnosis must name the candidate"


def test_a_record_from_another_run_is_refused_as_foreign() -> None:
    """Refuse a record another run wrote, even for the same commit."""
    with pytest.raises(ForeignRunError) as refusal:
        require_candidate(_provenance(), commit=COMMIT, run_id="9999")

    assert "1234" in str(refusal.value), "the diagnosis must name the recorded run"


def test_a_later_attempt_of_the_same_run_is_accepted() -> None:
    """Accept a retried producer, which replaces bytes from the same commit."""
    retried = ProducerProvenance(
        commit=COMMIT,
        run_id="1234",
        run_attempt="3",
        target="linux-x86_64",
        platform="linux",
        assets={"netsuke_1_amd64.deb": DIGEST},
    )

    require_candidate(retried, commit=COMMIT, run_id="1234")


def test_complete_target_coverage_is_accepted() -> None:
    """Accept one record for every configured target."""
    claims = [_provenance(target="a"), _provenance(target="b")]

    require_target_coverage(claims, expected_targets=["a", "b"])


def test_a_missing_target_is_refused() -> None:
    """Refuse a release that lost one target's artefacts."""
    with pytest.raises(MissingTargetError) as refusal:
        require_target_coverage([_provenance(target="a")], expected_targets=["a", "b"])

    assert "b" in str(refusal.value), "the diagnosis must name the uncovered target"


def test_an_undeclared_target_is_reported_with_the_uncovered_targets() -> None:
    """Report a foreign target as a coverage failure, not as extra artefacts."""
    with pytest.raises(MissingTargetError) as refusal:
        require_target_coverage(
            [_provenance(target="a"), _provenance(target="z")],
            expected_targets=["a", "b"],
        )

    message = str(refusal.value)
    assert "'b'" in message, "the diagnosis must name the uncovered target"
    assert "'z'" in message, "the diagnosis must name the undeclared target"


def test_two_records_for_one_target_are_refused() -> None:
    """Refuse two producers claiming one configured target."""
    with pytest.raises(DuplicateTargetError):
        require_target_coverage(
            [_provenance(target="a"), _provenance(target="a")],
            expected_targets=["a"],
        )


def test_declared_names_are_sorted_and_unique() -> None:
    """Return the flat consumer set in a stable order."""
    claims = [
        _provenance(target="a", assets={"b.deb": DIGEST}),
        _provenance(target="b", assets={"a.rpm": OTHER_DIGEST}),
    ]

    assert declared_names(claims) == ("a.rpm", "b.deb"), (
        "the names must be sorted across every producer"
    )


def test_one_name_claimed_twice_is_refused() -> None:
    """Refuse two producers that would both own one published asset."""
    claims = [
        _provenance(target="a", assets={"shared.deb": DIGEST}),
        _provenance(target="b", assets={"shared.deb": OTHER_DIGEST}),
    ]

    with pytest.raises(DuplicateAssetError) as refusal:
        declared_names(claims)

    assert "shared.deb" in str(refusal.value), (
        "the diagnosis must name the doubly claimed asset"
    )


def test_trusted_digests_union_every_producer() -> None:
    """Return one digest table spanning every producer's claims."""
    claims = [
        _provenance(target="a", assets={"b.deb": DIGEST}),
        _provenance(target="b", assets={"a.rpm": OTHER_DIGEST}),
    ]

    assert trusted_digests(claims) == {"a.rpm": OTHER_DIGEST, "b.deb": DIGEST}, (
        "the trusted record must union every producer's digests"
    )


def test_trusted_digests_refuse_a_doubly_claimed_name() -> None:
    """Refuse to pick one of two producers' answer for the same bytes."""
    claims = [
        _provenance(target="a", assets={"shared.deb": DIGEST}),
        _provenance(target="b", assets={"shared.deb": OTHER_DIGEST}),
    ]

    with pytest.raises(DuplicateAssetError):
        trusted_digests(claims)


def test_the_trusted_record_lists_exactly_the_declared_names() -> None:
    """Keep the digest table and the name list describing the same asset set."""
    claims = [
        _provenance(target="a", assets={"b.deb": DIGEST, "netsuke.tar.gz": DIGEST}),
        _provenance(target="b", assets={"a.rpm": OTHER_DIGEST}),
    ]

    assert tuple(trusted_digests(claims)) == declared_names(claims), (
        "publication must never see a name without a digest or a digest without a name"
    )

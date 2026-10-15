"""Behavioural coverage for crates.io registry reconciliation.

Before the release publishes a crate it asks the registry what is already
there, and classifies the answer into one of five states. The classification
is the safety property: only a byte-identical published version may be skipped,
and a yank must be judged before identity so a withdrawn release is never
reported as merely present.

The fixtures use both real response shapes. The version endpoint answers with
``{"version": {…}}`` and the crate endpoint with ``{"versions": [ … ]}``;
drafting for only one of them would have raised on the endpoint the release
actually calls, so both are covered.

Run via ``make test-release-publication``.
"""

import json

import pytest
from release_reconcile_crates import (
    NOT_FOUND,
    OK,
    RegistryResponseError,
    RegistryState,
    classify_registry_state,
    plan_crates_action,
)

#: A well-formed digest, matching what the registry reports for a published
#: version. Which 64 characters they are does not matter; that both sides use
#: the same string does.
INTENDED = "f" * 64
OTHER = "e" * 64


def _version_body(*, checksum: str, yanked: bool) -> str:
    """Render the version endpoint's response, as crates.io sends it."""
    return json.dumps({"version": {"checksum": checksum, "yanked": yanked}})


def _crate_body(*records: tuple[str, bool]) -> str:
    """Render the crate endpoint's response, as crates.io sends it."""
    return json.dumps({"versions": [{"checksum": c, "yanked": y} for c, y in records]})


def test_absent_version_is_absent() -> None:
    """Require a 404 to classify as absent, so the release proceeds to publish."""
    state = classify_registry_state(
        None, intended_sha256=INTENDED, http_status=NOT_FOUND
    )
    assert state is RegistryState.ABSENT
    assert plan_crates_action(state) == "publish"


def test_identical_version_is_identical() -> None:
    """Require a published version with the intended digest to be identical."""
    state = classify_registry_state(
        _version_body(checksum=INTENDED, yanked=False),
        intended_sha256=INTENDED,
        http_status=OK,
    )
    assert state is RegistryState.IDENTICAL
    assert plan_crates_action(state) == "skip"


def test_crate_endpoint_shape_is_also_understood() -> None:
    """Require the plural ``versions`` shape to classify identically.

    The release calls both endpoints. Handling one shape only would raise on
    the other, which is a failure the singular-shape tests cannot see.
    """
    state = classify_registry_state(
        _crate_body((INTENDED, False)),
        intended_sha256=INTENDED,
        http_status=OK,
    )
    assert state is RegistryState.IDENTICAL


def test_published_version_with_a_different_digest_conflicts() -> None:
    """Require a published version carrying other bytes to conflict.

    This is the case that must never skip: the version exists, but it is not
    what this release intends to publish.
    """
    state = classify_registry_state(
        _version_body(checksum=OTHER, yanked=False),
        intended_sha256=INTENDED,
        http_status=OK,
    )
    assert state is RegistryState.CONFLICT
    assert plan_crates_action(state) == "fail"


def test_yanked_version_is_yanked_not_identical() -> None:
    """Require a yank to be reported as a yank, even when the bytes match.

    Yank is judged before identity deliberately. A yanked version that happens
    to carry the intended digest is still withdrawn, and reporting it as
    "identical" would let the release treat a withdrawn crate as complete.
    """
    state = classify_registry_state(
        _version_body(checksum=INTENDED, yanked=True),
        intended_sha256=INTENDED,
        http_status=OK,
    )
    assert state is RegistryState.YANKED
    assert plan_crates_action(state) == "fail"


def test_a_later_version_record_does_not_mask_the_intended_one() -> None:
    """Require every record to be considered, not only the first.

    The crate endpoint returns all versions. A classifier reading only the
    first would report a conflict for a release whose intended version is
    present but not first in the list.
    """
    state = classify_registry_state(
        _crate_body((OTHER, False), (INTENDED, False)),
        intended_sha256=INTENDED,
        http_status=OK,
    )
    assert state is RegistryState.IDENTICAL


def test_an_unrecognised_body_is_indeterminate() -> None:
    """Require an unreadable answer to be indeterminate rather than absent.

    Indeterminate must not plan a publish. A registry that answered something
    unexpected has not told us the version is absent, and treating silence as
    absence is how a release publishes over existing bytes.
    """
    state = classify_registry_state(
        "not json at all", intended_sha256=INTENDED, http_status=OK
    )
    assert state is RegistryState.INDETERMINATE
    assert plan_crates_action(state) != "publish"


def test_a_version_with_no_checksum_is_refused() -> None:
    """Require a record missing its checksum to be refused, not classified.

    The distinction from the unparseable-body case below is deliberate. A body
    that is not JSON at all is a transient or proxied answer and is classified
    ``INDETERMINATE`` so a caller may retry. A well-formed crates.io response
    whose version record carries no checksum is a shape this code cannot read,
    and the module documents that as a raised
    :class:`RegistryResponseError` rather than a state — the registry does not
    omit the field, so silence here means the assumption about its schema is
    wrong, and a state would swallow that.
    """
    with pytest.raises(RegistryResponseError):
        classify_registry_state(
            json.dumps({"version": {"yanked": False}}),
            intended_sha256=INTENDED,
            http_status=OK,
        )


@pytest.mark.parametrize("status", [500, 502, 503, 429])
def test_a_server_error_is_indeterminate(status: int) -> None:
    """Require a retryable server status not to be read as absence."""
    state = classify_registry_state(None, intended_sha256=INTENDED, http_status=status)
    assert state is RegistryState.INDETERMINATE
    assert plan_crates_action(state) == "fail"


def test_only_identical_skips() -> None:
    """Require ``skip`` to be reachable for exactly one state.

    Stated as a property over the whole enum rather than for named states, so
    a state added later cannot quietly acquire a skipping plan.
    """
    skipping = {state for state in RegistryState if plan_crates_action(state) == "skip"}
    assert skipping == {RegistryState.IDENTICAL}
    publishing = {
        state for state in RegistryState if plan_crates_action(state) == "publish"
    }
    assert publishing == {RegistryState.ABSENT}


@pytest.mark.parametrize(
    ("intended", "state"),
    [
        # A digest the registry does not report is a conflict.
        ("d" * 64, RegistryState.CONFLICT),
        # A caller passing a non-digest gets the same answer. Recorded, not
        # endorsed: this function does not validate its argument, and the
        # argument's shape is owned by the identity record it is read from.
        ("not-a-digest", RegistryState.CONFLICT),
    ],
)
def test_a_digest_the_registry_does_not_report_conflicts(
    intended: str, state: RegistryState
) -> None:
    """Require any non-matching intended digest to produce a conflict.

    The second case pins that a malformed argument is *not* separately
    rejected. A caller reading the identity record is responsible for its
    shape; adding a check here would not remove that responsibility, only
    split one validation across two places.
    """
    assert (
        classify_registry_state(
            _version_body(checksum=INTENDED, yanked=False),
            intended_sha256=intended,
            http_status=OK,
        )
        is state
    )

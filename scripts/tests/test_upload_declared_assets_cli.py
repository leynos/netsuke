"""Test the decision the upload step makes about a release it did not build.

Each case states what the release already holds and what the payload holds,
then asserts on two things: the plan written to disk, and the ``gh``
invocations the run would have made. The second is the one that matters most --
a run that reports success and sends nothing is indistinguishable from a run
that reports success and sends the right bytes, unless the invocation itself is
asserted.

The fixtures and the ``gh`` stub live in :mod:`release_upload_support`; this
module asserts the *decision*, and its sibling
``test_upload_declared_assets_status`` asserts the exit status each refusal
reports.

Run via ``make test-release-publication``.
"""

import typing as typ

import pytest
import release_digest_record
import release_payload_handoff
import release_upload_plan
from release_upload_support import DEB, RPM, Exchange, Scenario
from upload_declared_assets_cli import settle

if typ.TYPE_CHECKING:
    import pathlib


@pytest.fixture
def scenario(tmp_path: pathlib.Path) -> Scenario:
    """Build a two-asset payload with its trusted record.

    Returns
    -------
    Scenario
        The payload, record, and the request that names them.
    """
    return Scenario(tmp_path, contents={DEB: b"deb bytes", RPM: b"rpm bytes"})


@pytest.fixture
def exchange() -> Exchange:
    """Build a release holding no assets, recording every call it receives.

    Returns
    -------
    Exchange
        The stub a case hands to :func:`settle` as its runner.
    """
    return Exchange(attached={})


def test_an_empty_release_receives_every_declared_asset(
    scenario: Scenario,
) -> None:
    """Upload the whole payload when the release holds nothing."""
    exchange = Exchange(attached={})
    actions = settle(scenario.request(), runner=exchange).actions
    assert sorted(exchange.uploads()[0][4:]) == sorted([
        str(scenario.payload / DEB),
        str(scenario.payload / RPM),
    ]), "every declared asset must be carried by the upload"
    assert {action.value for _name, action in actions} == {"skip-verified"}, (
        "each transferred asset must be verified from the re-read release"
    )


def test_an_identical_attached_asset_is_skipped(scenario: Scenario) -> None:
    """Send nothing when the release already holds the recorded bytes."""
    digests = scenario.digests()
    exchange = Exchange(
        attached={DEB: f"sha256:{digests[DEB]}", RPM: f"sha256:{digests[RPM]}"}
    )
    actions = settle(scenario.request(), runner=exchange).actions
    assert not exchange.uploads(), "nothing may be sent to a release that agrees"
    assert {action.value for _name, action in actions} == {"skip-verified"}, (
        "every asset must be reported as verified, not as transferred"
    )


def test_only_the_missing_asset_is_uploaded(scenario: Scenario) -> None:
    """Upload the gap and leave the verified asset alone."""
    exchange = Exchange(attached={DEB: f"sha256:{scenario.digests()[DEB]}"})
    actions = settle(scenario.request(), runner=exchange).actions
    assert len(exchange.uploads()) == 1, "exactly one transfer must be made"
    assert exchange.uploads()[0][4:] == [str(scenario.payload / RPM)], (
        "only the missing asset may be sent"
    )
    plan = dict(actions)
    assert plan[DEB].value == "skip-verified", (
        "the already-attached asset must still be recorded as verified"
    )
    assert plan[RPM].value == "skip-verified", (
        "the sent asset must be recorded as verified after the re-read"
    )


def test_a_conflicting_asset_blocks_every_upload(scenario: Scenario) -> None:
    """Refuse to send anything when one attached asset disagrees.

    Uploading the others first would leave the release holding a mixture of
    this run's bytes and an earlier run's.
    """
    exchange = Exchange(attached={DEB: "sha256:" + "ff" * 32})
    actions = settle(scenario.request(), runner=exchange).actions
    assert not exchange.uploads(), "a conflict must block the whole transfer"
    assert dict(actions)[DEB].value == "conflict", (
        "the disagreeing asset must be recorded as a conflict"
    )
    assert dict(actions)[RPM].value == "upload", "the gap is still visible in the plan"


def test_a_conflicting_asset_is_never_clobbered(scenario: Scenario) -> None:
    """Refuse to replace attached bytes that disagree with the record."""
    exchange = Exchange(attached={DEB: "sha256:" + "ff" * 32})
    _ = settle(scenario.request(), runner=exchange)
    assert not exchange.clobbered(), "a conflicting asset must never be replaced"


def test_an_unreported_digest_is_established_by_downloading(
    scenario: Scenario,
) -> None:
    """Hash the attached bytes when the listing carries no usable digest."""
    exchange = Exchange(
        attached={DEB: None, RPM: f"sha256:{scenario.digests()[RPM]}"},
        downloads={DEB: scenario.contents[DEB]},
    )
    actions = settle(scenario.request(), runner=exchange).actions
    assert dict(actions)[DEB].value == "skip-verified", (
        "the downloaded bytes must be accepted as the recorded ones"
    )
    assert not exchange.uploads(), "a verified asset must not be sent again"


def test_a_download_that_fails_makes_the_asset_unverifiable(
    scenario: Scenario,
) -> None:
    """Refuse an attached asset whose bytes cannot be established at all.

    Refused, but *not* called a conflict: nothing read the bytes, so the run
    has no evidence they disagree. Calling it a disagreement would send an
    operator to inspect a release whose fault may be in the listing instead.
    """
    exchange = Exchange(attached={DEB: None})
    actions = settle(scenario.request(), runner=exchange).actions
    assert dict(actions)[DEB].value == "unverifiable", (
        "an unestablishable digest must be refused, not assumed"
    )
    assert not exchange.uploads(), "an unverifiable asset must block the transfer"


def test_downloaded_bytes_that_differ_are_a_conflict(scenario: Scenario) -> None:
    """Refuse attached bytes that are not the recorded ones."""
    exchange = Exchange(attached={DEB: None}, downloads={DEB: b"another commit"})
    actions = settle(scenario.request(), runner=exchange).actions
    assert dict(actions)[DEB].value == "conflict", (
        "bytes from another commit must be refused as a conflict"
    )


def test_a_dry_run_sends_nothing(scenario: Scenario) -> None:
    """Plan the transfer without making it."""
    exchange = Exchange(attached={})
    actions = settle(scenario.request(is_dry_run=True), runner=exchange).actions
    assert not exchange.uploads(), "a dry run must never transfer a byte"
    assert {action.value for _name, action in actions} == {"upload"}, (
        "a dry run must still report what it would have sent"
    )


def test_a_dry_run_still_reads_the_release(scenario: Scenario) -> None:
    """List the release even when the run will not write to it."""
    exchange = Exchange(attached={})
    _ = settle(scenario.request(is_dry_run=True), runner=exchange)
    assert exchange.calls, "a dry run must still plan against the real release"


def test_the_release_is_relisted_after_the_upload(scenario: Scenario) -> None:
    """Verify the transfer from GitHub rather than from the exit status."""
    exchange = Exchange(attached={})
    _ = settle(scenario.request(), runner=exchange)
    listings = [call for call in exchange.calls if call[2] == "view"]
    assert len(listings) == 2, "the plan must describe what GitHub now holds"


def test_the_returned_plan_describes_the_post_upload_release(
    scenario: Scenario,
) -> None:
    """Report every asset verified, not merely sent."""
    exchange = Exchange(attached={})
    actions = settle(scenario.request(), runner=exchange).actions
    assert all(action.value == "skip-verified" for _name, action in actions), (
        "the plan must describe the post-upload release, not the pre-upload one"
    )


def test_a_payload_that_lost_an_asset_is_refused(
    scenario: Scenario, exchange: Exchange
) -> None:
    """Refuse a hand-off that does not hold the declared set."""
    _ = (scenario.payload / RPM).unlink()
    with pytest.raises(release_payload_handoff.PayloadHandoffError, match="omits"):
        settle(scenario.request(), runner=exchange)
    assert not exchange.calls, "a refused payload must not reach the network"


def test_a_payload_that_gained_an_asset_is_refused(
    scenario: Scenario, exchange: Exchange
) -> None:
    """Refuse a hand-off carrying a file nobody selected."""
    _ = (scenario.payload / "extra.rpm").write_bytes(b"extra")
    with pytest.raises(release_payload_handoff.PayloadHandoffError, match="adds"):
        settle(scenario.request(), runner=exchange)
    assert not exchange.calls, "a refused payload must not reach the network"


def test_a_payload_publishing_a_name_twice_is_refused(
    scenario: Scenario, exchange: Exchange
) -> None:
    """Refuse a nested copy of a declared name before anything is sent.

    The nested file is a regular file, so a hand-off check that indexed the
    directory by published name would find a file to hash and accept the tree.
    """
    _ = (scenario.payload / "sub").mkdir()
    _ = (scenario.payload / "sub" / DEB).write_bytes(b"deb bytes")
    with pytest.raises(release_payload_handoff.PayloadHandoffError, match="adds"):
        settle(scenario.request(), runner=exchange)
    assert not exchange.calls, "a refused payload must not reach the network"


def test_a_root_symlink_beside_a_nested_file_is_refused(
    scenario: Scenario, exchange: Exchange
) -> None:
    """Refuse the entry ``gh`` would be handed, not the copy that vouches.

    ``sub/<deb>`` is a regular file holding the recorded bytes while
    ``<deb>`` at the root is a link pointing outside the payload entirely, so
    a check that hashed whichever match the walk found first would approve a
    tree whose transfer reads a different file.
    """
    elsewhere = scenario.root / "elsewhere"
    elsewhere.mkdir()
    (elsewhere / DEB).write_bytes(b"not the payload")
    (scenario.payload / "sub").mkdir()
    (scenario.payload / "sub" / DEB).write_bytes(b"deb bytes")
    (scenario.payload / DEB).unlink()
    (scenario.payload / DEB).symlink_to(elsewhere / DEB)
    with pytest.raises(
        release_payload_handoff.PayloadHandoffError, match="cannot upload"
    ):
        settle(scenario.request(), runner=exchange)
    assert not exchange.calls, "a refused payload must not reach the network"


def test_a_payload_whose_bytes_changed_is_refused(
    scenario: Scenario, exchange: Exchange
) -> None:
    """Refuse a payload the trusted record no longer describes."""
    _ = (scenario.payload / DEB).write_bytes(b"rebuilt")
    with pytest.raises(release_digest_record.RecordedDigestMismatchError):
        settle(scenario.request(), runner=exchange)
    assert not exchange.calls, "a refused payload must not reach the network"


def test_a_refused_payload_never_reaches_the_network(
    scenario: Scenario, exchange: Exchange
) -> None:
    """Verify the payload before the first call, not after the upload."""
    _ = (scenario.payload / DEB).write_bytes(b"rebuilt")
    with pytest.raises(release_digest_record.RecordedDigestMismatchError):
        settle(scenario.request(), runner=exchange)
    assert not exchange.calls, "a refused payload must not reach the network"


def test_a_transport_failure_is_reported(scenario: Scenario) -> None:
    """Surface a failed transfer rather than reporting the release complete."""
    exchange = Exchange(attached={}, fail_upload=True)
    with pytest.raises(Exception, match="502"):
        settle(scenario.request(), runner=exchange)


def test_no_upload_ever_carries_clobber(scenario: Scenario) -> None:
    """Pin the property the whole module exists for."""
    exchange = Exchange(attached={})
    _ = settle(scenario.request(), runner=exchange)
    assert exchange.uploads(), "the fixture must exercise a real transfer"
    assert not exchange.clobbered(), "no upload may ever carry --clobber"


def test_an_undeclared_attached_asset_is_ignored(scenario: Scenario) -> None:
    """Leave an attached file the record never declared alone.

    It is not this step's to reconcile, and refusing over it would block a
    release over a file the plan never claimed.
    """
    digests = scenario.digests()
    exchange = Exchange(
        attached={
            "unrelated.txt": "sha256:" + "ee" * 32,
            DEB: f"sha256:{digests[DEB]}",
            RPM: f"sha256:{digests[RPM]}",
        }
    )
    actions = settle(scenario.request(), runner=exchange).actions
    assert not exchange.uploads(), "an undeclared asset must not provoke a transfer"
    assert "unrelated.txt" not in dict(actions), (
        "an undeclared asset must not appear in the plan"
    )


def test_the_plan_round_trips_through_the_reconciliation_vocabulary(
    scenario: Scenario,
) -> None:
    """Express every action the reconciliation can produce in the plan.

    The plan's values are the reconciliation's own, so an action the plan
    could not serialize would be a release decision with no audit trail.
    """
    exchange = Exchange(attached={DEB: "sha256:" + "ff" * 32})
    actions = settle(scenario.request(), runner=exchange).actions
    release_upload_plan.write_plan(
        scenario.plan, actions, release_upload_plan.Blockers.of(actions)
    )
    recorded = scenario.stored_plan()
    # The plan sorts by name, and "-" precedes "_", so the RPM leads.
    assert recorded["actions"] == [[RPM, "upload"], [DEB, "conflict"]], (
        "the plan must express every action the reconciliation produced"
    )

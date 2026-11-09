"""Execute the inventory verifier against a release GitHub actually holds.

Each case here runs the action's real shell bodies, through the real release
scripts, over a publisher tree built from bytes this module chooses. The
harness that stubs ``uv`` and ``gh``, stages the workspace, and reads the
manifest lives in ``verify_release_inventory_harness.py``; the build, release,
and scenario defaults the cases vary are in ``verify_release_inventory_cases.py``.

Read together with the refusal cases, these hold three properties to account: a
release that matches its build verifies and records every digest; a release that
disagrees with the build is refused whatever its own sidecars say; and a listing
that cannot be turned into a download plan stops before anything is fetched.

Run via ``make test-workflow-contracts``.
"""

import dataclasses
import hashlib
import typing as typ

import pytest
from composite_action_execution import interpreter_baseline
from verify_release_inventory_cases import (
    ARCHIVE,
    FIRST_ASSET_ID,
    PACKAGE,
    SIDECAR,
    default_publisher,
    default_scenario,
    draft_release,
    final_diagnostic,
    release_assets,
    sidecar,
)
from verify_release_inventory_harness import execute
from verify_release_inventory_manifest import DOWNLOAD_STEP, RUN_STEPS
from verify_release_inventory_scenario import RemoteAsset, Scenario

if typ.TYPE_CHECKING:
    import pathlib


def test_the_steps_verify_a_release_the_build_produced(
    tmp_path: pathlib.Path,
) -> None:
    """Download every attachment, verify it, and record each digest."""
    publisher = default_publisher()
    run = execute(
        tmp_path,
        Scenario(
            publisher=publisher, remote=draft_release(release_assets(publisher.files))
        ),
    )
    assert len(run.completed) == len(RUN_STEPS), final_diagnostic(run)
    assert run.downloaded == tuple(sorted(publisher.files)), (
        "the download directory must hold exactly the release's attachments"
    )
    assert run.plan == tuple(
        (str(FIRST_ASSET_ID + offset), name)
        for offset, name in enumerate(publisher.files)
    ), "the plan must list each attachment by number and name, in listing order"
    expected = {
        name: hashlib.sha256(body).hexdigest() for name, body in publisher.files.items()
    }
    assert run.verified_record == expected, (
        "the verified record must hold each asset's own digest"
    )
    assert (
        run.outputs.get("verified-record-path") == "verified/verified-digests.json"
    ), "the verification step must publish the path it wrote"
    assert run.output_path.is_file(), (
        "the verification step must create the output directory it was given"
    )
    assert set(run.baselines) == {interpreter_baseline()}, run.baselines
    downloads = [
        line for line in run.gh_log.splitlines() if "/releases/assets/" in line
    ]
    assert len(downloads) == len(publisher.files), run.gh_log
    assert all("Accept: application/octet-stream" in line for line in downloads), (
        "every attachment download must ask for the bytes rather than the metadata"
    )


def test_a_substituted_archive_that_agrees_with_its_sidecar_is_refused(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a download that agrees with itself but not with the build.

    The archive and its sidecar are replaced together, so the inventory and the
    sidecar agreement both hold. Only the record produced from the build's own
    tree before upload can see the substitution, and that is why it is the input
    the action passes to the verifier.
    """
    publisher = default_publisher()
    forged = dict(publisher.files)
    forged[ARCHIVE] = b"a different archive entirely"
    forged[SIDECAR] = sidecar(forged[ARCHIVE])
    run = execute(
        tmp_path,
        Scenario(publisher=publisher, remote=draft_release(release_assets(forged))),
    )
    assert run.completed[-1].returncode != 0, final_diagnostic(run)
    assert ARCHIVE in final_diagnostic(run), (
        "the diagnostic must name the substituted asset"
    )
    assert not run.output_path.exists(), "a rejected release must leave no record"


def _refuse_release_holding(
    tmp_path: pathlib.Path, served: dict[str, bytes], named: str
) -> None:
    """Run a release that carries ``served`` and assert it is refused.

    Both inventory disagreements end the same way: the step fails, the
    diagnostic names the asset that caused it, and no verified record is
    written. Only which asset is named differs, so that is the argument.

    Parameters
    ----------
    tmp_path
        The directory the run keeps its workspace in.
    served
        The bytes the release carries, keyed by attachment name.
    named
        The asset the diagnostic must name.
    """
    publisher = default_publisher()
    run = execute(
        tmp_path,
        Scenario(publisher=publisher, remote=draft_release(release_assets(served))),
    )
    assert run.completed[-1].returncode != 0, final_diagnostic(run)
    assert named in final_diagnostic(run), (
        f"the diagnostic must name {named}, the asset at fault"
    )
    assert not run.output_path.exists(), "a refused release must leave no record"


def test_a_declared_asset_the_release_does_not_carry_is_refused(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a release whose inventory is short of its declaration."""
    served = {
        name: body
        for name, body in default_publisher().files.items()
        if name != PACKAGE
    }
    _refuse_release_holding(tmp_path, served, PACKAGE)


def test_an_asset_the_declaration_does_not_name_is_refused(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a release that carries more than the build declared."""
    undeclared = "netsuke-0.1.0-extra.tar.gz"
    served = dict(default_publisher().files)
    served[undeclared] = b"an uninvited asset"
    _refuse_release_holding(tmp_path, served, undeclared)


def test_a_record_that_does_not_cover_the_declaration_is_refused(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse a trusted record that misses a declared asset.

    A record that covers only part of the release would leave the rest
    uncompared, so the verifier refuses it rather than verifying a subset.
    """
    publisher = dataclasses.replace(default_publisher(), recorded=(ARCHIVE, SIDECAR))
    run = execute(
        tmp_path,
        Scenario(
            publisher=publisher, remote=draft_release(release_assets(publisher.files))
        ),
    )
    assert run.completed[-1].returncode != 0, final_diagnostic(run)
    assert PACKAGE in final_diagnostic(run), (
        "the diagnostic must name the unrecorded asset"
    )
    assert not run.output_path.exists(), (
        "a refused record must leave no verified record"
    )


@pytest.mark.parametrize(
    "asset",
    [
        pytest.param(
            RemoteAsset(FIRST_ASSET_ID + 3, "extra.tar.gz", b"x", size=0),
            id="empty-attachment",
        ),
        pytest.param(
            RemoteAsset(FIRST_ASSET_ID + 3, "../escape.tar.gz", b"x"),
            id="escaping-name",
        ),
        pytest.param(
            RemoteAsset(FIRST_ASSET_ID + 3, ARCHIVE, b"a second copy"),
            id="duplicate-name",
        ),
        pytest.param(
            RemoteAsset(FIRST_ASSET_ID + 3, "extra.tar.gz", b"x", state="starter"),
            id="unfinished-upload",
        ),
    ],
)
def test_a_listing_that_cannot_be_downloaded_stops_before_the_fetch(
    tmp_path: pathlib.Path, asset: RemoteAsset
) -> None:
    """Refuse the plan without fetching anything.

    Each of these listings would otherwise put bytes into the download
    directory that the verifier could not account for, so the plan step is the
    last place the run can stop before that happens. Reaching the download step
    would already have written the attacker-chosen name.
    """
    publisher = default_publisher()
    assets = (*release_assets(publisher.files), asset)
    run = execute(tmp_path, Scenario(publisher=publisher, remote=draft_release(assets)))
    assert len(run.completed) < len(RUN_STEPS), "the run must stop before verifying"
    assert run.completed[-1].returncode != 0, final_diagnostic(run)
    assert not run.download_root.exists() or not any(run.download_root.iterdir()), (
        "a refused plan must not leave a downloaded asset behind"
    )
    assert not run.output_path.exists(), "a refused plan must not verify anything"


def test_a_failed_download_stops_the_job(tmp_path: pathlib.Path) -> None:
    """Stop at the download step rather than verifying a partial release.

    A partial download cannot be told from a truncated attachment that somehow
    matched, so the step fails instead of retrying and the verifier never runs.
    """
    publisher = default_publisher()
    assets = tuple(
        dataclasses.replace(asset, download_fails=asset.name == PACKAGE)
        for asset in release_assets(publisher.files)
    )
    run = execute(tmp_path, Scenario(publisher=publisher, remote=draft_release(assets)))
    assert len(run.completed) == RUN_STEPS.index(DOWNLOAD_STEP) + 1, (
        "the run must stop at the download step"
    )
    assert run.completed[-1].returncode != 0, final_diagnostic(run)
    assert not run.output_path.exists(), "nothing was verified, so nothing is recorded"


def test_the_default_scenario_verifies(tmp_path: pathlib.Path) -> None:
    """Verify the shared default so every case that varies it starts from green."""
    run = execute(tmp_path, default_scenario())
    assert len(run.completed) == len(RUN_STEPS), final_diagnostic(run)
    assert run.verified_record, "the default release must record its assets"

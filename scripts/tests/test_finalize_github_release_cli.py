"""Exercise the release-finalizing CLI the finalizer job invokes.

The reconciliation planner is proved in its own module. These tests pin the
seam: that the declared set comes from the verifier's record rather than from
whatever GitHub happens to hold, that a conflict stops the release with the
offending names in the diagnosis, and that an incomplete release is reported as
needing an upload rather than as a conflict. No test here contacts GitHub.
"""

import json
import typing as typ

import pytest
import release_reconcile_github
from finalize_github_release_cli import (
    ACTION_EXITS,
    EXIT_CONFLICT,
    EXIT_OK,
    EXIT_UPLOAD_REQUIRED,
    build_parser,
    main,
)

if typ.TYPE_CHECKING:
    import pathlib

ARCHIVE = "netsuke-build-0.1.0-x86_64-unknown-linux-gnu.tar.gz"
SIDECAR = f"{ARCHIVE}.sha256"
ARCHIVE_DIGEST = "ab" * 32
SIDECAR_DIGEST = "cd" * 32


def _identity(destination: pathlib.Path, assets: dict[str, str]) -> pathlib.Path:
    """Write the verified-digest record the inventory job produced."""
    _ = destination.write_text(json.dumps({"assets": assets}), encoding="utf-8")
    return destination


def _reported(destination: pathlib.Path, reported: dict[str, object]) -> pathlib.Path:
    """Write the digest mapping the workflow step recorded from GitHub."""
    _ = destination.write_text(json.dumps(reported), encoding="utf-8")
    return destination


def _argv(
    identity: pathlib.Path, assets: pathlib.Path, output: pathlib.Path
) -> list[str]:
    """Return a finalize invocation naming all three files."""
    return [
        "--identity",
        str(identity),
        "--assets",
        str(assets),
        "--output",
        str(output),
    ]


class _PlanRecord(typ.TypedDict):
    """Describe the reconciliation plan the CLI records as JSON.

    The plan is a two-field document, and naming the field types is what lets
    the assertions below read ``actions`` and ``conflicts`` as the collections
    they are. Declaring the whole record as ``dict[str, object]`` would make
    each lookup opaque and force a cast at every use.
    """

    actions: list[list[str]]
    conflicts: list[str]


def _plan(output: pathlib.Path) -> _PlanRecord:
    """Read back the reconciliation plan the CLI wrote."""
    return json.loads(output.read_text(encoding="utf-8"))


def test_the_cli_permits_un_drafting_a_fully_verified_release(
    tmp_path: pathlib.Path,
) -> None:
    """Accept a draft whose assets all match the verified record."""
    identity = _identity(
        tmp_path / "identity.json",
        {ARCHIVE: ARCHIVE_DIGEST, SIDECAR: SIDECAR_DIGEST},
    )
    assets = _reported(
        tmp_path / "assets.json",
        {
            ARCHIVE: f"sha256:{ARCHIVE_DIGEST}",
            SIDECAR: f"sha256:{SIDECAR_DIGEST}",
        },
    )
    output = tmp_path / "plan.json"

    code = main(_argv(identity, assets, output))

    assert code == EXIT_OK, f"a verified release must proceed, got {code}"
    plan = _plan(output)
    assert plan["conflicts"] == [], "a verified release has no conflicts"
    assert [action for _, action in plan["actions"]] == [
        "skip-verified",
        "skip-verified",
    ], f"every asset must be verified, got {plan['actions']}"


def test_the_cli_requires_an_upload_for_a_missing_asset(
    tmp_path: pathlib.Path,
) -> None:
    """Report absence as an upload, not as a conflict."""
    identity = _identity(
        tmp_path / "identity.json",
        {ARCHIVE: ARCHIVE_DIGEST, SIDECAR: SIDECAR_DIGEST},
    )
    assets = _reported(tmp_path / "assets.json", {ARCHIVE: f"sha256:{ARCHIVE_DIGEST}"})
    output = tmp_path / "plan.json"

    code = main(_argv(identity, assets, output))

    assert code == EXIT_UPLOAD_REQUIRED, (
        f"an incomplete release must report the missing asset, got {code}"
    )
    assert _plan(output)["conflicts"] == [], (
        "a missing asset is not a conflict; nothing disagrees"
    )


def test_the_cli_refuses_an_asset_that_disagrees(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Stop on a digest mismatch and name every conflicting asset."""
    identity = _identity(
        tmp_path / "identity.json",
        {ARCHIVE: ARCHIVE_DIGEST, SIDECAR: SIDECAR_DIGEST},
    )
    assets = _reported(
        tmp_path / "assets.json",
        {
            ARCHIVE: f"sha256:{'ef' * 32}",
            SIDECAR: f"sha256:{'01' * 32}",
        },
    )
    output = tmp_path / "plan.json"

    code = main(_argv(identity, assets, output))

    assert code == EXIT_CONFLICT, f"a disagreement must stop the release, got {code}"
    assert sorted(_plan(output)["conflicts"]) == sorted([ARCHIVE, SIDECAR]), (
        "the plan must name every conflicting asset in one run"
    )
    err = capsys.readouterr().err
    assert ARCHIVE in err, "the diagnosis must name the archive conflict"
    assert SIDECAR in err, "the diagnosis must name the sidecar conflict too"


def test_the_cli_refuses_an_unreadable_asset_digest(
    tmp_path: pathlib.Path,
) -> None:
    """Treat a reported digest that cannot be read as a conflict, not a match."""
    identity = _identity(tmp_path / "identity.json", {ARCHIVE: ARCHIVE_DIGEST})
    assets = _reported(tmp_path / "assets.json", {ARCHIVE: "not-a-digest"})
    output = tmp_path / "plan.json"

    code = main(_argv(identity, assets, output))

    assert code == EXIT_CONFLICT, (
        f"an unreadable digest proves nothing and must not permit un-drafting, "
        f"got {code}"
    )


def test_the_cli_ignores_assets_the_record_does_not_declare(
    tmp_path: pathlib.Path,
) -> None:
    """Leave undeclared extras to the inventory check, which owns that finding."""
    identity = _identity(tmp_path / "identity.json", {ARCHIVE: ARCHIVE_DIGEST})
    assets = _reported(
        tmp_path / "assets.json",
        {ARCHIVE: f"sha256:{ARCHIVE_DIGEST}", "extra.tar.gz": f"sha256:{'ff' * 32}"},
    )
    output = tmp_path / "plan.json"

    code = main(_argv(identity, assets, output))

    assert code == EXIT_OK, (
        f"an undeclared extra is not this stage's finding, got {code}"
    )
    assert [name for name, _ in _plan(output)["actions"]] == [ARCHIVE], (
        "the plan must decide only the declared assets"
    )


def test_the_exit_table_covers_every_action_the_planner_declares() -> None:
    """Require the table to name every action, so none falls to a default."""
    declared = {action.value for action in release_reconcile_github.AssetAction}
    assert set(ACTION_EXITS) == declared, (
        f"the exit table must cover exactly the planner's actions; "
        f"missing {sorted(declared - set(ACTION_EXITS))!r}, "
        f"extra {sorted(set(ACTION_EXITS) - declared)!r}"
    )


def test_the_exit_table_treats_only_a_verified_asset_as_success() -> None:
    """Require both other actions to stop the release before it is un-drafted."""
    successes = sorted(action for action, code in ACTION_EXITS.items() if code == 0)
    assert successes == ["skip-verified"], (
        f"only a verified asset may proceed to un-drafting, found {successes!r}"
    )


def test_the_conflict_code_is_distinct_from_the_upload_code() -> None:
    """Keep the two refusals apart, since they send an operator elsewhere."""
    assert EXIT_CONFLICT != EXIT_UPLOAD_REQUIRED, (
        "a conflict and a missing asset are different failures and must not "
        "share an exit code"
    )


@pytest.mark.parametrize("missing", ["--identity", "--assets", "--output"])
def test_the_cli_requires_all_three_files(missing: str) -> None:
    """Refuse to guess any of them, since each names a different input."""
    complete = {
        "--identity": "identity.json",
        "--assets": "assets.json",
        "--output": "plan.json",
    }
    argv = [
        part
        for flag, value in complete.items()
        if flag != missing
        for part in (flag, value)
    ]

    with pytest.raises(SystemExit):
        _ = build_parser().parse_args(argv)

"""Exercise the release-finalizing CLI the finalizer job invokes.

The reconciliation planner is proved in its own module. These tests pin the
seam: that the declared set comes from the build's trusted record rather than
from whatever GitHub happens to hold, that a conflict stops the release with
the offending names in the diagnosis, and that an incomplete release is reported
as needing an upload rather than as a conflict. No test here contacts GitHub.

Which record is read is the substance of that first claim. Reading the
verifier's output would compare GitHub against a digest derived from GitHub,
and the comparison would agree with itself however the upload was tampered
with; reading the build's record is what makes an asset GitHub reports
different from the one the build produced stop the release before it is
un-drafted.
"""

import json
import typing as typ

import pytest
import release_reconcile_github
from finalize_github_release_cli import (
    ACTION_EXITS,
    EXIT_CONFLICT,
    EXIT_INPUT_UNREADABLE,
    EXIT_OK,
    EXIT_OUTPUT_UNWRITABLE,
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
    """Write the trusted digest record the declaration stage produced."""
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
    """Accept a draft whose assets all match the trusted record."""
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
    assert "trusted digest record" in err, (
        "the diagnosis must distinguish the build's trusted input from an inventory"
    )


@pytest.mark.parametrize(
    ("listing", "reason"),
    [
        pytest.param(None, "the listing file is absent", id="unreadable-file"),
        pytest.param("not json", "the listing is malformed", id="malformed-json"),
        pytest.param("[]", "the listing is not an object", id="not-an-object"),
    ],
)
def test_the_cli_refuses_an_unreadable_asset_listing(
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
    listing: str | None,
    reason: str,
) -> None:
    """Refuse absent or malformed observations rather than allowing release."""
    identity = _identity(tmp_path / "identity.json", {ARCHIVE: ARCHIVE_DIGEST})
    assets = tmp_path / "assets.json"
    if listing is not None:
        _ = assets.write_text(listing, encoding="utf-8")
    output = tmp_path / "plan.json"

    code = main(_argv(identity, assets, output))

    assert code == EXIT_INPUT_UNREADABLE, f"{reason} must be refused, got {code}"
    assert not output.exists(), "an unreadable listing must leave no plan behind"
    assert capsys.readouterr().err, "the refusal must be diagnosed"


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


def test_the_cli_reports_a_conflict_ahead_of_a_missing_asset(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Report the more urgent finding when a release holds both.

    ``EXIT_UPLOAD_REQUIRED`` is numerically larger than ``EXIT_CONFLICT``, so a
    verdict computed by taking the maximum over the exit table picks the upload
    code. The step branches on this value: it would try to upload into a
    release whose real problem is an asset that must never be overwritten, and
    the conflict diagnostic would never print. Both findings must appear in one
    run, with the conflict deciding the code.
    """
    identity = _identity(
        tmp_path / "identity.json",
        {ARCHIVE: ARCHIVE_DIGEST, SIDECAR: SIDECAR_DIGEST},
    )
    # The archive conflicts, the sidecar is simply absent.
    assets = _reported(tmp_path / "assets.json", {ARCHIVE: f"sha256:{'ef' * 32}"})
    output = tmp_path / "plan.json"

    code = main(_argv(identity, assets, output))

    assert code == EXIT_CONFLICT, f"a conflict must outrank a missing asset, got {code}"
    assert _plan(output)["conflicts"] == [ARCHIVE], (
        "the plan must still name the conflicting asset"
    )
    assert ARCHIVE in capsys.readouterr().err, (
        "the conflict diagnostic must print even though an asset is also missing"
    )
    assert EXIT_CONFLICT < EXIT_UPLOAD_REQUIRED, (
        "this test is only meaningful while the numeric order inverts the "
        "intended precedence"
    )


@pytest.mark.parametrize(
    ("reported_digest", "has_conflict"),
    [
        pytest.param(f"sha256:{ARCHIVE_DIGEST}", False, id="verified-asset"),
        pytest.param(f"sha256:{'ef' * 32}", True, id="conflicting-asset"),
    ],
)
def test_the_cli_reports_an_unwritable_plan_and_keeps_conflict_diagnostics(
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
    *,
    reported_digest: str,
    has_conflict: bool,
) -> None:
    """Give plan-write failures a distinct status without hiding conflicts."""
    identity = _identity(tmp_path / "identity.json", {ARCHIVE: ARCHIVE_DIGEST})
    assets = _reported(tmp_path / "assets.json", {ARCHIVE: reported_digest})
    output = tmp_path / "plan-directory"
    output.mkdir()

    code = main(_argv(identity, assets, output))

    assert code == EXIT_OUTPUT_UNWRITABLE, (
        f"a plan-write failure must be distinct, got {code}"
    )
    err = capsys.readouterr().err
    assert "reconciliation plan write failed" in err, (
        "the output failure must have an actionable diagnostic"
    )
    if has_conflict:
        assert ARCHIVE in err, "the failed write must not hide the conflict name"


@pytest.mark.parametrize(
    ("record", "reason"),
    [
        pytest.param(None, "the record is absent", id="absent-file"),
        pytest.param("not json", "the record is not JSON", id="malformed-json"),
        pytest.param("[]", "the record is a list", id="not-an-object"),
        pytest.param("{}", "the record carries no assets member", id="no-assets"),
        pytest.param('{"assets": []}', "assets is a list", id="assets-not-a-mapping"),
        pytest.param('{"assets": {}}', "assets declares nothing", id="assets-empty"),
        pytest.param(
            '{"assets": {"a.tar.gz": 1}}',
            "a digest is not a string",
            id="digest-not-a-string",
        ),
        pytest.param(
            '{"assets": {"a.tar.gz": "ab"}}',
            "a digest is not a full SHA-256",
            id="digest-too-short",
        ),
    ],
)
def test_the_cli_refuses_a_record_that_cannot_support_a_decision(
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
    record: str | None,
    reason: str,
) -> None:
    """Refuse every unusable record, because the alternative is un-drafting.

    A record read with ``.get("assets", {})`` turns a missing field into an
    empty plan, and an empty plan is indistinguishable from a fully verified
    release: every asset matches because there are no assets. The step reads
    ``EXIT_OK`` as permission to un-draft, and un-drafting cannot be undone. So
    absence of evidence must be an error, not a vacuous success.
    """
    identity = tmp_path / "identity.json"
    if record is not None:
        _ = identity.write_text(record, encoding="utf-8")
    assets = _reported(tmp_path / "assets.json", {ARCHIVE: f"sha256:{ARCHIVE_DIGEST}"})
    output = tmp_path / "plan.json"

    code = main(_argv(identity, assets, output))

    assert code == EXIT_INPUT_UNREADABLE, (
        f"{reason} must be refused rather than read as an empty release, got {code}"
    )
    assert not output.exists(), (
        "a record that supports no decision must leave no plan behind, or the "
        "step could read a stale one"
    )
    assert capsys.readouterr().err, "the refusal must be diagnosed"


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

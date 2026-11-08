"""Exercise the draft-asset command's input, policy, and output contract.

The policy module's own rules are specified beside it; these cases are about
the command's four statuses and about what an operator finds on disk after a
run. A failed invocation must leave an earlier record untouched, and a
successful one must replace it completely, because the workflow step reads the
file only when the status permits it.
"""

import json
import typing as typ

import pytest
from release_draft_assets_cli import (
    EXIT_INPUT_UNREADABLE,
    EXIT_OK,
    EXIT_OUTPUT_UNWRITABLE,
    EXIT_POLICY_REJECTED,
    main,
)

if typ.TYPE_CHECKING:
    import pathlib

TAG = "v0.1.0"


def _write_listing(path: pathlib.Path, pages: object) -> pathlib.Path:
    """Write one saved ``gh api --slurp`` listing and return its path."""
    _ = path.write_text(json.dumps(pages), encoding="utf-8")
    return path


def _release(identifier: int, *, draft: bool = True) -> dict[str, object]:
    """Return one release entry for the requested tag."""
    return {"id": identifier, "tag_name": TAG, "draft": draft}


def _asset(identifier: int, name: str) -> dict[str, object]:
    """Return one uploaded, non-empty asset entry."""
    return {"id": identifier, "name": name, "state": "uploaded", "size": 5}


def test_select_release_records_the_draft_identifier(tmp_path: pathlib.Path) -> None:
    """Write the one value the asset listing is addressed by."""
    listing = _write_listing(tmp_path / "releases.json", [[_release(7)]])
    output = tmp_path / "release-id.txt"

    code = main([
        "select-release",
        "--listing",
        str(listing),
        "--tag",
        TAG,
        "--output",
        str(output),
    ])

    assert code == EXIT_OK, f"a single matching draft must be selected, got {code}"
    assert output.read_text(encoding="utf-8") == "7\n", (
        "the selected identifier must be the only content recorded"
    )


def test_plan_downloads_records_tab_separated_pairs(tmp_path: pathlib.Path) -> None:
    """Write the plan in the shape the download loop reads."""
    listing = _write_listing(
        tmp_path / "assets.json", [[_asset(11, "a.tar.gz"), _asset(12, "b.deb")]]
    )
    output = tmp_path / "plan.txt"

    code = main([
        "plan-downloads",
        "--listing",
        str(listing),
        "--output",
        str(output),
    ])

    assert code == EXIT_OK, f"a usable listing must plan, got {code}"
    assert output.read_text(encoding="utf-8") == "11\ta.tar.gz\n12\tb.deb\n", (
        "the plan must record each asset as a tab-separated pair"
    )


def test_a_policy_rejection_preserves_an_earlier_record(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Refuse a published release and leave the previous identifier in place."""
    listing = _write_listing(tmp_path / "releases.json", [[_release(7, draft=False)]])
    output = tmp_path / "release-id.txt"
    _ = output.write_text("previous\n", encoding="utf-8")

    code = main([
        "select-release",
        "--listing",
        str(listing),
        "--tag",
        TAG,
        "--output",
        str(output),
    ])

    assert code == EXIT_POLICY_REJECTED, f"a published release must be refused, {code}"
    assert output.read_text(encoding="utf-8") == "previous\n", (
        "a refused run must not replace the earlier record"
    )
    assert "published, not a draft" in capsys.readouterr().err, (
        "the diagnostic must say the matched release is already published"
    )


@pytest.mark.parametrize(
    ("mode", "extra"),
    [
        pytest.param("select-release", ["--tag", TAG], id="select-release"),
        pytest.param("plan-downloads", [], id="plan-downloads"),
    ],
)
def test_malformed_json_is_an_input_fault(
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
    mode: str,
    extra: list[str],
) -> None:
    """Distinguish a file that is not JSON from a listing that refuses."""
    listing = tmp_path / "listing.json"
    _ = listing.write_text("not json", encoding="utf-8")
    output = tmp_path / "result.txt"
    _ = output.write_text("previous\n", encoding="utf-8")

    code = main([
        mode,
        "--listing",
        str(listing),
        *extra,
        "--output",
        str(output),
    ])

    assert code == EXIT_INPUT_UNREADABLE, f"malformed JSON is an input fault, {code}"
    assert output.read_text(encoding="utf-8") == "previous\n", (
        "an unreadable listing must not replace the earlier record"
    )
    assert "release listing unreadable" in capsys.readouterr().err, (
        "the diagnostic must name the listing as unreadable"
    )


def test_an_absent_listing_is_an_input_fault(tmp_path: pathlib.Path) -> None:
    """Report a wrong path separately from a listing that cannot be used."""
    code = main([
        "plan-downloads",
        "--listing",
        str(tmp_path / "absent.json"),
        "--output",
        str(tmp_path / "plan.txt"),
    ])

    assert code == EXIT_INPUT_UNREADABLE, f"an absent listing is an input fault, {code}"


def test_an_unwritable_destination_is_an_output_fault(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Report a destination whose parent does not exist, without a record."""
    listing = _write_listing(tmp_path / "assets.json", [[_asset(11, "a.tar.gz")]])
    output = tmp_path / "absent" / "plan.txt"

    code = main([
        "plan-downloads",
        "--listing",
        str(listing),
        "--output",
        str(output),
    ])

    assert code == EXIT_OUTPUT_UNWRITABLE, (
        f"an unwritable path is an output fault, {code}"
    )
    assert not output.exists(), "a failed write must leave no record"
    assert "unwritable" in capsys.readouterr().err, (
        "the diagnostic must report the destination as unwritable"
    )


def test_a_successful_run_replaces_an_existing_record(
    tmp_path: pathlib.Path,
) -> None:
    """Replace an earlier plan completely rather than appending to it."""
    listing = _write_listing(tmp_path / "assets.json", [[_asset(11, "a.tar.gz")]])
    output = tmp_path / "plan.txt"
    _ = output.write_text("stale\n", encoding="utf-8")

    code = main([
        "plan-downloads",
        "--listing",
        str(listing),
        "--output",
        str(output),
    ])

    assert code == EXIT_OK, code
    assert output.read_text(encoding="utf-8") == "11\ta.tar.gz\n", (
        "a successful run must replace the earlier record, not extend it"
    )


def _complete_values(tmp_path: pathlib.Path, mode: str) -> dict[str, str]:
    """Return the arguments a successful invocation of ``mode`` needs.

    Each mode is given a listing of the shape it reads, so the control below
    runs the real command path rather than failing on an unreadable input. Only
    ``select-release`` receives ``--tag``; passing it to ``plan-downloads``
    would make every row fail on an unrecognized argument instead of on the
    value the row removes.

    Returns
    -------
    dict[str, str]
        Every flag and value the mode needs to succeed, keyed by flag.
    """
    pages = [[_release(7)]] if mode == "select-release" else [[_asset(11, "a")]]
    listing = _write_listing(tmp_path / f"{mode}.json", pages)
    values = {"--listing": str(listing), "--output": str(tmp_path / "result.txt")}
    if mode == "select-release":
        values["--tag"] = TAG
    return values


@pytest.mark.parametrize(
    ("mode", "missing"),
    [
        pytest.param("select-release", "--listing", id="selection-listing"),
        pytest.param("select-release", "--tag", id="selection-tag"),
        pytest.param("select-release", "--output", id="selection-output"),
        pytest.param("plan-downloads", "--listing", id="plan-listing"),
        pytest.param("plan-downloads", "--output", id="plan-output"),
    ],
)
def test_the_commands_refuse_to_guess_an_argument(
    tmp_path: pathlib.Path, mode: str, missing: str
) -> None:
    """Refuse an invocation that omits one of the values it needs."""
    argv = [
        mode,
        *[
            part
            for flag, value in _complete_values(tmp_path, mode).items()
            if flag != missing
            for part in (flag, value)
        ],
    ]

    assert main(argv) == EXIT_INPUT_UNREADABLE, "an invalid invocation must not succeed"


@pytest.mark.parametrize("mode", ["select-release", "plan-downloads"])
def test_a_complete_invocation_is_accepted(tmp_path: pathlib.Path, mode: str) -> None:
    """Control: the arguments the refusal rows remove are sufficient on their own.

    Without this case a row could pass because every invocation of that mode
    fails, which would leave the refusal test proving nothing about the
    argument it removes.
    """
    argv = [
        mode,
        *[
            part
            for flag, value in _complete_values(tmp_path, mode).items()
            for part in (flag, value)
        ],
    ]

    assert main(argv) == EXIT_OK, "a complete invocation must be accepted"

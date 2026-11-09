"""Exercise the finalizer's candidate resolution and read-only snapshot.

These commands never change the release. Their failure modes are therefore
about refusing to record a decision at all: an identity that names another
candidate, a package record that cannot be decoded, an API response for
another tag, and an unwritable destination. Each test asserts that no output
was left behind, because a stale record is what a later step would read.
"""

import json
import typing as typ

import pytest
from finalize_github_release_io import (
    EXIT_INPUT_UNREADABLE,
    EXIT_OUTPUT_UNWRITABLE,
    EXIT_REFUSED,
    EXIT_REMOTE_FAILED,
)
from finalize_github_release_support import (
    ARCHIVE,
    COMMIT,
    DIGEST,
    REPOSITORY,
    TAG,
    VIEW_ARGUMENTS,
    identity_record,
    release_view,
    run_finalizer,
)

if typ.TYPE_CHECKING:
    import pathlib

    from cmd_mox import CmdMox

pytest_plugins = ("cmd_mox.pytest_plugin",)


def _package_record(
    destination: pathlib.Path, identity: dict[str, object]
) -> pathlib.Path:
    """Write the envelope ``package-crate`` retains."""
    envelope = {"crate": "netsuke-build-1.2.3.crate", "identity": identity}
    envelope["sha256"] = DIGEST
    _ = destination.write_text(json.dumps(envelope), encoding="utf-8")
    return destination


def _resolve_arguments(source: pathlib.Path, output: pathlib.Path) -> list[str]:
    """Return a complete ``resolve`` invocation."""
    return [
        "resolve",
        "--identity",
        str(source),
        "--tag",
        TAG,
        "--commit",
        COMMIT,
        "--output",
        str(output),
    ]


def test_resolve_records_the_identity_the_package_stage_built(
    tmp_path: pathlib.Path,
) -> None:
    """Write the candidate record from the nested package identity."""
    source = _package_record(tmp_path / "identity.json", identity_record())
    output = tmp_path / "candidate.json"

    run_finalizer(*_resolve_arguments(source, output))

    assert json.loads(output.read_text(encoding="utf-8")) == identity_record(), (
        "the candidate record must carry exactly the validated identity"
    )


@pytest.mark.parametrize(
    "case",
    [
        pytest.param(("v9.9.9", COMMIT, "tag"), id="another-tag"),
        pytest.param((TAG, "1" * 40, "commit"), id="another-commit"),
    ],
)
def test_resolve_refuses_a_package_record_for_another_candidate(
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
    case: tuple[str, str, str],
) -> None:
    """Stop when the recorded identity is not the run's candidate."""
    requested_tag, recorded_commit, field = case
    source = _package_record(
        tmp_path / "identity.json", identity_record(commit=recorded_commit)
    )
    output = tmp_path / "candidate.json"
    arguments = _resolve_arguments(source, output)
    arguments[arguments.index("--tag") + 1] = requested_tag

    with pytest.raises(SystemExit) as error:
        run_finalizer(*arguments)

    assert error.value.code == EXIT_REFUSED, (
        "a record for another candidate must be refused, not recorded"
    )
    assert not output.exists(), "a refused candidate must leave no record behind"
    assert field in capsys.readouterr().err, (
        "the refusal must name the field that disagreed"
    )


def test_resolve_refuses_a_rehearsal_record_with_no_tag(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse an identity that names no tag, which no publication can use."""
    source = _package_record(tmp_path / "identity.json", identity_record(tag=None))
    output = tmp_path / "candidate.json"

    with pytest.raises(SystemExit) as error:
        run_finalizer(*_resolve_arguments(source, output))

    assert error.value.code == EXIT_REFUSED, (
        "a rehearsal record cannot be finalized as a release"
    )
    assert not output.exists(), "a tagless candidate must leave no record behind"


@pytest.mark.parametrize(
    "envelope",
    [
        pytest.param("[]", id="not-an-object"),
        pytest.param('{"crate": "x"}', id="no-identity"),
        pytest.param('{"identity": []}', id="identity-not-an-object"),
        pytest.param(
            '{"identity": {"tag": "v1.2.3"}}',
            id="identity-incomplete",
        ),
        pytest.param(
            json.dumps({"identity": identity_record(tag="v9.9.9")}),
            id="tag-does-not-spell-version",
        ),
    ],
)
def test_resolve_refuses_an_unusable_package_record(
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
    envelope: str,
) -> None:
    """Report a malformed package record as an unreadable input."""
    source = tmp_path / "identity.json"
    _ = source.write_text(envelope, encoding="utf-8")
    output = tmp_path / "candidate.json"

    with pytest.raises(SystemExit) as error:
        run_finalizer(*_resolve_arguments(source, output))

    assert error.value.code == EXIT_INPUT_UNREADABLE, (
        "a malformed package record is an input fault"
    )
    assert not output.exists(), "an unreadable package record must write nothing"
    assert capsys.readouterr().err, "the refusal must be diagnosed"


def test_resolve_reports_an_unwritable_candidate(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Give a write failure its own status rather than a traceback."""
    source = _package_record(tmp_path / "identity.json", identity_record())
    output = tmp_path / "absent-directory" / "candidate.json"

    with pytest.raises(SystemExit) as error:
        run_finalizer(*_resolve_arguments(source, output))

    assert error.value.code == EXIT_OUTPUT_UNWRITABLE, (
        "a failed write must not be reported as a refused candidate"
    )
    assert "could not be written" in capsys.readouterr().err, (
        "the write failure must be diagnosed"
    )


def test_snapshot_records_the_flags_and_reported_digests(
    tmp_path: pathlib.Path, cmd_mox: CmdMox
) -> None:
    """Write both the human-readable state and the planner's asset mapping."""
    view = (
        cmd_mox
        .mock("gh")
        .with_args(*VIEW_ARGUMENTS)
        .returns(stdout=release_view(is_draft=True, is_prerelease=False))
    )
    state_path = tmp_path / "state.json"
    assets_path = tmp_path / "assets.json"

    run_finalizer(
        "snapshot",
        "--tag",
        TAG,
        "--repository",
        REPOSITORY,
        "--state-path",
        str(state_path),
        "--assets-path",
        str(assets_path),
    )

    expected_state = {
        "assets": {ARCHIVE: f"sha256:{DIGEST}"},
        "isDraft": True,
        "isPrerelease": False,
        "tag": TAG,
    }
    assert json.loads(state_path.read_text(encoding="utf-8")) == expected_state, (
        "the snapshot must record the release's flags, tag, and asset digests"
    )
    assert json.loads(assets_path.read_text(encoding="utf-8")) == {
        ARCHIVE: f"sha256:{DIGEST}"
    }, "the planner's listing must keep GitHub's digest spelling"
    assert len(view.invocations) == 1, "the snapshot must make one read-only call"


def test_snapshot_stops_without_writing_when_the_api_fails(
    tmp_path: pathlib.Path,
    cmd_mox: CmdMox,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Leave no record when GitHub did not say what the release looks like."""
    _ = (
        cmd_mox
        .mock("gh")
        .with_args(*VIEW_ARGUMENTS)
        .returns(exit_code=1, stderr="HTTP 403: Resource not accessible\n")
    )
    state_path = tmp_path / "state.json"
    assets_path = tmp_path / "assets.json"

    with pytest.raises(SystemExit) as error:
        run_finalizer(
            "snapshot",
            "--tag",
            TAG,
            "--repository",
            REPOSITORY,
            "--state-path",
            str(state_path),
            "--assets-path",
            str(assets_path),
        )

    assert error.value.code == EXIT_REMOTE_FAILED, (
        "an unreadable release is a remote failure"
    )
    assert not state_path.exists(), "a failed snapshot must not record the state"
    assert not assets_path.exists(), "a failed snapshot must not record the assets"
    assert "HTTP 403" in capsys.readouterr().err, (
        "the API diagnostic must reach the workflow log"
    )


@pytest.mark.parametrize(
    "response",
    [
        pytest.param("{not json", id="malformed-json"),
        pytest.param(
            release_view(is_draft=True, is_prerelease=False, tag="v9.9.9"),
            id="another-release",
        ),
        pytest.param(json.dumps({"tagName": TAG, "assets": []}), id="missing-flags"),
    ],
)
def test_snapshot_refuses_an_unusable_api_response(
    tmp_path: pathlib.Path,
    cmd_mox: CmdMox,
    capsys: pytest.CaptureFixture[str],
    response: str,
) -> None:
    """Refuse a response that could describe another release or nothing."""
    _ = cmd_mox.mock("gh").with_args(*VIEW_ARGUMENTS).returns(stdout=response)
    state_path = tmp_path / "state.json"
    assets_path = tmp_path / "assets.json"

    with pytest.raises(SystemExit) as error:
        run_finalizer(
            "snapshot",
            "--tag",
            TAG,
            "--repository",
            REPOSITORY,
            "--state-path",
            str(state_path),
            "--assets-path",
            str(assets_path),
        )

    assert error.value.code == EXIT_REMOTE_FAILED, (
        "a response for another release is a remote failure, not a refusal"
    )
    assert not state_path.exists(), "an unusable response must record no state"
    assert not assets_path.exists(), "an unusable response must record no assets"
    assert "unusable response" in capsys.readouterr().err, (
        "the refusal must name the unusable response"
    )


def test_snapshot_refuses_a_response_that_names_one_asset_twice(
    tmp_path: pathlib.Path, cmd_mox: CmdMox
) -> None:
    """Refuse an ambiguous listing rather than silently keeping one digest."""
    payload = json.loads(release_view(is_draft=True, is_prerelease=False))
    payload["assets"].append({"name": ARCHIVE})
    _ = (
        cmd_mox
        .mock("gh")
        .with_args(*VIEW_ARGUMENTS)
        .returns(stdout=json.dumps(payload))
    )

    with pytest.raises(SystemExit) as error:
        run_finalizer(
            "snapshot",
            "--tag",
            TAG,
            "--repository",
            REPOSITORY,
            "--state-path",
            str(tmp_path / "state.json"),
            "--assets-path",
            str(tmp_path / "assets.json"),
        )

    assert error.value.code == EXIT_REMOTE_FAILED, (
        "an ambiguous asset listing must be refused, not silently reduced"
    )


def test_resolve_refuses_an_unreadable_record(
    tmp_path: pathlib.Path,
) -> None:
    """A missing record is an input fault, not an empty decision."""
    missing = tmp_path / "absent.json"

    with pytest.raises(SystemExit) as error:
        run_finalizer(*_resolve_arguments(missing, tmp_path / "candidate.json"))

    assert error.value.code == EXIT_INPUT_UNREADABLE, (
        "a missing record must not be treated as an empty decision"
    )

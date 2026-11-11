"""Exercise the one command that can make a release public.

The tests are organized around what must remain true however the run ends:
a release is never published over a plan that is not fully verified, and every
refusal names the asset that stopped it. The failures that follow a successful
API call — an unreadable read-back, an output that cannot be written — live in
``test_finalize_github_release_publish_failures``.
"""

import typing as typ

import pytest
from finalize_github_release_io import (
    EXIT_INPUT_UNREADABLE,
    EXIT_REFUSED,
    EXIT_REMOTE_FAILED,
    EXIT_UPLOAD_REQUIRED,
)
from finalize_github_release_support import (
    ARCHIVE,
    COMMIT,
    DIGEST,
    TAG,
    edit_arguments,
    identity_record,
    invocation_arguments,
    plan_record,
    publish_arguments,
    release_view,
    release_views,
    result_record,
    run_finalizer,
    sequenced_gh,
)

if typ.TYPE_CHECKING:
    import pathlib

    from cmd_mox import CmdMox

pytest_plugins = ("cmd_mox.pytest_plugin",)

PRERELEASE_TAG = "v1.2.3-rc.1"
PRERELEASE_VERSION = "1.2.3-rc.1"


def test_publish_un_drafts_a_verified_release(
    tmp_path: pathlib.Path, cmd_mox: CmdMox
) -> None:
    """View, edit once, and read back the release it made public."""
    output = tmp_path / "github-output"
    gh = sequenced_gh(
        cmd_mox,
        [
            *release_views((True, False)),
            ("", "", 0),
            *release_views((False, False)),
        ],
    )

    run_finalizer(*publish_arguments(tmp_path, github_output=output))

    assert [invocation_arguments(invocation)[:2] for invocation in gh.invocations] == [
        ("release", "view"),
        ("release", "edit"),
        ("release", "view"),
    ], "the release must be read, edited once, and read back"
    assert invocation_arguments(gh.invocations[1]) == edit_arguments(
        TAG, is_prerelease=False
    ), "the edit must un-draft without replacing any asset"
    assert result_record(tmp_path) == {
        "outcome": "finalized",
        "prerelease": False,
        "tag": TAG,
    }, "a verified release must be recorded as this run's finalization"
    assert output.read_text(encoding="utf-8") == (
        "outcome=finalized\nprerelease=false\n"
    ), "the step's outputs must name both the outcome and the flag"


def test_publish_sets_the_prerelease_flag_from_the_candidate(
    tmp_path: pathlib.Path, cmd_mox: CmdMox
) -> None:
    """Take the flag from the validated candidate, not from the live release."""
    output = tmp_path / "github-output"
    gh = sequenced_gh(
        cmd_mox,
        [
            *release_views((True, True), tag=PRERELEASE_TAG),
            ("", "", 0),
            *release_views((False, True), tag=PRERELEASE_TAG),
        ],
    )

    run_finalizer(
        *publish_arguments(
            tmp_path,
            candidate=identity_record(
                is_prerelease=True,
                tag=PRERELEASE_TAG,
                version=PRERELEASE_VERSION,
            ),
            github_output=output,
        )
    )

    assert invocation_arguments(gh.invocations[1]) == edit_arguments(
        PRERELEASE_TAG, is_prerelease=True
    ), "the edit must carry the candidate's prerelease flag, not the live one"
    assert "prerelease=true" in output.read_text(encoding="utf-8"), (
        "the workflow must learn the release is a prerelease"
    )
    assert result_record(tmp_path) == {
        "outcome": "finalized",
        "prerelease": True,
        "tag": PRERELEASE_TAG,
    }, "the recorded outcome must keep the prerelease classification"


def test_publish_treats_an_already_public_release_as_finalized(
    tmp_path: pathlib.Path, cmd_mox: CmdMox
) -> None:
    """Succeed without an edit when an interrupted run already published."""
    gh = sequenced_gh(cmd_mox, release_views((False, False)))

    run_finalizer(*publish_arguments(tmp_path))

    assert [invocation_arguments(invocation)[:2] for invocation in gh.invocations] == [
        ("release", "view")
    ], "an already-finalized release must not be edited again"
    assert result_record(tmp_path) == {
        "outcome": "already-finalized",
        "prerelease": False,
        "tag": TAG,
    }, "a public release must be recorded as already finalized"


def test_a_retry_after_a_failed_edit_finishes_without_a_second_edit(
    tmp_path: pathlib.Path, cmd_mox: CmdMox, capsys: pytest.CaptureFixture[str]
) -> None:
    """Resume from the state the previous attempt left behind."""
    sequenced_gh(
        cmd_mox,
        [
            *release_views((True, False)),
            ("", "HTTP 502\n", 1),
            *release_views((False, False)),
        ],
    )
    arguments = publish_arguments(tmp_path)

    with pytest.raises(SystemExit) as first:
        run_finalizer(*arguments)

    assert first.value.code == EXIT_REMOTE_FAILED, "a failed edit must fail the step"
    assert not (tmp_path / "result.json").exists(), (
        "a failed edit must not record a final outcome"
    )
    message = capsys.readouterr().err
    assert "HTTP 502" in message, "the API diagnostic must reach the workflow log"
    assert "state is unknown" in message, (
        "a failed edit may have been applied, so the state must not be reported "
        "as a draft"
    )

    run_finalizer(*arguments)

    assert result_record(tmp_path) == {
        "outcome": "already-finalized",
        "prerelease": False,
        "tag": TAG,
    }, "the retry must record the state the failed attempt left behind"


def test_a_retry_after_an_unreadable_readback_finishes_without_an_edit(
    tmp_path: pathlib.Path, cmd_mox: CmdMox, capsys: pytest.CaptureFixture[str]
) -> None:
    """Treat a publish whose read-back failed as finished on the next run."""
    gh = sequenced_gh(
        cmd_mox,
        [
            *release_views((True, False)),
            ("", "", 0),
            ("", "connection reset\n", 1),
            *release_views((False, False)),
        ],
    )
    arguments = publish_arguments(tmp_path)

    with pytest.raises(SystemExit) as first:
        run_finalizer(*arguments)

    assert first.value.code == EXIT_REMOTE_FAILED, (
        "an unreadable read-back must fail the step"
    )
    assert "state is unknown" in capsys.readouterr().err, (
        "an unreadable read-back cannot promise the release is still a draft"
    )

    run_finalizer(*arguments)

    assert [
        invocation_arguments(invocation)[:2] for invocation in gh.invocations
    ].count(("release", "edit")) == 1, (
        "the retry must not edit an already-public release"
    )
    assert result_record(tmp_path) == {
        "outcome": "already-finalized",
        "prerelease": False,
        "tag": TAG,
    }, "the retry must record the release as already finalized"


def test_publish_refuses_a_release_that_lost_a_declared_asset(
    tmp_path: pathlib.Path, cmd_mox: CmdMox, capsys: pytest.CaptureFixture[str]
) -> None:
    """Stop before the edit when the draft no longer holds what was declared.

    The plan verified the asset from an earlier snapshot. The pre-edit read
    finds it gone, and the planner's own policy calls that an upload rather
    than a conflict: the declared upload contract can put the bytes back.
    """
    gh = sequenced_gh(
        cmd_mox,
        [(release_view(is_draft=True, is_prerelease=False, assets={}), "", 0)],
    )

    with pytest.raises(SystemExit) as error:
        run_finalizer(*publish_arguments(tmp_path))

    assert error.value.code == EXIT_UPLOAD_REQUIRED, (
        "a draft that lost a declared asset must be sent back to the uploader"
    )
    assert [invocation_arguments(invocation)[:2] for invocation in gh.invocations] == [
        ("release", "view")
    ], "a refused run must not edit the release"
    assert not (tmp_path / "result.json").exists(), (
        "a refused run must not record an outcome"
    )
    message = capsys.readouterr().err
    assert ARCHIVE in message, "the refusal must name the asset that went missing"
    assert "remains a draft" in message, "the operator must know the release state"


def test_publish_refuses_a_release_whose_asset_changed(
    tmp_path: pathlib.Path, cmd_mox: CmdMox, capsys: pytest.CaptureFixture[str]
) -> None:
    """Stop before the edit when the draft's bytes no longer match the record.

    The plan verified the asset from an earlier snapshot; the pre-edit read
    reports a different digest. Publishing would make a release public whose
    inventory the trusted record does not describe, so the run refuses.
    """
    gh = sequenced_gh(
        cmd_mox,
        [
            (
                release_view(
                    is_draft=True,
                    is_prerelease=False,
                    assets={ARCHIVE: "sha256:" + "ef" * 32},
                ),
                "",
                0,
            )
        ],
    )

    with pytest.raises(SystemExit) as error:
        run_finalizer(*publish_arguments(tmp_path))

    assert error.value.code == EXIT_REFUSED, (
        "a draft whose bytes changed must not be published"
    )
    assert [invocation_arguments(invocation)[:2] for invocation in gh.invocations] == [
        ("release", "view")
    ], "a refused run must not edit the release"
    message = capsys.readouterr().err
    assert ARCHIVE in message, "the refusal must name the asset that changed"
    assert "disagree" in message, "the refusal must say the bytes disagree"
    assert "remains a draft" in message, "the operator must know the release state"


def test_publish_refuses_an_unreadable_digest_record(
    tmp_path: pathlib.Path, cmd_mox: CmdMox, capsys: pytest.CaptureFixture[str]
) -> None:
    """Never reconcile against a record that cannot support a decision."""
    gh = sequenced_gh(cmd_mox, release_views((True, False)))
    arguments = publish_arguments(tmp_path)
    _ = (tmp_path / "trusted.json").write_text('{"assets": {}}', encoding="utf-8")

    with pytest.raises(SystemExit) as error:
        run_finalizer(*arguments)

    assert error.value.code == EXIT_INPUT_UNREADABLE, (
        "an empty trusted record must be an input fault, not a verified release"
    )
    assert [invocation_arguments(invocation)[:2] for invocation in gh.invocations] == [
        ("release", "view")
    ], "an unreadable record must stop the run before the edit"
    assert "digest record" in capsys.readouterr().err, (
        "the diagnosis must name the record that could not be used"
    )


def test_publish_tolerates_an_asset_the_record_never_declared(
    tmp_path: pathlib.Path, cmd_mox: CmdMox
) -> None:
    """Leave an undeclared attachment alone, exactly as the planner does.

    The reconciliation decides only about names the trusted record declares, so
    a file it never claimed must not stop a release the record verified. The
    pre-edit pass applies the same rule, so an extra attachment is tolerated
    twice for the same reason rather than by a second, weaker check.
    """
    gh = sequenced_gh(
        cmd_mox,
        [
            (
                release_view(
                    is_draft=True,
                    is_prerelease=False,
                    assets={ARCHIVE: f"sha256:{DIGEST}", "notes.txt": None},
                ),
                "",
                0,
            ),
            ("", "", 0),
            *release_views((False, False)),
        ],
    )

    run_finalizer(*publish_arguments(tmp_path))

    assert [
        invocation_arguments(invocation)[:2] for invocation in gh.invocations
    ].count(("release", "edit")) == 1, (
        "an undeclared attachment must not block the record's verified release"
    )


def test_publish_refuses_an_upload_plan_before_reaching_github(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Leave a missing asset to the declared upload contract, not this stage."""
    arguments = publish_arguments(tmp_path, plan=plan_record(action="upload"))

    with pytest.raises(SystemExit) as error:
        run_finalizer(*arguments)

    assert error.value.code == EXIT_UPLOAD_REQUIRED, (
        "a missing asset is the uploader's to fix, not this stage's to guess at"
    )
    assert not (tmp_path / "result.json").exists(), (
        "a release awaiting an upload must not record a finalization"
    )
    message = capsys.readouterr().err
    assert ARCHIVE in message, "the refusal must name the asset to upload"
    assert "remains a draft" in message, "the operator must know the release state"


@pytest.mark.parametrize(
    ("action", "word"),
    [
        pytest.param("conflict", "disagree", id="conflicting-bytes"),
        pytest.param("unverifiable", "could not be verified", id="unread-digest"),
    ],
)
def test_publish_refuses_a_plan_that_cannot_be_verified(
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
    action: str,
    word: str,
) -> None:
    """Stop on either blockage, with the diagnosis that fits it."""
    arguments = publish_arguments(tmp_path, plan=plan_record(action=action))

    with pytest.raises(SystemExit) as error:
        run_finalizer(*arguments)

    assert error.value.code == EXIT_REFUSED, (
        "an unverifiable plan must be refused, whatever the cause"
    )
    message = capsys.readouterr().err
    assert ARCHIVE in message, "the refusal must name the blocked asset"
    assert word in message, "the refusal must distinguish the two blockages"
    assert "remains a draft" in message, "the operator must know the release state"


def test_publish_refuses_a_plan_with_no_decisions(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Never un-draft on the empty plan that resembles a verified release."""
    arguments = publish_arguments(tmp_path)
    _ = (tmp_path / "plan.json").write_text('{"actions": []}', encoding="utf-8")

    with pytest.raises(SystemExit) as error:
        run_finalizer(*arguments)

    assert error.value.code == EXIT_REFUSED, (
        "an empty plan must never be read as a verified release"
    )
    assert "no asset decisions" in capsys.readouterr().err, (
        "the refusal must say the plan decided nothing"
    )


def test_publish_refuses_a_malformed_plan(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Report a plan that cannot be decoded as an unreadable input."""
    arguments = publish_arguments(tmp_path)
    _ = (tmp_path / "plan.json").write_text('{"actions": "all"}', encoding="utf-8")

    with pytest.raises(SystemExit) as error:
        run_finalizer(*arguments)

    assert error.value.code == EXIT_INPUT_UNREADABLE, (
        "an undecodable plan is an input fault, not a refusal"
    )
    assert "not a usable plan" in capsys.readouterr().err, (
        "the diagnosis must name the plan as unusable"
    )


@pytest.mark.parametrize(
    "case",
    [
        pytest.param(("v9.9.9", COMMIT, "tag"), id="another-tag"),
        pytest.param((TAG, "1" * 40, "commit"), id="another-commit"),
    ],
)
def test_publish_refuses_a_candidate_for_another_revision(
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
    case: tuple[str, str, str],
) -> None:
    """Refuse before the API call when the candidate is not this run's."""
    requested_tag, recorded_commit, field = case
    arguments = publish_arguments(
        tmp_path, candidate=identity_record(commit=recorded_commit)
    )
    arguments[arguments.index("--tag") + 1] = requested_tag

    with pytest.raises(SystemExit) as error:
        run_finalizer(*arguments)

    assert error.value.code == EXIT_REFUSED, (
        "a candidate for another revision must be refused"
    )
    assert field in capsys.readouterr().err, (
        "the refusal must name the field that disagreed"
    )
    assert not (tmp_path / "result.json").exists(), (
        "a refused candidate must not record a finalization"
    )


def test_publish_refuses_an_unreadable_candidate(tmp_path: pathlib.Path) -> None:
    """Treat a candidate record that cannot be decoded as an input fault."""
    arguments = publish_arguments(tmp_path)
    _ = (tmp_path / "candidate.json").write_text('{"tag": 7}', encoding="utf-8")

    with pytest.raises(SystemExit) as error:
        run_finalizer(*arguments)

    assert error.value.code == EXIT_INPUT_UNREADABLE, (
        "a candidate that cannot be decoded must be reported as unreadable"
    )

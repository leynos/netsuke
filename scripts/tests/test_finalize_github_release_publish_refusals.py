"""Specify the refusals a publishing run makes before it reaches GitHub.

Every test here stops the run from a record it was given, not from an API
answer: an upload still owed, a plan that cannot be verified, a plan that
cannot be decoded, and a candidate that names another revision. Each refusal
leaves the release a draft and names the document that stopped it. The
refusals that read the live release, and the failures that follow a successful
edit, live in ``test_finalize_github_release_publish`` and
``test_finalize_github_release_publish_failures``.
"""

import typing as typ

import pytest
from finalize_github_release_io import (
    EXIT_INPUT_UNREADABLE,
    EXIT_REFUSED,
    EXIT_UPLOAD_REQUIRED,
)
from finalize_github_release_records_support import (
    ARCHIVE,
    COMMIT,
    TAG,
    PublishRecords,
    identity_record,
    plan_record,
)
from finalize_github_release_support import publish_arguments, run_finalizer

if typ.TYPE_CHECKING:
    import pathlib


def test_publish_refuses_an_upload_plan_before_reaching_github(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Leave a missing asset to the declared upload contract, not this stage."""
    arguments = publish_arguments(
        tmp_path, PublishRecords(plan=plan_record(action="upload"))
    )

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
    assert "has not edited the release" in message, (
        "a refusal made from the plan alone has read no release state"
    )


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
    arguments = publish_arguments(
        tmp_path, PublishRecords(plan=plan_record(action=action))
    )

    with pytest.raises(SystemExit) as error:
        run_finalizer(*arguments)

    assert error.value.code == EXIT_REFUSED, (
        "an unverifiable plan must be refused, whatever the cause"
    )
    message = capsys.readouterr().err
    assert ARCHIVE in message, "the refusal must name the blocked asset"
    assert word in message, "the refusal must distinguish the two blockages"
    assert "has not edited the release" in message, (
        "a refusal made from the plan alone has read no release state"
    )


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
        tmp_path, PublishRecords(candidate=identity_record(commit=recorded_commit))
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

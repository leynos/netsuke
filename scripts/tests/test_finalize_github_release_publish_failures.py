"""Exercise what the publishing command does when the API or a file fails.

Each case here begins with a plan that permits publication, so the failure is
the only thing under test. The assertions say what state the release is left
in: still a draft when the API refused or ignored the edit, already public
when only the outcome record could not be written, and never holding bytes
this command replaced.
"""

import typing as typ

import pytest
from finalize_github_release_io import (
    EXIT_OUTPUT_UNWRITABLE,
    EXIT_REFUSED,
    EXIT_REMOTE_FAILED,
)
from finalize_github_release_support import (
    DIGEST,
    invocation_arguments,
    publish_arguments,
    release_views,
    run_finalizer,
    sequenced_gh,
)

if typ.TYPE_CHECKING:
    import pathlib

    from cmd_mox import CmdMox

pytest_plugins = ("cmd_mox.pytest_plugin",)


def test_publish_reports_an_api_failure_before_the_edit(
    tmp_path: pathlib.Path,
    cmd_mox: CmdMox,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Leave the draft untouched when the release state cannot be read."""
    sequenced_gh(cmd_mox, [("", "HTTP 403\n", 1)])

    with pytest.raises(SystemExit) as error:
        run_finalizer(*publish_arguments(tmp_path))

    assert error.value.code == EXIT_REMOTE_FAILED, (
        "an unreadable release must fail the step before any edit"
    )
    assert not (tmp_path / "result.json").exists(), (
        "a release that was never read must not record an outcome"
    )
    assert "HTTP 403" in capsys.readouterr().err, (
        "the API diagnostic must reach the workflow log"
    )


def test_publish_reports_a_readback_that_is_still_a_draft(
    tmp_path: pathlib.Path,
    cmd_mox: CmdMox,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Distinguish an accepted call that did not publish from a successful one."""
    sequenced_gh(
        cmd_mox,
        [*release_views((True, False)), ("", "", 0), *release_views((True, False))],
    )

    with pytest.raises(SystemExit) as error:
        run_finalizer(*publish_arguments(tmp_path))

    assert error.value.code == EXIT_REMOTE_FAILED, (
        "an accepted edit that changed nothing must fail the step"
    )
    assert "still a draft" in capsys.readouterr().err, (
        "the diagnosis must say the release was not published"
    )
    assert not (tmp_path / "result.json").exists(), (
        "a release that stayed a draft must not record a success"
    )


def test_publish_reports_a_public_release_with_the_wrong_flag(
    tmp_path: pathlib.Path,
    cmd_mox: CmdMox,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Refuse to call a public release finalized when its flag contradicts it."""
    sequenced_gh(
        cmd_mox,
        [*release_views((True, False)), ("", "", 0), *release_views((False, True))],
    )

    with pytest.raises(SystemExit) as error:
        run_finalizer(*publish_arguments(tmp_path))

    assert error.value.code == EXIT_REFUSED, (
        "a public release whose flag contradicts it must be refused"
    )
    message = capsys.readouterr().err
    assert "public" in message, "the refusal must say the release is public"
    assert "cannot be made a draft again" in message, (
        "the refusal must say the release cannot be reverted to a draft"
    )


def test_publish_reports_an_unwritable_result_after_publishing(
    tmp_path: pathlib.Path,
    cmd_mox: CmdMox,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Fail the step, but say the release itself is already public."""
    sequenced_gh(
        cmd_mox,
        [*release_views((True, False)), ("", "", 0), *release_views((False, False))],
    )

    arguments = publish_arguments(tmp_path)
    arguments[arguments.index("--result-path") + 1] = str(
        tmp_path / "absent" / "result.json"
    )

    with pytest.raises(SystemExit) as error:
        run_finalizer(*arguments)

    assert error.value.code == EXIT_OUTPUT_UNWRITABLE, (
        "an unrecordable outcome must fail the step even after publishing"
    )
    message = capsys.readouterr().err
    assert "could not be written" in message, "the write failure must be diagnosed"
    assert "already final" in message, (
        "the operator must be told the release is public despite the failure"
    )


def test_publish_reports_an_unwritable_github_output(
    tmp_path: pathlib.Path,
    cmd_mox: CmdMox,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Say the release is final even when its outcome could not be recorded."""
    sequenced_gh(cmd_mox, release_views((False, False)))

    with pytest.raises(SystemExit) as error:
        run_finalizer(
            *publish_arguments(tmp_path, github_output=tmp_path / "absent" / "output")
        )

    assert error.value.code == EXIT_OUTPUT_UNWRITABLE, (
        "an unwritable step output is an output fault, not a publication fault"
    )
    assert "already final" in capsys.readouterr().err, (
        "the operator must be told the release is already final"
    )


def test_publish_only_reads_and_un_drafts(
    tmp_path: pathlib.Path, cmd_mox: CmdMox
) -> None:
    """Prove the command never reaches an asset-mutating subcommand."""
    gh = sequenced_gh(
        cmd_mox,
        [*release_views((True, False)), ("", "", 0), *release_views((False, False))],
    )

    run_finalizer(*publish_arguments(tmp_path))

    forbidden = ("upload", "delete", "clobber", "create", "delete-asset")
    for invocation in gh.invocations:
        arguments = invocation_arguments(invocation)
        assert arguments[:2] in {
            ("release", "view"),
            ("release", "edit"),
        }, f"unexpected gh subcommand: {arguments!r}"
        assert not set(arguments) & set(forbidden), (
            f"the finalizer must never replace bytes: {arguments!r}"
        )
    assert DIGEST not in " ".join(
        argument
        for invocation in gh.invocations
        for argument in invocation_arguments(invocation)
    ), "the finalizer must not send an asset digest to the API"

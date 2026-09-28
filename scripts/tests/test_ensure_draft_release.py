"""Test draft-release creation through the allowlisted GitHub CLI."""

import typing as typ

import ensure_draft_release as release_script
import pytest

if typ.TYPE_CHECKING:
    from cmd_mox import CmdMox, Invocation

pytest_plugins = ("cmd_mox.pytest_plugin",)


def test_existing_release_is_left_untouched(
    cmd_mox: CmdMox, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A successful release lookup does not create a second release."""
    tag = "v1.2.3"
    lookup = cmd_mox.mock("gh").with_args("release", "view", tag).returns(exit_code=0)
    monkeypatch.setenv("INPUT_TAG", tag)

    release_script.app([], result_action="return_value")

    assert lookup.invocations, "the release lookup must run"
    assert len(lookup.invocations) == 1, "the existing release must be viewed once"


def test_missing_release_is_created_with_tag_as_one_argument(
    cmd_mox: CmdMox, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A shell-significant tag remains one argument to both gh commands."""
    tag = "v1.2.3'; touch injected"
    lookup_args = ["release", "view", tag]
    creation_args = [
        "release",
        "create",
        tag,
        "--draft",
        "--verify-tag",
        "--notes",
        f"Automated release for {tag}",
    ]

    def respond(invocation: Invocation) -> tuple[str, str, int]:
        """Return gh's lookup and creation statuses for this tag."""
        if invocation.args == lookup_args:
            return "", "", 1
        if invocation.args == creation_args:
            return "", "", 0
        return "", "unexpected gh arguments", 127

    gh = cmd_mox.mock("gh").runs(respond).times(2)
    monkeypatch.setenv("INPUT_TAG", tag)

    release_script.app([], result_action="return_value")

    assert [invocation.args for invocation in gh.invocations] == [
        lookup_args,
        creation_args,
    ], "the shell-significant tag must be one argument to each gh call"


def test_failed_release_creation_propagates_gh_status(
    cmd_mox: CmdMox, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A failed create fails the workflow step with gh's exit code."""
    tag = "v1.2.3"
    lookup_args = ["release", "view", tag]
    creation_args = [
        "release",
        "create",
        tag,
        "--draft",
        "--verify-tag",
        "--notes",
        f"Automated release for {tag}",
    ]

    def respond(invocation: Invocation) -> tuple[str, str, int]:
        """Return gh's lookup failure and requested creation failure."""
        if invocation.args == lookup_args:
            return "", "", 1
        if invocation.args == creation_args:
            return "", "", 42
        return "", "unexpected gh arguments", 127

    gh = cmd_mox.mock("gh").runs(respond).times(2)
    monkeypatch.setenv("INPUT_TAG", tag)

    with pytest.raises(SystemExit) as error:
        release_script.app([])

    assert error.value.code == 42, "the workflow step must preserve gh's failure status"
    assert [invocation.args for invocation in gh.invocations] == [
        lookup_args,
        creation_args,
    ], "gh must view the release before attempting creation"

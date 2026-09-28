#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts>=4.25.3,<5", "cuprum>=0.1.0,<0.2.0"]
# ///
"""Ensure the requested GitHub release exists as a draft."""

import sys
import typing as typ

from cuprum import CommandResult, Program, ProgramCatalogue, sh
from cuprum.catalogue import ProjectSettings

import cyclopts
from cyclopts import App, Parameter

GITHUB_CATALOGUE = ProgramCatalogue(
    projects=(
        ProjectSettings(
            name="ensure-draft-release",
            programs=(Program("gh"),),
            documentation_locations=(),
            noise_rules=(),
        ),
    )
)
app = App(config=cyclopts.config.Env("INPUT_", command=False))


def run_gh(*arguments: str, echo: bool = False) -> CommandResult:
    """Run one allowlisted GitHub CLI command.

    Returns
    -------
    CommandResult
        The captured output and exit status from ``gh``.

    Examples
    --------
    ``run_gh("release", "view", "v1.2.3")`` passes the tag as one argument.
    """
    command = sh.make(Program("gh"), catalogue=GITHUB_CATALOGUE)(*arguments)
    return command.run_sync(echo=echo)


def _lookup_reported_missing(result: CommandResult) -> bool:
    """Classify a lookup as a confirmed missing release.

    Only the GitHub CLI's explicit not-found diagnostic proves absence; other
    status-1 errors can indicate authentication, connectivity, or API failures.

    Returns
    -------
    bool
        Whether the lookup result explicitly reports a missing release.

    Examples
    --------
    >>> missing = CommandResult(
    ...     Program("gh"), ("gh", "release", "view", "v1.2.3"), 1, 42, None,
    ...     "release not found",
    ... )
    >>> _lookup_reported_missing(missing)
    True
    """
    return (
        result.exit_code == 1 and "release not found" in (result.stderr or "").lower()
    )


@app.default
def ensure_draft_release(*, tag: typ.Annotated[str, Parameter(required=True)]) -> None:
    """Create a verified draft release when the tag has no release yet.

    Raises
    ------
    SystemExit
        If lookup fails for a reason other than a missing release, or creating
        the draft release fails, with ``gh``'s exit status.

    Examples
    --------
    With ``INPUT_TAG=v1.2.3``, view that release and create it as a draft if
    the view command fails.
    """
    existing_release = run_gh("release", "view", tag)
    if existing_release.exit_code == 0:
        return
    if existing_release.stderr:
        sys.stderr.write(existing_release.stderr)
    # The CLI uses status 1 for ordinary errors, so create only on its
    # explicit not-found diagnostic; network and API failures must stop here.
    is_missing_release = _lookup_reported_missing(existing_release)
    if not is_missing_release:
        raise SystemExit(existing_release.exit_code)

    created_release = run_gh(
        "release",
        "create",
        tag,
        "--draft",
        "--verify-tag",
        "--notes",
        f"Automated release for {tag}",
        echo=True,
    )
    if created_release.exit_code != 0:
        raise SystemExit(created_release.exit_code)


if __name__ == "__main__":
    app()

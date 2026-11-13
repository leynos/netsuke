"""Run the draft-release decisions a workflow step needs as a command.

The release workflow holds the token and performs the network calls; this CLI
turns the listings those calls saved into the two values the next steps need:
the identifier of the draft release to verify, and the ordered plan of assets
to download. Keeping both decisions here means the shell step contains no
``jq`` expression that could disagree with :mod:`release_draft_assets`.

Reading GitHub's own field names is
:mod:`release_github_draft_listing`'s work and the plan's text format is
:mod:`release_download_plan`'s, so this module only composes them: translate the
decoded listing, decide, render, record. All three modules fail through
:class:`release_draft_assets.DraftAssetError`, which is what keeps the
rejection status one value.

The status codes follow inventory verification, because both commands stop the
same release for the same classes of fault:

``0``
    The listing supports the decision and the result was recorded.
``1``
    The listing is readable but cannot support the decision.
``2``
    A listing could not be read or is not JSON.
``3``
    The result could not be recorded.

A result is staged in a temporary sibling directory and moved into place only
after it is written completely, so a failed run leaves an earlier record
untouched. Consumers must therefore check this command's status rather than
merely whether the output exists.

This module performs no network calls and launches no child process; the
workflow step that holds the credential owns those.

Run via the release workflow's ``verify-release-inventory`` action; behavioural
coverage lives in ``scripts/tests/test_release_draft_assets_cli.py``.
"""

import argparse
import functools
import json
import pathlib
import sys
import tempfile
import typing as typ

import release_download_plan
import release_draft_assets
import release_github_draft_listing

if typ.TYPE_CHECKING:
    import collections.abc as cabc

EXIT_OK = 0
EXIT_POLICY_REJECTED = 1
EXIT_INPUT_UNREADABLE = 2
EXIT_OUTPUT_UNWRITABLE = 3


class _InputUnreadableError(Exception):
    """Carry a listing-read failure to the command that reports it."""


def build_parser() -> argparse.ArgumentParser:
    """Build the parser for the draft-asset CLI.

    Returns
    -------
    argparse.ArgumentParser
        A parser with the ``select-release`` and ``plan-downloads`` modes.
    """
    parser = argparse.ArgumentParser(
        prog="release_draft_assets_cli",
        description="Select a draft release or plan the assets to download.",
    )
    mode = parser.add_subparsers(dest="command", required=True)

    select = mode.add_parser(
        "select-release", help="name the one draft release carrying a tag"
    )
    select.add_argument(
        "--listing",
        required=True,
        type=pathlib.Path,
        help="the file holding slurped release pages from gh api",
    )
    select.add_argument("--tag", required=True, help="the release tag to select")
    select.add_argument(
        "--output", required=True, type=pathlib.Path, help="where to write the id"
    )

    plan = mode.add_parser(
        "plan-downloads", help="list the assets to download, as id and name"
    )
    plan.add_argument(
        "--listing",
        required=True,
        type=pathlib.Path,
        help="the file holding slurped asset pages from gh api",
    )
    plan.add_argument(
        "--output", required=True, type=pathlib.Path, help="where to write the plan"
    )
    return parser


def _read_listing(path: pathlib.Path) -> object:
    """Decode one saved listing, distinguishing an unreadable file.

    Returns
    -------
    object
        The decoded JSON value, unvalidated.

    Raises
    ------
    _InputUnreadableError
        If the path cannot be read or does not hold JSON.
    """
    try:
        return typ.cast("object", json.loads(path.read_bytes()))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        msg = f"{path}: {error}"
        raise _InputUnreadableError(msg) from error


def _write_staged(output: pathlib.Path, text: str) -> None:
    r"""Replace ``output`` with ``text`` only after writing it completely.

    A temporary sibling directory keeps replacement on the same filesystem, so
    an existing record survives a failure part-way through.

    Examples
    --------
    >>> with tempfile.TemporaryDirectory() as directory:
    ...     destination = pathlib.Path(directory) / "plan.txt"
    ...     _write_staged(destination, "11\ta.tar.gz\n")
    ...     print(repr(destination.read_text(encoding="utf-8")))
    '11\ta.tar.gz\n'
    """
    with tempfile.TemporaryDirectory(
        dir=output.parent, prefix=f".{output.name}."
    ) as tmp:
        staged = pathlib.Path(tmp) / output.name
        _ = staged.write_text(text, encoding="utf-8")
        _ = staged.replace(output)


def _record(output: pathlib.Path, text: str) -> int:
    """Write a result and report whether it could be recorded.

    Returns
    -------
    int
        ``EXIT_OK`` on success or ``EXIT_OUTPUT_UNWRITABLE`` on failure.
    """
    try:
        _write_staged(output, text)
    except OSError as error:
        print(f"release draft record unwritable: {error}", file=sys.stderr)
        return EXIT_OUTPUT_UNWRITABLE
    return EXIT_OK


def _render_selection(pages: object, tag: str) -> str:
    """Render the selected draft release's identifier as the record's text."""
    releases = release_github_draft_listing.releases_from_pages(pages)
    return f"{release_draft_assets.select_draft_release(releases, tag=tag)}\n"


def _render_plan(pages: object) -> str:
    """Render the planned downloads as the record's tab-separated text."""
    assets = release_draft_assets.plan_downloads(
        release_github_draft_listing.assets_from_pages(pages)
    )
    return release_download_plan.format_plan(assets)


def _record_decision(
    arguments: argparse.Namespace,
    decide: cabc.Callable[[object], str],
) -> int:
    """Decode the listing, apply ``decide``, and record the text it returns.

    Each mode supplies only its own decision. Reading the listing, separating
    the two failure classes, and the staged write are identical for both, so
    keeping them here is what stops the status codes drifting apart.

    Returns
    -------
    int
        ``EXIT_OK`` when the result was recorded, or the status naming the
        failure that stopped it.
    """
    try:
        text = decide(_read_listing(arguments.listing))
    except _InputUnreadableError as error:
        print(f"release listing unreadable: {error}", file=sys.stderr)
        return EXIT_INPUT_UNREADABLE
    except release_draft_assets.DraftAssetError as error:
        print(f"release inventory rejected: {error}", file=sys.stderr)
        return EXIT_POLICY_REJECTED
    return _record(arguments.output, text)


def _select_release(arguments: argparse.Namespace) -> int:
    """Select the draft release and record its identifier."""
    return _record_decision(
        arguments, functools.partial(_render_selection, tag=arguments.tag)
    )


def _plan_downloads(arguments: argparse.Namespace) -> int:
    """Plan the downloads and record them as identifier and name pairs."""
    return _record_decision(arguments, _render_plan)


def main(argv: list[str] | None = None) -> int:
    """Run the selected mode and return its exit code.

    Parameters
    ----------
    argv
        Command-line arguments. Defaults to ``sys.argv[1:]``.

    Returns
    -------
    int
        One of ``EXIT_OK``, ``EXIT_POLICY_REJECTED``,
        ``EXIT_INPUT_UNREADABLE``, or ``EXIT_OUTPUT_UNWRITABLE``.

    Notes
    -----
    An invalid invocation is reported as ``EXIT_INPUT_UNREADABLE`` because
    argparse already writes its own usage diagnostic and every non-success
    status here stops the release. Keeping to four statuses means the workflow
    step can classify the outcome without enumerating argparse's.

    Examples
    --------
    A missing listing is an input fault, not a policy rejection:

    >>> main(["select-release", "--listing", "/nonexistent/releases.json",
    ...       "--tag", "v1.2.3", "--output", "/dev/null"])
    2
    """
    parser = build_parser()
    try:
        arguments = parser.parse_args(argv)
    except SystemExit as error:
        return EXIT_OK if error.code == EXIT_OK else EXIT_INPUT_UNREADABLE
    match arguments.command:
        case "select-release":
            return _select_release(arguments)
        case "plan-downloads":
            return _plan_downloads(arguments)
        case other:
            print(f"unknown command {other!r}", file=sys.stderr)
            return EXIT_INPUT_UNREADABLE


if __name__ == "__main__":
    raise SystemExit(main())

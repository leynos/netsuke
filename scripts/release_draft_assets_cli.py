"""Run the draft-release decisions a workflow step needs as a command.

The release workflow holds the token and performs the network calls; this CLI
turns the listings those calls saved into the two values the next steps need:
the identifier of the draft release to verify, and the ordered plan of assets
to download. Keeping both decisions here means the shell step contains no
``jq`` expression that could disagree with :mod:`release_draft_assets`.

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
import json
import pathlib
import sys
import tempfile
import typing as typ

import release_draft_assets

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


def _format_plan(plan: cabc.Sequence[tuple[int, str]]) -> str:
    r"""Render a download plan as the tab-separated lines the step reads.

    Returns
    -------
    str
        One newline-terminated ``id<TAB>name`` line per planned asset.

    Examples
    --------
    >>> _format_plan([(11, "a.tar.gz"), (12, "b.deb")])
    '11\ta.tar.gz\n12\tb.deb\n'
    """
    return "".join(f"{identifier}\t{name}\n" for identifier, name in plan)


def _select_release(arguments: argparse.Namespace) -> int:
    """Select the draft release and record its identifier."""
    try:
        pages = _read_listing(arguments.listing)
        release_id = release_draft_assets.select_draft_release(pages, tag=arguments.tag)
    except _InputUnreadableError as error:
        print(f"release listing unreadable: {error}", file=sys.stderr)
        return EXIT_INPUT_UNREADABLE
    except release_draft_assets.DraftAssetError as error:
        print(f"release inventory rejected: {error}", file=sys.stderr)
        return EXIT_POLICY_REJECTED
    return _record(arguments.output, f"{release_id}\n")


def _plan_downloads(arguments: argparse.Namespace) -> int:
    """Plan the downloads and record them as identifier and name pairs."""
    try:
        plan = release_draft_assets.plan_downloads(_read_listing(arguments.listing))
    except _InputUnreadableError as error:
        print(f"release listing unreadable: {error}", file=sys.stderr)
        return EXIT_INPUT_UNREADABLE
    except release_draft_assets.DraftAssetError as error:
        print(f"release inventory rejected: {error}", file=sys.stderr)
        return EXIT_POLICY_REJECTED
    return _record(arguments.output, _format_plan(plan))


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

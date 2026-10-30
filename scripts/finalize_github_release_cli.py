"""Decide what a re-run should do with a draft release's existing assets.

Un-drafting makes a release public, and a release that is already public
cannot be made private again. So the decision of whether to proceed is taken
before anything is un-drafted, and it is taken from data the step records
rather than from the live release: the step lists what GitHub already has,
this module compares that against the trusted digest record, and the step
un-drafts only when the answer is that every asset is already the right one.

The record read here is the one the *build* wrote, not the one the verifier
wrote. Both carry the same shape, and for a correctly verified release they
hold the same digests, so the distinction costs nothing when everything is
right — and it is the whole check when something is wrong. Reading the
verifier's output would compare GitHub against a record derived from GitHub,
which agrees with itself however badly the upload was tampered with.

The separation is what makes the check testable. A reconciler that asked
GitHub and acted on the answer inline could only be exercised against a live
release; this one is exercised over recorded responses, including the partial
and conflicting cases that are hardest to arrange for real.

Reconciliation never clobbers. An asset whose digest differs is a conflict,
and a conflict stops the release rather than being overwritten — a published
digest is a promise to whoever already downloaded it.

An absent or unusable digest record is refused rather than read as "nothing
declared". An empty expected set plans no actions, and no actions is the same
shape as a fully verified release: the step would un-draft on no evidence at
all. The record is written by the declaration stage only after it has hashed a
non-empty declaration, so a missing one is a wiring fault, not a release with
no assets.
"""

import argparse
import json
import pathlib
import sys

import release_digest_record
import release_digest_record_files
import release_reconcile_github

EXIT_OK = 0
EXIT_CONFLICT = 1
EXIT_UPLOAD_REQUIRED = 2
EXIT_INPUT_UNREADABLE = 3
EXIT_OUTPUT_UNWRITABLE = 4

#: Every action :func:`release_reconcile_github.plan_asset_actions` can return,
#: paired with the exit code this CLI reports. Enumerated so an action added to
#: the policy module without a branch here is caught by the consistency test
#: rather than silently treated as permission to un-draft.
ACTION_EXITS = {
    "skip-verified": EXIT_OK,
    "upload": EXIT_UPLOAD_REQUIRED,
    "conflict": EXIT_CONFLICT,
}


def build_parser() -> argparse.ArgumentParser:
    """Build the parser for the release-finalizing CLI.

    Returns
    -------
    argparse.ArgumentParser
        A parser requiring the trusted digest record, the recorded asset
        listing, and a destination for the plan.
    """
    parser = argparse.ArgumentParser(
        prog="finalize_github_release_cli",
        description="Plan a draft release's reconciliation from recorded assets.",
    )
    parser.add_argument(
        "--identity",
        required=True,
        type=pathlib.Path,
        help=(
            "the trusted digest record the declaration stage wrote before "
            "upload, mapping each declared asset to the digest the build "
            "produced for it"
        ),
    )
    parser.add_argument(
        "--assets",
        required=True,
        type=pathlib.Path,
        help="JSON mapping of asset name to the digest GitHub reports",
    )
    parser.add_argument(
        "--output",
        required=True,
        type=pathlib.Path,
        help="where to write the reconciliation plan",
    )
    return parser


class InputUnreadableError(ValueError):
    """Report a digest record that cannot support a publish decision."""


def _normalise_github_digest(reported: object) -> str | None:
    """Convert GitHub's vendor digest into the policy's bare digest value.

    GitHub reports ``sha256:<hex>`` while the trusted record stores bare
    hexadecimal. Keeping that representation at this adapter boundary leaves
    the reconciliation policy independent of the provider's response format.

    Returns
    -------
    str | None
        The bare lowercase digest, or ``None`` when GitHub's value is unusable.
    """
    if not isinstance(reported, str) or not reported.startswith("sha256:"):
        return None
    digest = reported.removeprefix("sha256:").strip().lower()
    return digest or None


def _expected(identity_path: pathlib.Path) -> dict[str, str]:
    """Return the digest each declared asset must have, keyed by name.

    The declared set is the record's assets rather than the listing's, so an
    undeclared extra GitHub holds is not this function's business: the
    inventory check is what reports it.

    Parameters
    ----------
    identity_path
        The trusted digest record the declaration stage wrote before upload.

    Returns
    -------
    dict[str, str]
        Asset name to its expected bare hexadecimal digest.

    Raises
    ------
    InputUnreadableError
        If the record cannot be read or does not map names to digests in a
        shape that supports a decision.

    Notes
    -----
    The record's parsing and its every refusal live in
    :func:`release_digest_record_files.read_digest_record`, so the verifier and this
    module cannot disagree about what a usable record is. What stays here is
    the translation into :class:`InputUnreadableError`, because this CLI's exit
    codes are its own: ``EXIT_INPUT_UNREADABLE`` tells the workflow step to
    stop rather than to re-plan an upload.

    An absent or unusable record is refused rather than read as "nothing
    declared". An empty expected set plans no actions, and no actions is the
    same shape as a fully verified release: the step would un-draft on no
    evidence at all. Un-drafting cannot be undone, so absence of evidence has
    to be an error rather than a vacuous success.

    Examples
    --------
    >>> import json, tempfile
    >>> from pathlib import Path
    >>> digest = "ab" * 32
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     good = Path(tmp) / "good.json"
    ...     _ = good.write_text(
    ...         json.dumps({"assets": {"a.tar.gz": digest}}), encoding="utf-8"
    ...     )
    ...     _expected(good) == {"a.tar.gz": digest}
    True
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     empty = Path(tmp) / "empty.json"
    ...     _ = empty.write_text(json.dumps({"assets": {}}), encoding="utf-8")
    ...     try:
    ...         _expected(empty)
    ...     except InputUnreadableError:
    ...         print("refused")
    refused
    """
    try:
        return release_digest_record_files.read_digest_record(identity_path)
    except release_digest_record.DigestRecordError as error:
        raise InputUnreadableError(str(error)) from error


def main(argv: list[str] | None = None) -> int:
    """Plan the reconciliation and record it, refusing a conflict.

    Parameters
    ----------
    argv
        Command-line arguments. Defaults to ``sys.argv[1:]``.

    Returns
    -------
    int
        ``EXIT_OK`` when every asset is already verified and the release may be
        un-drafted, ``EXIT_CONFLICT`` when an asset disagrees,
        ``EXIT_UPLOAD_REQUIRED`` when assets are merely missing, or
        ``EXIT_INPUT_UNREADABLE`` when the digest record cannot support a
        decision at all, or ``EXIT_OUTPUT_UNWRITABLE`` when the reconciliation
        plan cannot be recorded.

    Examples
    --------
    A record declaring one asset GitHub has not received is an upload, not a
    conflict. ``/dev/null`` stands in for the destination, because a doctest may
    not leave a file behind:

    >>> import json, tempfile
    >>> from pathlib import Path
    >>> digest = "ab" * 32
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     identity = Path(tmp) / "identity.json"
    ...     _ = identity.write_text(
    ...         json.dumps({"assets": {"a.tar.gz": digest}}), encoding="utf-8"
    ...     )
    ...     assets = Path(tmp) / "assets.json"
    ...     _ = assets.write_text("{}", encoding="utf-8")
    ...     main(["--identity", str(identity), "--assets", str(assets),
    ...           "--output", "/dev/null"])
    2
    """
    arguments = build_parser().parse_args(argv)
    try:
        expected = _expected(arguments.identity)
    except InputUnreadableError as error:
        print(f"digest record unreadable: {error}", file=sys.stderr)
        return EXIT_INPUT_UNREADABLE
    return _reconcile_listing(expected, arguments.assets, arguments.output)


def _reconcile_listing(
    expected: dict[str, str], assets_path: pathlib.Path, output_path: pathlib.Path
) -> int:
    """Reconcile and record the GitHub asset listing for one trusted record.

    Returns
    -------
    int
        The exit code for the reconciliation plan, or
        ``EXIT_OUTPUT_UNWRITABLE`` when it cannot be recorded.

    Examples
    --------
    >>> import tempfile
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     missing = pathlib.Path(tmp) / "missing.json"
    ...     output = pathlib.Path(tmp) / "plan.json"
    ...     _reconcile_listing({}, missing, output)
    3
    """
    try:
        present = json.loads(assets_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        print(f"asset listing unreadable: {error}", file=sys.stderr)
        return EXIT_INPUT_UNREADABLE
    if not isinstance(present, dict):
        print("asset listing unreadable: expected a JSON object", file=sys.stderr)
        return EXIT_INPUT_UNREADABLE
    normalized = {
        name: _normalise_github_digest(digest) for name, digest in present.items()
    }
    actions = release_reconcile_github.plan_asset_actions(
        expected=expected, present=normalized
    )
    conflicting = release_reconcile_github.conflicts(actions)
    try:
        _write_plan(output_path, actions, conflicting)
    except OSError as error:
        print(
            f"reconciliation plan write failed: {output_path}: {error}", file=sys.stderr
        )
        output_error = error
    else:
        output_error = None
    if conflicting:
        print(
            "the release holds assets that disagree with the trusted digest record: "
            + ", ".join(conflicting),
            file=sys.stderr,
        )
    if output_error is not None:
        return EXIT_OUTPUT_UNWRITABLE
    return _verdict(actions)


def _write_plan(
    output: pathlib.Path,
    actions: tuple[tuple[str, release_reconcile_github.AssetAction], ...],
    conflicting: tuple[str, ...],
) -> None:
    """Write the reconciliation plan the job's artefacts expose.

    The plan is written for refusing runs too, so an operator sees which
    action every asset was assigned rather than only the first name that
    stopped the run. The caller maps ``Path.write_text`` failures to a distinct
    output-unwritable exit status.
    """
    output.write_text(
        json.dumps(
            {
                "actions": [[name, action.value] for name, action in actions],
                "conflicts": list(conflicting),
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )


def _verdict(
    actions: tuple[tuple[str, release_reconcile_github.AssetAction], ...],
) -> int:
    """Return the exit code for a planned action set, by stated precedence.

    Parameters
    ----------
    actions
        Output of :func:`release_reconcile_github.plan_asset_actions`.

    Returns
    -------
    int
        ``EXIT_CONFLICT`` if any asset conflicts, otherwise
        ``EXIT_UPLOAD_REQUIRED`` if any is missing, otherwise ``EXIT_OK``.

    Notes
    -----
    Written as an ordered scan rather than a ``max`` over the exit table. The
    codes are chosen so a step can branch on them, not so that their numeric
    order expresses severity, and a conflict is the more urgent finding: it
    says the release has already published something wrong, where a missing
    asset says only that a re-run has work left to do.

    Examples
    --------
    >>> from release_reconcile_github import AssetAction
    >>> _verdict((("a", AssetAction.UPLOAD), ("b", AssetAction.CONFLICT)))
    1
    >>> _verdict((("a", AssetAction.UPLOAD), ("b", AssetAction.SKIP_VERIFIED)))
    2
    >>> _verdict(())
    0
    """
    codes = {ACTION_EXITS[action.value] for _, action in actions}
    for code in (EXIT_CONFLICT, EXIT_UPLOAD_REQUIRED):
        if code in codes:
            return code
    return EXIT_OK


if __name__ == "__main__":
    raise SystemExit(main())

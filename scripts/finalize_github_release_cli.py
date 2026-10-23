"""Decide what a re-run should do with a draft release's existing assets.

Un-drafting makes a release public, and a release that is already public
cannot be made private again. So the decision of whether to proceed is taken
before anything is un-drafted, and it is taken from data the step records
rather than from the live release: the step lists what GitHub already has,
this module compares that against the verified-digest record, and the step
un-drafts only when the answer is that every asset is already the right one.

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
all. The record is written by the verifier only after it has compared a
declaration, so a missing one is a wiring fault, not a release with no assets.
"""

import argparse
import json
import pathlib
import sys

import release_reconcile_github

EXIT_OK = 0
EXIT_CONFLICT = 1
EXIT_UPLOAD_REQUIRED = 2
EXIT_INPUT_UNREADABLE = 3

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
        A parser requiring the verified-digest record, the recorded asset
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
            "the verified-digest record the inventory stage wrote, mapping "
            "each declared asset to the digest its bytes were verified against"
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


def _expected(identity_path: pathlib.Path) -> dict[str, str]:
    """Return the digest each declared asset must have, keyed by name.

    The declared set is the record's assets rather than the listing's, so an
    undeclared extra GitHub holds is not this function's business: the
    inventory check is what reports it.

    Parameters
    ----------
    identity_path
        The verified-digest record written by the inventory stage.

    Returns
    -------
    dict[str, str]
        Asset name to its expected bare hexadecimal digest.

    Raises
    ------
    InputUnreadableError
        If the file cannot be read, is not a JSON object, carries no
        ``assets`` member, or carries an ``assets`` value that is not a
        non-empty mapping of name to digest.

    Notes
    -----
    Every refusal here exists for the same reason. A ``.get("assets", {})``
    default turns a missing field into an empty plan, and an empty plan returns
    ``EXIT_OK`` — which the step reads as permission to un-draft. Un-drafting
    cannot be undone, so absence of evidence must be an error rather than a
    vacuous success. An empty mapping is refused for the same reason: a release
    with no assets cannot be correct, and the verifier only writes this record
    after comparing a non-empty declaration.

    Examples
    --------
    >>> import json, tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     good = Path(tmp) / "good.json"
    ...     _ = good.write_text(json.dumps({"assets": {"a.tar.gz": "ab"}}),
    ...                         encoding="utf-8")
    ...     _expected(good)
    {'a.tar.gz': 'ab'}
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     empty = Path(tmp) / "empty.json"
    ...     _ = empty.write_text(json.dumps({"assets": {}}), encoding="utf-8")
    ...     try:
    ...         _expected(empty)
    ...     except InputUnreadableError:
    ...         print("refused")
    refused
    """
    declared = _declared_assets(_record_object(identity_path))
    return {str(name): str(digest) for name, digest in declared.items()}


def _record_object(identity_path: pathlib.Path) -> dict[str, object]:
    """Return the digest record's top-level JSON object.

    Raises
    ------
    InputUnreadableError
        If the file cannot be read or does not hold a JSON object.
    """
    try:
        record = json.loads(identity_path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        msg = f"the digest record {identity_path} could not be read: {error}"
        raise InputUnreadableError(msg) from error
    if not isinstance(record, dict):
        msg = f"the digest record must be a JSON object, got {type(record).__name__}"
        raise InputUnreadableError(msg)
    return record


def _declared_assets(record: dict[str, object]) -> dict[object, object]:
    """Return the record's non-empty ``assets`` mapping.

    Raises
    ------
    InputUnreadableError
        If ``assets`` is absent, is not a mapping, or is empty.
    """
    declared = record.get("assets")
    if not isinstance(declared, dict):
        msg = (
            "the digest record must carry an 'assets' mapping, got "
            f"{type(declared).__name__}"
        )
        raise InputUnreadableError(msg)
    if not declared:
        msg = "the digest record declares no assets, so there is nothing to verify"
        raise InputUnreadableError(msg)
    return declared


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
        decision at all.

    Notes
    -----
    The plan is written in every case the record permits, including the
    refusing ones. An operator reading the job's artefacts sees which action
    each asset was assigned, rather than only the first name that stopped the
    run.

    ``EXIT_CONFLICT`` outranks ``EXIT_UPLOAD_REQUIRED`` when a release holds
    both kinds of finding. Taking the numeric maximum would pick the upload
    code, because ``2 > 1`` — and the step branches on the code, so the run
    would report a missing asset and skip the conflict diagnostic for a release
    whose real problem is an asset that must not be overwritten. The precedence
    is therefore stated rather than derived from the numbers, and the conflict
    message is printed whenever any conflict is present, whatever the final
    code.

    Examples
    --------
    A record declaring one asset GitHub has not received is an upload, not a
    conflict. ``/dev/null`` stands in for the destination, because a doctest may
    not leave a file behind:

    >>> import json, tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     identity = Path(tmp) / "identity.json"
    ...     _ = identity.write_text(
    ...         json.dumps({"assets": {"a.tar.gz": "ab"}}), encoding="utf-8"
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
    present = json.loads(arguments.assets.read_text(encoding="utf-8"))
    actions = release_reconcile_github.plan_asset_actions(
        expected=expected, present={str(k): v for k, v in present.items()}
    )
    conflicting = release_reconcile_github.conflicts(actions)
    _write_plan(arguments.output, actions, conflicting)
    if conflicting:
        print(
            "the release holds assets that disagree with the inventory record: "
            + ", ".join(conflicting),
            file=sys.stderr,
        )
    return _verdict(actions)


def _write_plan(
    output: pathlib.Path,
    actions: tuple[tuple[str, release_reconcile_github.AssetAction], ...],
    conflicting: tuple[str, ...],
) -> None:
    """Write the reconciliation plan the job's artefacts expose.

    The plan is written for refusing runs too, so an operator sees which
    action every asset was assigned rather than only the first name that
    stopped the run.
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

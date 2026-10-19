"""Decide what a re-run should do with a draft release's existing assets.

Un-drafting makes a release public, and a release that is already public
cannot be made private again. So the decision of whether to proceed is taken
before anything is un-drafted, and it is taken from data the step records
rather than from the live release: the step lists what GitHub already has,
this module compares that against the identity record, and the step un-drafts
only when the answer is that every asset is already the right one.

The separation is what makes the check testable. A reconciler that asked
GitHub and acted on the answer inline could only be exercised against a live
release; this one is exercised over recorded responses, including the partial
and conflicting cases that are hardest to arrange for real.

Reconciliation never clobbers. An asset whose digest differs is a conflict,
and a conflict stops the release rather than being overwritten — a published
digest is a promise to whoever already downloaded it.
"""

import argparse
import json
import pathlib
import sys

import release_reconcile_github

EXIT_OK = 0
EXIT_CONFLICT = 1
EXIT_UPLOAD_REQUIRED = 2

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
        A parser requiring the identity record, the recorded asset listing,
        and a destination for the plan.
    """
    parser = argparse.ArgumentParser(
        prog="finalize_github_release_cli",
        description="Plan a draft release's reconciliation from recorded assets.",
    )
    parser.add_argument(
        "--identity",
        required=True,
        type=pathlib.Path,
        help="the identity record the packaging stage wrote",
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


def _expected(identity_path: pathlib.Path) -> dict[str, str]:
    """Return the digest each declared asset must have, keyed by name.

    The declared set is the identity's assets rather than the listing's, so an
    undeclared extra GitHub holds is not this function's business: the
    inventory check is what reports it.

    Parameters
    ----------
    identity_path
        The identity record written at packaging time.

    Returns
    -------
    dict[str, str]
        Asset name to its expected bare hexadecimal digest.

    Raises
    ------
    TypeError
        If the identity record's ``assets`` value is not a mapping.
    """
    identity = json.loads(identity_path.read_text(encoding="utf-8"))
    declared = identity.get("assets", {})
    if not isinstance(declared, dict):
        msg = f"the identity record's assets must be a mapping, got {type(declared)}"
        raise TypeError(msg)
    return {str(name): str(digest) for name, digest in declared.items()}


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
        un-drafted, ``EXIT_UPLOAD_REQUIRED`` when assets are missing, or
        ``EXIT_CONFLICT`` when an asset disagrees.

    Notes
    -----
    The plan is written in every case, including the refusing ones. An operator
    reading the job's artefacts sees which action each asset was assigned,
    rather than only the first name that stopped the run.

    Examples
    --------
    Nothing recorded against nothing declared is a verified empty release, so
    the plan permits un-drafting. ``/dev/null`` stands in for the destination,
    because a doctest may not leave a file behind:

    >>> import json, tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     identity = Path(tmp) / "identity.json"
    ...     _ = identity.write_text(json.dumps({"assets": {}}), encoding="utf-8")
    ...     assets = Path(tmp) / "assets.json"
    ...     _ = assets.write_text("{}", encoding="utf-8")
    ...     main(["--identity", str(identity), "--assets", str(assets),
    ...           "--output", "/dev/null"])
    0
    """
    arguments = build_parser().parse_args(argv)
    expected = _expected(arguments.identity)
    present = json.loads(arguments.assets.read_text(encoding="utf-8"))
    actions = release_reconcile_github.plan_asset_actions(
        expected=expected, present={str(k): v for k, v in present.items()}
    )
    arguments.output.write_text(
        json.dumps(
            {
                "actions": [[name, action.value] for name, action in actions],
                "conflicts": list(release_reconcile_github.conflicts(actions)),
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    worst = max(
        (ACTION_EXITS[action.value] for _, action in actions),
        default=EXIT_OK,
    )
    if worst == EXIT_CONFLICT:
        print(
            "the release holds assets that disagree with the identity record: "
            + ", ".join(release_reconcile_github.conflicts(actions)),
            file=sys.stderr,
        )
    return worst


if __name__ == "__main__":
    raise SystemExit(main())

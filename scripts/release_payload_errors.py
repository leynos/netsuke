"""Name every way a staged payload can fail selection before upload.

Selection reads three independent accounts of what a release publishes: the
staging configuration and manifest, the producer records that travelled with
the downloaded artefacts, and the tree the producers actually staged. Each can
disagree with the others, and each disagreement is a different thing for an
operator to fix. Keeping the vocabulary here lets the selection policy raise a
typed refusal and the CLI map a family of them to one exit code without
matching on message text.

Messages live in the exception classes rather than at the raise sites, so a
caller composes a typed refusal and never an ad-hoc string. This module decides
nothing and imports nothing; that is what lets the policy module and its CLI
depend on it without a cycle and keeps each file under the repository's
400-line cap.

Run via ``scripts/select_release_payload_cli.py``; behavioural coverage lives in
``scripts/tests/test_release_payload_selection.py``.
"""

__all__ = [
    "ClaimedDeclarationMismatchError",
    "PayloadError",
    "PayloadRefusedError",
    "StagedFileMissingError",
    "TargetPlatformMismatchError",
]


class PayloadError(ValueError):
    """Report a staged payload that cannot be published as declared."""


class PayloadRefusedError(PayloadError):
    """Report every staged-file defect found in one pass.

    Selection defects are collected rather than raised one at a time. A tree
    missing two assets and carrying a stray third is repaired in one edit if
    the operator is told all three at once; aborting at the first would make
    them re-run the job to discover the next.
    """

    def __init__(self, offenders: list[str]) -> None:
        """Build one diagnosis naming every offender.

        Parameters
        ----------
        offenders
            Human-readable descriptions, in the order selection found them.

        Examples
        --------
        >>> refusal = PayloadRefusedError(["'a.deb' was not staged"])
        >>> str(refusal)
        "staged payload refused: 'a.deb' was not staged"
        """
        super().__init__("staged payload refused: " + "; ".join(offenders))


class StagedFileMissingError(PayloadError):
    """Report a staged path that is not a regular file below regular parents.

    The policy refuses names; this refuses *paths*. A payload copied through a
    symlinked leaf or a symlinked producer directory would hold bytes from
    outside the downloaded tree, and the trusted record would then vouch for
    files this run never received.
    """

    def __init__(self, path: object, *, expected: str) -> None:
        """Build the diagnosis naming the path and what it should have been.

        Parameters
        ----------
        path
            The staged path that was refused.
        expected
            The entry type the path was required to be, such as
            ``"a regular file"``.

        Examples
        --------
        >>> str(StagedFileMissingError("/tmp/x", expected="a regular file"))
        '/tmp/x is not a regular file'
        """
        super().__init__(f"{path} is not {expected}")


class ClaimedDeclarationMismatchError(PayloadError):
    """Report producers that do not claim exactly what the release declares."""

    def __init__(self, declared: tuple[str, ...], claimed: tuple[str, ...]) -> None:
        """Build the diagnosis naming both directions of the disagreement.

        Parameters
        ----------
        declared
            Names the configuration says the release publishes.
        claimed
            Names the producer records actually claim.

        Examples
        --------
        >>> error = ClaimedDeclarationMismatchError(("a.deb",), ("a.deb", "b.rpm"))
        >>> "b.rpm" in str(error)
        True
        """
        missing = sorted(set(declared) - set(claimed))
        extra = sorted(set(claimed) - set(declared))
        super().__init__(
            "producer claims and the declaration disagree: producers omit "
            f"{missing!r}, producers add {extra!r}"
        )


class TargetPlatformMismatchError(PayloadError):
    """Report producers whose recorded platform their target does not serve."""

    def __init__(self, offenders: list[tuple[str, str, str]]) -> None:
        """Build the diagnosis naming every disagreeing target at once.

        Parameters
        ----------
        offenders
            ``(target, recorded, configured)`` triples, one per disagreement.

        Examples
        --------
        >>> error = TargetPlatformMismatchError([("linux-x86_64", "macos", "linux")])
        >>> "'linux-x86_64' recorded 'macos'" in str(error)
        True
        >>> "serves 'linux'" in str(error)
        True
        """
        described = "; ".join(
            f"{target!r} recorded {recorded!r}, which serves {configured!r}"
            for target, recorded, configured in offenders
        )
        super().__init__(
            "producer platforms disagree with the configuration: " + described
        )

"""What the CodeScene uploader may no longer be asked for.

At the pinned revision the shared uploader's committed ``cli-manifest.json``
is the trust anchor for the CodeScene CLI archive, and the action *rejects* a
non-empty ``installer-checksum`` with a hard failure rather than ignoring it.
A workflow that still passes the repository variable therefore breaks the
upload step the moment that variable holds anything, and the variable itself
can only ever repeat the manifest's own digest.

Four concerns are asserted, each in its own test so a failure names the defect
rather than a bundle:

* no workflow passes ``installer-checksum``;
* no workflow references the ``CODESCENE_CLI_SHA256`` variable that fed it;
* every uploader reference is pinned to the one approved revision;
* the dispatch workflow that refreshed the variable does not exist.

Each test asserts over a collection whose contents are checked first. A
contract that ranges over an empty collection is satisfied by deleting the
thing it guards, so deleting the workflow directory or the upload step fails
these contracts rather than passing them.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ
from pathlib import Path

REPOSITORY_ROOT: typ.Final[Path] = Path(__file__).resolve().parents[2]
WORKFLOW_DIRECTORY: typ.Final[Path] = REPOSITORY_ROOT / ".github" / "workflows"

#: The one approved revision of the shared uploader. Asserted as an allowlist
#: rather than as a floor: ordering two commit SHAs cannot be computed from a
#: checkout, so naming the approved revision is what keeps this hermetic. It
#: fails closed on any other value, including a tag or a branch name.
APPROVED_PIN: typ.Final[str] = "a5765019912a8ab6882b12db049c7cde635f3a85"

#: Matches the revision of each uploader reference. The ``@`` separator is
#: part of the pattern so a differently owned action whose name merely starts
#: with the same text cannot match.
UPLOADER_REFERENCE: typ.Final[re.Pattern[str]] = re.compile(
    r"leynos/shared-actions/\.github/actions/upload-codescene-coverage@(\S+)"
)

#: The deprecated input. It carried the SHA-256 of an installer script the
#: action no longer downloads.
DEPRECATED_INPUT: typ.Final[str] = "installer-checksum"

#: The repository variable whose only consumer was that input.
DEPRECATED_VARIABLE: typ.Final[str] = "CODESCENE_CLI_SHA256"

#: The ``workflow_dispatch`` that hashed the installer script and wrote the
#: variable back through the API, named without an extension. Other
#: repositories in the estate carry it; this contract keeps it from arriving
#: here under either GitHub extension.
REFRESH_WORKFLOW_STEM: typ.Final[str] = "get-codescene-sha"

#: The extensions GitHub accepts for a workflow document. Every reader here
#: uses this one list, so a contract cannot range over a narrower set than the
#: one the platform actually runs.
WORKFLOW_EXTENSIONS: typ.Final[tuple[str, ...]] = (".yml", ".yaml")


def workflow_sources() -> dict[str, str]:
    """Return every workflow's source text, keyed by file name.

    Both GitHub extensions are read: a workflow written with the other one
    would otherwise escape every contract below without failing anything.

    Returns
    -------
    dict[str, str]
        File name to UTF-8 source, in file-name order.
    """
    paths = sorted(
        path
        for extension in WORKFLOW_EXTENSIONS
        for path in WORKFLOW_DIRECTORY.glob(f"*{extension}")
    )
    sources = {path.name: path.read_text(encoding="utf-8") for path in paths}
    assert sources, (
        f"no workflow was found under {WORKFLOW_DIRECTORY}, so every contract "
        "in this module would pass having read nothing"
    )
    return sources


def test_no_workflow_passes_the_deprecated_installer_checksum() -> None:
    """The uploader rejects a non-empty value, so no workflow may pass it.

    This is not tidying. The action fails its own input validation on a
    non-empty value, so the coverage upload on main stops the moment the
    variable behind the input holds anything.
    """
    offenders = sorted(
        name
        for name, source in workflow_sources().items()
        if DEPRECATED_INPUT in source
    )
    assert not offenders, (
        f"{DEPRECATED_INPUT} is deprecated and rejected by the uploader at "
        f"{APPROVED_PIN}; remove it from {', '.join(offenders)}"
    )


def test_no_workflow_references_the_deprecated_checksum_variable() -> None:
    """The variable existed only to feed the rejected input, so it must go.

    Held apart from the input contract because the two regress apart: an
    ``env`` line or a guard can name the variable in a workflow that passes no
    input at all, and such a reference is what a later reader would take as
    evidence the variable is still wanted.
    """
    offenders = sorted(
        name
        for name, source in workflow_sources().items()
        if DEPRECATED_VARIABLE in source
    )
    assert not offenders, (
        f"{DEPRECATED_VARIABLE} fed {DEPRECATED_INPUT} and has no remaining "
        f"consumer; remove it from {', '.join(offenders)}"
    )


def test_every_uploader_reference_is_pinned_to_the_approved_revision() -> None:
    """One approved revision, so a stale pin cannot reintroduce the input.

    The references are checked for content before they are checked for
    compliance. Deleting the upload step would otherwise satisfy this contract
    instead of failing it, and this repository publishes coverage from main.

    Every match is retained as its own ``(workflow, revision)`` pair rather
    than collapsed into a mapping keyed by workflow. A mapping keeps only the
    last match per file, so one workflow holding a stale reference followed by
    an approved one would satisfy a contract whose whole claim is "every
    reference".
    """
    references = [
        (name, match.group(1))
        for name, source in workflow_sources().items()
        for match in UPLOADER_REFERENCE.finditer(source)
    ]
    assert references, (
        "no upload-codescene-coverage reference was found, so the pin "
        "assertion would pass vacuously; main is expected to publish coverage"
    )
    wrong = [(name, pin) for name, pin in references if pin != APPROVED_PIN]
    assert not wrong, (
        f"every upload-codescene-coverage reference must be pinned to "
        f"{APPROVED_PIN}; found {wrong}"
    )


def test_the_checksum_refresh_workflow_is_absent() -> None:
    """Nothing reads the variable it would write, so it is dead code here.

    Asserted against the filesystem rather than the parsed workflows: a
    dispatch-only workflow appears in no job or step list any other contract
    reads, so its absence is the only property that can be stated.

    Both extensions are checked. A real refresh workflow written as ``.yaml``
    would also fail the variable contract above, because it names the
    variable, but this clause must not lean on that: a placeholder of that
    name which references nothing is exactly the shape this clause exists to
    catch, and under one extension only it would have passed.
    """
    present = [
        f"{REFRESH_WORKFLOW_STEM}{extension}"
        for extension in WORKFLOW_EXTENSIONS
        if (WORKFLOW_DIRECTORY / f"{REFRESH_WORKFLOW_STEM}{extension}").exists()
    ]
    assert not present, (
        f"{present} maintains {DEPRECATED_VARIABLE}, which no workflow reads; "
        "delete it rather than keeping a dispatch that writes an unread "
        "repository variable"
    )

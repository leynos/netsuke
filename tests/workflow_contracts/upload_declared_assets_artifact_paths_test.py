"""Hold each platform's artifact upload paths to the record its producer wrote.

A producer record that never leaves the runner is worse than no record: the
release's download step then finds no claims, and every target is reported
missing. The producer step's own cases assert *where* it writes, and the
publishing cases assert the tree they are handed round-trips. Neither connects
the two, so a platform whose upload paths stopped covering the record's
directory would keep every existing case green while publishing a tree no
producer vouched for.

These cases close that seam structurally, over the manifest as written. Each
platform's upload step is read from ``build-and-package.yml``, every ``path:``
entry is resolved the way a runner would resolve it, and the producer's own
record directory -- derived by the same helper the producer cases use -- must
lie at or below one of them.

Run via ``make test-workflow-contracts``.
"""

import pathlib
import re

import pytest
import upload_declared_assets_producer_env as producer_env
import upload_declared_assets_producer_tree as tree
import upload_declared_assets_staging as staging
from upload_declared_assets_staging import Facts, Target
from workflow_loading import (
    PACKAGE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
)

#: The job in ``build-and-package.yml`` that owns both the producer step and
#: the artifact uploads.
BUILD_JOB = "build"

#: One artifact-upload step per platform, named as the workflow names it.
UPLOAD_STEPS = {
    "linux": "Upload Linux artefacts",
    "windows": "Upload Windows artefacts",
    "macos": "Upload macOS artefacts",
}

#: Every ``${{ ... }}`` span a path entry can carry.
_EXPRESSION = re.compile(r"\$\{\{[^}]*\}\}")


@pytest.fixture(name="facts", scope="module")
def facts_fixture() -> Facts:
    """Return the release facts every case is bound to."""
    return staging.release_facts()


def _steps() -> list[dict[str, object]]:
    """Return the build job's steps, which hold all three uploads."""
    return job_steps(load_workflow(PACKAGE_WORKFLOW_PATH), BUILD_JOB)


def _paths(step: dict[str, object]) -> tuple[str, ...]:
    """Return one upload step's ``path:`` entries, whatever form they take.

    Returns
    -------
    tuple[str, ...]
        The entries, in the order the workflow lists them.

    Notes
    -----
    ``actions/upload-artifact`` accepts a single string or a list, and either
    may name several paths: a YAML block scalar parses as one string whose
    newlines separate the entries, while a sequence parses as a list. The
    manifest uses all three shapes, so a reader that handled only one would
    silently drop the paths the others declare -- and the entry that carries
    the record is exactly what such a reader would lose.
    """
    declared = require_mapping(step.get("with"), "upload step inputs").get("path")
    if isinstance(declared, str):
        entries = declared.splitlines()
    elif isinstance(declared, list):
        entries = [str(entry) for entry in declared]
    else:
        pytest.fail(f"an upload step must declare a path, got {declared!r}")
    return tuple(entry.strip() for entry in entries if entry.strip())


def _interpolate(entry: str, *, facts: Facts, target: Target) -> pathlib.PurePosixPath:
    """Resolve every expression in one path entry, as a runner would.

    Returns
    -------
    pathlib.PurePosixPath
        The entry with its expressions substituted, kept POSIX because the
        workflow's runners are.

    Notes
    -----
    The expressions are resolved by the producer step's own resolver rather
    than a second table here. A value the producer's environment resolves one
    way must not resolve another way in the path that has to carry the record
    it wrote, and two tables would let exactly that happen unnoticed.
    """
    resolved = _EXPRESSION.sub(
        lambda match: producer_env.resolve(match.group(0), facts=facts, target=target),
        entry,
    )
    return pathlib.PurePosixPath(resolved)


def _covers(entry: pathlib.PurePosixPath, record_dir: pathlib.PurePosixPath) -> bool:
    """Report whether uploading ``entry`` carries what ``record_dir`` holds.

    Returns
    -------
    bool
        True when the record directory is ``entry`` itself or lies below it.

    Notes
    -----
    A directory entry carries everything below it; a file entry carries only
    itself. Comparing path parts rather than string prefixes is what keeps
    ``dist/win`` from reading as covering ``dist/win-other``.
    """
    return record_dir.parts[: len(entry.parts)] == entry.parts


@pytest.mark.parametrize("target", staging.release_facts().targets, ids=lambda t: t.key)
def test_the_upload_carries_the_record_its_producer_wrote(
    facts: Facts, target: Target
) -> None:
    """Upload a path that covers the directory the producer recorded into.

    This is the claim the whole provenance chain rests on. A record the upload
    does not carry is a record publication never reads, and the release then
    reports the target missing rather than the record mislaid -- which sends
    the operator to the wrong job. The record's own directory is derived by
    the helper the producer cases assert against, so a platform whose step
    changed where it writes is held to the upload rather than to a copy of the
    expectation here.
    """
    step = named_step(_steps(), UPLOAD_STEPS[target.platform])
    declared = _paths(step)
    record_dir = tree.record_location(target, facts)
    assert record_dir.is_relative_to(staging.DIST), (
        f"{target.key} must record below the uploaded root, got {record_dir}"
    )
    covered = [
        entry
        for entry in (_interpolate(raw, facts=facts, target=target) for raw in declared)
        if _covers(entry, pathlib.PurePosixPath(record_dir.as_posix()))
    ]
    assert covered, (
        f"{target.platform} upload path(s) {declared} do not cover the record "
        f"directory {record_dir}"
    )


@pytest.mark.parametrize("platform", sorted(UPLOAD_STEPS))
def test_the_upload_runs_only_on_its_own_platform(platform: str) -> None:
    """Gate each upload on the platform whose tree it names.

    The three steps sit in one job, so an ungated one would run on every
    platform and try to upload paths the others never create. A step gated on
    the wrong platform is quieter still: it succeeds on a platform whose tree
    happens to exist and never runs for the one it was written for, so the
    gate is compared rather than merely required.
    """
    gate = str(named_step(_steps(), UPLOAD_STEPS[platform]).get("if", ""))
    assert f"inputs.platform == '{platform}'" in gate, (
        f"{UPLOAD_STEPS[platform]} must be gated on its own platform, got {gate!r}"
    )

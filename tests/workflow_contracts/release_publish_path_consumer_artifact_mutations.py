"""Mutate the consumer smoke's evidence downloads to prove they are refused.

Each applier names one plausible regression in how the smoke receives the
package record and the trusted record, and applies it to a deep copy. They live
apart from the other smoke mutations because they answer a different question:
the wiring mutations ask whether the job depends on, waits for, and guards the
right stages, while these ask whether the two downloads that carry the evidence
are still the ones the action reads.

What each regression must break is stated in
``release_publish_path_consumer_expectations``, alongside the claims for every
other smoke mutation.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from release_publish_path_consumer_mutation_support import (
    ConsumerSmokeMutationError,
    ConsumerSmokeMutationIssue,
    Job,
    smoke_job,
    steps,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

DOWNLOAD_PREFIX = "actions/download-artifact@"


def _first_download(release: Job) -> dict[str, object]:
    """Return the job's first artefact download, which every mutation edits.

    Returns
    -------
    dict[str, object]
        The download step, mutated in place by the caller.

    Raises
    ------
    ConsumerSmokeMutationError
        If the job downloads no artefact, so the mutation would change nothing
        the contract reads.
    """
    for step in steps(smoke_job(release)):
        if str(step.get("uses", "")).startswith(DOWNLOAD_PREFIX):
            return step
    raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_DOWNLOAD)


def _select_pattern_artifact(release: Job, _action: Job) -> None:
    """Replace one exact artefact download with a wildcard pattern download."""
    _first_download(release)["with"] = {"pattern": "release-*", "path": "downloaded"}


def _redirect_artifact_destination(release: Job, _action: Job) -> None:
    """Send one permitted artefact to a directory the action never reads.

    The artefact name, the action inputs, and the other download all stay as
    reviewed, so only a contract that ties each destination to the file the
    action opens notices the verifier will read a path that never arrived.

    Raises
    ------
    ConsumerSmokeMutationError
        If the download passes no inputs to retarget.
    """
    step = _first_download(release)
    values = step.get("with")
    if not isinstance(values, dict):
        raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_DOWNLOAD)
    values["path"] = "elsewhere"


def _drop_artifact_download(release: Job, _action: Job) -> None:
    """Remove one artefact download, leaving the other exact and in place.

    The surviving download still names its artefact and passes every check made
    of a download that exists, so only a contract that requires both to be
    present notices the evidence the smoke no longer receives.

    Raises
    ------
    ConsumerSmokeMutationError
        If the job downloads no artefact, so the mutation would change nothing
        the contract reads.
    """
    declared = steps(smoke_job(release))
    for index, step in enumerate(declared):
        if str(step.get("uses", "")).startswith(DOWNLOAD_PREFIX):
            del declared[index]
            return
    raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_DOWNLOAD)


def _disable_artifact_download(release: Job, _action: Job) -> None:
    """Leave one evidence download named and placed but never run.

    Its name, its destination, and the other download all stay as reviewed, so
    only a contract that reads the step's condition notices the action will
    open a directory this download never filled.
    """
    _first_download(release)["if"] = "false"


def _delay_artifact_download(release: Job, _action: Job) -> None:
    """Move one evidence download after the smoke that reads its bytes.

    The download still runs, names its artefact, and lands where the input
    reads it, so only a contract that compares the two steps' order notices the
    verifier opens a path nothing has written yet.

    Raises
    ------
    ConsumerSmokeMutationError
        If the job downloads no artefact, so the mutation would change nothing
        the contract reads.
    """
    declared = steps(smoke_job(release))
    for index, step in enumerate(declared):
        if str(step.get("uses", "")).startswith(DOWNLOAD_PREFIX):
            declared.append(declared.pop(index))
            return
    raise ConsumerSmokeMutationError(ConsumerSmokeMutationIssue.MISSING_DOWNLOAD)


ARTIFACT_MUTATIONS: dict[str, cabc.Callable[[Job, Job], None]] = {
    "select-consumer-smoke-pattern-artifact": _select_pattern_artifact,
    "redirect-consumer-smoke-artifact": _redirect_artifact_destination,
    "drop-consumer-smoke-artifact-download": _drop_artifact_download,
    "disable-consumer-smoke-artifact-download": _disable_artifact_download,
    "delay-consumer-smoke-artifact-download": _delay_artifact_download,
}


__all__ = ["ARTIFACT_MUTATIONS"]

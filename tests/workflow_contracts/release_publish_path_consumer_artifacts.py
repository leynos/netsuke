"""Hold the consumer smoke's two evidence downloads to their exact shape.

The smoke receives the candidate's package record and the publisher's trusted
record from workflow artefact storage; everything else it reads comes from the
public hosts. A download that took a wildcard, landed somewhere the action
never opens, was disabled, or ran after the smoke had already read its inputs
would leave the verifier judging less than the release it reports on, while the
download step's name and inputs still read as correct.

Presence alone is therefore not enough: each permitted artefact must be
downloaded by its exact name, into the directory the action's input reads, with
no condition, and before the step that calls the action. The rest of the job's
shape lives in ``release_publish_path_consumer_shape``, which owns the low-level
readers and the constants naming the documents.

Run via ``make test-workflow-contracts``.
"""

from release_publish_path_consumer_shape import (
    CONSUMER_SMOKE_JOB,
    DOWNLOAD_ARTIFACT_PREFIX,
    LOCAL_SMOKE_ACTION,
    SMOKE_ACTION_INPUTS,
    SMOKE_ARTIFACT_INPUTS,
    SMOKE_ARTIFACTS,
    steps_of,
)


def check_smoke_artifacts(job: dict[str, object], violations: list[str]) -> None:
    """Require every permitted artefact to be downloaded by its exact name.

    A download that is present must name its artefact and no pattern. A
    permitted artefact with no usable download is refused too: the smoke binds
    the crate and the archive to that evidence, so a run missing half of it
    would verify less than the release it reports on. A download counts only
    when it runs and runs in time: a conditional download is refused outright,
    and one declared after the smoke invocation is refused because the action
    would already have read nothing.
    """
    steps = steps_of(job)
    smoke_index = _smoke_invocation_index(steps)
    named: set[object] = set()
    for index, step in enumerate(steps):
        if not str(step.get("uses", "")).startswith(DOWNLOAD_ARTIFACT_PREFIX):
            continue
        name = _require_named_artifact(step, violations)
        if name is None:
            continue
        if smoke_index is not None and index >= smoke_index:
            # The action reads its inputs when it runs, so a download declared
            # after it leaves the verifier opening a path nothing has written
            # yet while the download step itself still reads as correct.
            violations.append(
                f"{CONSUMER_SMOKE_JOB}.artifacts: {name!r} must be downloaded "
                "before the smoke reads it"
            )
            continue
        named.add(name)
    violations.extend(
        f"{CONSUMER_SMOKE_JOB}.artifacts: nothing downloads {name}, so the "
        "smoke would run without its evidence"
        for name in sorted(SMOKE_ARTIFACTS - named)
    )


def _smoke_invocation_index(steps: list[dict[str, object]]) -> int | None:
    """Return where the job calls the smoke action, or ``None`` when it does not.

    Returns
    -------
    int | None
        The index of the local-action step, so a download can be required to
        precede it. A missing invocation is reported by its own check; this one
        stays silent rather than naming the same fault twice.
    """
    return next(
        (
            index
            for index, step in enumerate(steps)
            if str(step.get("uses", "")) == LOCAL_SMOKE_ACTION
        ),
        None,
    )


def _require_named_artifact(
    step: dict[str, object], violations: list[str]
) -> object | None:
    """Require one download to name its artefact and its destination.

    Returns
    -------
    object | None
        The artefact name when the download is the one this contract permits,
        so the caller counts only downloads that could actually stand as
        evidence, or ``None`` when the download is refused here.
    """
    if "if" in step:
        # A conditional download is not evidence: the job guard is the only
        # condition allowed to decide what the smoke reads, and a step that
        # skipped itself would leave the action reading a directory never
        # filled while every name and destination still matched.
        violations.append(
            f"{CONSUMER_SMOKE_JOB}.artifacts: a download must run whenever "
            f"the job does, got if: {step['if']!r}"
        )
        return None
    values = step.get("with")
    if not isinstance(values, dict):
        violations.append(
            f"{CONSUMER_SMOKE_JOB}.artifacts: every download must name its artefact"
        )
        return None
    if "pattern" in values:
        violations.append(
            f"{CONSUMER_SMOKE_JOB}.artifacts: a pattern download would let "
            "artefact storage stand in for the public download"
        )
    name = values.get("name")
    if str(name) not in SMOKE_ARTIFACTS:
        violations.append(
            f"{CONSUMER_SMOKE_JOB}.artifacts: {name!r} is not "
            "one of the permitted artefact names"
        )
        return None
    destination = _artifact_destination(name)
    if values.get("path") != destination:
        violations.append(
            f"{CONSUMER_SMOKE_JOB}.artifacts: {name!r} must be downloaded to "
            f"{destination!r}, where the action's input reads it, got "
            f"{values.get('path')!r}"
        )
    return name


def _artifact_destination(name: object) -> str:
    """Return the directory the action reads one permitted artefact from.

    Returns
    -------
    str
        The first path segment of the input that reads the artefact.
    """
    input_name = SMOKE_ARTIFACT_INPUTS[str(name)]
    return SMOKE_ACTION_INPUTS[input_name].split("/", maxsplit=1)[0]


__all__ = ["check_smoke_artifacts"]

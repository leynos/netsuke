"""Contract tests separating the PR ratchet from the main coverage upload.

Pull requests generate coverage and compare it with the ratcheted baseline;
they do not publish coverage artefacts or contact CodeScene. The main workflow
generates the matching report, advances the ratchet baseline, and uploads that
authoritative report to CodeScene. These tests pin both halves of that split.

The negative half is matched structurally rather than by retired name: a
renamed step, a differently spelled artefact path, or a resurrected privileged
consumer must still fail. The detectors are driven against synthetic workflow
text as well, so a detector that stopped matching cannot make the repository
assertion pass by finding nothing. Shared parsing helpers live in
``workflow_loading.py``.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from workflow_loading import (
    COVERAGE_MAIN_WORKFLOW_PATH,
    COVERAGE_PR_WORKFLOW_PATH,
    REPO_ROOT,
    all_workflow_documents,
    job_steps,
    load_workflow,
    named_step,
    parse_workflow_text,
    require_mapping,
    unique_step_index,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

COVERAGE_STEP = "Test and Measure Coverage"
PR_COVERAGE_ARTEFACT_STEP = "Upload PR coverage artefact"
CODESCENE_UPLOAD_STEP = "Upload coverage data to CodeScene"

WORKFLOWS_DIRECTORY = REPO_ROOT / ".github" / "workflows"

GENERATE_COVERAGE_ACTION = "leynos/shared-actions/.github/actions/generate-coverage@"
UPLOAD_COVERAGE_ACTION = (
    "leynos/shared-actions/.github/actions/upload-codescene-coverage@"
)
PUBLISH_ARTEFACT_ACTION = "actions/upload-artifact"

CREDENTIAL_ENVIRONMENT_KEY = "CS_ACCESS_TOKEN"
COVERAGE_REPORT_PATH = "lcov.info"

PULL_REQUEST_TRIGGER = "pull_request"
SUBMISSION_TRIGGER = "workflow_run"


def action_of(step: dict[str, object]) -> str:
    """Return a step's action reference without its version.

    Splitting on the version separator rather than matching a prefix keeps
    ``upload-codescene-coverage-legacy`` from reading as the real action.

    Parameters
    ----------
    step : dict[str, object]
        One parsed workflow step.

    Returns
    -------
    str
        The action reference without its version, or the empty string when
        the step runs a command instead of an action.
    """
    uses = step.get("uses")
    return uses.split("@", 1)[0] if isinstance(uses, str) else ""


def declares_trigger(document: dict[str, object], trigger: str) -> bool:
    """Return whether a parsed workflow declares the given trigger.

    Parameters
    ----------
    document : dict[str, object]
        One parsed workflow document.
    trigger : str
        The trigger name, such as ``pull_request``.

    Returns
    -------
    bool
        True when the workflow declares the trigger in any of the scalar,
        sequence, or mapping forms ``on:`` accepts.
    """
    triggers = document.get("on")
    if isinstance(triggers, str):
        return triggers == trigger
    if isinstance(triggers, list):
        return trigger in triggers
    if isinstance(triggers, dict):
        return trigger in triggers
    return False


def steps_in_all_jobs(document: dict[str, object]) -> list[dict[str, object]]:
    """Return every step of every job in one parsed workflow.

    Parameters
    ----------
    document : dict[str, object]
        One parsed workflow document.

    Returns
    -------
    list[dict[str, object]]
        Every step mapping, in declaration order. A job without a step list
        contributes nothing rather than failing, because this is a scan for
        prohibited references and not an assertion about job shape.
    """
    jobs = require_mapping(document.get("jobs"), "jobs")
    steps: list[dict[str, object]] = []
    for name, declaration in jobs.items():
        job = require_mapping(declaration, f"job {name}")
        raw_steps = job.get("steps", [])
        if isinstance(raw_steps, list):
            steps.extend(step for step in raw_steps if isinstance(step, dict))
    return steps


def _iter_strings(value: object) -> cabc.Iterator[str]:
    """Yield every string nested anywhere in a parsed YAML value."""
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for key, item in value.items():
            yield from _iter_strings(key)
            yield from _iter_strings(item)
    elif isinstance(value, list):
        for item in value:
            yield from _iter_strings(item)


def _publishes_the_coverage_report(step: dict[str, object]) -> bool:
    """Return whether a step publishes the coverage report as an artefact."""
    if action_of(step) != PUBLISH_ARTEFACT_ACTION:
        return False
    with_ = step.get("with")
    if not isinstance(with_, dict) or "path" not in with_:
        # A publish step naming no path uploads the workspace, which holds the
        # generated report. Fail closed rather than read it as an exemption.
        return True
    return COVERAGE_REPORT_PATH in str(with_["path"])


def coverage_surface_offenders(
    name: str, document: dict[str, object], raw_text: str
) -> list[str]:
    """Return every prohibited coverage-surface reference in one workflow.

    Parameters
    ----------
    name : str
        The workflow file's name, used in failure messages.
    document : dict[str, object]
        The workflow's parsed document.
    raw_text : str
        The workflow's raw text. The credential is matched here as well as in
        the parsed values, so a reference inside a comment or an unparsed
        shape is still reported.

    Returns
    -------
    list[str]
        One description per violation, empty when the workflow is clean.
    """
    offenders = [
        f"{name}: step {index} publishes the coverage report as an artefact"
        for index, step in enumerate(steps_in_all_jobs(document))
        if _publishes_the_coverage_report(step)
    ]
    offenders.extend(
        f"{name}: step {index} invokes the CodeScene coverage action"
        for index, step in enumerate(steps_in_all_jobs(document))
        if action_of(step) == UPLOAD_COVERAGE_ACTION.split("@", 1)[0]
    )
    if CREDENTIAL_ENVIRONMENT_KEY in raw_text:
        offenders.append(f"{name}: raw text references {CREDENTIAL_ENVIRONMENT_KEY}")
    offenders.extend(
        f"{name}: parsed value references {CREDENTIAL_ENVIRONMENT_KEY}"
        for value in _iter_strings(document)
        if CREDENTIAL_ENVIRONMENT_KEY in value
    )
    return offenders


def _pull_request_workflows() -> list[tuple[str, dict[str, object], str]]:
    """Return name, document, and raw text for every PR-triggered workflow."""
    documents = all_workflow_documents(WORKFLOWS_DIRECTORY)
    return [
        (name, document, (WORKFLOWS_DIRECTORY / name).read_text(encoding="utf-8"))
        for name, document in sorted(documents.items())
        if declares_trigger(document, PULL_REQUEST_TRIGGER)
    ]


def _assert_with_inputs(
    step: dict[str, object], description: str, expected: dict[str, object]
) -> None:
    """Validate that a step's ``with`` block supplies the expected inputs."""
    with_ = require_mapping(step.get("with"), f"{description}'s with block")
    actual = {key: with_.get(key) for key in expected}
    assert actual == expected, f"{description} must pass {expected!r}, got {actual!r}"


def test_pr_coverage_stays_local_and_uses_the_main_ratchet() -> None:
    """Keep pull-request coverage inside CI and compare it with main."""
    steps = job_steps(load_workflow(), "build-test")
    coverage_step = named_step(steps, COVERAGE_STEP)
    _assert_with_inputs(
        coverage_step,
        COVERAGE_STEP,
        {
            "language": "rust",
            "output-path": "lcov.info",
            "format": "lcov",
            "with-ratchet": "true",
        },
    )
    assert not [
        step for step in steps if step.get("name") == PR_COVERAGE_ARTEFACT_STEP
    ], "pull requests must not publish their coverage report"
    assert not COVERAGE_PR_WORKFLOW_PATH.exists(), (
        "pull requests must not have a privileged CodeScene submission workflow"
    )


def test_no_pull_request_workflow_touches_the_coverage_publication_surface() -> None:
    """Keep publication, CodeScene, and the credential out of PR workflows."""
    workflows = _pull_request_workflows()
    assert workflows, (
        "no pull-request-triggered workflow was read, so this contract would "
        "pass without examining anything"
    )
    offenders = [
        offender
        for name, document, raw_text in workflows
        for offender in coverage_surface_offenders(name, document, raw_text)
    ]
    assert not offenders, (
        "pull requests must not publish coverage or reach the CodeScene "
        f"credential: {offenders!r}"
    )


def test_no_pr_reachable_workflow_run_consumer_touches_that_surface() -> None:
    """Forbid a resurrected privileged coverage consumer under any file name."""
    documents = all_workflow_documents(WORKFLOWS_DIRECTORY)
    offenders = [
        offender
        for name, document in sorted(documents.items())
        if declares_trigger(document, SUBMISSION_TRIGGER)
        for offender in coverage_surface_offenders(
            name, document, (WORKFLOWS_DIRECTORY / name).read_text(encoding="utf-8")
        )
    ]
    assert not offenders, (
        "no workflow_run consumer may publish coverage or reach the CodeScene "
        f"credential: {offenders!r}"
    )


def _synthetic_document(text: str) -> tuple[dict[str, object], str]:
    """Parse synthetic workflow text into a document and its raw text."""
    parsed = parse_workflow_text(text, "synthetic workflow")
    assert isinstance(parsed, dict), "synthetic workflow must parse to a mapping"
    return parsed, text


def test_the_detectors_report_renamed_publishers_and_secret_references() -> None:
    """Fail the detectors against shapes the retired files no longer have."""
    renamed, renamed_text = _synthetic_document(
        """
on: pull_request
jobs:
  build-test:
    steps:
      - name: Publish measurements
        uses: actions/upload-artifact@v4
        with:
          name: something-else-entirely
          path: lcov.info
"""
    )
    assert _publishes_the_coverage_report(steps_in_all_jobs(renamed)[0]), (
        "a renamed publisher must still be detected by its action and path"
    )
    assert coverage_surface_offenders("synthetic.yml", renamed, renamed_text), (
        "a renamed publisher must produce an offender"
    )

    codescene, codescene_text = _synthetic_document(
        """
on: pull_request
jobs:
  build-test:
    steps:
      - name: Measure
        uses: leynos/shared-actions/.github/actions/upload-codescene-coverage@abc123
"""
    )
    assert coverage_surface_offenders("synthetic.yml", codescene, codescene_text), (
        "a CodeScene coverage invocation must be detected under any step name"
    )

    secret, secret_text = _synthetic_document(
        """
on: pull_request
jobs:
  build-test:
    steps:
      - name: Measure
        env:
          CS_ACCESS_TOKEN: ${{ secrets.CS_ACCESS_TOKEN }}
        run: make test
"""
    )
    assert coverage_surface_offenders("synthetic.yml", secret, secret_text), (
        "a credential reference must be detected in parsed values"
    )
    assert coverage_surface_offenders(
        "synthetic.yml", {"on": "pull_request", "jobs": {}}, secret_text
    ), "a credential reference must be detected in raw text alone"


def test_a_clean_synthetic_workflow_produces_no_offenders() -> None:
    """Keep the detectors from firing on a workflow inside the boundary."""
    clean, clean_text = _synthetic_document(
        """
on: pull_request
jobs:
  build-test:
    steps:
      - name: Test and Measure Coverage
        uses: leynos/shared-actions/.github/actions/generate-coverage@abc123
        with:
          with-ratchet: 'true'
      - name: Show sccache statistics
        run: sccache --show-stats
"""
    )
    assert not coverage_surface_offenders("synthetic.yml", clean, clean_text)


def test_main_coverage_upload_reads_the_generated_lcov_report() -> None:
    """Main uploads the LCOV report it produces before calling CodeScene."""
    steps = job_steps(load_workflow(COVERAGE_MAIN_WORKFLOW_PATH), "coverage-upload")
    coverage_index = unique_step_index(steps, COVERAGE_STEP)
    upload_index = unique_step_index(steps, CODESCENE_UPLOAD_STEP)
    assert coverage_index < upload_index, (
        "main must generate coverage before uploading it to CodeScene"
    )

    coverage_step = steps[coverage_index]
    upload_step = steps[upload_index]
    assert str(coverage_step.get("uses", "")).startswith(GENERATE_COVERAGE_ACTION), (
        "main coverage production must use generate-coverage, got "
        f"{coverage_step.get('uses')!r}"
    )
    assert str(upload_step.get("uses", "")).startswith(UPLOAD_COVERAGE_ACTION), (
        "main coverage upload must use upload-codescene-coverage, got "
        f"{upload_step.get('uses')!r}"
    )

    _assert_with_inputs(
        coverage_step,
        "main coverage production",
        {"language": "rust", "output-path": "lcov.info", "format": "lcov"},
    )
    _assert_with_inputs(
        upload_step,
        "main CodeScene upload",
        {"path": "lcov.info", "format": "lcov"},
    )

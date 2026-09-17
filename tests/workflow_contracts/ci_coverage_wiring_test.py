"""Contract tests separating the PR ratchet from the main coverage upload.

Pull requests generate coverage and compare it with the ratcheted baseline;
they do not publish coverage artefacts or contact CodeScene. The main workflow
generates the matching report, advances the ratchet baseline, and uploads that
authoritative report to CodeScene. These tests pin both halves of that split.

The negative half is matched structurally rather than by retired name: a
renamed step, a differently spelled artefact path, or a resurrected privileged
consumer must still fail. Both pull-request triggers are read, because
`pull_request_target` runs in the base repository's context and can read its
secrets. The detectors are driven against synthetic workflow text as well, so a
detector that stopped matching cannot make the repository assertion pass by
finding nothing. Shared parsing helpers live in ``workflow_loading.py`` and the
predicates under test live in ``ci_coverage_wiring_invariants.py``.

Run via ``make test-workflow-contracts``.
"""

from ci_coverage_wiring_invariants import (
    COVERAGE_REPORT_PATH,
    CREDENTIAL_ENVIRONMENT_KEY,
    GENERATE_COVERAGE_ACTION,
    PUBLICATION_OPT_OUT_INPUT,
    PUBLICATION_OPT_OUT_VALUE,
    PUBLISH_ARTEFACT_ACTION,
    PULL_REQUEST_TARGET_TRIGGER,
    PULL_REQUEST_TRIGGER,
    SUBMISSION_TRIGGER,
    UPLOAD_COVERAGE_ACTION,
    coverage_surface_offenders,
    declares_trigger,
    publishes_the_coverage_report,
    steps_in_all_jobs,
)
from timeout_budgets import WORKFLOWS_DIRECTORY
from workflow_loading import (
    COVERAGE_MAIN_WORKFLOW_PATH,
    COVERAGE_PR_WORKFLOW_PATH,
    all_workflow_documents,
    job_steps,
    load_workflow,
    named_step,
    parse_workflow_text,
    require_mapping,
    unique_step_index,
)

COVERAGE_STEP = "Test and Measure Coverage"
PR_COVERAGE_ARTEFACT_STEP = "Upload PR coverage artefact"
CODESCENE_UPLOAD_STEP = "Upload coverage data to CodeScene"


def _pull_request_workflows() -> list[tuple[str, dict[str, object], str]]:
    """Return name, document, and raw text for every PR-reachable workflow.

    Both pull-request triggers are read. `pull_request_target` runs in the base
    repository's context and can read its secrets, so a coverage step there
    would be the more serious variant of the same violation rather than an
    unrelated one.

    Returns
    -------
    list[tuple[str, dict[str, object], str]]
        One entry per PR-reachable workflow, in file-name order, as the file
        name, its parsed document, and its raw text.
    """
    documents = all_workflow_documents(WORKFLOWS_DIRECTORY)
    return [
        (name, document, _workflow_text(name))
        for name, document in sorted(documents.items())
        if declares_trigger(document, PULL_REQUEST_TRIGGER)
        or declares_trigger(document, PULL_REQUEST_TARGET_TRIGGER)
    ]


def _workflow_text(name: str) -> str:
    """Return the raw text of one workflow file by name."""
    return (WORKFLOWS_DIRECTORY / name).read_text(encoding="utf-8")


def _assert_with_inputs(
    step: dict[str, object], description: str, expected: dict[str, object]
) -> None:
    """Validate that a step's ``with`` block supplies the expected inputs."""
    with_ = require_mapping(step.get("with"), f"{description}'s with block")
    actual = {key: with_.get(key) for key in expected}
    assert actual == expected, f"{description} must pass {expected!r}, got {actual!r}"


def _synthetic_document(text: str) -> tuple[dict[str, object], str]:
    """Parse synthetic workflow text into a document and its raw text."""
    parsed = parse_workflow_text(text, "synthetic workflow")
    assert isinstance(parsed, dict), "synthetic workflow must parse to a mapping"
    return parsed, text


def test_pr_coverage_stays_local_and_uses_the_main_ratchet() -> None:
    """Keep pull-request coverage inside CI and compare it with main."""
    steps = job_steps(load_workflow(), "build-test")
    coverage_step = named_step(steps, COVERAGE_STEP)
    _assert_with_inputs(
        coverage_step,
        COVERAGE_STEP,
        {
            "language": "rust",
            "output-path": COVERAGE_REPORT_PATH,
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
    assert any(
        declares_trigger(document, PULL_REQUEST_TARGET_TRIGGER)
        for _, document, _ in workflows
    ), (
        "no pull_request_target workflow was read, so the variant that can read "
        "base-repository secrets would escape this contract"
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
        for offender in coverage_surface_offenders(name, document, _workflow_text(name))
    ]
    assert not offenders, (
        "no workflow_run consumer may publish coverage or reach the CodeScene "
        f"credential: {offenders!r}"
    )


def test_the_detectors_report_renamed_publishers_and_secret_references() -> None:
    """Fail the detectors against shapes the retired files no longer have."""
    renamed, renamed_text = _synthetic_document(
        f"""
on: pull_request
jobs:
  build-test:
    steps:
      - name: Publish measurements
        uses: {PUBLISH_ARTEFACT_ACTION}@v4
        with:
          name: something-else-entirely
          path: {COVERAGE_REPORT_PATH}
"""
    )
    assert publishes_the_coverage_report(steps_in_all_jobs(renamed)[0]), (
        "a renamed publisher must still be detected by its action and path"
    )
    assert coverage_surface_offenders("synthetic.yml", renamed, renamed_text), (
        "a renamed publisher must produce an offender"
    )

    codescene, codescene_text = _synthetic_document(
        f"""
on: pull_request
jobs:
  build-test:
    steps:
      - name: Measure
        uses: {UPLOAD_COVERAGE_ACTION}@abc123
"""
    )
    assert coverage_surface_offenders("synthetic.yml", codescene, codescene_text), (
        "a CodeScene coverage invocation must be detected under any step name"
    )

    secret, secret_text = _synthetic_document(
        f"""
on: pull_request
jobs:
  build-test:
    steps:
      - name: Measure
        env:
          {CREDENTIAL_ENVIRONMENT_KEY}: ${{{{ secrets.{CREDENTIAL_ENVIRONMENT_KEY} }}}}
        run: make test
"""
    )
    assert coverage_surface_offenders("synthetic.yml", secret, secret_text), (
        "a credential reference must be detected in parsed values"
    )
    assert coverage_surface_offenders(
        "synthetic.yml", {"on": "pull_request", "jobs": {}}, secret_text
    ), "a credential reference must be detected in raw text alone"


def test_the_prong_reads_every_pull_request_trigger_the_parser_accepts() -> None:
    """Read both PR triggers, in each spelling `on:` accepts.

    `declares_trigger` decides which workflows the prong examines, so a form it
    failed to read would leave that whole workflow unexamined. Both triggers
    are checked because the base-repository variant is the more serious one to
    miss. The scalar, sequence, and mapping spellings are each parsed from real
    YAML text rather than hand-built, so this also holds the reader's error
    resolution of the `on` key.
    """
    for trigger in (PULL_REQUEST_TRIGGER, PULL_REQUEST_TARGET_TRIGGER):
        for source in (
            f"on: {trigger}\n",
            f"on: [{trigger}]\n",
            f"on:\n  {trigger}:\n",
        ):
            document, _text = _synthetic_document(source + "jobs: {}\n")
            assert declares_trigger(document, trigger), (
                f"{trigger} written as {source.strip()!r} must be read"
            )

    # A near miss must not be read as its longer relative. `pull_request`
    # is a prefix of `pull_request_target`, and a substring test would
    # accept one for the other.
    for source in ("on: pull_request\n", "on: [pull_request]\n"):
        document, _text = _synthetic_document(source + "jobs: {}\n")
        assert not declares_trigger(document, PULL_REQUEST_TARGET_TRIGGER), (
            f"{source.strip()!r} must not read as {PULL_REQUEST_TARGET_TRIGGER}"
        )


def test_a_clean_synthetic_workflow_produces_no_offenders() -> None:
    """Keep the detectors from firing on a workflow inside the boundary."""
    clean, clean_text = _synthetic_document(
        f"""
on: pull_request
jobs:
  build-test:
    steps:
      - name: Test and Measure Coverage
        uses: {GENERATE_COVERAGE_ACTION}@abc123
        with:
          with-ratchet: 'true'
          {PUBLICATION_OPT_OUT_INPUT}: '{PUBLICATION_OPT_OUT_VALUE}'
      - name: Show sccache statistics
        run: sccache --show-stats
"""
    )
    assert not coverage_surface_offenders("synthetic.yml", clean, clean_text), (
        "a workflow inside the boundary must produce no offender"
    )


def test_the_pull_request_lane_declines_the_actions_own_archive() -> None:
    """Require the PR lane to opt out of the coverage action's archive step.

    The action archives the report it generated under a step of its own, and a
    composite action offers no other way to withhold it. Because that step
    belongs to the action rather than the workflow, no scanner over this
    repository's steps can see it: the boundary is only observable as the opt
    out the caller passes. Without this assertion the prohibition above holds
    while the report is still published on every pull request.
    """
    coverage_step = named_step(job_steps(load_workflow(), "build-test"), COVERAGE_STEP)
    _assert_with_inputs(
        coverage_step,
        COVERAGE_STEP,
        {PUBLICATION_OPT_OUT_INPUT: PUBLICATION_OPT_OUT_VALUE},
    )


def test_the_detector_reports_a_coverage_call_that_keeps_the_archive() -> None:
    """Fail closed when the opt out is absent, misspelled, or on another step.

    The rejections are the ways a caller reaches the archive while looking as
    though it had opted out: omitting the input, supplying a value the action
    does not compare against, and opting out on one coverage step while a
    second still archives.
    """
    omitting, omitting_text = _synthetic_document(
        f"""
on: pull_request
jobs:
  build-test:
    steps:
      - name: Test and Measure Coverage
        uses: {GENERATE_COVERAGE_ACTION}@abc123
        with:
          with-ratchet: 'true'
"""
    )
    assert coverage_surface_offenders("synthetic.yml", omitting, omitting_text), (
        "a coverage call that omits the opt out must be reported"
    )

    for value in ("true", "False", "no", ""):
        wrong, wrong_text = _synthetic_document(
            f"""
on: pull_request
jobs:
  build-test:
    steps:
      - name: Test and Measure Coverage
        uses: {GENERATE_COVERAGE_ACTION}@abc123
        with:
          {PUBLICATION_OPT_OUT_INPUT}: '{value}'
"""
        )
        assert coverage_surface_offenders("synthetic.yml", wrong, wrong_text), (
            f"{PUBLICATION_OPT_OUT_INPUT}: {value!r} must not read as an opt out; "
            "the action compares against its own spelling"
        )

    # The opt out is a property of the step that archives, so a workflow is not
    # clean merely because some step carries it. A detector that read the
    # workflow as a whole would pass this while the second call published.
    partly, partly_text = _synthetic_document(
        f"""
on: pull_request
jobs:
  build-test:
    steps:
      - name: Test and Measure Coverage
        uses: {GENERATE_COVERAGE_ACTION}@abc123
        with:
          {PUBLICATION_OPT_OUT_INPUT}: '{PUBLICATION_OPT_OUT_VALUE}'
      - name: Measure Python Coverage
        uses: {GENERATE_COVERAGE_ACTION}@abc123
        with:
          with-ratchet: 'true'
"""
    )
    offenders = coverage_surface_offenders("synthetic.yml", partly, partly_text)
    assert len(offenders) == 1, (
        f"exactly the second coverage call must be reported, got {offenders!r}"
    )


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
        {"language": "rust", "output-path": COVERAGE_REPORT_PATH, "format": "lcov"},
    )
    _assert_with_inputs(
        upload_step,
        "main CodeScene upload",
        {"path": COVERAGE_REPORT_PATH, "format": "lcov"},
    )

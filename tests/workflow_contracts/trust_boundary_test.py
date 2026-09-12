"""Hold the PR coverage secret boundary and poisoning-regression harness.

The untrusted CI run can write ``BASH_ENV`` or ``GITHUB_PATH`` into its own
per-job environment files. This structural harness proves it exports only the
LCOV artefact, while the trusted ``workflow_run`` starts another job on a fresh
runner, consumes no persisted shell state, and exposes the secret only to its
submission action after validation has completed.
"""

import copy
import typing as typ

import pytest
from python_action_dispatch import assert_python_action_dispatch
from trust_boundary_invariants import (
    CREDENTIAL_ENVIRONMENT_KEY,
    REQUIRED_SECRET_JOB_PERMISSIONS,
    TOKEN_PRESENCE_GUARD,
    TRUSTED_CHECKOUT_REF,
    contains_text,
    is_isolated_secret_job,
    job_references_secret,
)
from workflow_loading import (
    COVERAGE_PR_WORKFLOW_PATH,
    REPO_ROOT,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    workflow_job,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

UNTRUSTED_CI_PATH = REPO_ROOT / ".github" / "workflows" / "ci.yml"
ARTEFACT_NAME = "pr-coverage-lcov"
ARTEFACT_PATH = "lcov.info"
SUBMISSION_STEP = "Check coverage against CodeScene gates"
REPORT_STEP = "Report CodeScene coverage gate"
ACTION_SCRIPT_PATH = REPO_ROOT / ".github" / "scripts" / "coverage_pr_submission.py"
REPORTING_SCRIPT_PATH = REPO_ROOT / ".github" / "scripts" / "coverage_pr_reporting.py"
EXPECTED_SUBMISSION_CONDITION = (
    "github.event.workflow_run.conclusion == 'success' && "
    "github.event.workflow_run.event == 'pull_request' && "
    "github.event.workflow_run.head_repository.full_name == github.repository"
)
EXPECTED_EXCLUDED_FORK_CONDITION = (
    "github.event.workflow_run.conclusion == 'success' && "
    "github.event.workflow_run.event == 'pull_request' && "
    "github.event.workflow_run.head_repository.full_name != github.repository"
)
EXPECTED_PR_ARTEFACT_STEP = {
    "name": "Upload PR coverage artefact",
    "if": "github.event_name == 'pull_request'",
    "uses": "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
    "with": {
        "name": ARTEFACT_NAME,
        "path": ARTEFACT_PATH,
        "retention-days": 3,
        "if-no-files-found": "error",
    },
}


EXPECTED_FORK_TELEMETRY_ENV = {
    "STARTED_AT_MS": "${{ steps.start_report_excluded_fork.outputs.started_at_ms }}",
    "OUTCOME": "${{ steps.report_excluded_fork.outcome }}",
    "ORIGINATING_WORKFLOW_RUN_ID": "${{ github.event.workflow_run.id }}",
    "ORIGINATING_COMMIT_SHA": "${{ github.event.workflow_run.head_sha }}",
    "NETSUKE_CODESCENE_COVERAGE_METRICS_FILE": "${{ runner.temp }}"
    "/codescene-pr-coverage-metrics.jsonl",
    "NETSUKE_CODESCENE_COVERAGE_TRACES_FILE": "${{ runner.temp }}"
    "/codescene-pr-coverage-traces.jsonl",
    "OPERATION": "codescene-check-run-publication",
}


def _workflow_paths() -> list[Path]:
    """Return every checked-in GitHub Actions workflow path."""
    return sorted((REPO_ROOT / ".github" / "workflows").glob("*.yml"))


def _is_pull_request_workflow(workflow: dict[str, object]) -> bool:
    """Return whether one parsed workflow has a pull-request trigger."""
    triggers = require_mapping(workflow.get("on"), "workflow trigger")
    return "pull_request" in triggers


def _pull_request_workflow_jobs() -> list[tuple[str, str, dict[str, object]]]:
    """Return ``(workflow, job, declaration)`` for pull-request workflow jobs."""
    jobs: list[tuple[str, str, dict[str, object]]] = []
    for workflow_path in _workflow_paths():
        workflow = load_workflow(workflow_path)
        if not _is_pull_request_workflow(workflow):
            continue
        declarations = require_mapping(workflow.get("jobs"), "the workflow jobs")
        jobs.extend(
            (
                workflow_path.name,
                str(job_name),
                require_mapping(declaration, f"the {job_name} job"),
            )
            for job_name, declaration in declarations.items()
        )
    return jobs


def _pull_request_checkouts() -> list[tuple[str, str, dict[str, object]]]:
    """Return ``(workflow, job, step)`` for every pull-request checkout step."""
    # A job that delegates to a reusable workflow declares no step list.
    return [
        (workflow_name, job_name, step)
        for workflow_name, job_name, declaration in _pull_request_workflow_jobs()
        if isinstance(steps := declaration.get("steps"), list)
        for step in steps
        if "actions/checkout@" in str(step.get("uses", ""))
    ]


def test_pull_request_workflows_never_reference_codescene_secret() -> None:
    """Forbid the secret in parsed values and raw PR-workflow text alike."""
    for workflow_path in _workflow_paths():
        workflow = load_workflow(workflow_path)
        if not _is_pull_request_workflow(workflow):
            continue
        assert not contains_text(workflow, CREDENTIAL_ENVIRONMENT_KEY), (
            f"{workflow_path.name} must not reference "
            f"{CREDENTIAL_ENVIRONMENT_KEY} in parsed values"
        )
        assert CREDENTIAL_ENVIRONMENT_KEY not in workflow_path.read_text(
            encoding="utf-8"
        ), (
            f"{workflow_path.name} must not reference "
            f"{CREDENTIAL_ENVIRONMENT_KEY} in raw YAML"
        )


def test_pull_request_workflows_never_persist_checkout_credentials() -> None:
    """Keep the workflow token out of PR-controlled git configuration.

    Every job in a pull-request-triggered workflow runs code the pull request
    controls, and ``actions/checkout`` writes an authenticated
    ``http.extraheader`` into the job's ``.git/config`` when it persists
    credentials. A later untrusted step could then push with the workflow
    token, so each checkout in those workflows must opt out explicitly. The
    untrusted ``build-test`` job is named because it is the job that produces
    the coverage artefact this trust boundary consumes.
    """
    checkouts = _pull_request_checkouts()
    assert any(
        workflow_name == UNTRUSTED_CI_PATH.name and job_name == "build-test"
        for workflow_name, job_name, _ in checkouts
    ), f"{UNTRUSTED_CI_PATH.name} must be covered by this checkout contract"
    for workflow_name, job_name, step in checkouts:
        with_ = require_mapping(
            step.get("with"), f"{workflow_name} {job_name} checkout's with block"
        )
        assert with_.get("persist-credentials") is False, (
            f"{workflow_name} job {job_name} must set persist-credentials: false"
        )


def test_pull_request_checkouts_ignore_jobs_without_a_step_list(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Ignore reusable and malformed jobs that declare no actionable steps."""
    workflow_path = tmp_path / "synthetic.yml"
    workflow_path.write_text(
        "on:\n  pull_request:\n"
        "jobs:\n"
        "  reusable:\n    uses: owner/repo/.github/workflows/x.yml@main\n"
        "  malformed:\n    steps: not-a-list\n",
        encoding="utf-8",
    )
    monkeypatch.setattr(f"{__name__}._workflow_paths", lambda: [workflow_path])

    assert _pull_request_checkouts() == [], "no checkout from a job without steps"


def test_submission_workflow_uses_trusted_workflow_run_boundary() -> None:
    """Require completed CI PR runs to cross into a distinct trusted workflow."""
    workflow = load_workflow(COVERAGE_PR_WORKFLOW_PATH)
    triggers = require_mapping(workflow.get("on"), "coverage submission trigger")
    workflow_run = require_mapping(triggers.get("workflow_run"), "workflow_run trigger")
    assert workflow_run == {"workflows": ["CI"], "types": ["completed"]}, (
        "submission must run only after completed CI"
    )

    job = workflow_job(workflow, "submit-coverage")
    condition = str(job.get("if", ""))
    assert condition == EXPECTED_SUBMISSION_CONDITION, (
        "submission eligibility must require successful same-repository PR CI"
    )
    assert job.get("permissions") == REQUIRED_SECRET_JOB_PERMISSIONS, (
        "submission must retain least-privilege permissions"
    )


def test_submission_workflow_reports_excluded_forks_neutrally() -> None:
    """Publish the required Check Run without downloading fork-controlled data."""
    workflow = load_workflow(COVERAGE_PR_WORKFLOW_PATH)
    job = workflow_job(workflow, "report-excluded-fork")
    assert job.get("if") == EXPECTED_EXCLUDED_FORK_CONDITION, (
        "the excluded-fork report must cover only successful fork PR CI runs"
    )
    assert job.get("permissions") == {"checks": "write"}, (
        "the excluded-fork report must have only Check Run permission"
    )
    steps = job_steps(workflow, "report-excluded-fork")
    assert not any(
        "actions/checkout@" in str(step.get("uses", "")) for step in steps
    ), "the excluded-fork report must not check out PR content"
    assert not any(
        "download-artifact@" in str(step.get("uses", "")) for step in steps
    ), "the excluded-fork report must not download the hostile artefact"
    assert not job_references_secret(steps, CREDENTIAL_ENVIRONMENT_KEY), (
        "the excluded-fork report must not receive the CodeScene credential"
    )
    report = named_step(steps, "Report excluded fork CodeScene coverage gate")
    script = "\n".join([
        ACTION_SCRIPT_PATH.read_text(encoding="utf-8"),
        REPORTING_SCRIPT_PATH.read_text(encoding="utf-8"),
    ])
    assert_python_action_dispatch(
        report,
        ".github/scripts/coverage_pr_submission.py",
        "report-excluded-fork",
    )
    for required_fragment in (
        '"name": CHECK_RUN_NAME',
        '"head_sha": value("ORIGINATING_COMMIT_SHA")',
        '"external_id": value("ORIGINATING_WORKFLOW_RUN_ID")',
        "reporting.fork_summary(",
    ):
        assert required_fragment in script, (
            "the excluded-fork report must retain "
            f"{required_fragment!r} in its trusted reporting path"
        )


def test_excluded_fork_publication_records_only_bounded_telemetry() -> None:
    """Record fixed-label telemetry around the excluded-fork Check Run."""
    workflow = load_workflow(COVERAGE_PR_WORKFLOW_PATH)
    steps = job_steps(workflow, "report-excluded-fork")
    start = named_step(steps, "Start excluded fork Check Run publication telemetry")
    report = named_step(steps, "Report excluded fork CodeScene coverage gate")
    observe = named_step(steps, "Record excluded fork Check Run publication telemetry")

    assert [step.get("name") for step in steps] == [
        start["name"],
        report["name"],
        observe["name"],
    ], "fork telemetry must bracket the Check Run publication"
    for step, module_name, command in (
        (start, "coverage_pr_submission.py", "start-telemetry"),
        (report, "coverage_pr_submission.py", "report-excluded-fork"),
        (observe, "coverage_pr_submission_observability.py", "record-telemetry"),
    ):
        assert_python_action_dispatch(step, f".github/scripts/{module_name}", command)
    assert observe.get("if") == "always()", (
        "fork telemetry must record a failed publication too"
    )
    assert observe.get("env") == EXPECTED_FORK_TELEMETRY_ENV, (
        "the fork telemetry must carry only bounded, fixed correlation fields"
    )


def test_secret_job_checks_out_only_trusted_tooling_and_validates_first() -> None:
    """Keep untrusted code and artefact bytes out of secret-bearing execution."""
    workflow = load_workflow(COVERAGE_PR_WORKFLOW_PATH)
    job = workflow_job(workflow, "submit-coverage")
    steps = job_steps(workflow, "submit-coverage")
    assert is_isolated_secret_job(job, steps), (
        "the CodeScene job must isolate its step-local credential"
    )

    checkout_steps = [
        step for step in steps if "actions/checkout@" in str(step.get("uses", ""))
    ]
    assert len(checkout_steps) == 1, "the trusted job must have one checkout"
    checkout_with = require_mapping(checkout_steps[0].get("with"), "trusted checkout")
    assert checkout_with.get("ref") == TRUSTED_CHECKOUT_REF, (
        "the trusted job must check out the default branch only"
    )

    names = [step.get("name") for step in steps]
    validation_index = names.index("Validate hostile coverage artefact")
    submission_index = names.index(SUBMISSION_STEP)
    assert validation_index < submission_index, (
        "hostile artefact validation must precede the secret-bearing action"
    )

    submission = steps[submission_index]
    assert submission.get("if") == TOKEN_PRESENCE_GUARD, (
        "the submission must skip when the credential is absent"
    )
    assert submission.get("env") == {
        CREDENTIAL_ENVIRONMENT_KEY: "${{ secrets.CS_ACCESS_TOKEN }}"
    }, "the credential must be supplied only through the step environment"
    assert not job.get("env"), "the CodeScene secret must not be job scoped"


def test_poisoned_untrusted_environment_cannot_cross_to_submission_runner() -> None:
    """Keep BASH_ENV and GITHUB_PATH poisoning inside the untrusted runner."""
    ci_workflow = load_workflow(UNTRUSTED_CI_PATH)
    ci_steps = job_steps(ci_workflow, "build-test")
    assert not job_references_secret(ci_steps, CREDENTIAL_ENVIRONMENT_KEY), (
        "untrusted PR CI must not receive the CodeScene credential"
    )

    artefact_steps = [
        step for step in ci_steps if step.get("name") == "Upload PR coverage artefact"
    ]
    assert artefact_steps == [EXPECTED_PR_ARTEFACT_STEP], (
        "untrusted CI may export only the bounded LCOV artefact"
    )

    trusted_text = COVERAGE_PR_WORKFLOW_PATH.read_text(encoding="utf-8")
    for poisoned_name in ("BASH_ENV", "GITHUB_PATH"):
        assert poisoned_name not in trusted_text, (
            f"trusted workflow must not consume untrusted {poisoned_name} state"
        )
    assert "workflow_run" in trusted_text, "submission must start in a fresh workflow"


def test_submission_report_uses_the_checked_in_outcome_seam() -> None:
    """Require the Check Run to use the local, testable outcome decision."""
    workflow = load_workflow(COVERAGE_PR_WORKFLOW_PATH)
    report = named_step(job_steps(workflow, "submit-coverage"), REPORT_STEP)

    outcome_module = ACTION_SCRIPT_PATH
    assert outcome_module.is_file(), "the Check Run outcome seam must be checked in"
    script = "\n".join([
        outcome_module.read_text(encoding="utf-8"),
        REPORTING_SCRIPT_PATH.read_text(encoding="utf-8"),
    ])
    assert "def coverage_conclusion(" in script, (
        "the checked-in outcome seam must export the conclusion function"
    )
    assert_python_action_dispatch(
        report,
        ".github/scripts/coverage_pr_submission.py",
        "report-coverage",
    )
    for required_fragment in (
        "coverage_conclusion(",
        '"head_sha": value("ORIGINATING_COMMIT_SHA")',
        '"external_id": value("ORIGINATING_WORKFLOW_RUN_ID")',
    ):
        assert required_fragment in script, (
            "the final Check Run must retain "
            f"{required_fragment!r} in its trusted reporting path"
        )


@pytest.mark.parametrize(
    ("job_name", "step_name", "expected_command", "wrong_command"),
    [
        pytest.param(
            "submit-coverage",
            REPORT_STEP,
            "report-coverage",
            "report-excluded-fork",
            id="same-repository-report",
        ),
        pytest.param(
            "report-excluded-fork",
            "Report excluded fork CodeScene coverage gate",
            "report-excluded-fork",
            "report-coverage",
            id="excluded-fork-report",
        ),
    ],
)
def test_reporting_dispatch_rejects_a_command_mentioned_outside_sys_argv(
    job_name: str,
    step_name: str,
    expected_command: str,
    wrong_command: str,
) -> None:
    """Reject reporting scripts whose argv command differs from their contract."""
    workflow = load_workflow(COVERAGE_PR_WORKFLOW_PATH)
    steps = copy.deepcopy(job_steps(workflow, job_name))
    report = named_step(steps, step_name)
    report["run"] = (
        "import runpy\nimport sys\nsys.argv = [\n"
        '    ".github/scripts/coverage_pr_submission.py",\n'
        f'"{wrong_command}", "{expected_command}"]\n'
        'runpy.run_path(sys.argv[0], run_name="__main__")'
    )

    with pytest.raises(AssertionError):
        assert_python_action_dispatch(
            report, ".github/scripts/coverage_pr_submission.py", expected_command
        )

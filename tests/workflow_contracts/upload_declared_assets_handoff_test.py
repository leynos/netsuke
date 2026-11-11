"""Hold the action's composite output mapping and the publication failure step.

The upload step never fails its own job: it reports a conflict or a transport
fault through ``upload-error`` and ``error-message``, and ``publish-release``
raises them afterwards so the telemetry step still records what happened. That
split means the two halves can drift apart silently -- the step can write an
output the composite does not map, or the workflow can read a name the mapping
never exports -- and every other execution case would stay green because it
reads ``GITHUB_OUTPUT`` directly rather than through the mapping.

These cases close that seam. Everything is resolved from ``publish-release``
rather than from step names searched across the whole workflow, because the
staging job ``release`` carries a same-named upload step that calls a shared
action; the case that pins the job's ``uses:`` is what keeps the rest honest.
From there the cases run the action's real steps, evaluate the real
``outputs:`` expressions in ``action.yml`` against what the run wrote, resolve
the failure step's own ``if:`` and ``env:`` expressions, and execute its script
with those values. The success half asserts the failure step is skipped; the
conflict half asserts it exits non-zero and prints the uploader's diagnostic.

Run via ``make test-workflow-contracts``.
"""

import pathlib
import re

# The case's whole point is to execute the step's own script and observe its
# exit status, so the module is needed; the script is read from this
# repository's release workflow, never from a run's output.
import subprocess  # ruff: ignore[suspicious-subprocess-import]

from sccache_statistics_condition_test import evaluate
from upload_declared_assets_harness import Release, execute
from upload_declared_assets_manifest import (
    ACTION,
    UPLOAD_STEP,
    action_document,
    manifest_step_id,
)
from upload_declared_assets_staging import (
    recorded_digests,
    release_facts,
    stage_workspace,
)
from workflow_loading import job_steps, load_workflow, named_step, require_mapping

#: The workflow that calls the action and raises its deferred error.
RELEASE_WORKFLOW = pathlib.Path(".github/workflows/release.yml")

#: The job that performs the real publication. The staging job ``release``
#: carries a same-named pair of steps that calls a shared action, so resolving
#: by step name alone would test the wrong job's hand-off.
PUBLISH_JOB = "publish-release"

#: The action the publication job must call.
UPLOAD_ACTION = "./.github/actions/upload-declared-assets"

#: The workflow's name for the step that calls the action, and the step that
#: reports its refusal. These are the *workflow's* names: the action's own
#: writing step carries a different name, which the manifest module owns.
WORKFLOW_UPLOAD_STEP = "Upload artefacts to release"
FAILURE_STEP = "Check asset upload errors"

#: The composite outputs the failure step reads.
ERROR_OUTPUT = "upload-error"
MESSAGE_OUTPUT = "error-message"

#: The workflow's id for the step that calls the action.
UPLOAD_STEP_ID = "upload_assets"

#: The only composite ``value:`` form the mapping may use, capturing the step
#: id and output name so both halves of the claim are checked.
_STEP_OUTPUT = r"steps\.([\w-]+)\.outputs\.([\w-]+)"


def _composite_outputs() -> dict[str, str]:
    """Return the action's ``outputs:`` expressions, keyed by output name.

    Returns
    -------
    dict[str, str]
        Output name mapped to the ``${{ ... }}`` expression the manifest
        exports it from.
    """
    document = action_document()
    declared = require_mapping(document.get("outputs"), f"{ACTION}.outputs")
    return {
        name: str(require_mapping(value, f"{ACTION}.outputs.{name}")["value"])
        for name, value in declared.items()
    }


def _resolve(
    expression: str,
    step_outputs: dict[str, dict[str, str]],
    expected_step: str,
) -> str:
    """Resolve one ``${{ steps.<id>.outputs.<name> }}`` expression.

    Only that form is accepted. A literal would mean the declaration stopped
    reading a step output, which is exactly the defect these cases exist to
    catch, so the case fails rather than reading a value nothing wrote.

    The id the expression names is what selects the outputs: the expression's
    own step is looked up in ``step_outputs``, and ``expected_step`` is then
    asserted against it. ``expected_step`` is supplied by the caller because
    the action's writing step and the workflow's calling step carry different
    ids for the same outputs, and each caller is asserting its own relationship
    rather than a shared constant.

    Returns
    -------
    str
        The value GitHub would substitute. A name the named step did not write
        resolves to an empty string, exactly as it would on a runner.
    """
    text = expression.strip().removeprefix("${{").removesuffix("}}").strip()
    match = re.fullmatch(_STEP_OUTPUT, text)
    assert match is not None, (
        f"the expression must read a step output, got {expression!r}"
    )
    step, name = match.groups()
    assert step == expected_step, (
        f"the expression must read {expected_step}'s outputs, got {step!r}"
    )
    assert step in step_outputs, (
        f"the expression reads steps.{step}.outputs, but no step with that id "
        "ran; the outputs it names do not exist"
    )
    return step_outputs[step].get(name, "")


def _mapped(step_outputs: dict[str, dict[str, str]]) -> dict[str, str]:
    """Return what the caller sees, after the composite mapping.

    Each ``outputs:`` expression is resolved against the outputs of the step
    its own id names, not against a step id this case chose. That is the claim
    GitHub enforces: a manifest that renamed the writing step, or whose
    expressions named an id no step carried, would substitute an empty string
    -- and an empty ``upload-error`` reads as "no error", so a refused upload
    would publish silently.

    Returns
    -------
    dict[str, str]
        The mapped composite outputs.
    """
    # The id is read from the manifest rather than assumed, so renaming the
    # writing step fails here instead of silently resolving nothing.
    writing_step = manifest_step_id(UPLOAD_STEP)
    mapped = {
        name: _resolve(expr, step_outputs, writing_step)
        for name, expr in _composite_outputs().items()
    }
    assert set(mapped) == {ERROR_OUTPUT, MESSAGE_OUTPUT}, (
        "the composite must export exactly the deferred error pair, got "
        f"{sorted(mapped)}"
    )
    return mapped


def _publication_steps() -> list[dict[str, object]]:
    """Return ``publish-release``'s steps, in order.

    Resolving the job by name is the point: ``release`` carries a same-named
    upload step that calls a shared action, so a case that searched the whole
    workflow for ``Upload artefacts to release`` could pass against the
    staging job and never touch the publication hand-off.

    Returns
    -------
    list[dict[str, object]]
        The publication job's steps, in declaration order.
    """
    workflow = load_workflow(pathlib.Path(RELEASE_WORKFLOW))
    return job_steps(workflow, PUBLISH_JOB)


def _failure_step_condition() -> str:
    """Return the ``if:`` that guards the publication failure step.

    Returns
    -------
    str
        The guard expression, exactly as the workflow declares it.
    """
    step = named_step(_publication_steps(), FAILURE_STEP)
    condition = step.get("if")
    assert isinstance(condition, str), "the failure step must carry an if: guard"
    return condition


def _failure_message_expression() -> str:
    """Return the ``env:`` expression that supplies the printed diagnostic.

    Reading the expression rather than restating its meaning keeps the case
    honest: if the workflow started sourcing the message from another step or
    output, the assertion below would fail instead of passing on a literal the
    case supplied itself.

    Returns
    -------
    str
        The ``${{ ... }}`` expression bound to ``ERROR_MESSAGE``.
    """
    step = named_step(_publication_steps(), FAILURE_STEP)
    environment = require_mapping(step.get("env"), f"{FAILURE_STEP} environment")
    expression = environment.get("ERROR_MESSAGE")
    assert isinstance(expression, str), (
        "the failure step must read its diagnostic from the environment"
    )
    return expression


def _run_failure_step(mapped: dict[str, str]) -> subprocess.CompletedProcess[str]:
    """Execute the failure step's own script with the mapped outputs.

    ``mapped`` is what the *workflow's* calling step exposes, so the failure
    step's ``env:`` expression is resolved against that step's id rather than
    the action's internal one.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The finished process, so a case can assert on its status and output.
    """
    step = named_step(_publication_steps(), FAILURE_STEP)
    script = step.get("run")
    assert isinstance(script, str), "the failure step must carry its own script"
    environment = {
        "PATH": "/usr/bin:/bin",
        "ERROR_MESSAGE": _resolve(
            _failure_message_expression(),
            {UPLOAD_STEP_ID: mapped},
            UPLOAD_STEP_ID,
        ),
    }
    # The argv is fixed and the script comes from this repository's own
    # workflow, so there is no untrusted input to check here.
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true]
        ["/bin/bash", "--noprofile", "--norc", "-eo", "pipefail", "-c", script],
        env=environment,
        capture_output=True,
        text=True,
        check=False,
    )


def _guard_holds(mapped: dict[str, str], *, job_failed: bool = False) -> bool:
    """Report whether the failure step's own guard fires for these outputs."""
    outputs = {(UPLOAD_STEP_ID, name): value for name, value in mapped.items()}
    return evaluate(_failure_step_condition(), outputs, job_failed=job_failed)


def test_the_publication_job_calls_the_declared_asset_action() -> None:
    """Invoke this repository's action from the job that really publishes.

    ``release`` carries a same-named step that calls a shared third-party
    action, so a suite that matched by step name alone could resolve the
    staging job and never notice that the publication job had stopped calling
    the new action. Naming both the job and the ``uses:`` here is what makes
    every other case in this module about the publication hand-off.
    """
    upload = named_step(_publication_steps(), WORKFLOW_UPLOAD_STEP)
    assert upload.get("uses") == UPLOAD_ACTION, (
        f"{PUBLISH_JOB} must call {UPLOAD_ACTION}, got {upload.get('uses')!r}"
    )


def test_the_composite_exports_both_outputs_this_workflow_reads() -> None:
    """Export the deferred error pair, which is all the failure step names.

    The step reads ``upload-error`` in its guard and ``error-message`` in its
    environment. A manifest that renamed either, or stopped exporting one,
    would leave the workflow reading an empty string -- and an empty
    ``upload-error`` reads as "no error", so a conflict would pass the
    publication silently. Comparing the exported set exactly is what keeps
    that from being a mapping detail nobody notices.
    """
    assert set(_composite_outputs()) == {ERROR_OUTPUT, MESSAGE_OUTPUT}, (
        "the composite must export exactly the outputs the workflow consumes"
    )
    step = named_step(_publication_steps(), FAILURE_STEP)
    assert ERROR_OUTPUT in str(step.get("if", "")), (
        "the guard must read the mapped error output"
    )
    assert _failure_message_expression().find(MESSAGE_OUTPUT) >= 0, (
        "the failure step must print the mapped diagnostic"
    )


def test_a_successful_upload_exports_no_error_and_skips_the_failure_step(
    tmp_path: pathlib.Path,
) -> None:
    """Leave the publication green when the uploader reconciled everything."""
    facts = release_facts()
    workspace = stage_workspace(tmp_path, facts)
    run = execute(tmp_path, workspace, facts)
    mapped = _mapped(run.step_outputs)
    assert mapped[ERROR_OUTPUT] == "false", mapped[MESSAGE_OUTPUT]
    assert not mapped[MESSAGE_OUTPUT], "a successful upload must carry no diagnostic"
    assert not _guard_holds(mapped), (
        "a success must not trigger the publication failure step"
    )


def test_a_conflict_exports_the_diagnostic_and_fails_publication(
    tmp_path: pathlib.Path,
) -> None:
    """Fail the publication with the uploader's own diagnostic on a conflict."""
    facts = release_facts()
    workspace = stage_workspace(tmp_path, facts)
    offender = min(recorded_digests(workspace))
    attached = [{"name": offender, "digest": f"sha256:{'0' * 64}"}]
    run = execute(tmp_path, workspace, facts, release=Release(attached=tuple(attached)))

    mapped = _mapped(run.step_outputs)
    assert mapped[ERROR_OUTPUT] == "true", "a conflict must surface through the mapping"
    assert offender in mapped[MESSAGE_OUTPUT], mapped[MESSAGE_OUTPUT]
    assert _guard_holds(mapped), "a conflict must trigger the publication failure step"

    finished = _run_failure_step(mapped)
    assert finished.returncode != 0, "the failure step must fail the job"
    assert offender in finished.stdout, (
        f"the job log must carry the uploader's diagnostic, got {finished.stdout!r}"
    )


def test_the_failure_step_prints_the_diagnostic_verbatim(
    tmp_path: pathlib.Path,
) -> None:
    r"""Print the mapped diagnostic unchanged, whatever it contains.

    The step reads the message from the environment and prints it with
    ``printf '%s\\n'``, so the message is data rather than script. This case
    holds that: every asset name a conflict can name is printed as it stands,
    even though the names carry characters that a shell would otherwise act
    on.
    """
    facts = release_facts()
    workspace = stage_workspace(tmp_path, facts)
    offender = min(recorded_digests(workspace))
    attached = [{"name": offender, "digest": f"sha256:{'0' * 64}"}]
    run = execute(tmp_path, workspace, facts, release=Release(attached=tuple(attached)))

    mapped = _mapped(run.step_outputs)
    finished = _run_failure_step(mapped)
    assert mapped[MESSAGE_OUTPUT] in finished.stdout, (
        "the job log must carry the mapped diagnostic verbatim, got "
        f"{finished.stdout!r} for {mapped[MESSAGE_OUTPUT]!r}"
    )

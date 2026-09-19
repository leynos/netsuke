"""Hold the trunk lane to the report-delivery contract CodeScene depends on.

`coverage-main.yml` is the only lane that hands a coverage report to CodeScene,
and it does so from the workspace: the report is written by the shared coverage
action and read by the shared upload action in the same job, with nothing
archiving, downloading, or restoring it in between. The contract is therefore
about ordering, about the two steps agreeing on a path and a format, and about
the upload being configured so it can actually run and actually verify
something.

A green run does not prove any of that. The upload asserts the file exists and
fails when it is missing, so a disagreement surfaces as a failure — but only
after the instrumented build, which is the most expensive thing the lane does,
and only on the trunk, where the result is already merged. Measured from the
outside the same fault looks like a CodeScene check that waits for a report and
then gives up hours later, which names neither the step nor the input at fault.

The predicates under test live in ``codescene_upload_invariants.py`` and the
shared parsing helpers in ``workflow_loading.py``. Following the sibling
suites, the detectors are also driven against synthetic workflow text: a
detector that stopped matching would otherwise let the repository assertion
pass by finding nothing to object to.

Run via ``make test-workflow-contracts``.
"""

import copy
import typing as typ

import pytest
from ci_coverage_wiring_invariants import (
    COVERAGE_REPORT_PATH,
    GENERATE_COVERAGE_ACTION,
    UPLOAD_COVERAGE_ACTION,
)
from codescene_upload_invariants import (
    CHECKSUM_INPUTS,
    CODESCENE_UPLOAD_STEP,
    COVERAGE_FORMAT_INPUT,
    COVERAGE_FORMAT_VALUE,
    COVERAGE_STEP,
    CREDENTIAL_ENVIRONMENT_KEY,
    CREDENTIAL_INPUT,
    OUTPUT_PATH_INPUT,
    PUBLICATION_OPT_OUT_INPUT,
    REPORT_VALIDATION_STEP,
    REPORT_VALIDATOR_SCRIPT,
    UPLOAD_PATH_INPUT,
    upload_contract_offenders,
)
from lane_steps import step_named
from workflow_loading import (
    COVERAGE_MAIN_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    parse_workflow_text,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

TRUNK_JOB = "coverage-upload"

#: A full 40-character lowercase commit SHA, standing in for whatever
#: revision the dependency updater last pinned. The contract checks the
#: reference's identity and the pin's *shape*, never the revision, so any
#: value of that shape exercises the rule the repository actually enforces.
FIXTURE_PIN = "0" * 40

#: A lane satisfying every clause of the contract. The negative cases below
#: load it, vary one field, and assert the offender that results; a template
#: that was itself non-compliant would make each of them pass for the wrong
#: reason, so `test_the_clean_lane_template_produces_no_offenders` holds it.
CLEAN_LANE = f"""
jobs:
  {TRUNK_JOB}:
    steps:
      - name: {COVERAGE_STEP}
        uses: {GENERATE_COVERAGE_ACTION}@{FIXTURE_PIN}
        with:
          language: rust
          {OUTPUT_PATH_INPUT}: {COVERAGE_REPORT_PATH}
          {COVERAGE_FORMAT_INPUT}: {COVERAGE_FORMAT_VALUE}
      - name: {REPORT_VALIDATION_STEP}
        run: |
          staged="$(mktemp --directory)"
          cp -- {COVERAGE_REPORT_PATH} "${{staged}}/{COVERAGE_REPORT_PATH}"
          uv run --no-project --python 3.14 \
            {REPORT_VALIDATOR_SCRIPT} --artifact-dir "${{staged}}"
      - name: {CODESCENE_UPLOAD_STEP}
        if: env.{CREDENTIAL_ENVIRONMENT_KEY} != ''
        env:
          {CREDENTIAL_ENVIRONMENT_KEY}: ${{{{ secrets.{CREDENTIAL_ENVIRONMENT_KEY} }}}}
        uses: {UPLOAD_COVERAGE_ACTION}@{FIXTURE_PIN}
        with:
          {UPLOAD_PATH_INPUT}: {COVERAGE_REPORT_PATH}
          {COVERAGE_FORMAT_INPUT}: {COVERAGE_FORMAT_VALUE}
          {CREDENTIAL_INPUT}: ${{{{ env.{CREDENTIAL_ENVIRONMENT_KEY} }}}}
"""


def trunk_steps() -> list[dict[str, object]]:
    """Return the trunk lane's parsed steps, in declaration order."""
    return job_steps(load_workflow(COVERAGE_MAIN_WORKFLOW_PATH), TRUNK_JOB)


def clean_steps() -> list[dict[str, object]]:
    """Return a fresh copy of the clean lane's steps.

    Parsed from workflow text through the shared loader rather than assembled
    as a Python literal, so the cases below exercise the same value resolution
    the repository file does. Copied so a case that mutates a step cannot reach
    the next case's baseline.

    Returns
    -------
    list[dict[str, object]]
        The parsed steps, owned by the caller.
    """
    document = parse_workflow_text(CLEAN_LANE, "synthetic workflow")
    assert isinstance(document, dict), "synthetic workflow must parse to a mapping"
    return copy.deepcopy(job_steps(document, TRUNK_JOB))


def _inputs(step: dict[str, object]) -> dict[str, object]:
    """Return a step's ``with`` block, which the clean lane always declares."""
    with_ = step.get("with")
    assert isinstance(with_, dict), "the clean lane declares the step's inputs"
    return with_


def _step(steps: cabc.Sequence[dict[str, object]], name: str) -> dict[str, object]:
    """Return the uniquely named step of a lane, which the clean lane has."""
    found = step_named(steps, name)
    assert found is not None, f"the lane must declare {name!r}"
    return found


def test_the_clean_lane_template_produces_no_offenders() -> None:
    """Keep the cases below from asserting against a broken baseline."""
    assert not upload_contract_offenders(clean_steps()), (
        "the template the negative cases vary must itself satisfy the contract"
    )


def test_the_trunk_lane_satisfies_the_report_delivery_contract() -> None:
    """Hold the repository's only CodeScene submission lane to the contract."""
    offenders = upload_contract_offenders(trunk_steps())
    assert not offenders, "; ".join(offenders)


@pytest.mark.parametrize(
    "step_name", [COVERAGE_STEP, REPORT_VALIDATION_STEP, CODESCENE_UPLOAD_STEP]
)
def test_the_detectors_report_a_lane_whose_steps_are_renamed(step_name: str) -> None:
    """Match the lane structurally, not by the names it happens to use.

    A renamed step would otherwise stop the ordering assertion from finding
    what it is about, and the whole contract would pass over an empty set.
    """
    steps = clean_steps()
    _step(steps, step_name)["name"] = f"{step_name} (renamed)"
    assert upload_contract_offenders(steps), (
        f"renaming {step_name!r} must be reported as a missing step"
    )


@pytest.mark.parametrize(
    "step_name", [COVERAGE_STEP, REPORT_VALIDATION_STEP, CODESCENE_UPLOAD_STEP]
)
def test_the_detectors_report_a_reordered_lane(step_name: str) -> None:
    """Fail beside the generation, or after the upload that reads it.

    Two distinct faults, both of which leave the report sent unchecked: a step
    placed before the report exists has nothing to read, and a check placed
    after the upload is a check the upload never waited for.
    """
    steps = clean_steps()
    moved = steps.pop(steps.index(_step(steps, step_name)))
    match step_name:
        case _ if step_name == REPORT_VALIDATION_STEP:
            # Move it to the front, before the report is written.
            steps.insert(0, moved)
        case _ if step_name == CODESCENE_UPLOAD_STEP:
            # Move the upload ahead of the check that is meant to gate it.
            steps.insert(steps.index(_step(steps, REPORT_VALIDATION_STEP)), moved)
        case _:
            # Move the generation after the step that reads its output.
            steps.append(moved)
    assert upload_contract_offenders(steps), (
        f"misordering {step_name!r} must be reported"
    )


@pytest.mark.parametrize(
    ("replacement", "expected"),
    [
        # A step that only looks at the filesystem does not read the report.
        ("test -s lcov.info", "validate_coverage_artifact.py"),
        # A validator run over the report in place validates the workspace,
        # which holds more than the one report the validator accepts.
        (
            "python scripts/validate_coverage_artifact.py lcov.info",
            "--artifact-dir",
        ),
        # A staged directory that is never filled validates nothing. The
        # validator would refuse the empty directory and the lane would fail
        # for the wrong reason, having read no part of the report.
        (
            'staged="$(mktemp --directory)"\n'
            "uv run --no-project --python 3.14 "
            'scripts/validate_coverage_artifact.py --artifact-dir "$staged"',
            "must copy",
        ),
    ],
)
def test_the_detectors_report_a_validation_step_that_checks_nothing(
    replacement: str, expected: str
) -> None:
    """Fail a check that does not read the report the upload will send.

    The generation action reports success for an empty report, so a step that
    only asserts the file exists would pass on exactly the artefact CodeScene
    rejects — and it would do so in the lane that runs most expensively. The
    same is true of a step that reads the right validator over a directory it
    never put the report into: everything about the invocation looks correct,
    and none of it touches the artefact.
    """
    steps = clean_steps()
    _step(steps, REPORT_VALIDATION_STEP)["run"] = replacement
    offenders = [
        offender
        for offender in upload_contract_offenders(steps)
        if expected in offender
    ]
    assert offenders, (
        f"a validation step running {replacement!r} must be reported for "
        f"omitting {expected}"
    )


@pytest.mark.parametrize(
    ("step_name", "input_name", "replacement"),
    [
        # The write and the read are two independent inputs for one file.
        (COVERAGE_STEP, OUTPUT_PATH_INPUT, "coverage.xml"),
        (CODESCENE_UPLOAD_STEP, UPLOAD_PATH_INPUT, "coverage.xml"),
        # The upload infers nothing from the extension, so a format the
        # generator no longer writes would be parsed as the wrong shape.
        (COVERAGE_STEP, COVERAGE_FORMAT_INPUT, "cobertura"),
        (CODESCENE_UPLOAD_STEP, COVERAGE_FORMAT_INPUT, "cobertura"),
    ],
)
def test_the_detectors_report_a_path_or_format_disagreement(
    step_name: str, input_name: str, replacement: str
) -> None:
    """Fail a lane whose two steps disagree about what was produced."""
    steps = clean_steps()
    _inputs(_step(steps, step_name))[input_name] = replacement
    assert upload_contract_offenders(steps), (
        f"{step_name!r} passing {input_name}={replacement!r} must be reported: "
        f"the report is written and read through different inputs"
    )


@pytest.mark.parametrize("checksum_input", CHECKSUM_INPUTS)
def test_the_detectors_report_a_reintroduced_checksum_input(
    checksum_input: str,
) -> None:
    """Fail the lane if any checksum input returns under any spelling.

    This repository declares no repository variables, so a checksum bound to
    one resolves to the empty string and verifies nothing. The pinned action
    merely skips the check; the revision that renames the input rejects a
    non-empty value outright, so carrying it makes a routine Dependabot bump
    fail the trunk upload.
    """
    steps = clean_steps()
    _inputs(_step(steps, CODESCENE_UPLOAD_STEP))[checksum_input] = "abc123"
    assert upload_contract_offenders(steps), (
        f"{checksum_input!r} must be reported on the upload step"
    )


def test_the_detectors_report_an_upload_that_cannot_be_gated_on() -> None:
    """Fail an upload that is ungated, or gated on what it does not hold.

    A fork, or any repository that has not set the secret, runs this lane with
    the token absent. Without a gate naming the same value the action receives,
    the upload runs with an empty credential and fails the trunk run over a
    secret that was never required.
    """

    def mutated(field: str, replacement: object) -> list[dict[str, object]]:
        """Return the clean lane with one credential field varied."""
        steps = clean_steps()
        upload = _step(steps, CODESCENE_UPLOAD_STEP)
        match field:
            case "if":
                upload["if"] = replacement
            case "env":
                upload.pop("env", None)
            case _:
                _inputs(upload)[CREDENTIAL_INPUT] = replacement
        return steps

    cases: list[tuple[str, object]] = [
        # A literal token is a credential this repository does not own, and
        # one the `if` gate cannot have compared against.
        ("token", "cs-secret-in-the-clear"),
        # No environment, so the credential the gate names is never exported.
        ("env", None),
        # A gate naming something else is not a gate on the credential.
        ("if", "github.event_name == 'push'"),
        # A gate with no condition is not a gate.
        ("if", ""),
        # A name that merely contains the credential's is a different secret,
        # and an unset one: the gate would compare '' against '' and never
        # open, so the lane would read as gated while submitting nothing.
        ("if", f"${{{{ env.NOT_{CREDENTIAL_ENVIRONMENT_KEY} != '' }}}}"),
        # The same containment trap on the value handed to the action.
        (
            "token",
            f"${{{{ env.NOT_{CREDENTIAL_ENVIRONMENT_KEY} }}}}",
        ),
        # A gate on the credential read from the wrong namespace. The `if` is
        # evaluated against `env`, so a `secrets.` reference there is not the
        # exported variable and does not prove the step is gated on it.
        ("if", f"${{{{ secrets.{CREDENTIAL_ENVIRONMENT_KEY} != '' }}}}"),
    ]
    for field, replacement in cases:
        assert upload_contract_offenders(mutated(field, replacement)), (
            f"{field}={replacement!r} must be reported on the upload step"
        )


@pytest.mark.parametrize("step_name", [COVERAGE_STEP, CODESCENE_UPLOAD_STEP])
def test_the_detectors_report_a_lane_that_suppresses_its_own_upload(
    step_name: str,
) -> None:
    """Fail a lane that sets the publication opt-out on either step.

    The opt-out belongs to the pull-request lane, which uses it to keep the
    report local. Set here it would suppress the archive that survives a
    failed run, or — on the upload step, which has no such input — state an
    intent the action does not implement.
    """
    steps = clean_steps()
    _inputs(_step(steps, step_name))[PUBLICATION_OPT_OUT_INPUT] = "false"
    assert upload_contract_offenders(steps), (
        f"{PUBLICATION_OPT_OUT_INPUT} on {step_name!r} must be reported"
    )


def test_the_detectors_report_a_variable_the_repository_does_not_declare() -> None:
    """Fail the lane for a ``vars.`` reference to anything undefined.

    This is the narrower form of the defect that motivated the contract: the
    checksum input's value came from a repository variable this repository has
    never declared, so it resolved to the empty string while reading as though
    it verified the installer.
    """
    steps = clean_steps()
    _step(steps, CODESCENE_UPLOAD_STEP)["if"] = "${{ vars.CODESCENE_CLI_SHA256 != '' }}"
    assert upload_contract_offenders(steps), (
        "a gate on an undeclared repository variable must be reported; it "
        "would never open"
    )


@pytest.mark.parametrize("step_name", [COVERAGE_STEP, CODESCENE_UPLOAD_STEP])
@pytest.mark.parametrize(
    ("pin", "fault"),
    [
        ("v1.2.3", "a tag"),
        ("main", "a branch"),
        ("a576501", "an abbreviated SHA"),
        ("A5765019912A8AB6882B12DB049C7CDE635F3A85", "an uppercase SHA"),
        ("", "no pin at all"),
    ],
)
def test_the_detectors_report_an_unpinned_action_reference(
    step_name: str, pin: str, fault: str
) -> None:
    """Fail a reference that names the action at anything but a full commit SHA.

    A tag or branch resolves to whatever its owner publishes next, so a
    reviewed pin can be replaced without a commit here. The revision itself is
    not asserted — that belongs to the dependency updater — so each case
    varies only the pin's shape.
    """
    steps = clean_steps()
    action = (
        GENERATE_COVERAGE_ACTION
        if step_name == COVERAGE_STEP
        else UPLOAD_COVERAGE_ACTION
    )
    _step(steps, step_name)["uses"] = f"{action}@{pin}" if pin else action
    assert upload_contract_offenders(steps), (
        f"{step_name!r} pinned to {fault} must be reported"
    )

"""The synthetic lane `codescene_upload_contract_test.py` drives its cases on.

A contract test has to be shown a lane that satisfies it before it can vary one
field and assert the offender that results; otherwise a broken template makes
every negative case pass for the wrong reason. That template, the fixtures it
interpolates, and the accessors the cases read it back through are data and
mechanics rather than contracts, so they live here rather than growing the test
file past the repository's 400-line file limit. This module holds no tests of
its own.

The template is parsed from workflow text through the shared loader rather than
assembled as a Python literal, so the cases exercise the same value resolution
the repository file does: an `if` written bare, an expression inside `env`, and
a literal block whose newlines survive parsing are all shapes the predicates
have to cope with, and a hand-built dict would not reproduce them.

Run via ``make test-workflow-contracts``.
"""

import copy
import typing as typ

from ci_coverage_wiring_invariants import (
    COVERAGE_REPORT_PATH,
    GENERATE_COVERAGE_ACTION,
    UPLOAD_COVERAGE_ACTION,
)
from codescene_credential_invariants import (
    CREDENTIAL_ENVIRONMENT_KEY,
    CREDENTIAL_INPUT,
)
from codescene_upload_invariants import (
    CODESCENE_UPLOAD_STEP,
    COVERAGE_FORMAT_INPUT,
    COVERAGE_FORMAT_VALUE,
    COVERAGE_STEP,
    OUTPUT_PATH_INPUT,
    REPORT_VALIDATION_STEP,
    REPORT_VALIDATOR_SCRIPT,
    UPLOAD_PATH_INPUT,
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

#: The job holding the trunk lane's report-delivery steps.
TRUNK_JOB = "coverage-upload"

#: A full 40-character lowercase commit SHA, standing in for whatever
#: revision the dependency updater last pinned. The contract checks the
#: reference's identity and the pin's *shape*, never the revision, so any
#: value of that shape exercises the rule the repository actually enforces.
FIXTURE_PIN = "0" * 40

#: A lane satisfying every clause of the contract. The negative cases load it,
#: vary one field, and assert the offender that results; a template that was
#: itself non-compliant would make each of them pass for the wrong reason, so
#: the test module holds it to the contract before varying anything.
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


def inputs_of(step: dict[str, object]) -> dict[str, object]:
    """Return a step's ``with`` block, which the clean lane always declares."""
    with_ = step.get("with")
    assert isinstance(with_, dict), "the clean lane declares the step's inputs"
    return with_


def step_of(steps: cabc.Sequence[dict[str, object]], name: str) -> dict[str, object]:
    """Return the uniquely named step of a lane, which the clean lane has."""
    found = step_named(steps, name)
    assert found is not None, f"the lane must declare {name!r}"
    return found

"""What the trunk lane must prove before CodeScene is handed a report.

`coverage-main.yml` is the only lane that submits coverage to CodeScene, and
it does so with the action's `upload` mode: the report is generated in the
same job, read straight from the workspace, and checked in place before it is
sent. Nothing archives, downloads, or otherwise restores it between those
steps, so the delivery contract is a statement about ordering, about the input
names the steps agree on, and about the report actually being where the upload
step looks.

That last part is the one a green run does not prove. The upload step asserts
the file exists and fails when it is missing, so a workflow that passed a path
the generation step never wrote would fail loudly — but only after the
instrumented build, which is the most expensive thing the lane does, and only
on the trunk where a failure is already merged. The predicates here state the
agreement up front so a mismatched pair is rejected before it runs at all.

Four failures motivate the shape rather than any particular spelling of it:

- The upload step was handed a checksum input whose value came from
  `vars.CODESCENE_CLI_SHA256`. This repository declares no variables at all, so
  the value interpolated to the empty string: the step read as though it
  verified the CodeScene CLI installer and in fact verified nothing. The
  action's current revision renames that input to `archive-checksum` and
  rejects a non-empty `installer-checksum` outright, so a routine Dependabot
  bump would fail the trunk upload on a value that was already inert.
- The report path is written by one step and read by another through two
  independent inputs. A renamed `output-path` with an unrenamed `path` leaves
  the upload reading a file nothing produced.
- The generation action reports success for a report it wrote nothing into.
  Existence is all the upload checks, so an empty `lcov.info` reaches
  CodeScene, which refuses it in its own time and in its own words: the
  observed failure is a check run reporting "No valid coverage report found in
  the build pipeline" against a commit whose job passed every step. The lane
  therefore reads the report as data before it sends it, through the
  standalone validator this repository already owns.
- The generator's own archive is what makes the report survive a failed run.
  A trunk lane that suppressed the archive would remove the only artefact a
  failure can be diagnosed from, and the upload reads the workspace rather than
  the archive, so the loss would not appear as a failure at all.

These predicates read parsed workflow values rather than files, so
``codescene_upload_contract_test`` can hold the repository's own trunk lane to
the contract and drive shapes the repository does not have. The scan for
``vars.`` references the last bullet depends on is general to any step rather
than particular to this lane, so it lives in ``workflow_variable_scan``.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from ci_coverage_wiring_invariants import (
    COVERAGE_REPORT_PATH,
    GENERATE_COVERAGE_ACTION,
    PUBLICATION_OPT_OUT_INPUT,
    UPLOAD_COVERAGE_ACTION,
    action_of,
)
from workflow_variable_scan import unbound_variable_references

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: The step name that generates the report this lane publishes.
COVERAGE_STEP: typ.Final[str] = "Test and Measure Coverage"

#: The step name that reads the generated report as data before it is sent.
#: A report the generation action calls successful can still be empty or
#: truncated, and the upload asserts only that the file exists, so this is the
#: only place in the lane where a malformed report is caught before the
#: instrumented build is over.
REPORT_VALIDATION_STEP: typ.Final[str] = "Validate the report before submitting it"

#: The standalone hostile-data validator the lane runs. It owns the LCOV
#: contract and is exercised by `make test-coverage-artifact`.
REPORT_VALIDATOR_SCRIPT: typ.Final[str] = "scripts/validate_coverage_artifact.py"

#: The step name that submits the report to CodeScene.
CODESCENE_UPLOAD_STEP: typ.Final[str] = "Upload coverage data to CodeScene"

#: The input the generator writes the report to, and the input the upload reads
#: it from. Two independent names for one file, so they are compared rather
#: than assumed to agree.
OUTPUT_PATH_INPUT: typ.Final[str] = "output-path"
UPLOAD_PATH_INPUT: typ.Final[str] = "path"

#: The format both steps must agree on. The upload infers nothing from the
#: file's extension, so a generator moved to another format while the upload
#: still names `lcov` would be parsed as the wrong shape.
COVERAGE_FORMAT_INPUT: typ.Final[str] = "format"
COVERAGE_FORMAT_VALUE: typ.Final[str] = "lcov"

#: The credential the upload reads. The step must be gated on it rather than
#: running with an empty value, and the value must come from a secret.
CREDENTIAL_ENVIRONMENT_KEY: typ.Final[str] = "CS_ACCESS_TOKEN"
CREDENTIAL_INPUT: typ.Final[str] = "access-token"

#: The expression namespace a credential must be read from. Spelling the
#: prefix rather than the whole expression keeps the check independent of the
#: credential's name and of the whitespace inside the braces.
CREDENTIAL_SOURCE_PREFIX: typ.Final[str] = "secrets."

#: Checksum inputs the pinned upload action accepts, and the input name a
#: future revision renames them to. Every one of them is listed so a
#: reintroduction under any spelling is caught: the value this repository can
#: supply resolves to empty, so the input is either useless or (on the
#: renaming revision) a hard failure.
CHECKSUM_INPUTS: typ.Final[tuple[str, ...]] = (
    "installer-checksum",
    "archive-checksum",
)


def step_named(
    steps: cabc.Sequence[dict[str, object]], name: str
) -> dict[str, object] | None:
    """Return the single step called ``name``, or None when there is none.

    Parameters
    ----------
    steps
        Parsed workflow steps in declaration order.
    name
        The step name to find.

    Returns
    -------
    dict[str, object] or None
        The matching step, or None when no step carries that name. A duplicated
        name returns the first match; the caller's ordering assertion is what
        makes a second one matter, and the repository assertion below pins the
        count.
    """
    return next((step for step in steps if step.get("name") == name), None)


def inputs_of(step: dict[str, object]) -> dict[str, object]:
    """Return a step's ``with`` block, or an empty mapping when it has none."""
    with_ = step.get("with")
    return with_ if isinstance(with_, dict) else {}


def upload_contract_offenders(
    steps: cabc.Sequence[dict[str, object]],
) -> list[str]:
    """Return every way the trunk lane breaks the report-delivery contract.

    Parameters
    ----------
    steps
        The trunk lane's parsed steps, in declaration order.

    Returns
    -------
    list[str]
        One description per violation, empty when the lane is clean. The
        descriptions name the fault rather than the expected value, so a
        failure says what to change.
    """
    coverage = step_named(steps, COVERAGE_STEP)
    validation = step_named(steps, REPORT_VALIDATION_STEP)
    upload = step_named(steps, CODESCENE_UPLOAD_STEP)
    # Spelled as three separate tests rather than `any(step is None ...)`: the
    # guard has to narrow each name to a step for the calls below, which a
    # predicate over a tuple cannot do.
    if coverage is None or validation is None or upload is None:
        missing = [
            name
            for name, step in (
                (COVERAGE_STEP, coverage),
                (REPORT_VALIDATION_STEP, validation),
                (CODESCENE_UPLOAD_STEP, upload),
            )
            if step is None
        ]
        return [f"the trunk lane is missing the step(s) {missing!r}"]

    offenders: list[str] = []
    if steps.index(coverage) >= steps.index(validation):
        offenders.append(
            f"{REPORT_VALIDATION_STEP!r} must follow {COVERAGE_STEP!r}; there is "
            f"no report to read before the step that writes it"
        )
    if steps.index(validation) >= steps.index(upload):
        offenders.append(
            f"{REPORT_VALIDATION_STEP!r} must precede {CODESCENE_UPLOAD_STEP!r}; "
            f"a report sent before it is checked is sent unchecked"
        )

    if action_of(upload) != UPLOAD_COVERAGE_ACTION:
        offenders.append(
            f"{CODESCENE_UPLOAD_STEP!r} must invoke {UPLOAD_COVERAGE_ACTION}"
        )
    if action_of(coverage) != GENERATE_COVERAGE_ACTION:
        offenders.append(f"{COVERAGE_STEP!r} must invoke {GENERATE_COVERAGE_ACTION}")

    offenders.extend(_validation_offenders(validation))
    offenders.extend(_path_offenders(coverage, upload))
    offenders.extend(_checksum_offenders(upload))
    offenders.extend(_credential_offenders(upload))
    offenders.extend(_archive_offenders(coverage, upload))
    return offenders


def _validation_offenders(validation: dict[str, object]) -> list[str]:
    """Return faults in the step that reads the report as data.

    The step must run the checked-in validator, over a directory built at run
    time. Both parts matter. A step that merely asserts the file exists would
    not reject the empty report the generation action can call a success, which
    is the fault the uploader cannot see. A step that staged the report into a
    directory committed to the tree, or read the report from wherever it was
    written, would be validating something other than the artefact about to be
    sent.

    Returns
    -------
    list[str]
        One entry per fault in the step, empty when it reads the report through
        the checked-in validator.
    """
    script = str(validation.get("run", ""))
    offenders: list[str] = []
    if REPORT_VALIDATOR_SCRIPT not in script:
        offenders.append(
            f"{REPORT_VALIDATION_STEP!r} must run {REPORT_VALIDATOR_SCRIPT}, "
            f"which owns the LCOV contract for a hostile report"
        )
    if "--artifact-dir" not in script:
        offenders.append(
            f"{REPORT_VALIDATION_STEP!r} must pass --artifact-dir; the validator "
            f"reads a directory holding exactly one {COVERAGE_REPORT_PATH!r}"
        )
    return offenders


def _path_offenders(
    coverage: dict[str, object], upload: dict[str, object]
) -> list[str]:
    """Return mismatches between the written and the submitted report."""
    generated = inputs_of(coverage)
    submitted = inputs_of(upload)
    offenders: list[str] = []

    wrote = generated.get(OUTPUT_PATH_INPUT)
    read = submitted.get(UPLOAD_PATH_INPUT)
    if wrote != read:
        offenders.append(
            f"{COVERAGE_STEP!r} writes {OUTPUT_PATH_INPUT}={wrote!r} but "
            f"{CODESCENE_UPLOAD_STEP!r} reads {UPLOAD_PATH_INPUT}={read!r}; the "
            f"upload would look for a file nothing wrote"
        )
    if wrote != COVERAGE_REPORT_PATH:
        offenders.append(
            f"the trunk report must be {COVERAGE_REPORT_PATH!r}, got {wrote!r}"
        )
    if generated.get(COVERAGE_FORMAT_INPUT) != COVERAGE_FORMAT_VALUE:
        offenders.append(
            f"{COVERAGE_STEP!r} must generate {COVERAGE_FORMAT_VALUE!r}, got "
            f"{generated.get(COVERAGE_FORMAT_INPUT)!r}"
        )
    if submitted.get(COVERAGE_FORMAT_INPUT) != COVERAGE_FORMAT_VALUE:
        offenders.append(
            f"{CODESCENE_UPLOAD_STEP!r} must submit {COVERAGE_FORMAT_VALUE!r}, got "
            f"{submitted.get(COVERAGE_FORMAT_INPUT)!r}"
        )
    return offenders


def _checksum_offenders(upload: dict[str, object]) -> list[str]:
    """Return every checksum input the upload step still carries."""
    submitted = inputs_of(upload)
    return [
        f"{CODESCENE_UPLOAD_STEP!r} must not pass {name!r}; this repository "
        f"defines no variable that could bind it, so it verifies nothing today "
        f"and the action's renaming revision rejects a non-empty value outright"
        for name in CHECKSUM_INPUTS
        if name in submitted
    ]


def _credential_offenders(upload: dict[str, object]) -> list[str]:
    """Return every fault in how the upload step receives its credential.

    Three faults are distinguishable and each has its own consequence. A
    credential the step does not carry in its own environment cannot be gated
    on, because the ``if`` condition is evaluated against ``env``. A
    credential that is not read from a secret is one this repository does not
    own. A step that is not gated at all runs with an empty token whenever the
    secret is absent, which is every run from a fork or from a repository that
    has not set it, and turns a missing optional secret into a failed trunk
    run.

    The credential reaches the action through an expression rather than a
    literal, which is what makes the ``if`` gate mean anything: the step
    evaluates the same value the action is handed. The expression may name the
    secret directly or read the environment variable the step exported it to,
    so the test is that the credential is named, not how it is spelled.

    Returns
    -------
    list[str]
        One entry per fault, empty when the step both holds the credential and
        gates on it.
    """
    environment = upload.get("env")
    declared = (
        environment.get(CREDENTIAL_ENVIRONMENT_KEY)
        if isinstance(environment, dict)
        else None
    )
    submitted = inputs_of(upload)
    token = submitted.get(CREDENTIAL_INPUT)
    condition = upload.get("if")

    offenders: list[str] = []
    if not (isinstance(declared, str) and CREDENTIAL_SOURCE_PREFIX in declared):
        offenders.append(
            f"{CODESCENE_UPLOAD_STEP!r} must declare "
            f"env.{CREDENTIAL_ENVIRONMENT_KEY} from a github secret, got "
            f"{declared!r}"
        )
    if not (isinstance(token, str) and CREDENTIAL_ENVIRONMENT_KEY in token):
        offenders.append(
            f"{CODESCENE_UPLOAD_STEP!r} must pass {CREDENTIAL_INPUT} the "
            f"{CREDENTIAL_ENVIRONMENT_KEY} it gated on, got {token!r}"
        )
    if not (isinstance(condition, str) and CREDENTIAL_ENVIRONMENT_KEY in condition):
        offenders.append(
            f"{CODESCENE_UPLOAD_STEP!r} must be conditional on "
            f"{CREDENTIAL_ENVIRONMENT_KEY} being present, got {condition!r}; an "
            f"ungated step fails the trunk run over a missing optional secret"
        )
    return offenders


def _archive_offenders(
    coverage: dict[str, object], upload: dict[str, object]
) -> list[str]:
    """Return faults in the generator's archive, which outlives the job."""
    generated = inputs_of(coverage)
    submitted = inputs_of(upload)
    offenders: list[str] = []
    if PUBLICATION_OPT_OUT_INPUT in generated:
        offenders.append(
            f"{COVERAGE_STEP!r} must not set {PUBLICATION_OPT_OUT_INPUT}; the "
            f"generator's own archive is the only copy of the report that "
            f"survives a failed run, and the upload reads the workspace"
        )
    if PUBLICATION_OPT_OUT_INPUT in submitted:
        offenders.append(
            f"{CODESCENE_UPLOAD_STEP!r} sets {PUBLICATION_OPT_OUT_INPUT}; that "
            f"input belongs to the generator, not to the upload"
        )
    unbound = unbound_variable_references(upload)
    if unbound:
        offenders.append(
            f"{CODESCENE_UPLOAD_STEP!r} reads undefined repository "
            f"variable(s) {unbound!r}; the repository declares none, so each "
            f"resolves to the empty string"
        )
    return offenders

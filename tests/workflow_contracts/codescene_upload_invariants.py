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
  verified the CodeScene CLI installer and in fact verified nothing. At the
  pinned revision the two inputs are `installer-checksum`, deprecated, and
  `archive-checksum`, its replacement, and the action rejects a non-empty
  `installer-checksum` outright, so this repository's own pin is already the
  revision that fails on that value rather than merely ignoring it.
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

These predicates read parsed workflow values rather than files, so the contract
tests can hold the repository's own trunk lane to the contract and drive shapes
the repository does not have. Four of the rules they rest on describe no
particular lane, so they live apart: the scan for ``vars.`` references the last
bullet depends on is general to any step (``workflow_variable_scan``), finding
a named step and checking that it calls the right action at an immutable pin is
what every lane contract does first (``lane_steps``), the credential the upload
is handed is a rule about the secret rather than about the report
(``codescene_credential_invariants``), and the third bullet's whole remedy —
reading the report as data before sending it — is stated over the validating
step alone
(``codescene_report_validation_invariants``).

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
from codescene_credential_invariants import credential_offenders
from codescene_report_validation_invariants import (
    REPORT_VALIDATION_STEP,
    validation_offenders,
)
from lane_steps import (
    action_reference_of,
    inputs_of,
    step_named,
    step_names_declared_twice,
)
from workflow_variable_scan import unbound_variable_references

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: The step name that generates the report this lane publishes.
COVERAGE_STEP: typ.Final[str] = "Test and Measure Coverage"

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

#: Checksum inputs the pinned upload action declares. Both are listed so a
#: reintroduction under either spelling is caught: `installer-checksum` is
#: deprecated and rejected when non-empty at this repository's own pin, and
#: `archive-checksum` is its replacement, which no workflow here binds.
CHECKSUM_INPUTS: typ.Final[tuple[str, ...]] = (
    "installer-checksum",
    "archive-checksum",
)


#: The three steps the report-delivery contract is about, in the order the
#: lane must declare them.
REPORT_STEP_NAMES: typ.Final[tuple[str, ...]] = (
    COVERAGE_STEP,
    REPORT_VALIDATION_STEP,
    CODESCENE_UPLOAD_STEP,
)


def report_steps(
    steps: cabc.Sequence[dict[str, object]],
) -> tuple[dict[str, object], dict[str, object], dict[str, object]] | None:
    """Return the generation, validation and upload steps, or None.

    A lane missing any one of the three is not worth reporting faults against,
    and neither is one that declares a name twice: the steps this contract can
    examine would be an arbitrary member of the pair, so every fault reported
    below would be a claim about a step the lane is not necessarily running.
    Both are answered once, here, and the caller guards on a single value. A
    third way to leave the lane without a single upload to read — a second step
    invoking the same action under another name — is asked about separately, by
    ``_upload_impostors``, because it is a fault to report rather than a reason
    to report nothing. The lookup loops over one name at a time rather than
    joining three ``is None`` tests: a three-operand boolean is rejected by
    `PLR0916`, and a predicate
    over a tuple of optionals is rejected by the type checker, which cannot
    narrow the individual names through it. Appending the narrowed step is what
    leaves the returned tuple non-optional.

    Parameters
    ----------
    steps
        Parsed workflow steps in declaration order.

    Returns
    -------
    tuple of three dicts, or None
        The three steps, in the order above, or None when the lane is missing
        one of them or declares one of them more than once.
    """
    if any(name in step_names_declared_twice(steps) for name in REPORT_STEP_NAMES):
        return None
    found: list[dict[str, object]] = []
    for name in REPORT_STEP_NAMES:
        step = step_named(steps, name)
        if step is None:
            return None
        found.append(step)
    coverage, validation, upload = found
    return coverage, validation, upload


def _upload_impostors(
    steps: cabc.Sequence[dict[str, object]], upload: dict[str, object]
) -> list[str]:
    """Return every other step submitting to CodeScene through the upload action.

    The three steps are found by the names this lane declares, so a copy of the
    upload under any other name is a second submission the rest of this contract
    never reads: its inputs, its gate and its pin all go unchecked while the
    certified step runs beside it. GitHub keys nothing on a step's name, so the
    impostor runs whether or not anything asked about it — which is what a
    copy-pasted step produces, since it keeps the action rather than the name.
    The action is the identity the contract is stated over, so it is matched
    through ``action_of``, which compares the reference without its version.
    Steps are compared by identity rather than by value, because a step equal to
    the upload is still a second step running it.

    Returns
    -------
    list[str]
        One entry per impostor, empty when the lane calls the action once.
    """
    impostors = [
        step
        for step in steps
        if action_of(step) == UPLOAD_COVERAGE_ACTION and step is not upload
    ]
    if not impostors:
        return []
    names = [step.get("name") for step in impostors]
    return [
        (
            f"the trunk lane must submit to CodeScene once, through "
            f"{CODESCENE_UPLOAD_STEP!r}, whose inputs and gate are what this "
            f"contract reads; {names!r} also invoke {UPLOAD_COVERAGE_ACTION}"
        )
    ]


def _absent_steps(
    steps: cabc.Sequence[dict[str, object]],
) -> list[str]:
    """Return the report-delivery steps the lane does not declare exactly once.

    A name the lane carries twice is reported alongside a name it does not
    carry at all, because both leave the lane without a single step this
    contract can make a claim about. Which of the two a repeated name is would
    not be visible from the lookup alone, so the repetition is reported here
    rather than silently resolved to whichever step was declared first.

    Returns
    -------
    list[str]
        One entry per name the lane is missing or has declared more than once,
        in the order the contract lists them.
    """
    repeated = step_names_declared_twice(steps)
    return [
        name
        for name in REPORT_STEP_NAMES
        if step_named(steps, name) is None or name in repeated
    ]


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
    found = report_steps(steps)
    if found is None:
        absent = _absent_steps(steps)
        return [
            (
                f"the trunk lane must declare each of the report-delivery steps "
                f"exactly once; {absent!r} is missing or repeated"
            )
        ]
    coverage, validation, upload = found

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

    for step, name, action in (
        (upload, CODESCENE_UPLOAD_STEP, UPLOAD_COVERAGE_ACTION),
        (coverage, COVERAGE_STEP, GENERATE_COVERAGE_ACTION),
    ):
        pin = action_reference_of(step, name, action)
        if pin is not None:
            offenders.append(pin)

    offenders.extend(_upload_impostors(steps, upload))
    offenders.extend(validation_offenders(validation))
    offenders.extend(_path_offenders(coverage, upload))
    offenders.extend(_checksum_offenders(upload))
    offenders.extend(credential_offenders(upload, inputs_of(upload)))
    offenders.extend(_archive_offenders(coverage, upload))
    offenders.extend(_variable_offenders(upload))
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
        f"defines no variable that could bind it, so it verifies nothing"
        for name in CHECKSUM_INPUTS
        if name in submitted
    ]


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
    return offenders


def _variable_offenders(upload: dict[str, object]) -> list[str]:
    """Return faults for each repository variable the upload reads undeclared.

    The scan is general to any step, and this rule is the first failure the
    module's own contract was written for: the repository declares no variables
    at all, so a ``vars.`` reference resolves to the empty string rather than to
    a value. It is asked of the upload alone, because the upload is where a
    reference is read as though it had been verified.

    Returns
    -------
    list[str]
        One fault per undefined reference, and nothing when there is none.
    """
    unbound = unbound_variable_references(upload)
    if not unbound:
        return []
    return [
        (
            f"{CODESCENE_UPLOAD_STEP!r} reads undefined repository "
            f"variable(s) {unbound!r}; the repository declares none, so each "
            f"resolves to the empty string"
        )
    ]

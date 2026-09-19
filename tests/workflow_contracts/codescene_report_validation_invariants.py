"""What the step reading the report as data must be shown to do.

The generation action reports success for a report it wrote nothing into, and
the upload asserts only that the file exists, so an empty `lcov.info` would
reach CodeScene unchallenged. Measured from outside, that failure is a check
run reporting "No valid coverage report found in the build pipeline" against a
commit whose job passed every step: it names neither the step nor the input at
fault, and it arrives hours after the report was sent. The lane therefore reads
the report as data before it sends it, through the standalone validator this
repository already owns, and this module holds that invocation to account.

Three clauses, each of which a script can satisfy in appearance while breaking
in substance:

- The step runs the checked-in validator. A step that asserts the file exists
  would pass on exactly the artefact CodeScene rejects.
- The validator is handed a directory built at run time, not the workspace.
  The validator reads a directory holding exactly one report, so a workspace
  either presents the wrong artefact among others or is refused outright.
- The report is *copied* into that directory. A directory named and never
  filled validates whatever else is there, which on a runner is nothing; and a
  directory the report is *moved* into leaves the workspace holding no copy for
  the upload that has yet to read it.

The predicates read a script as text, so they are stated over the commands the
script must contain rather than over any particular arrangement of them.

Separated from ``codescene_upload_invariants`` so neither module outgrows the
400-line limit the Python lint gate enforces.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

from ci_coverage_wiring_invariants import COVERAGE_REPORT_PATH

#: The step name that reads the generated report as data before it is sent.
#: A report the generation action calls successful can still be empty or
#: truncated, and the upload asserts only that the file exists, so this is the
#: only place in the lane where a malformed report is caught before the
#: instrumented build is over.
REPORT_VALIDATION_STEP: typ.Final[str] = "Validate the report before submitting it"

#: The standalone hostile-data validator the lane runs. It owns the LCOV
#: contract and is exercised by `make test-coverage-artifact`.
REPORT_VALIDATOR_SCRIPT: typ.Final[str] = "scripts/validate_coverage_artifact.py"


def validation_offenders(validation: dict[str, object]) -> list[str]:
    """Return faults in the step that reads the report as data.

    The step must run the checked-in validator over a directory built at run
    time, and must have put the report into that directory. All three parts
    matter, and the third is the one a script can omit while still reading as
    correct. A step that merely asserts the file exists would not reject the
    empty report the generation action can call a success, which is the fault
    the uploader cannot see. A step that staged the report into a directory
    committed to the tree, or read the report from wherever it was written,
    would be validating something other than the artefact about to be sent.
    And a step that names a staged directory without copying the report into it
    validates whatever that directory happens to hold, which on a runner is
    nothing at all — the validator then fails, or passes over an empty set,
    without ever having read the report this lane is about.

    Parameters
    ----------
    validation
        The parsed step that is meant to read the report before it is sent.

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
        # Nothing below can be established about a script that does not run the
        # validator, and reporting it twice would read as two faults in a step
        # that has one.
        return offenders
    staged = _staged_directory(script)
    if staged is None:
        offenders.append(
            f"{REPORT_VALIDATION_STEP!r} must pass --artifact-dir a directory "
            f"built at run time; the validator reads a directory holding "
            f"exactly one {COVERAGE_REPORT_PATH!r}, so a script that hands it "
            f"the workspace either validates the wrong artefact or refuses it "
            f"for holding more than one"
        )
        return offenders
    if not _copies_report_into(script, staged):
        offenders.append(
            f"{REPORT_VALIDATION_STEP!r} must copy {COVERAGE_REPORT_PATH!r} "
            f"into the {staged!r} directory it passes --artifact-dir; a script "
            f"that names a directory but never fills it validates whatever "
            f"else is there"
        )
    return offenders


def _staged_directory(script: str) -> str | None:
    """Return the directory the script passes to ``--artifact-dir``.

    The flag takes the directory as its argument, so the pair is read together:
    a script that mentions the flag but supplies no directory, or supplies one
    it never created, is not staging anything.

    Returns
    -------
    str | None
        The argument as written, or `None` when the flag is absent or bare.
    """
    match = re.search(
        r"--artifact-dir[=\s]+(?P<directory>\S+)",
        script,
    )
    if match is None:
        return None
    # A `"${staged}"` argument names the same directory as `${staged}`.
    return match.group("directory").strip("\"'${}")


def _copies_report_into(script: str, directory: str) -> bool:
    """Return whether the script copies the report into ``directory``.

    The copy is what binds the validated artefact to the submitted one: the
    generation action writes the report into the workspace, and the upload
    reads it from there, so a staged directory only means something if the
    report was put into it. An empty staged directory would make the validator
    fail for the wrong reason on a report that was fine.

    Only copying commands count. ``mv`` would put the report in the staged
    directory and take it out of the workspace, so a later upload reading the
    workspace would find nothing: the validation would pass and the submission
    would fail, which is the ordering this contract exists to prevent. The
    command list is deliberately the two that leave the source in place.

    Returns
    -------
    bool
        Whether one line of the script names both the report and the
        directory in a copying command.
    """
    return any(
        re.search(
            rf"\b(?:cp|install)\b[^\n]*{re.escape(COVERAGE_REPORT_PATH)}[^\n]*"
            rf"{re.escape(directory)}",
            line,
        )
        for line in script.splitlines()
    )

"""What the step reading the report as data must be shown to do.

The generation action reports success for a report it wrote nothing into, and
the upload asserts only that the file exists, so an empty `lcov.info` would
reach CodeScene unchallenged. Measured from outside, that failure is a check
run reporting "No valid coverage report found in the build pipeline" against a
commit whose job passed every step: it names neither the step nor the input at
fault, and it arrives hours after the report was sent. The lane therefore reads
the report as data before it sends it, through the standalone validator this
repository already owns, and this module holds that invocation to account.

Four clauses, each of which a script can satisfy in appearance while breaking
in substance:

- The step runs the checked-in validator. A step that asserts the file exists
  would pass on exactly the artefact CodeScene rejects.
- The validator is handed a directory built at run time, not the workspace.
  The validator reads a directory holding exactly one report, so a workspace
  either presents the wrong artefact among others or is refused outright.
- The script *creates* that directory. The validator reads a directory rather
  than a file, so a script naming one it never made hands it nothing to read:
  on a runner the validator fails, and anywhere the directory happens to
  already exist it validates whatever was left there — an artefact that is not
  this run's.
- The report is *copied* into that directory. A directory named and never
  filled validates whatever else is there, which on a runner is nothing; and a
  directory the report is *moved* into leaves the workspace holding no copy for
  the upload that has yet to read it.

Every clause is a statement about a *command*, and a shell script is not a
list of lines: a `run: |` block happens to put one command per line, but a
one-liner joining several with `&&` does not. So each clause is asked of
command segments rather than of lines. What a script's commands are, which
words one of them is handed, and which names capture a command's output are
questions about shell text rather than about this lane, so they live in
``shell_command_scan``.

Separated from ``codescene_upload_invariants`` so neither module outgrows the
400-line limit the Python lint gate enforces.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

from ci_coverage_wiring_invariants import COVERAGE_REPORT_PATH
from shell_command_scan import (
    MULTIPLEXER_COMMAND,
    assigned_from,
    command_operands,
    command_segments,
    names_a_component,
    script_operands,
)

#: How `mktemp` is asked for a directory rather than a file. Both spellings are
#: accepted: which one a script uses is style, and recognising only one would
#: report a script that did create its directory.
MKTEMP_DIRECTORY_FLAG: typ.Final[str] = r"(?:--directory|-d)\b"

#: The step name that reads the generated report as data before it is sent.
#: A report the generation action calls successful can still be empty or
#: truncated, and the upload asserts only that the file exists, so this is the
#: only place in the lane where a malformed report is caught before the
#: instrumented build is over.
REPORT_VALIDATION_STEP: typ.Final[str] = "Validate the report before submitting it"

#: The standalone hostile-data validator the lane runs. It owns the LCOV
#: contract and is exercised by `make test-coverage-artifact`.
REPORT_VALIDATOR_SCRIPT: typ.Final[str] = "scripts/validate_coverage_artifact.py"

#: The commands that put a file somewhere while leaving it where it was. Only
#: these count as staging the report: `mv` would put the report in the staged
#: directory and take it out of the workspace, so the upload, which has yet to
#: read the workspace, would find nothing — the validation passing and the
#: submission failing, which is the ordering this contract exists to prevent.
COPYING_COMMANDS: typ.Final[tuple[str, ...]] = ("cp", "install")

#: The commands that execute a script handed to them as an operand. The
#: validator is a Python program, so it runs when one of these is given it as a
#: file to run — `uv run ... script.py`, or `python script.py`. The list is
#: deliberately short and stated rather than inferred: a script named to any
#: other command is an argument that command may print, test, or ignore, which
#: is the difference between running the validator and naming it.
INTERPRETERS: typ.Final[tuple[str, ...]] = (MULTIPLEXER_COMMAND, "python", "python3")


def validation_offenders(validation: dict[str, object]) -> list[str]:
    """Return faults in the step that reads the report as data.

    The step must run the checked-in validator over a directory it created at
    run time, and must have put the report into that directory. Each part
    matters, and the later ones are the ones a script can omit while still
    reading as correct. A step that merely asserts the file exists would not
    reject the empty report the generation action can call a success, which is
    the fault the uploader cannot see. A step that named a directory committed
    to the tree, or read the report from wherever it was written, would be
    validating something other than the artefact about to be sent. A step that
    names a staged directory without creating it validates whatever that
    directory happens to hold, which on a runner is nothing at all — the
    validator then fails, or passes over the contents of a directory another
    run left behind, without ever having read the report this lane is about.
    And a step that creates the directory and never fills it validates an empty
    set, failing for the wrong reason on a report that was fine.

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
    if not _runs_validator(script):
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
    if staged not in _created_directories(script):
        offenders.append(
            f"{REPORT_VALIDATION_STEP!r} must create the {staged!r} directory "
            f"it passes --artifact-dir; a script that names a directory it "
            f"never made hands the validator nothing to read, or another run's "
            f"leavings to read as this run's"
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


def _runs_validator(script: str) -> bool:
    """Return whether ``script`` runs the validator rather than naming it.

    A substring search is answered by a script that only *names* the validator:
    `echo "scripts/validate_coverage_artifact.py --artifact-dir ${staged}"`
    contains it and runs nothing, and a comment does the same. Since the step
    exists to read the report as data before it is sent, a script that read as
    correct while executing no part of the validator would pass over exactly
    the malformed report the step was added for.

    The validator is a Python program, so running it means handing it to an
    interpreter as the file to execute. The script is read as command segments,
    and the segment has to invoke an interpreter whose operands name the
    validator — `uv run --no-project --python X script.py` and `python script.py`
    alike. Anything else is a mention: another command's argument, a comment, or
    text inside a quoted string.

    Returns
    -------
    bool
        Whether one command in the script runs the validator.
    """
    return any(_runs_validator_in(segment) for segment in command_segments(script))


def _runs_validator_in(segment: str) -> bool:
    """Return whether one command segment hands the validator to an interpreter.

    Asked of one segment so no reading spans two commands: a clause stated over
    the whole script could be satisfied by one command naming the validator and
    another naming the report, which is not a script that ran anything.

    Returns
    -------
    bool
        Whether this segment runs the validator.
    """
    return any(
        operand.strip("\"'") == REPORT_VALIDATOR_SCRIPT
        for interpreter in INTERPRETERS
        for operand in script_operands(segment, interpreter)
    )


def _created_directories(script: str) -> set[str]:
    """Return the names ``script`` gives to directories it creates.

    The validator reads a directory, so naming one is not enough: a script that
    passes the name of a directory it never made hands the validator nothing to
    read. On a runner the validator then fails, and anywhere the directory
    already exists it validates whatever was left there — an artefact that is
    not this run's, read under a claim that it is.

    Two spellings count, and each is read from one segment at a time, so a
    variable is never associated with a directory a neighbour command made. A
    shell variable set from a command that makes a directory —
    `staged="$(mktemp --directory)"`, which is what a staged scratch directory
    normally looks like — counts through :func:`_mktemp_directories`; a
    directory named to `mkdir`, which is how a script that stages somewhere
    fixed tends to read, counts through :func:`_mkdir_directories`.

    Returns
    -------
    set[str]
        The variable names and literal directory operands, so a caller
        holding either can ask whether the script created it.
    """
    created: set[str] = set()
    for segment in command_segments(script):
        created |= _mktemp_directories(segment) | _mkdir_directories(segment)
    return created


def _mktemp_directories(segment: str) -> set[str]:
    """Return the names a segment captures a directory in, through `mktemp`.

    A plain `mktemp` names a file, which is not a directory to stage, so the
    directory flag has to be present: both spellings of it count, since which
    one a script uses is style. The name credited is the one the capture was
    assigned to, so a segment assigning two names and capturing one directory
    credits only the name holding it.

    Returns
    -------
    set[str]
        The variable names holding a directory this segment made, empty when it
        asks `mktemp` for no directory.
    """
    if not re.search(r"\bmktemp\b", segment) or not re.search(
        MKTEMP_DIRECTORY_FLAG, segment
    ):
        return set()
    return assigned_from(segment, "mktemp")


def _mkdir_directories(segment: str) -> set[str]:
    """Return the directories a segment names to `mkdir`.

    `mkdir` is read where a command *runs*, so a segment that only prints the
    words makes nothing: `echo "mkdir staged"` names a directory and creates
    none, and crediting it would record one the script never made.

    Returns
    -------
    set[str]
        The literal directory operands with quoting and variable syntax
        stripped, empty when the segment runs no `mkdir`.
    """
    if not re.search(r"\bmkdir\b", segment):
        return set()
    return {name.strip("\"'${}") for name in command_operands(segment, "mkdir")}


def _staged_directory(script: str) -> str | None:
    """Return the directory the script passes to ``--artifact-dir``.

    The flag takes the directory as its argument, so the pair is read together:
    a script that mentions the flag but supplies no directory is not staging
    anything. Whether the directory is *created* is a separate question, asked
    of the commands rather than of this argument, so that the two faults report
    what is wrong with each: a script staging nowhere and a script staging into
    a directory it never made need different fixes.

    Returns
    -------
    str | None
        The argument as written, with any quoting and variable syntax stripped,
        or `None` when the flag is absent or bare.
    """
    # The value is taken from the same line. `\s` here would match a newline,
    # and a flag left bare at the end of a line would then take the first word
    # of the *next* command as its argument — reading a directory named
    # `mkdir` out of `mkdir mkdir`, and certifying an invocation the validator
    # was handed no directory for at all.
    match = re.search(
        r"--artifact-dir[=\t ]+(?P<directory>\S+)",
        script,
    )
    if match is None:
        return None
    # A `"${staged}"` argument names the same directory as `${staged}`, and the
    # name is what the creation check is stated over.
    directory = match.group("directory").strip("\"'${}")
    return directory or None


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

    Both the report and the directory have to appear in the *same* command, so
    this asks it of one segment at a time. Asked of a whole line instead, a
    `cp` of some other file followed by a command naming the directory would
    satisfy it, and the staged directory would be validated empty.

    Returns
    -------
    bool
        Whether one command in the script names both the report and the
        directory in a copying command.
    """
    return any(
        _copies_from_into(segment, COVERAGE_REPORT_PATH, directory)
        for segment in command_segments(script)
    )


def _copies_from_into(segment: str, source: str, directory: str) -> bool:
    """Whether one segment copies named ``source`` into named ``directory``.

    Both names must appear, the source must come *first*, and the destination
    operand must name the directory: `cp` and `install` take the source then the
    destination, so a command naming both in the other order reads the report
    out of the staged directory rather than putting it in. Order is what makes
    that difference visible, and a match indifferent to it would certify the
    wrong direction as the one this contract asks for.

    A destination naming the directory as a *prefix* counts — `"${staged}/x"` is
    the ordinary spelling, where the operand is a path under the directory and
    the file lands inside it. A destination naming it as a later component does
    not: the file would land somewhere else entirely.

    The destination is the *last* operand, not the second. `cp a b c dir` copies
    three sources into one directory, and the second operand is then another
    source; reading it as the destination would satisfy the contract on a
    command whose report never reaches ``directory`` at all.

    Returns
    -------
    bool
        Whether the segment is a copying command from ``source`` into a path
        under ``directory``.
    """
    return any(
        _copies_in_order(command_operands(segment, command), source, directory)
        for command in COPYING_COMMANDS
    )


def _copies_in_order(operands: list[str], source: str, directory: str) -> bool:
    """Whether one copying command's operands carry ``source`` into ``directory``.

    A command with fewer than two operands copies nothing: `cp x` is missing its
    destination entirely.

    Returns
    -------
    bool
        Whether these operands are a copy of ``source`` into ``directory``.
    """
    if len(operands) < 2:
        return False
    # The report is the *first* operand, and it is one of the sources. A later
    # operand naming it is not what this asks: the copy has to take the report
    # as its input.
    if not names_a_component(operands[0], source):
        return False
    # The GNU `-t` spelling puts the destination before the sources, so it fails
    # this ordering and is reported. That is the safe direction: the spelling
    # this repository uses is the ordinary one, and a script that switched to
    # `-t` would be told to stage the report the usual way.
    return names_a_component(operands[-1], directory, prefix=True)

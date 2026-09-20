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

Every clause is a statement about a *command*, and a shell script is not a list
of lines. A `run: |` block happens to put one command per line, but a one-liner
joining several with `&&` does not, and a scan that ran line-wide would let a
match begin in the command that copies something and finish in the command that
names the staged directory — reporting a report that never went in. So the
predicates are stated over command segments, and a segment is where each of
them starts and stops.

Separated from ``codescene_upload_invariants`` so neither module outgrows the
400-line limit the Python lint gate enforces.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

from ci_coverage_wiring_invariants import COVERAGE_REPORT_PATH

#: Matches one shell separator, splitting a line into the simple commands it
#: runs. `&&` and `||` are matched before the single-character class so each is
#: consumed whole rather than as its first character followed by another.
COMMAND_SEPARATOR: typ.Final[re.Pattern[str]] = re.compile(r"&&|\|\||[;|]")

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


def _command_segments(script: str) -> list[str]:
    """Return the script's commands, one segment per simple shell command.

    A shell script is not a list of lines. A `run: |` block happens to put one
    command per line, but the same shell accepts a one-liner joining several
    with `&&`, and a predicate stated over lines would read such a line as a
    single command: a match could begin in the command that copies a file and
    finish in the command that names the staged directory, reporting a report
    that was never put into it.

    Splitting on the separators is a deliberate over-approximation. A `;` or a
    `|` inside a quoted string is not a separator, and cutting there can
    therefore divide a command in two — which is the safe direction. A rule
    asserting that two things appear in one command is only ever made *harder*
    to satisfy by cutting more finely, so a script misjudged by the split is
    reported rather than passed, and its author moves the report path and the
    directory into the same command.

    Returns
    -------
    list[str]
        The segments, in order, including any that are blank.
    """
    return [
        segment
        for line in script.splitlines()
        for segment in COMMAND_SEPARATOR.split(line)
    ]


def _created_directories(script: str) -> set[str]:
    """Return the names ``script`` gives to directories it creates.

    The validator reads a directory, so naming one is not enough: a script that
    passes the name of a directory it never made hands the validator nothing to
    read. On a runner the validator then fails, and anywhere the directory
    already exists it validates whatever was left there — an artefact that is
    not this run's, read under a claim that it is.

    Two spellings count. A shell variable set from a command that makes a
    directory — `staged="$(mktemp --directory)"`, which is what a staged
    scratch directory normally looks like — and a directory named to `mkdir`,
    which is how a script that stages somewhere fixed tends to read. Each is
    recognised in the same command segment as the assignment or the command, so
    a variable is never associated with a directory made in a neighbour
    command.

    Returns
    -------
    set[str]
        The variable names and literal directory operands, so a caller
        holding either can ask whether the script created it.
    """
    created: set[str] = set()
    for segment in _command_segments(script):
        if re.search(r"\bmktemp\b", segment):
            # `$(mktemp --directory)` and `$(mktemp -d)` each carry the flag; a
            # plain `mktemp` names a file, which is not a directory to stage.
            if re.search(MKTEMP_DIRECTORY_FLAG, segment):
                created |= _assigned_variables(segment)
            continue
        if re.search(r"\bmkdir\b", segment):
            created |= {
                name.strip("\"'${}") for name in _command_operands(segment, "mkdir")
            }
    return created


def _assigned_variables(segment: str) -> set[str]:
    """Return the names a command segment assigns to."""
    return set(re.findall(r"(?:^|[;&|]\s*|\s)([A-Za-z_]\w*)=", segment))


def _command_operands(segment: str, command: str) -> set[str]:
    """Return the words a command segment hands to ``command``.

    Flags are dropped, so `mkdir --parents -- x` yields `x` rather than the
    options that precede it. A word that names a directory and begins with a
    hyphen is not recognised, which costs nothing here: neither the staged
    directory nor any directory this repository stages is named that way.

    Returns
    -------
    set[str]
        The operand words, empty when the segment does not run ``command``.
    """
    match = re.search(
        rf"\b{re.escape(command)}\b(?P<operands>[^\n]*)",
        segment,
    )
    if match is None:
        return set()
    return {
        word for word in match.group("operands").split() if not word.startswith("-")
    }


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
    match = re.search(
        r"--artifact-dir[=\s]+(?P<directory>\S+)",
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
        re.search(
            rf"\b(?:cp|install)\b(?=[^\n]*{re.escape(COVERAGE_REPORT_PATH)})"
            rf"(?=[^\n]*{re.escape(directory)})[^\n]*",
            segment,
        )
        for segment in _command_segments(script)
    )

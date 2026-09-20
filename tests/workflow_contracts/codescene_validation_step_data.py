"""The validating-step scripts the report-delivery cases are driven with.

A contract test has to be shown a lane that satisfies it before it can vary one
field and assert the offender that results; the inverse holds here, where the
variation *is* the step. Each entry is one such variation — a `run: |` block
that reads as a check and checks nothing, or a one-liner whose line is not its
command — written as the shell text it is, because that is what the predicate
under test reads and a structure expressing the same script would test the
structure. They live here rather than growing the test module past the
repository's 400-line file limit. This module holds no tests of its own.

Every script is built from [`validator_reads`], so a case states only what it
varies: which file is copied, into what, and whether anything created the
directory first.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from ci_coverage_wiring_invariants import COVERAGE_REPORT_PATH
from codescene_report_validation_invariants import REPORT_VALIDATOR_SCRIPT

#: A directory these cases name and never create. It stands in for staging
#: somewhere fixed — a path reused across runs — which is the shape that reads
#: as reuse and behaves as a read of whatever an earlier run left behind.
STAGED_DIRECTORY: typ.Final[str] = "staging"


#: The validator's invocation up to and including the flag, so a case supplies
#: only what the flag is handed. A case with nothing to hand it ends the line
#: there, which is the shape a missing argument takes.
#:
#: The invocation names no interpreter: the contract reads which script is run
#: and which directory it is handed, and says nothing about the version `uv`
#: resolves. A literal here would be a number no test owns — the lane itself
#: reads the baseline from the Makefile — and would sit in the fixture reading
#: as though something held it to the repository's pin.
VALIDATOR_INVOCATION: typ.Final[str] = (
    f"uv run --no-project {REPORT_VALIDATOR_SCRIPT} --artifact-dir"
)


def validator_reads(directory: str) -> str:
    """Return the command that reads ``directory`` through the validator.

    The module path is the same in every case, so it is stated once. What a
    case varies is the argument, and a case varying nothing else reads as the
    one command it is.

    Parameters
    ----------
    directory
        The argument the flag is handed, spelled as the case needs it: a
        quoted or bare variable, or a literal path. An empty string leaves the
        flag with no argument, ending the command at the flag.

    Returns
    -------
    str
        A single shell command, naming no trailing newline.
    """
    return " ".join(part for part in (VALIDATOR_INVOCATION, directory) if part)


#: Statements that read as a check on the report and establish nothing.
#: Each is paired with the text its offender must be reported for, so a case
#: says which fault it expects rather than merely that one occurred.
CHECKS_NOTHING_CASES: typ.Final[list[tuple[str, str]]] = [
    # A step that only looks at the filesystem does not read the report.
    ("test -s lcov.info", REPORT_VALIDATOR_SCRIPT),
    # A validator run over the report in place validates the workspace, which
    # holds more than the one report the validator accepts.
    (
        f"python {REPORT_VALIDATOR_SCRIPT} {COVERAGE_REPORT_PATH}",
        "--artifact-dir",
    ),
    # A staged directory that is never filled validates nothing. The validator
    # would refuse the empty directory and the lane would fail for the wrong
    # reason, having read no part of the report.
    (
        'staged="$(mktemp --directory)"\n' + validator_reads('"$staged"'),
        "must copy",
    ),
    # Moving the report rather than copying it empties the workspace the upload
    # reads it from, so the validation would pass over a report the submission
    # could no longer find.
    (
        'staged="$(mktemp --directory)"\n'
        f'mv -- {COVERAGE_REPORT_PATH} "${{staged}}/{COVERAGE_REPORT_PATH}"\n'
        + validator_reads('"${staged}"'),
        "must copy",
    ),
    # A staged directory the script names but never creates. On a runner the
    # validator is handed a path that does not exist; anywhere the directory
    # survives from an earlier run it reads artefacts belonging to that run,
    # under a claim that they are this one's.
    (
        validator_reads(STAGED_DIRECTORY),
        "must create",
    ),
    # The same fault through a variable, which is the shape a script reaches
    # for once the directory is used more than once: assigning the name is not
    # creating the directory. The copy is present and correct, so the fault
    # reported is the creation rather than the staging.
    (
        f"staged={STAGED_DIRECTORY}\n"
        f'cp -- {COVERAGE_REPORT_PATH} "$staged/{COVERAGE_REPORT_PATH}"\n'
        + validator_reads('"$staged"'),
        "must create",
    ),
    # The invocation *printed* rather than run. Everything else in the script
    # is correct — the directory is made, the report is staged — so the only
    # thing wrong is that the validator never executed, and a reading that
    # found the validator's name in the text would accept it. The step would
    # then submit the very report it exists to reject.
    (
        (
            'staged="$(mktemp --directory)"\n'
            f'cp -- {COVERAGE_REPORT_PATH} "${{staged}}/{COVERAGE_REPORT_PATH}"\n'
            f'echo "{VALIDATOR_INVOCATION} ${{staged}}"'
        ),
        REPORT_VALIDATOR_SCRIPT,
    ),
    # The same name in a comment, which the shell also runs as nothing.
    (
        (
            'staged="$(mktemp --directory)"\n'
            f'cp -- {COVERAGE_REPORT_PATH} "${{staged}}/{COVERAGE_REPORT_PATH}"\n'
            f"# {VALIDATOR_INVOCATION} ${{staged}}"
        ),
        REPORT_VALIDATOR_SCRIPT,
    ),
    # An assignment whose value only *spells* the command that would create a
    # directory. The shell reads `name=value` as an assignment before the
    # command word and as an ordinary argument after it, so this creates
    # nothing — and a reading that credited the argument would record a
    # directory the script never made, then accept the copy into it.
    (
        'echo staged="$(mktemp --directory)"\n'
        f'cp -- {COVERAGE_REPORT_PATH} "$staged/{COVERAGE_REPORT_PATH}"\n'
        + validator_reads('"$staged"'),
        "must create",
    ),
]

#: One-liners, where a line carries several commands. Each is paired with the
#: text its offender must be reported for, or `None` when the script satisfies
#: the contract — the control that keeps the scan from being scoped so finely
#: that a correct one-liner is rejected.
COMMAND_SCOPED_CASES: typ.Final[list[tuple[str, str | None]]] = [
    # The copy and the directory appear on one line but not in one command:
    # the `cp` copies an unrelated file, and the directory is named only by the
    # validator that is handed it. Read line-wide this passes, and the
    # validator then reads an empty staged directory.
    (
        'staged="$(mktemp --directory)"; '
        "cp -- notes.txt notes.txt.bak; " + validator_reads('"$staged"'),
        "must copy",
    ),
    # The same shape joined with `&&`, so the fault is the separator rather
    # than the particular one used.
    (
        'staged="$(mktemp --directory)" && '
        "cp -- notes.txt notes.txt.bak && " + validator_reads('"$staged"'),
        "must copy",
    ),
    # The control: the same one-liner with the copy actually staging the
    # report, which the contract must accept.
    (
        'staged="$(mktemp --directory)" && '
        f'cp -- {COVERAGE_REPORT_PATH} "${{staged}}/{COVERAGE_REPORT_PATH}" && '
        + validator_reads('"$staged"'),
        None,
    ),
    # The flag left bare at the end of a line, with the command that would
    # create a directory on the next. An argument read across the newline is
    # the *next* command's first word, which names a directory nothing staged
    # the report into — and the invocation the validator is handed has no
    # directory argument at all.
    (
        validator_reads("") + "\nmkdir staged\n"
        f'cp -- {COVERAGE_REPORT_PATH} "staged/{COVERAGE_REPORT_PATH}"',
        "--artifact-dir",
    ),
]

#: Directions the copy can be written in. `cp` takes the source first, so a
#: command naming the staged path as its source is taking the report *out* of
#: the directory this contract exists to put it into.
COPY_DIRECTION_CASES: typ.Final[list[tuple[str, str | None]]] = [
    # The staged path as the source and the workspace as the destination. The
    # validator is then handed a directory the report has just left.
    (
        'staged="$(mktemp --directory)"\n'
        f'cp -- "${{staged}}/{COVERAGE_REPORT_PATH}" {COVERAGE_REPORT_PATH}\n'
        + validator_reads('"${staged}"'),
        "must copy",
    ),
    # The control: the same command with its operands the right way round.
    (
        'staged="$(mktemp --directory)"\n'
        f'cp -- {COVERAGE_REPORT_PATH} "${{staged}}/{COVERAGE_REPORT_PATH}"\n'
        + validator_reads('"${staged}"'),
        None,
    ),
    # Three sources, one directory: the report is the first operand and the
    # directory is the *last*, so the destination is not the second. Reading
    # the second as the destination would accept a command that copies the
    # report into a path the validator is never handed.
    (
        'staged="$(mktemp --directory)"\n'
        f'cp -- {COVERAGE_REPORT_PATH} "${{staged}}/other" extra\n'
        + validator_reads('"${staged}"'),
        "must copy",
    ),
    # The control: the same three-operand form with the directory last, which
    # is the shape that does put the report under it.
    (
        'staged="$(mktemp --directory)"\n'
        f'cp -- {COVERAGE_REPORT_PATH} extra "${{staged}}/{COVERAGE_REPORT_PATH}"\n'
        + validator_reads('"${staged}"'),
        None,
    ),
]

#: How a directory can be created for a name that did not receive it. A
#: segment-wide reading of the assignments records every name on the line as
#: made, so a staged directory that is a bare literal passes the creation
#: check on the strength of a neighbour command that made something else.
CREATION_BINDING_CASES: typ.Final[list[tuple[str, str | None]]] = [
    # `mktemp` creates a directory for `scratch`, which `staged` is not.
    (
        f'staged={STAGED_DIRECTORY} scratch="$(mktemp -d)"\n'
        f'cp -- {COVERAGE_REPORT_PATH} "$staged/{COVERAGE_REPORT_PATH}"\n'
        + validator_reads('"$staged"'),
        "must create",
    ),
    # Only the name holding the command counts, so the same shape with
    # `mktemp` assigned to the staged name is accepted.
    (
        f'staged="$(mktemp -d)" scratch={STAGED_DIRECTORY}\n'
        f'cp -- {COVERAGE_REPORT_PATH} "$staged/{COVERAGE_REPORT_PATH}"\n'
        + validator_reads('"$staged"'),
        None,
    ),
]

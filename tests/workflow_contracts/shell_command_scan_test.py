"""Specify the shell reading the workflow contracts rely on.

Three questions are asked of a script's text: which commands it runs, which
words one of them is handed, and which names capture a command's output. These
cases pin the two properties the answers rest on — a command is read where it
*runs* rather than wherever it is mentioned, and the words it is handed keep
the order they were written in.

The mention cases are the load-bearing ones. A rule that certifies work was
done is satisfied by finding the words of the command, so a segment that only
prints or comments those words would satisfy it while running nothing: `echo
"cp a b"` would certify a copy. The span cases are the other direction, and
keep the reading from being tightened so far that a command the lane really
runs stops being recognised.

Run via ``make test-workflow-contracts``.
"""

import pytest
from shell_command_scan import (
    assigned_from,
    command_operands,
    command_segments,
    names_a_component,
)

#: A command segment that runs `cp`, spelled the way the trunk lane writes it.
INVOKES_CP = 'cp -- lcov.info "${staged}/lcov.info"'


@pytest.mark.parametrize(
    ("label", "segment"),
    [
        # The words of the command inside a printed string.
        ("quoted argument", 'echo "cp -- lcov.info ${staged}/lcov.info"'),
        # The same words in a comment, which the shell also runs as nothing.
        ("comment", "# cp -- lcov.info ${staged}/lcov.info"),
        # Another command handed this one's name as an argument.
        ("argument", "echo cp lcov.info"),
        # A path naming a *different* executable, whose remaining words happen
        # to read like the command's.
        ("other executable", "/bin/echo cp lcov.info"),
    ],
)
def test_a_mention_of_a_command_is_not_an_invocation(label: str, segment: str) -> None:
    """Read a command where it runs, not wherever its name appears.

    A mention is satisfied by the same words a rule looks for, and runs
    nothing: a contract reading one would certify a copy the script never made.
    Each case here is a segment whose text contains the words of a copying
    command without copying anything.
    """
    assert not command_operands(segment, "cp"), (
        f"a {label} naming cp is not an invocation of it"
    )


@pytest.mark.parametrize(
    ("label", "segment"),
    [
        # The plain form, as the trunk lane writes it.
        ("bare", INVOKES_CP),
        # Indented, which is what a `run: |` block produces for every command.
        ("indented", f"          {INVOKES_CP}"),
        # Named through an absolute path, which is how a workflow pins an
        # executable when PATH is not trusted.
        ("absolute path", '/usr/bin/cp -- lcov.info "${staged}/lcov.info"'),
        # After environment assignments, which precede the command they apply
        # to rather than being part of it.
        ("with assignment", f"STAGE=x {INVOKES_CP}"),
        ("quoted assignment", f'STAGE="a b" {INVOKES_CP}'),
    ],
)
def test_a_command_is_read_in_every_form_a_lane_writes(
    label: str, segment: str
) -> None:
    """Keep recognising the direct forms, so the reading is not too fine.

    Tightening the reading to reject mentions must not reject a command the
    lane really runs. Each case here runs the copying command and must yield
    its operands.
    """
    assert command_operands(segment, "cp") == ["lcov.info", '"${staged}/lcov.info"'], (
        f"the {label} form runs cp and its operands must be read"
    )


@pytest.mark.parametrize(
    ("label", "segment"),
    [
        # A wrapper the shell runs *before* the command, so the command is not
        # the first word. Reading past it would accept a run this scanner has
        # not understood, so the form is left unrecognised.
        ("wrapper", 'sudo cp -- lcov.info "${staged}/lcov.info"'),
        # The command in a pipeline element, where the leading separator is
        # part of the segment's text.
        ("after separator", 'true && cp -- lcov.info "${staged}/lcov.info"'),
    ],
)
def test_an_unrecognised_form_is_not_silently_accepted(
    label: str, segment: str
) -> None:
    """Leave a form the reading does not cover unrecognised rather than read.

    The reading covers the form the lanes are written in. A command reached
    through a wrapper, or sitting after a separator inside one segment, is not
    that form; reporting it as unrecognised surfaces the gap instead of
    guessing at it.
    """
    assert not command_operands(segment, "cp"), (
        f"the {label} form is not one this reading covers"
    )


@pytest.mark.parametrize(
    ("label", "segment"),
    [
        # An assignment-shaped *argument* to a command that runs nothing. The
        # shell reads `name=value` as an assignment before the command word and
        # as an ordinary argument after it, so the directory is never created —
        # and crediting it records one the script never made.
        ("after a command word", 'echo staged="$(mktemp -d)"'),
        ("as a later argument", 'printf %s staged="$(mktemp -d)"'),
    ],
)
def test_only_a_leading_assignment_counts(label: str, segment: str) -> None:
    """Read an assignment as such only where the shell does.

    Past the command word a `name=value` word is an ordinary argument, and
    nothing is assigned. Crediting it would record a directory the script never
    made, and a staged directory nothing created is one the validator is handed
    empty — on a runner — or holding another run's leavings.
    """
    assert not assigned_from(segment, "mktemp"), (
        f"an assignment-shaped {label} assigns nothing"
    )


@pytest.mark.parametrize(
    ("label", "segment"),
    [
        # A string that merely spells the command. Nothing is run, so no
        # output is captured.
        ("quoted spelling", 'staged="mktemp -d"'),
        # The command inside a printed string.
        ("printed spelling", 'echo "staged=$(mktemp -d)"'),
    ],
)
def test_a_captured_command_has_to_have_been_run(label: str, segment: str) -> None:
    """Credit an assignment only when it captures the command's output.

    A value naming the command as text captures nothing, so treating it as a
    capture records a directory the script never made — and a staged directory
    that satisfies the creation check that way is one the validator is handed
    with nothing in it.
    """
    assert not assigned_from(segment, "mktemp"), (
        f"a {label} of the command runs nothing to capture"
    )


@pytest.mark.parametrize(
    ("label", "segment", "expected"),
    [
        ("quoted", 'staged="$(mktemp --directory)"', {"staged"}),
        ("unquoted", "staged=$(mktemp -d)", {"staged"}),
        # Two names on one line, only one of which holds the command.
        (
            "beside another",
            'staged=staging scratch="$(mktemp -d)"',
            {"scratch"},
        ),
        # The names reversed, so the reader cannot be right by position alone.
        (
            "reversed",
            'scratch="$(mktemp -d)" staged=staging',
            {"scratch"},
        ),
    ],
)
def test_a_captured_command_is_bound_to_its_own_name(
    label: str, segment: str, expected: set[str]
) -> None:
    """Bind the capture to the name that received it, and to no other.

    A segment can assign several names and run one command. Read as a set, each
    name on the line is credited with the directory, and a staged directory
    that is a bare literal nothing created then passes the creation check on
    the strength of the neighbour assignment that made something else.
    """
    assert assigned_from(segment, "mktemp") == expected, (
        f"only the {label} name holding the command may be credited"
    )


def test_a_continued_line_is_joined_into_the_command_it_completes() -> None:
    """Join a line continuation, because the shell joins it.

    The backslash and the newline after it are removed before the command is
    parsed, so the two lines are one command. Read apart, the continued
    fragment would pass for a command of its own: an `echo` continued into a
    line naming a script would read as that script being run, which is the
    opposite of what the shell does with it.
    """
    script = "echo hello \\\n  scripts/validate_coverage_artifact.py"
    assert command_segments(script) == [
        "echo hello   scripts/validate_coverage_artifact.py"
    ], "the continuation is removed and the two lines are one command"


def test_a_continuation_inside_a_quoted_string_is_still_joined() -> None:
    """Join it wherever it appears, which is also what the shell does.

    The shell removes a trailing backslash before it reads quotes at all, so
    the join is not conditional on being outside one. Joining is the safe
    direction here as everywhere: it can only merge two segments into one,
    which makes a rule asking after a single command harder to satisfy.
    """
    script = 'echo "one \\\n  two"'
    assert command_segments(script) == ['echo "one   two"'], (
        "the shell removes the backslash and newline before parsing quotes"
    )


def test_a_line_is_split_into_the_commands_it_runs() -> None:
    """Separate the commands a one-liner joins, so no rule spans the join.

    A rule asking that two things appear in one command is only made harder to
    satisfy by cutting a line more finely, so the split is the safe direction:
    a script misjudged here is reported rather than passed.
    """
    script = 'staged="$(mktemp --directory)"; cp -- notes.txt notes.txt.bak; true'
    assert command_segments(script) == [
        'staged="$(mktemp --directory)"',
        " cp -- notes.txt notes.txt.bak",
        " true",
    ], "each simple command on the line is its own segment"


@pytest.mark.parametrize(
    ("operand", "name", "prefix", "expected"),
    [
        # --- the source position: the operand has to name the report ---
        pytest.param("lcov.info", "lcov.info", False, True, id="the-report-itself"),
        pytest.param("./lcov.info", "lcov.info", False, True, id="the-report-relative"),
        # A file whose name merely ends in the report's is a different file.
        pytest.param("my_lcov.info", "lcov.info", False, False, id="a-longer-name"),
        # A file whose name merely begins with the report's is a different file.
        pytest.param("lcov.info.bak", "lcov.info", False, False, id="a-shorter-name"),
        # --- the destination position: the directory has to be named ---
        # The ordinary spelling, where the file lands under the directory.
        pytest.param(
            '"${staged}/lcov.info"', "staged", True, True, id="the-report-under-it"
        ),
        pytest.param('"${staged}"', "staged", True, True, id="the-directory-alone"),
        # A later component is a different location entirely: the file copied
        # there lands nowhere the caller is about to read.
        pytest.param(
            '"/tmp/${staged}"', "staged", True, False, id="it-as-a-later-component"
        ),
        # A sibling whose name merely begins with the directory's is another
        # directory, and the file copied into it misses the staged one.
        pytest.param(
            '"${staged}-old/lcov.info"',
            "staged",
            True,
            False,
            id="a-sibling-name",
        ),
        pytest.param(
            '"/x/staged2/lcov.info"', "staged", True, False, id="a-sibling-directory"
        ),
    ],
)
def test_a_name_is_read_as_a_path_component(
    operand: str, name: str, *, prefix: bool, expected: bool
) -> None:
    """Compare names component-wise, so a path is not read as a substring.

    The two positions ask different questions of the same operand. A *source*
    names the file, and the file it names has to be the report: a component
    equal to the name, so neither `my_lcov.info` nor `lcov.info.bak` is it. A
    *destination* names the directory, and the directory has to be the head of
    the path — `"${staged}/x"` puts a file under it, while `"/tmp/${staged}"`
    puts one elsewhere and `"${staged}-old/x"` misses it from the start.
    """
    assert names_a_component(operand, name, prefix=prefix) is expected, (
        f"{operand!r} vs {name!r} (prefix={prefix})"
    )

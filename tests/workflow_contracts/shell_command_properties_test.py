"""Properties for the shell-command scanner, over generated scripts.

The example-based suite in ``shell_command_scan_test`` states one spelling at a
time what a form reads as. These properties state the same contracts over scripts
built from the grammar, using ``shell_command_forms`` as the oracle: the plan
that built a script says what each scanner function should return, so a
disagreement is between the scanner and an independent account of the text
rather than between two readings of it.

No shell is executed, here or in the model. A generated script never has to be
something a shell would accept, and the properties are stated over what the
scanners should *read*, which keeps the suite free of the platform differences
a real shell would introduce.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from hypothesis import example, given, settings
from hypothesis import strategies as st
from shell_command_forms import (
    COMMAND_NAMES,
    MENTIONABLE_COMMANDS,
    PLAIN_OPERANDS,
    PRINT_COMMANDS,
    SINGLE_COMMANDS,
    Assignment,
    Invocation,
    Operands,
    Script,
    mention_forms,
    scripts,
)
from shell_command_scan import (
    assigned_from,
    command_operands,
    command_segments,
    script_operands,
)

#: A segment long enough to hold the longest generated plan's words.
#:
#: Three assignments, a path-prefixed command, and four operands is eight
#: words; the extra room lets a caller ask for a split point past the end,
#: which is a continuation the shell would not join and which the model
#: therefore renders as no continuation at all.
MAXIMUM_WORDS: typ.Final[int] = 12

#: The command a generated script runs when it runs one at all.
#:
#: `cp` is used because a copying command is the one whose operand *order*
#: carries meaning, so it is the command a caller most often asks about.
COPY_COMMAND: typ.Final[str] = "cp"

#: Commands no generated script runs, and so no segment may read as.
#:
#: The complement of the vocabulary a plan draws from, plus `true`, which only
#: ever appears as the command a commented-out mention runs. A property asks
#: that a segment reads as none of these, which fails if the scanner matched a
#: command name it was handed as an argument or found in a comment.
UNRUN_COMMANDS: typ.Final[tuple[str, ...]] = (
    *[name for name in COMMAND_NAMES if name != COPY_COMMAND],
    "true",
    "sudo",
    "xargs",
)


def _segments_of(invocation: Invocation, text: str) -> list[str]:
    r"""Return the segments ``text`` reads as, requiring exactly one.

    A generated plan is one simple command, so anything but a single segment is
    the scanner dividing a command it should have read whole. The requirement
    is stated once, here, rather than in each property that needs it: a
    property about operands would otherwise compare against the operands of
    some segment rather than of *the* segment, and a cut command would pass.

    Parameters
    ----------
    invocation
        The plan ``text`` was rendered from, named for the failure message.
    text
        The rendered command.

    Returns
    -------
    list[str]
        The single segment, as a list so a caller can destructure it.
    """
    segments = command_segments(text)
    assert len(segments) == 1, (
        f"{invocation.command!r} is one command, one segment: {text!r} -> {segments!r}"
    )
    return segments


@settings(max_examples=200, derandomize=True, deadline=None)
@example(
    invocation=Invocation(
        command=COPY_COMMAND,
        operands=Operands.of(("lcov.info", "${STAGED}/lcov.info")),
        path_prefix="/usr/bin/",
    )
)
@example(
    invocation=Invocation(
        command=COPY_COMMAND,
        operands=Operands.of(("a", "--b")),
    )
)
@given(invocation=SINGLE_COMMANDS)
def test_a_single_command_reads_as_exactly_its_own_plan(
    invocation: Invocation,
) -> None:
    """Read one command's own command, operands, and captured names.

    The three are asserted together because they are one reading: a plan that
    recovered the operands but attributed them to another command would satisfy
    an operand property alone, and a rule built on that reading would certify
    the wrong command's work.
    """
    text = invocation.render()
    (segment,) = _segments_of(invocation, text)

    assert command_operands(segment, invocation.command) == list(
        invocation.operands.read
    ), f"operands of {invocation.command!r} in {text!r}"
    assert assigned_from(segment, "mktemp") == invocation.captured_names(), (
        f"captured names in {text!r}"
    )
    for other in UNRUN_COMMANDS:
        if other == invocation.command:
            continue
        assert not command_operands(segment, other), (
            f"{text!r} runs {invocation.command!r}, not {other!r}"
        )


@settings(max_examples=200, derandomize=True, deadline=None)
@example(
    invocation=Invocation(
        command=COPY_COMMAND,
        operands=Operands.of(("report", "staging")),
    ),
    split_after=1,
)
@example(
    invocation=Invocation(
        command=COPY_COMMAND,
        operands=Operands.of(("staging",)),
        assignments=(Assignment(name="STAGED", literal="", captures=True),),
    ),
    split_after=2,
)
@given(
    invocation=SINGLE_COMMANDS,
    split_after=st.integers(min_value=0, max_value=MAXIMUM_WORDS),
)
def test_a_continuation_is_the_command_it_continues(
    invocation: Invocation,
    split_after: int,
) -> None:
    """Read a continued command exactly as the command it continues.

    A backslash-newline is removed by the shell before it parses the command,
    so splitting at *any* point is the identity. This is the property that
    makes the join safe: were the continuation read apart, the fragment would
    pass for a command of its own and a plan naming a script would read as that
    script being run.
    """
    text = invocation.continued(split_after)
    (segment,) = _segments_of(invocation, text)

    assert command_operands(segment, invocation.command) == list(
        invocation.operands.read
    ), f"operands survive the continuation in {text!r}"
    assert assigned_from(segment, "mktemp") == invocation.captured_names(), (
        f"captures survive the continuation in {text!r}"
    )


@settings(max_examples=150, derandomize=True, deadline=None)
@given(script=scripts())
def test_a_script_reads_as_the_commands_it_joins(script: Script) -> None:
    """Read one segment per joined command, each with its own plan.

    The operators are where a line-based reading fails: a plan asserting that
    two things appear in one command would read a one-liner as a single command
    and find them across a boundary the shell does not respect. Reading one
    segment per command is what makes such an assertion answer about the
    command it names.
    """
    text = script.render()
    segments = command_segments(text)
    assert len(segments) == len(script.invocations), f"{text!r} -> {segments!r}"
    for segment, invocation in zip(segments, script.invocations, strict=True):
        expected = list(invocation.operands.read)
        assert command_operands(segment, invocation.command) == expected, (
            f"operands of {invocation.command!r} in {segment!r}"
        )
        assert assigned_from(segment, "mktemp") == invocation.captured_names(), (
            f"captured names in {segment!r}"
        )


@settings(max_examples=150, derandomize=True, deadline=None)
@example(
    command=COPY_COMMAND,
    operands=PLAIN_OPERANDS.example(),
)
@given(
    command=st.sampled_from(MENTIONABLE_COMMANDS),
    operands=st.lists(PLAIN_OPERANDS, max_size=3),
)
def test_naming_a_command_is_not_running_it(
    command: str,
    operands: list[str],
) -> None:
    """Read a mention as the printing command, never as the command named.

    A rule asking that a file be copied is satisfied by finding `cp` where a
    command runs. A segment that prints, quotes, or comments `cp src dest`
    mentions it and runs `echo`, and reading the mention would certify work the
    script never did.
    """
    for text, runs in mention_forms(command, tuple(operands)):
        segments = command_segments(text)
        assert [
            name
            for name in COMMAND_NAMES
            if any(command_operands(segment, name) for segment in segments)
        ] == [name for name in runs if name in COMMAND_NAMES], (
            f"{text!r} should run {runs!r}"
        )
        assert not any(command_operands(segment, command) for segment in segments), (
            f"{text!r} only mentions {command!r}"
        )


@settings(max_examples=120, derandomize=True, deadline=None)
@example(operands=("scripts/validate_coverage_artifact.py",))
@example(operands=("run", "scripts/validate_coverage_artifact.py"))
@example(operands=("--locked", "run", "scripts/validate_coverage_artifact.py"))
@example(operands=("python", "scripts/coverage.py"))
@given(
    operands=st.lists(
        st.sampled_from((
            "run",
            "scripts/coverage.py",
            "--locked",
            "python",
            "uv",
        )),
        max_size=3,
    )
)
def test_only_the_multiplexer_subcommand_hands_over_a_script(
    operands: list[str],
) -> None:
    """Return a multiplexer's operands only after its `run` subcommand.

    `uv` reads the word after it as a subcommand *name*, so a path handed to it
    is refused rather than run. A reader looking for the path among `uv`'s
    operands would report a script as run that `uv` never ran, so the
    subcommand has to lead the operands for any of them to be a script.

    The comparison is against the *flag-free* operands, because that is the
    list the subcommand is read from: `uv --locked run x` hands over `x` just as
    `uv run x` does. Were the check made against the operands as written, a
    multiplexer invocation carrying a flag would be read as running no script
    at all — the unsafe direction, since a rule asking that a script be run
    would then be reported rather than passed.
    """
    words = [word for word in operands if not word.startswith("-")]
    expected = words[1:] if words[:1] == ["run"] else []
    segment = f"uv {' '.join(operands)}"
    assert script_operands(segment, "uv") == expected, segment

    # A command that is not the multiplexer has no subcommand to skip, so its
    # operands are the script's candidates as written.
    interpreter = f"python {' '.join(operands)}"
    assert script_operands(interpreter, "python") == words, interpreter


@settings(max_examples=60, derandomize=True, deadline=None)
@given(command=st.sampled_from(PRINT_COMMANDS))
def test_a_path_shaped_word_names_the_command_after_it(command: str) -> None:
    r"""Read a command name after any `/`-suffixed word, by design.

    The prefix pattern admits `\\S*/` wherever it appears, so a word spelling
    `a/cp` is read as the path `a/` naming `cp`, and the words after it are
    read as that command's operands. This is the over-approximation the module
    documents: a word that merely contains a slash is not a path, and reading
    it as one can report a command the shell would never run.

    The prefix only bites where no command has been read yet, which is why the
    word is written as the *leading* word here. A command occupies the position
    the prefix is matched from — after it, a path-shaped word is an ordinary
    argument, and `echo a/cp b` runs `echo` with the words `a/cp` and `b`.
    """
    text = f"a/{COPY_COMMAND} b"
    (segment,) = _segments_of(Invocation(command=command), text)
    assert command_operands(segment, COPY_COMMAND) == ["b"], text
    assert command_operands(segment, command) == [], text

    # The same words after a command are that command's operands, not a
    # second invocation.
    handed = f"{command} a/{COPY_COMMAND} b"
    (segment,) = _segments_of(Invocation(command=command), handed)
    assert command_operands(segment, COPY_COMMAND) == [], handed
    assert command_operands(segment, command) == [f"a/{COPY_COMMAND}", "b"], handed

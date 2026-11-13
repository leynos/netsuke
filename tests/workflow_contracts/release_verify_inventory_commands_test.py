"""Hold the shell-reading controls to the shapes bash really runs.

The contract test asserts that the verification step's scans pass over the
checked-in action. Each such assertion is only as good as its reach: a scan
anchored at the start of a line misses a call inside a substitution, a scan of
raw lines misses a command continued across a backslash-newline, and a scan of
command names misses one built from a variable. These cases are the controls
for ``release_verify_inventory_commands`` -- they show the scans do reach those
shapes, and refuse the ones they cannot read rather than passing them
unexamined.

Run via ``make test-workflow-contracts``.
"""

import pytest
from release_verify_inventory_commands import (
    GH_RELEASE_CALL,
    forbidden_gh_flags,
    gh_api_invocations,
    indirect_command_words,
    reject_secrets,
    run_tokens,
    script_lines,
)


@pytest.mark.parametrize(
    "reference",
    [
        "${{ secrets.CARGO_REGISTRY_TOKEN }}",
        "${{ secrets['CARGO_REGISTRY_TOKEN'] }}",
        "${{ toJSON(secrets) }}",
        "${{ format('Hello {0}', secrets.CARGO_REGISTRY_TOKEN) }}",
    ],
)
def test_the_secret_scan_reaches_nested_values(reference: str) -> None:
    """Reject a secret reference however it is spelled and however deeply nested.

    The control for :func:`reject_secrets`. Without it the scan could be
    vacuous — checking only top-level keys — and every assertion calling it
    would still pass. The bracketed and whole-context spellings are the ones a
    dotted-substring check would let through.
    """
    with pytest.raises(AssertionError):
        reject_secrets({"steps": [{"env": {"TOKEN": reference}}]})


def test_a_call_inside_a_substitution_is_still_scanned() -> None:
    """Find a write hidden in ``$(...)``, which a line-anchored scan misses.

    The contract suite's mutation cases show the contract rejects the shape;
    this one shows why. The scan has to locate the call wherever it sits on the
    line, and the flag check has to see the ``-f`` that turns it into a POST.
    """
    script = 'body=$(gh api --method GET -f x=y "repos/o/r/releases")\n'
    (tokens,) = gh_api_invocations(script)
    assert forbidden_gh_flags(tokens) == ["-f"], tokens


def test_a_continued_command_is_scanned_after_joining() -> None:
    r"""Read a command split across a backslash-newline as bash does.

    ``gh \`` then ``api --method POST`` is one command whose method override
    no raw-line scan can see, and a separate ordinary call would satisfy the
    expected count. The ``midword`` spellings split the subcommand name itself,
    which bash joins with nothing between the halves, so a join that inserted a
    space would still miss them. Both shapes must expose the write, and the
    release subcommand split either way must be caught by the release check.
    """
    script = (
        'gh \\\n  api --method POST "repos/o/r/releases"\n'
        'gh a\\\npi --method POST "repos/o/r/releases"\n'
        "gh \\\n  release delete v1\n"
        "gh re\\\nlease delete v1\n"
    )
    invocations = gh_api_invocations(script)
    assert [forbidden_gh_flags(tokens) for tokens in invocations] == [
        ["--method"],
        ["--method"],
    ], invocations
    releases = [line for line in script_lines(script) if GH_RELEASE_CALL.search(line)]
    assert len(releases) == 2, (
        f"a continued release command must still be recognised, got {releases}"
    )


@pytest.mark.parametrize(
    "line",
    [
        'api=gh; "$api" api --method POST "repos/o/r/releases"',
        'body=$(api=gh; "$api" api --method POST "repos/o/r/releases")',
        'method=gh; ${method} api --method POST "repos/o/r/releases"',
    ],
)
def test_a_command_word_built_from_a_variable_is_rejected(line: str) -> None:
    """Reject a command the literal scan cannot name, wherever it sits.

    Each line executes a POST while naming no ``gh api`` call the scan can
    read, so the count and flag checks both pass. The last two spellings place
    the call inside a command substitution and behind a braced expansion, which
    a scan of command words has to reach rather than assume away.
    """
    indirect = indirect_command_words(line)
    assert indirect, f"{line!r} builds a command from a variable"


def test_a_test_expression_operand_is_not_a_command() -> None:
    """Read ``||`` inside ``[[ ]]`` as a test operator, not a separator.

    The action's own loop conditions use that shape, so a scan that treated
    every ``||`` as a command boundary would refuse the action it is meant to
    hold.
    """
    script = (
        'while read -r name || [[ -n "$name" ]]; do\n  printf \'%s\\n\' "$name"\ndone\n'
    )
    assert not indirect_command_words(script)


def test_a_line_the_tokenizer_cannot_read_is_refused() -> None:
    """Refuse a line whose quoting the word scan cannot resolve.

    A double quote inside a command substitution belongs to the substitution,
    but a tokenizer that does not model one reads it as closing the enclosing
    quote. The scan cannot then say which word is a command, so it refuses the
    line rather than let it pass unexamined.
    """
    with pytest.raises(AssertionError):
        indirect_command_words("printf '%s\\n' \"$(printf '%s' '\"')\"")


def test_a_second_verifier_invocation_is_rejected() -> None:
    """Refuse a step that runs the pinned verifier twice.

    The first call's tokens match the pin, so a contract that read only the
    first would accept a second call handing the verifier another trusted
    record.
    """
    command = (
        "uv run --no-project --python 3.14 scripts/verify_release_inventory_cli.py"
        " --directory d --expected e --trusted t --output o"
    )
    with pytest.raises(AssertionError):
        run_tokens(f"{command}\n{command.replace('--trusted t', '--trusted x')}\n")

"""Hold the shell-reading controls to the shapes bash really runs.

The contract test asserts that the verification step's scans pass over the
checked-in action. Each such assertion is only as good as its reach: a scan
anchored at the start of a line misses a call inside a substitution, a scan of
raw lines misses a command continued across a backslash-newline, a scan of
command names misses one built from a variable, a scan of the raw text misses a
subcommand written in quotes, and no scan of positions can tell what an
interpreter is handed. These cases are the controls
for the ``release_verify_inventory_command_words`` and
``release_verify_inventory_commands`` modules -- they show the scans do reach
those shapes, and refuse the ones they cannot read rather than passing them
unexamined.

Run via ``make test-workflow-contracts``.
"""

import pytest
from release_verify_inventory_command_words import (
    forbidden_executor_words,
    indirect_command_words,
)
from release_verify_inventory_commands import (
    forbidden_gh_flags,
    gh_api_invocations,
    gh_release_invocations,
    reject_secrets,
    run_tokens,
    unread_gh_subcommands,
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
    releases = gh_release_invocations(script)
    assert len(releases) == 2, (
        f"a continued release command must still be recognised, got {releases}"
    )


@pytest.mark.parametrize(
    "line",
    [
        'api=gh; "$api" api --method POST "repos/o/r/releases"',
        'body=$(api=gh; "$api" api --method POST "repos/o/r/releases")',
        'method=gh; ${method} api --method POST "repos/o/r/releases"',
        'api=gh; command "$api" api --method POST "repos/o/r/releases"',
        'api=gh; env EXTRA=1 "$api" api --method POST "repos/o/r/releases"',
        'api=gh; xargs -n 1 "$api" api --method POST < releases.txt',
        'suffix=h; g"$suffix" api --method POST "repos/o/r/releases"',
        'suffix=h; command g"$suffix" api --method POST "repos/o/r/releases"',
        'api=gh; EXTRA=1 command "$api" api --method POST "repos/o/r/releases"',
        'api=gh; command -p "$api" api --method POST "repos/o/r/releases"',
        'api=gh; env -i "$api" api --method POST "repos/o/r/releases"',
        'api=gh; xargs -r "$api" api --method POST < releases.txt',
        'api=gh; command env "$api" api --method POST "repos/o/r/releases"',
        'api=gh; env command "$api" api --method POST "repos/o/r/releases"',
        'api=gh; nohup env -u HOME "$api" api --method POST "repos/o/r/releases"',
    ],
)
def test_a_command_word_built_from_a_variable_is_rejected(line: str) -> None:
    """Reject a command the literal scan cannot name, wherever it sits.

    Each line executes a POST while naming no ``gh api`` call the scan can
    read, so the count and flag checks both pass. The fourth spelling places
    the call inside a command substitution, the fifth behind a braced
    expansion, and the next three behind a wrapper -- ``command``, ``env`` and
    ``xargs`` -- whose options and assignments come before the word that is
    actually run. The next three reach the word by other routes: a command
    word that carries the expansion after a literal prefix, the same word
    behind a wrapper, and a wrapper behind an environment assignment. The last
    three put a valueless option first, which a scan that assumed every option
    consumed the next word would read as that word rather than as the command.
    The last three nest one wrapper inside another, where a scan that read the
    second wrapper as the command would take the expansion after it for an
    argument. A scan of command words has to reach each of those rather than
    assume away the ones it does not model.
    """
    indirect = indirect_command_words(line)
    assert indirect, f"{line!r} builds a command from a variable"


def test_a_wrapper_around_a_literal_command_is_not_indirect() -> None:
    """Accept a wrapper whose operand is written out.

    The control for the wrapper cases above: they must be refused because the
    command word is an expansion, not because a wrapper appeared. Without it,
    refusing every wrapped command would satisfy them while rejecting the
    wording a step may legitimately use.
    """
    assert not indirect_command_words(
        'command gh api --method GET "repos/o/r/releases"\n'
    ), "a wrapper around a literal command must not read as indirect"
    assert not indirect_command_words(
        'command -p gh api --method GET "repos/o/r/releases"\n'
    ), "a valueless wrapper option must not make a literal command indirect"
    assert not indirect_command_words(
        'command env gh api --method GET "repos/o/r/releases"\n'
    ), "a nested wrapper around a literal command must not read as indirect"


@pytest.mark.parametrize(
    "line",
    [
        'subcommand=api; gh "$subcommand" --method POST "repos/o/r/releases"',
        'subcommand=api; gh ${subcommand} --method POST "repos/o/r/releases"',
        'subcommand=api; gh "$subcommand" -f tag_name=v1',
        'prefix=a; gh "${prefix}pi" --method POST "repos/o/r/releases"',
        'subcommand=api; command gh "$subcommand" --method POST "repos/o/r"',
        'subcommand=api; gh "$subcommand" \\\n  --method POST "repos/o/r"',
        'subcommand=api; gh "a$(printf pi)" --method POST "repos/o/r"',
        "subcommand=checkout; gh $subcommand --method POST repos/o/r",
    ],
)
def test_a_gh_subcommand_the_scan_cannot_read_is_rejected(line: str) -> None:
    """Refuse a ``gh`` call whose subcommand the scan cannot read.

    The command word here is the literal ``gh``, which every position check
    accepts; the write hides in the word after it. Bash expands that word before
    ``gh`` runs, so ``gh "$subcommand" --method POST`` issues the POST that the
    ``gh api`` check has no call to inspect. The spellings cover a bare
    expansion, a braced one, a bare word with the write flags attached, a
    literal prefix joined to the expansion, a wrapper before the command, a
    call continued across a backslash-newline, an expansion inside a quoted
    word, and a subcommand outside the two the contract has a rule for.
    """
    unread = unread_gh_subcommands(line)
    assert unread, f"{line!r} names a gh subcommand the scan cannot read"


def test_a_quoted_gh_subcommand_is_read_as_the_call_it_names() -> None:
    """Read ``gh "api"`` as the api call bash runs, whatever the quotes.

    Quotes are bash's, not ``gh``'s: it receives ``api`` either way. A scan of
    the raw text finds no ``gh api`` in the quoted spelling, so a write written
    that way would be neither counted nor checked. The subcommand therefore
    comes from the tokenized words, which have no quotes left, and the call
    lands where the flag and release checks can judge it.
    """
    quoted_post = 'gh "api" --method POST -f tag_name=v1 "repos/o/r/releases"\n'
    (tokens,) = gh_api_invocations(quoted_post)
    assert tokens[:2] == ["gh", "api"], tokens
    assert forbidden_gh_flags(tokens) == ["--method", "-f"], tokens
    quoted_release = "gh 'release' delete v1\n"
    (release,) = gh_release_invocations(quoted_release)
    assert release[:2] == ["gh", "release"], release


def test_a_literal_gh_subcommand_is_not_rejected() -> None:
    """Accept the action's own listing call, whose arguments expand.

    The control for the cases above: they must be refused because the
    *subcommand* cannot be read, not because a variable or a quote appears in
    the call. The listing step names its release path with a variable, and a
    rule that refused any expanded argument would reject the action it is meant
    to hold.
    """
    for call in (
        'gh api --method GET --paginate --slurp "repos/$GITHUB_REPOSITORY/x"\n',
        'gh "api" --method GET --paginate --slurp "repos/$GITHUB_REPOSITORY/x"\n',
    ):
        assert not unread_gh_subcommands(call), (
            f"a readable subcommand must not be refused: {call!r}"
        )
        assert len(gh_api_invocations(call)) == 1, call


@pytest.mark.parametrize(
    "line",
    [
        'body=POST; bash -c "$body"',
        'body=POST; sh -c "$body"',
        'body=POST; eval "$body"',
        'body=POST; command eval "$body"',
        'body=POST; command -p bash -c "$body"',
        'body=POST; xargs -r bash -c "$body" < releases.txt',
        'body=POST; python3 -c "$body"',
        'body=POST; source "$body"',
    ],
)
def test_a_command_that_runs_unread_text_is_rejected(line: str) -> None:
    """Refuse a step that hands a run-time string to an interpreter.

    Placing a command word answers where bash looks for the command, not what
    an interpreter is then told to run: each line above assembles a ``gh api``
    call at run time, so the literal scan sees nothing to check however
    carefully the words before it are placed. The spells vary in how the
    interpreter is reached -- directly, behind ``eval``, behind a wrapper, or
    behind a wrapper's option -- because a rule that only refused the first
    would be satisfied by the rest.
    """
    refused = forbidden_executor_words(line)
    assert refused, f"{line!r} runs text the scan cannot read"


def test_a_pinned_script_path_is_not_an_interpreter_invocation() -> None:
    """Accept the action's own verifier call, which names its script by path.

    The control for the cases above: they must be refused because the command
    executes a run-time string, not because an interpreter's name appeared.
    ``uv`` runs a file from the repository, whose commands the contract reads
    directly.
    """
    assert not forbidden_executor_words(
        "uv run --no-project --python 3.14 "
        "scripts/verify_release_inventory_cli.py --directory d\n"
    ), "a pinned script path is not a run-time string"


def test_a_test_expression_operand_is_not_a_command() -> None:
    """Read ``||`` inside ``[[ ]]`` as a test operator, not a separator.

    The action's own loop conditions use that shape, so a scan that treated
    every ``||`` as a command boundary would refuse the action it is meant to
    hold.
    """
    script = (
        'while read -r name || [[ -n "$name" ]]; do\n  printf \'%s\\n\' "$name"\ndone\n'
    )
    assert not indirect_command_words(script), (
        "a variable used as a test operand must not read as a command"
    )


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

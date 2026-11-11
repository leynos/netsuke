"""Prove the shell reader agrees with bash on continuations and quoting.

``release_verify_inventory_support`` runs its read-only checks over these
logical lines, so a rule the reader gets wrong either hides a write (the
dangerous direction) or invents one the step does not make (a contract that
fails for the wrong reason). Each case here states one rule the shell applies
before it parses.

Run via ``make test-workflow-contracts``.
"""

import pytest
from release_verify_inventory_shell import logical_lines


def test_separate_lines_stay_separate_commands() -> None:
    """Keep ordinary commands apart; only a backslash joins two lines."""
    assert logical_lines("gh api one\ngh api two\n") == ["gh api one", "gh api two"], (
        "a newline with no backslash must end the command"
    )


def test_a_continuation_inside_a_word_joins_without_a_space() -> None:
    r"""Join ``gh a\`` and ``pi`` into ``gh api``, as deleting the pair does.

    This is the direction that hides a write: inserting a space would leave
    ``gh a pi``, which the call scan would not recognise.
    """
    assert logical_lines("gh a\\\npi --method POST\n") == ["gh api --method POST"], (
        "the deleted pair must leave the halves of the word adjacent"
    )


def test_a_continuation_after_a_space_keeps_the_space() -> None:
    r"""Keep the whitespace the author wrote before the backslash.

    The pair is deleted, not replaced, so ``gh \`` plus two spaces and ``api``
    becomes ``gh`` followed by three spaces and ``api``.
    """
    assert logical_lines("gh \\\n  api --method GET\n") == ["gh   api --method GET"], (
        "the space before the backslash must survive the join"
    )


def test_a_doubled_backslash_does_not_continue_the_line() -> None:
    r"""Read ``\\`` as an escaped backslash, not as a continuation.

    A continuation needs an odd run of backslashes before the newline; an even
    run escapes itself and the newline ends the command.
    """
    assert logical_lines("printf a\\\\\nls\n") == ["printf a\\\\", "ls"], (
        "an escaped backslash must not swallow the next line"
    )


def test_a_single_quoted_continuation_is_literal() -> None:
    r"""Keep a backslash-newline inside single quotes as ordinary text.

    Bash deletes the pair only where a backslash still escapes. Inside single
    quotes it is literal, so the command spans both lines without being joined.
    """
    assert logical_lines("printf 'a\\\nb'\n") == ["printf 'a\\\nb'"], (
        "single quotes must suspend the deletion"
    )


def test_a_double_quoted_continuation_is_removed() -> None:
    r"""Delete a backslash-newline inside double quotes, as bash does.

    Double quotes suspend the shell's other special characters but not this
    deletion, so the two halves of the word become one.
    """
    assert logical_lines('printf "a\\\nb"\n') == ['printf "ab"'], (
        "double quotes must not suspend the deletion"
    )


def test_a_comment_ending_in_a_backslash_names_no_command() -> None:
    r"""Drop a comment instead of letting its backslash swallow the next line.

    A comment is not a command at all, so the line after it must still be
    read -- and it is the line that carries the call being checked.
    """
    assert logical_lines("# note \\\ngh api --method GET x\n") == [
        "gh api --method GET x"
    ], "a comment must not consume the command that follows it"


def test_a_hash_inside_a_word_is_not_a_comment() -> None:
    """Keep a ``#`` that does not start a word, such as the ``$#`` parameter."""
    assert logical_lines("echo $#\n") == ["echo $#"], (
        "a hash inside a word must stay part of the command"
    )


def test_trailing_comment_text_is_dropped_from_a_command() -> None:
    """End a command at its comment, which names no part of it."""
    assert logical_lines("gh api x # trailing\ny\n") == ["gh api x", "y"], (
        "a trailing comment must not become part of the command"
    )


def test_an_escaped_space_keeps_a_hash_inside_the_word() -> None:
    r"""Read ``foo\ #`` as one argument, so the ``#`` opens no comment.

    The escaped space does not end the word, so the ``#`` is ordinary text and
    the command after the ``;`` still runs. A reader that treats any space as a
    word boundary discards that command -- the one carrying the call.
    """
    script = "printf '%s\\n' foo\\ #; gh api --method POST repos/o/r/releases\n"
    assert logical_lines(script) == [
        "printf '%s\\n' foo\\ #; gh api --method POST repos/o/r/releases"
    ], "a hash after an escaped space must stay inside the argument"


def test_an_escaped_space_does_not_hide_a_release_call() -> None:
    r"""Keep a ``gh release`` after ``foo\ #`` visible to the release check.

    The same reading decides this case: if the ``#`` opened a comment, the
    release subcommand would be dropped before any check saw it.
    """
    script = "true foo\\ #; gh release delete v1\n"
    assert logical_lines(script) == ["true foo\\ #; gh release delete v1"], (
        "a hash after an escaped space must not swallow a release command"
    )


def test_a_quote_inside_a_substitution_does_not_close_the_outer_quote() -> None:
    r"""Keep the enclosing quote open across a nested command substitution.

    Bash parses the text of ``$(...)`` as a fresh command, so the ``"`` inside
    it quotes that command and does not end the ``"`` that encloses the whole
    substitution. A reader with a single quote state closes the outer quote
    there, reads the following ``'`` as an opening quote, and swallows the rest
    of the script -- including the continued call on the next line.
    """
    script = (
        "printf '%s\\n' \"$(printf '%s' '\"')\"\n"
        "gh a\\\npi --method POST repos/o/r/releases\n"
    )
    assert logical_lines(script) == [
        "printf '%s\\n' \"$(printf '%s' '\"')\"",
        "gh api --method POST repos/o/r/releases",
    ], "a nested substitution must not close the quote that encloses it"


def test_an_unbalanced_quote_fails_instead_of_swallowing_the_script() -> None:
    """Refuse a script whose quoting the reader cannot resolve.

    The reader cannot say which command the remaining text belongs to, so it
    must fail rather than let a call pass unexamined.
    """
    with pytest.raises(AssertionError, match="ends inside a quote"):
        logical_lines("printf 'unterminated\ngh api --method POST x\n")


def test_a_trailing_backslash_at_the_end_of_the_script_is_kept() -> None:
    """Stop at the end of the script rather than reading past it."""
    assert logical_lines("gh api \\") == ["gh api \\"], (
        "a final backslash must not read past the end of the script"
    )

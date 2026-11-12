r"""Read a workflow step's shell script the way bash parses it.

The inventory verifier must issue only ``gh api --method GET`` and must never
call ``gh release``. Both contract checks read the step's ``run`` text, and a
check that reads raw lines can be satisfied by a write the shell still
performs: bash deletes every backslash-newline before it parses, so ``gh a\``
on one line and ``pi --method POST`` on the next are one ``gh api`` call.

Joining pairs of lines is wrong in both directions. The deletion inserts no
space, so a continuation inside a word joins the word; a backslash is literal
inside single quotes; and a doubled backslash escapes itself. None of those
three continues the line, so :class:`_Reader` tracks the quoting context and
the escapes rather than guessing from the last character.

A command substitution opens a quoting context of its own. Bash parses the
text of ``$(...)`` as a fresh command, so a quote inside it is not the quote
that encloses it: in ``"$(printf '%s' '"')"`` the inner ``"`` belongs to the
substitution and the outer double quote stays open. A reader with one quote
state closes the outer quote there, reads the following ``'`` as an opening
quote, and swallows the rest of the script -- including any continued call.
:class:`_Reader` therefore keeps a stack of contexts, and refuses a script
whose quoting it cannot resolve rather than silently dropping commands.

Two further contexts decide whether a later call is read at all. An ANSI-C
quote, ``$'...'``, is its own quoting context in which a backslash escapes the
apostrophe, so ``$'a\'b'`` is one word rather than a quote that ends at the
escape. And ``#`` opens a comment at the start of a word, which a shell
operator also starts: in ``true;# '`` bash ignores the rest of the line, while
a reader that waits for whitespace opens a quote and swallows the command
after it.

Run via ``make test-workflow-contracts``.
"""

import enum
import typing as typ


class _Quote(enum.Enum):
    """The quoting context the next character is read in."""

    PLAIN = enum.auto()
    SINGLE = enum.auto()
    DOUBLE = enum.auto()
    ANSI_C = enum.auto()


class _Frame(typ.NamedTuple):
    """One quoting context, and whether a command substitution opened it."""

    quote: _Quote
    is_substitution: bool = False


class UnbalancedScriptError(AssertionError):
    """Report a script that ends inside a quote or command substitution.

    The reader cannot attribute the remaining text to any command, so it
    fails rather than let a call pass unexamined. Subclassing
    :class:`AssertionError` keeps the failure a contract failure.
    """

    def __init__(self) -> None:
        """Build the error with the message the contract failure reports."""
        super().__init__(
            "the script ends inside a quote or command substitution, so the "
            "read-only scan cannot tell which commands it runs"
        )


#: The characters a backslash protects inside double quotes. Anywhere else in
#: double quotes the backslash is literal, which is what stops a doubled
#: backslash from acting as an escape inside an already-escaped pair.
_DOUBLE_QUOTE_ESCAPES = ('"', "\\", "$", "`")

#: The shell's control operators. Each ends the word before it, and bash counts
#: the start of a word as the start of a comment, so ``true;# '`` comments out
#: the rest of the line with no space in between.
_WORD_ENDING_OPERATORS = frozenset(";&|()<>")


def logical_lines(script: str) -> list[str]:
    r"""Return the commands bash would parse, one per logical line.

    A backslash-newline is deleted before parsing, joining the two physical
    lines with nothing between them -- not even a space. Inside single quotes
    the pair is ordinary text, because a backslash there is literal.

    Comment lines are dropped: a comment names no command, and one that ended
    in a backslash would otherwise swallow the line after it.

    Returns
    -------
    list[str]
        Each command, stripped of surrounding whitespace, in script order.
        Empty lines are omitted.

    Examples
    --------
    >>> logical_lines("gh \\\n  api --method GET repos/o/r/releases")
    ['gh   api --method GET repos/o/r/releases']
    """
    return _Reader(script).read()


class _Reader:
    """Accumulate the commands of one script, reading one character at a time.

    The state is exactly the state bash keeps while it looks for the end of a
    command: the stack of quoting contexts the next character is read in, what
    the current command has accumulated, and how much of the script has been
    read. A command substitution pushes a context of its own, so a quote
    inside ``$(...)`` closes that context rather than the one enclosing it.
    """

    def __init__(self, script: str) -> None:
        """Start a reader at the beginning of one script, outside any quote."""
        self._script = script
        self._lines: list[str] = []
        self._current: list[str] = []
        self._frames = [_Frame(_Quote.PLAIN)]
        self._index = 0
        self._escaped_last = False

    def read(self) -> list[str]:
        """Consume the script and return its commands, in script order."""
        while self._index < len(self._script):
            quote = self._frames[-1].quote
            if quote is _Quote.SINGLE:
                self._read_single_quoted()
            elif quote is _Quote.DOUBLE:
                self._read_double_quoted()
            elif quote is _Quote.ANSI_C:
                self._read_ansi_c_quoted()
            else:
                self._read_plain()
        self._require_balanced()
        self._emit()
        return self._lines

    def _read_single_quoted(self) -> None:
        """Read one character of single-quoted text, where nothing escapes."""
        if self._char() == "'":
            self._frames.pop()
        self._take_plain_char()

    def _read_double_quoted(self) -> None:
        """Read one character of double-quoted text, honouring continuations."""
        char = self._char()
        following = self._following()
        if char == '"':
            self._frames.pop()
            self._take_plain_char()
            return
        if char == "\\" and following == "\n":
            # Double quotes suspend the shell's other special characters but
            # not this deletion.
            self._index += 2
            return
        if char == "\\" and following in _DOUBLE_QUOTE_ESCAPES:
            self._current.extend((char, following))
            self._escaped_last = True
            self._index += 2
            return
        if self._take_substitution():
            return
        self._take_plain_char()

    def _read_ansi_c_quoted(self) -> None:
        r"""Read one character of ``$'...'`` text, where a backslash escapes."""
        char = self._char()
        if char == "\\" and self._following():
            # C escape rules, so ``\'`` protects the apostrophe from closing
            # the quote where ordinary single-quoted text would end it.
            self._current.extend((char, self._following()))
            self._escaped_last = True
            self._index += 2
            return
        if char == "'":
            self._frames.pop()
        self._take_plain_char()

    def _read_plain(self) -> None:
        """Read one character outside quotes, where every rule applies."""
        if not (self._take_opening() or self._take_ending()):
            self._take_plain_char()

    def _take_opening(self) -> bool:
        """Consume the character that opens or escapes a construct, if it does."""
        # Order matters only for ``$``, where ``$(`` and ``$'`` are
        # two-character delimiters a quote check would split.
        return (
            self._take_escape()
            or self._take_substitution()
            or self._take_ansi_quote()
            or self._take_quote()
        )

    def _take_ending(self) -> bool:
        """Consume the character that ends part of the command, if it does."""
        return (
            self._take_closing_paren() or self._take_comment() or self._take_newline()
        )

    def _take_ansi_quote(self) -> bool:
        r"""Consume ``$'`` and open the ANSI-C quoting context it starts."""
        if self._char() != "$" or self._following() != "'":
            return False
        self._push(_Quote.ANSI_C)
        self._current.extend(("$", "'"))
        self._escaped_last = False
        self._index += 2
        return True

    def _take_quote(self) -> bool:
        """Consume a quote and open the quoting context it starts."""
        match self._char():
            case "'":
                self._push(_Quote.SINGLE)
            case '"':
                self._push(_Quote.DOUBLE)
            case _:
                return False
        self._take_plain_char()
        return True

    def _take_closing_paren(self) -> bool:
        """Consume the ``)`` that closes a command substitution, if it does."""
        # A ``)`` outside a substitution is ordinary text: only the frame says
        # which one this is.
        if self._char() != ")" or not self._frames[-1].is_substitution:
            return False
        self._frames.pop()
        self._take_plain_char()
        return True

    def _take_comment(self) -> bool:
        """Consume a comment, which names no part of the command."""
        if self._char() != "#" or not self._at_word_start():
            return False
        self._index = self._comment_end()
        return True

    def _take_newline(self) -> bool:
        """End the command at a newline, unless a context is still open."""
        # A newline inside a quote or substitution belongs to that construct.
        if self._char() != "\n" or not self._at_top_level():
            return False
        self._emit()
        self._index += 1
        return True

    def _take_plain_char(self) -> None:
        """Append the current character as ordinary text and step past it."""
        self._current.append(self._char())
        self._escaped_last = False
        self._index += 1

    def _take_substitution(self) -> bool:
        r"""Consume ``$(`` and open a quoting context for the command inside."""
        if self._char() != "$" or self._following() != "(":
            return False
        self._push(_Quote.PLAIN, is_substitution=True)
        self._current.extend(("$", "("))
        self._escaped_last = False
        self._index += 2
        return True

    def _push(self, quote: _Quote, *, is_substitution: bool = False) -> None:
        """Open a quoting context, which its matching character closes."""
        self._frames.append(_Frame(quote, is_substitution))

    def _at_top_level(self) -> bool:
        """Report whether no quote or substitution is open."""
        return len(self._frames) == 1

    def _require_balanced(self) -> None:
        """Refuse a script that ends inside a quote or substitution.

        Raises
        ------
        UnbalancedScriptError
            If a context is still open, because the reader then cannot say
            which commands the remaining text belongs to. Failing is the safe
            direction: a swallowed command would pass the read-only checks.
        """
        if not self._at_top_level():
            raise UnbalancedScriptError

    def _take_escape(self) -> bool:
        r"""Consume a backslash and the character it protects, if there is one."""
        # Any pair but a continuation is kept, because the backslash still
        # escapes the character after it -- which stops ``\\`` continuing.
        if self._char() != "\\":
            return False
        following = self._following()
        if following == "\n":
            # The pair is deleted, so the character before it remains the last
            # one appended and the flag that describes it is still right.
            self._index += 2
            return True
        if not following:
            return False
        self._current.extend(("\\", following))
        self._escaped_last = True
        self._index += 2
        return True

    def _char(self) -> str:
        """Return the character the reader is on."""
        return self._script[self._index]

    def _following(self) -> str:
        """Return the character after the current one, or an empty string."""
        following = self._index + 1
        return self._script[following] if following < len(self._script) else ""

    def _at_word_start(self) -> bool:
        r"""Report whether the current position starts a word.

        ``#`` opens a comment only there; inside a word it is ordinary text, as
        in ``foo#bar`` or the ``$#`` parameter. Only an *unescaped* space ends
        a word, so ``foo\ #`` keeps the ``#`` inside the argument, and a
        control operator ends one too, so ``true;# '`` starts a comment.
        Waiting for whitespace alone reads that apostrophe as an opening quote
        and swallows the command after it.

        Returns
        -------
        bool
            Whether the text so far is empty, or ends in an unescaped space or
            control operator.
        """
        if not self._current:
            return True
        last = self._current[-1]
        if self._escaped_last:
            return False
        return last.isspace() or last in _WORD_ENDING_OPERATORS

    def _comment_end(self) -> int:
        """Return the index of the newline that ends the comment here."""
        # A comment with no newline runs to the end of the script; returning
        # the length keeps the reader from reading past it.
        end = self._script.find("\n", self._index)
        return len(self._script) if end < 0 else end

    def _emit(self) -> None:
        """Append the accumulated text as one command, unless it names none."""
        command = "".join(self._current).strip()
        if command:
            self._lines.append(command)
        self._current = []

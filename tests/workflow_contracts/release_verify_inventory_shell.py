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

Run via ``make test-workflow-contracts``.
"""

import enum


class _Quote(enum.Enum):
    """The quoting context the next character is read in."""

    PLAIN = enum.auto()
    SINGLE = enum.auto()
    DOUBLE = enum.auto()


#: The characters a backslash protects inside double quotes. Anywhere else in
#: double quotes the backslash is literal, which is what stops a doubled
#: backslash from acting as an escape inside an already-escaped pair.
_DOUBLE_QUOTE_ESCAPES = ('"', "\\", "$", "`")


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
    command: which quoting context the next character is in, what the current
    command has accumulated, and how much of the script has been read.
    """

    def __init__(self, script: str) -> None:
        """Start a reader at the beginning of one script, outside any quote."""
        self._script = script
        self._lines: list[str] = []
        self._current: list[str] = []
        self._quote = _Quote.PLAIN
        self._index = 0

    def read(self) -> list[str]:
        """Consume the script and return the commands it names, in order."""
        while self._index < len(self._script):
            if self._quote is _Quote.SINGLE:
                self._read_single_quoted()
            elif self._quote is _Quote.DOUBLE:
                self._read_double_quoted()
            else:
                self._read_plain()
        self._emit()
        return self._lines

    def _read_single_quoted(self) -> None:
        """Read one character of single-quoted text, where nothing escapes."""
        char = self._char()
        self._quote = _Quote.PLAIN if char == "'" else self._quote
        self._current.append(char)
        self._index += 1

    def _read_double_quoted(self) -> None:
        """Read one character of double-quoted text, honouring continuations."""
        char = self._char()
        following = self._following()
        if char == '"':
            self._quote = _Quote.PLAIN
        elif char == "\\" and following == "\n":
            # Double quotes suspend the shell's other special characters but
            # not this deletion.
            self._index += 2
            return
        elif char == "\\" and following in _DOUBLE_QUOTE_ESCAPES:
            self._current.extend((char, following))
            self._index += 2
            return
        self._current.append(char)
        self._index += 1

    def _read_plain(self) -> None:
        """Read one character outside quotes, where every rule applies."""
        char = self._char()
        following = self._following()
        if char == "'":
            self._quote = _Quote.SINGLE
        elif char == '"':
            self._quote = _Quote.DOUBLE
        elif char == "\\" and following == "\n":
            # Distinct from an escape: the pair is deleted, not replaced.
            self._index += 2
            return
        elif char == "\\" and following:
            self._current.extend((char, following))
            self._index += 2
            return
        elif char == "#" and self._at_word_start():
            self._index = self._comment_end()
            return
        elif char == "\n":
            self._emit()
            self._index += 1
            return
        self._current.append(char)
        self._index += 1

    def _char(self) -> str:
        """Return the character the reader is on."""
        return self._script[self._index]

    def _following(self) -> str:
        """Return the character after the current one, or an empty string."""
        following = self._index + 1
        return self._script[following] if following < len(self._script) else ""

    def _at_word_start(self) -> bool:
        """Report whether the current position starts a word.

        ``#`` opens a comment only there; inside a word it is ordinary text, as
        in ``foo#bar`` or the ``$#`` parameter.

        Returns
        -------
        bool
            Whether the text so far is empty or ends in whitespace.
        """
        return not self._current or self._current[-1].isspace()

    def _comment_end(self) -> int:
        """Return the index of the newline that ends the comment here.

        A comment with no newline after it runs to the end of the script;
        returning the length rather than a found index keeps the reader from
        reading past it.

        Returns
        -------
        int
            The index the reader resumes at.
        """
        end = self._script.find("\n", self._index)
        return len(self._script) if end < 0 else end

    def _emit(self) -> None:
        """Append the accumulated text as one command, unless it names none."""
        command = "".join(self._current).strip()
        if command:
            self._lines.append(command)
        self._current = []

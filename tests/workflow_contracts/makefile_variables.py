"""Reading a Makefile's variables and expanding them, one level.

A gate recipe composes its flags through a shared Make variable rather than
spelling them out, so a contract reading recipe text alone reports the warning
policy as absent when it is merely named indirectly. The reading and the one
level of substitution that follows it live here, split from the coverage
contract that uses them so neither module outgrows the repository's 400-line
file limit.

Nothing here touches the filesystem. Every function takes the text it works
on, so the behaviour can be pinned against values the test itself supplied
rather than values that happened to be in the file the day it ran; the read
itself belongs to :mod:`makefile_recipes`, which diagnoses the ways it can
fail.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc


def expand_makefile_variables(text: str, definitions: cabc.Mapping[str, str]) -> str:
    """Substitute one level of ``$(NAME)`` references from ``definitions``.

    Parameters
    ----------
    text
        Recipe text whose ``$(NAME)`` references should be substituted.
    definitions
        Make variable name to value, as read by :func:`makefile_definitions`.

    Returns
    -------
    str
        ``text`` with each defined ``$(NAME)`` replaced by its value.
    """
    return re.sub(
        r"\$\(([A-Z][A-Z0-9_]*)\)",
        lambda match: definitions.get(match.group(1), match.group(0)),
        text,
    )


def makefile_definitions(makefile: str) -> cabc.Mapping[str, str]:
    """Return the Makefile's ``NAME = value`` and ``NAME ?= value`` variables.

    Definitions are read from the Makefile rather than assumed: the gate
    recipes compose their flags through a shared variable rather than spelling
    them out, so a contract reading recipe text alone would report the warning
    policy as absent when it is merely named indirectly.

    Parameters
    ----------
    makefile
        The Makefile's contents.

    Returns
    -------
    cabc.Mapping[str, str]
        Variable name to value. Later definitions win, as in Make.
    """
    return dict(
        re.findall(r"^([A-Z][A-Z0-9_]*) \??= (.*)$", makefile, flags=re.MULTILINE)
    )

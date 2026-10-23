"""Ask what the release-publication gate's command line actually reaches.

A recipe names modules; this module answers what those names *mean*. Two
questions are asked of it, and both have a wrong answer that looks right:

- Is every suite under ``scripts/tests`` reachable from some recipe? A raw
  search of the file answers yes for a module a comment happens to mention,
  so the search runs over the text `make` would execute.
- Does a named source module carry doctest examples? A substring probe
  answers yes for a ``>>>`` in the prose *about* doctests, so the count is
  taken over parsed string constants, which is where pytest looks.

Kept apart from the contracts that assert with them so each file stays under
the repository's 400-line cap, following
:mod:`scripts.hoist_binstall_archives` and its sibling discovery module.

Run via ``make test-workflow-contracts``.
"""

import ast
import re
import typing as typ

from makefile_recipes import mask_makefile_comments

if typ.TYPE_CHECKING:
    from pathlib import Path

#: Matches a test module path in a recipe. Deliberately anchored to the
#: ``scripts/tests/`` directory rather than to any ``test_*.py`` argument:
#: targets such as ``test-workflow-contracts`` also run ``tests/`` and
#: ``.github/`` suites, and a pattern loose enough to catch those would make
#: the orphan check report them as uncollected.
TEST_MODULE = re.compile(r"scripts/tests/(test_[A-Za-z0-9_]+\.py)")


def collected_modules(makefile: str) -> set[str]:
    """Return every ``scripts/tests`` module name a recipe names.

    Parameters
    ----------
    makefile
        Whole Makefile text to scan.

    Returns
    -------
    set[str]
        Bare module file names, such as ``test_release_semver.py``.

    Notes
    -----
    The whole file is scanned rather than one recipe, because a module may be
    legitimately collected by any target — a new one here, or an existing
    target that grew a module. The question is whether the suite is reachable
    at all, not which target owns it.

    Mentions inside comments do not count. A comment naming a module is a
    maintainer's note, not a command line, and counting it turns the check
    into a search for prose that agrees with itself: a recipe could lose its
    module argument to a stray edit and still pass, because the comment
    beside it survived. :func:`makefile_recipes.mask_makefile_comments`
    blanks comments while preserving positions, so the pattern reads what
    `make` would execute.

    Examples
    --------
    >>> sorted(collected_modules("run: pytest scripts/tests/test_a.py"))
    ['test_a.py']
    >>> collected_modules("# legacy: pytest scripts/tests/test_b.py")
    set()
    """
    return set(TEST_MODULE.findall(mask_makefile_comments(makefile)))


def docstring_example_count(source: str) -> int:
    r"""Return how many string-constant lines of ``source`` hold a prompt.

    Parameters
    ----------
    source
        Python source text.

    Returns
    -------
    int
        The number of lines inside string constants, as Python parses them,
        whose first non-space character begins a doctest prompt.

    Notes
    -----
    Any string constant counts, not only a docstring. The two are the same
    hazard for this question: a prompt inside a message string is prose, and
    pytest collects no example from it either. Counting over the parse rather
    than over the text is what separates the two, since both spell the prompt
    the same way.

    Examples
    --------
    >>> docstring_example_count('def f():\n    "Say it."\n')
    0
    >>> docstring_example_count('# >>> not a constant\n')
    0
    >>> docstring_example_count(
    ...     "def f():\n    '''Add.\n\n    >>> 1 + 1\n    2\n    '''\n"
    ... )
    1
    """
    return sum(
        sum(1 for line in node.value.splitlines() if line.strip().startswith(">>>"))
        for node in ast.walk(ast.parse(source))
        if isinstance(node, ast.Constant) and isinstance(node.value, str)
    )


def suite_module_names(directory: Path) -> set[str]:
    """Return the bare file name of each test module in ``directory``.

    Parameters
    ----------
    directory
        A directory holding ``test_*.py`` modules.

    Returns
    -------
    set[str]
        The file names, without a directory component.
    """
    return {path.name for path in directory.glob("test_*.py")}


__all__ = [
    "TEST_MODULE",
    "collected_modules",
    "docstring_example_count",
    "suite_module_names",
]

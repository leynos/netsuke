"""One walker over every string in a parsed workflow value.

A credential scan has to see a name wherever YAML can put it: a value, a
mapping key such as an ``env`` entry, or an element of a list. The coverage
contracts share this walker, so a YAML shape the scans must learn is taught
once rather than to each copy.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc


def iter_strings(value: object) -> cabc.Iterator[str]:
    """Yield every string nested anywhere in a parsed YAML value, keys included.

    Parameters
    ----------
    value : object
        A parsed workflow value: a mapping, a list, a scalar, or ``None``.

    Yields
    ------
    str
        Each string in document order, a mapping's key before its value.
        Non-string scalars yield nothing.

    Examples
    --------
    >>> list(iter_strings({"env": {"TOKEN": "x"}, "steps": ["a", 1]}))
    ['env', 'TOKEN', 'x', 'steps', 'a']
    """
    match value:
        case str() as text:
            yield text
        case dict() as mapping:
            for key, item in mapping.items():
                yield from iter_strings(key)
                yield from iter_strings(item)
        case list() as sequence:
            for item in sequence:
                yield from iter_strings(item)
        case _:
            return

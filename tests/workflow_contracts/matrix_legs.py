"""Expand a job matrix into the legs GitHub actually creates.

A matrix multiplies its axes into a Cartesian product, removes the legs named
by ``exclude`` rows, and applies ``include`` rows to the surviving original
combinations: a row that matches none of them becomes a leg of its own, and a
row can add keys but cannot overwrite an axis value. Axis names compare
case-insensitively, as ``matrix.<axis>`` does, while the values keep their
declared spelling.

Keeping the expansion here leaves ``runner_matrix`` responsible for resolving
one ``runs-on`` selector against those legs, and keeps each module small
enough to read in one sitting.

Run via ``make test-workflow-contracts``.

Examples
--------
>>> matrix_legs({"os": ["a", "b"], "exclude": [{"os": "a"}]})
[{'os': 'b'}]
>>> matrix_legs({"os": ["a"], "include": [{"os": "a", "runner": "r1"}]})
[{'os': 'a', 'runner': 'r1'}]
"""

from itertools import product

_RESERVED_MATRIX_KEYS = frozenset({"include", "exclude"})


def _base_axes(matrix: dict[str, object]) -> list[tuple[str, list[str]]]:
    """Return the matrix axes GitHub multiplies, with values as strings.

    ``include`` and ``exclude`` describe structure rather than axes, so they
    are omitted. A declaration that is not a list contributes no value.

    Returns
    -------
    list[tuple[str, list[str]]]
        Each axis name with its declared values, in declaration order.

    Examples
    --------
    >>> _base_axes({"os": ["a", "b"], "include": [{"os": "c"}]})
    [('os', ['a', 'b'])]
    """
    return [
        (str(name), [str(item) for item in declared])
        for name, declared in matrix.items()
        if name not in _RESERVED_MATRIX_KEYS and isinstance(declared, list)
    ]


def _base_legs(axes: list[tuple[str, list[str]]]) -> list[dict[str, str]]:
    """Return the cartesian product of the axes as matrix legs.

    A matrix with no axis has no combinations. GitHub then creates one leg per
    ``include`` row rather than combining those rows.

    Returns
    -------
    list[dict[str, str]]
        One mapping per combination, in GitHub's product order.

    Examples
    --------
    >>> _base_legs([("os", ["a", "b"]), ("arch", ["x"])])
    [{'os': 'a', 'arch': 'x'}, {'os': 'b', 'arch': 'x'}]
    >>> _base_legs([])
    []
    """
    if not axes:
        return []
    names = [name for name, _ in axes]
    values = [values for _, values in axes]
    return [
        dict(zip(names, combination, strict=True)) for combination in product(*values)
    ]


def _string_row(entry: object) -> dict[str, str] | None:
    """Return an ``include`` or ``exclude`` row with string values.

    A row that is not a mapping describes no leg and is ignored.

    Returns
    -------
    dict[str, str] or None
        The row's key/value pairs as strings, or ``None`` for a non-mapping.

    Examples
    --------
    >>> _string_row({"os": "linux", "count": 2})
    {'os': 'linux', 'count': '2'}
    >>> _string_row("linux") is None
    True
    """
    if not isinstance(entry, dict):
        return None
    return {str(key): str(value) for key, value in entry.items()}


def _matches(leg: dict[str, str], selector: dict[str, str]) -> bool:
    """Return whether every key and value of a selector is present in a leg.

    Keys are compared case-insensitively, as GitHub resolves ``matrix.<axis>``.

    Returns
    -------
    bool
        Whether the leg matches the selector on all of its pairs.

    Examples
    --------
    >>> _matches({"OS": "linux", "arch": "x64"}, {"os": "linux"})
    True
    >>> _matches({"os": "linux"}, {"os": "windows"})
    False
    """
    by_name = {name.lower(): value for name, value in leg.items()}
    return all(by_name.get(name.lower()) == value for name, value in selector.items())


def _matches_original(
    leg: dict[str, str], row: dict[str, str], base_names: frozenset[str]
) -> bool:
    """Return whether an include row agrees with a base leg's own axes.

    A row may add keys freely, but it can join a leg only when every base axis
    it names already holds that value. This is what stops an include row from
    overwriting an original matrix value; it creates a new leg instead.

    Returns
    -------
    bool
        Whether the row can be applied to the leg without overwriting an axis.

    Examples
    --------
    >>> _matches_original({"os": "linux"}, {"os": "linux"}, frozenset({"os"}))
    True
    >>> _matches_original({"os": "linux"}, {"os": "windows"}, frozenset({"os"}))
    False
    >>> _matches_original({"os": "linux"}, {"runner": "r1"}, frozenset({"os"}))
    True
    """
    by_name = {name.lower(): value for name, value in leg.items()}
    return all(
        name.lower() not in base_names or by_name.get(name.lower()) == value
        for name, value in row.items()
    )


def _merge(leg: dict[str, str], row: dict[str, str]) -> None:
    """Apply an include row to a leg, letting a later row win.

    Added keys overwrite each other, including when include rows spell the same
    key differently, because matrix lookups are case-insensitive. An original
    axis is never overwritten: the caller only merges matching rows.

    Examples
    --------
    >>> leg = {"os": "linux"}
    >>> _merge(leg, {"runner": "first"})
    >>> _merge(leg, {"Runner": "second"})
    >>> leg
    {'os': 'linux', 'runner': 'second'}
    """
    for name, value in row.items():
        existing = next((key for key in leg if key.lower() == name.lower()), None)
        if existing is None:
            leg[name] = value
        else:
            leg[existing] = value


def _excluded_legs(
    legs: list[dict[str, str]], excludes: object
) -> list[dict[str, str]]:
    """Return the legs that no exclude row matches.

    Returns
    -------
    list[dict[str, str]]
        The surviving legs, in their original order.

    Examples
    --------
    >>> _excluded_legs(
    ...     [{"os": "a"}, {"os": "b"}], [{"os": "a"}]
    ... )
    [{'os': 'b'}]
    """
    if not isinstance(excludes, list):
        return legs
    selectors = [
        selector
        for selector in (_string_row(entry) for entry in excludes)
        if selector is not None
    ]
    return [
        leg
        for leg in legs
        if not any(_matches(leg, selector) for selector in selectors)
    ]


def _included_legs(
    legs: list[dict[str, str]], includes: object, base_names: frozenset[str]
) -> list[dict[str, str]]:
    """Return the legs after applying every include row to the originals.

    Include rows are matched only against the original legs, never against a
    leg an earlier row created. That is GitHub's order: a row that cannot join
    an original combination becomes a leg of its own and is then left alone.

    Returns
    -------
    list[dict[str, str]]
        The original legs with their final include values, then any legs the
        include rows added, in row order.

    Examples
    --------
    >>> _included_legs(
    ...     [{"os": "linux"}], [{"runner": "first"}, {"runner": "second"}],
    ...     frozenset({"os"}),
    ... )
    [{'os': 'linux', 'runner': 'second'}]
    """
    expanded = list(legs)
    original = len(expanded)
    if not isinstance(includes, list):
        return expanded
    for entry in includes:
        row = _string_row(entry)
        if row is not None:
            _apply_include_row(expanded, original, row, base_names)
    return expanded


def _apply_include_row(
    expanded: list[dict[str, str]],
    original: int,
    row: dict[str, str],
    base_names: frozenset[str],
) -> None:
    """Apply one include row, appending a leg when no original matches.

    Parameters
    ----------
    expanded
        The legs built so far, originals first; mutated in place.
    original
        How many of them came from the axes, and so may accept the row.
    row
        The include row's key/value pairs as strings.
    base_names
        The axis names the row must not overwrite, lowercased.

    Examples
    --------
    >>> legs = [{"os": "linux"}]
    >>> _apply_include_row(legs, 1, {"runner": "r1"}, frozenset({"os"}))
    >>> legs
    [{'os': 'linux', 'runner': 'r1'}]
    """
    matched = [
        leg for leg in expanded[:original] if _matches_original(leg, row, base_names)
    ]
    if not matched:
        expanded.append(dict(row))
        return
    for leg in matched:
        _merge(leg, row)


def matrix_legs(matrix: dict[str, object]) -> list[dict[str, str]]:
    """Expand a matrix into the legs GitHub would create.

    The base axes are multiplied first, ``exclude`` rows then remove matching
    legs, and ``include`` rows are applied to the surviving original legs.

    Returns
    -------
    list[dict[str, str]]
        The effective matrix legs, in GitHub's evaluation order.

    Examples
    --------
    >>> matrix_legs({"os": ["a", "b"], "exclude": [{"os": "a"}]})
    [{'os': 'b'}]
    >>> matrix_legs({"os": ["a"], "include": [{"os": "a", "runner": "r1"}]})
    [{'os': 'a', 'runner': 'r1'}]
    """
    axes = _base_axes(matrix)
    legs = _excluded_legs(_base_legs(axes), matrix.get("exclude"))
    base_names = frozenset(name.lower() for name, _ in axes)
    return _included_legs(legs, matrix.get("include"), base_names)

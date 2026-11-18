"""Resolve a matrix-selected ``runs-on`` value to the labels it holds.

A lane may choose its runner with ``${{ matrix.os }}`` rather than naming a
label, and GitHub then takes the value from the job's own matrix. That matrix
multiplies its axes into legs, removes legs named by ``exclude``, and applies
``include`` rows to the original combinations. The registry comparison in
``runner_shape_test`` must see the labels of the legs GitHub creates, because
an unregistered label is exactly what it exists to catch.

Only a whole expression reading one axis resolves; a selector whose axis the
job does not declare is returned as itself, so the comparison fails closed.

Run via ``make test-workflow-contracts``.

Examples
--------
>>> matrix_job = {"strategy": {"matrix": {"os": ["private-runner"]}}}
>>> selected_labels("${{ matrix.os }}", matrix_job)
['private-runner']
>>> selected_labels("${{ matrix.os }}", {})
['${{ matrix.os }}']
"""

from itertools import product

from actions_expressions import matrix_axis, static_label

#: Matrix keys GitHub reserves for its own structure rather than for an axis.
_RESERVED_MATRIX_KEYS = frozenset({"include", "exclude"})
#: The path prefix that makes a reusable-workflow call local to this repository.
_LOCAL_WORKFLOWS = "./.github/workflows/"


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
        if row is None:
            continue
        matched = [
            leg
            for leg in expanded[:original]
            if _matches_original(leg, row, base_names)
        ]
        if matched:
            for leg in matched:
                _merge(leg, row)
        else:
            expanded.append(dict(row))
    return expanded


def _matrix_legs(matrix: dict[str, object]) -> list[dict[str, str]]:
    """Expand a matrix into the legs GitHub would create.

    The base axes are multiplied first, ``exclude`` rows then remove matching
    legs, and ``include`` rows are applied to the surviving original legs.

    Returns
    -------
    list[dict[str, str]]
        The effective matrix legs, in GitHub's evaluation order.

    Examples
    --------
    >>> _matrix_legs({"os": ["a", "b"], "exclude": [{"os": "a"}]})
    [{'os': 'b'}]
    >>> _matrix_legs({"os": ["a"], "include": [{"os": "a", "runner": "r1"}]})
    [{'os': 'a', 'runner': 'r1'}]
    """
    axes = _base_axes(matrix)
    legs = _excluded_legs(_base_legs(axes), matrix.get("exclude"))
    base_names = frozenset(name.lower() for name, _ in axes)
    return _included_legs(legs, matrix.get("include"), base_names)


def matrix_axis_values(job: dict[str, object], axis: str) -> list[str]:
    """Return every distinct value a job's matrix gives one axis.

    The values come from the legs GitHub would create, so a value used only by
    an excluded leg is absent, a value an ``include`` row adds back is present,
    and a value a later ``include`` row overrides is reported once, in its
    final form. Axis names are matched case-insensitively, as GitHub resolves
    ``matrix.<axis>`` expressions.

    Parameters
    ----------
    job
        A parsed job mapping, which may declare no strategy or no matrix.
    axis
        The matrix axis the selector reads, such as ``"os"``.

    Returns
    -------
    list[str]
        The distinct values in surviving legs, in leg order; empty if none.

    Examples
    --------
    >>> matrix_axis_values({"strategy": {"matrix": {"os": ["a", "b"]}}}, "os")
    ['a', 'b']
    >>> matrix_axis_values(
    ...     {"strategy": {"matrix": {"os": ["a", "b"], "exclude": [{"os": "a"}]}}},
    ...     "os",
    ... )
    ['b']
    >>> matrix_axis_values(
    ...     {"strategy": {"matrix": {"os": ["a"], "include": [{"os": "c"}]}}},
    ...     "os",
    ... )
    ['a', 'c']
    >>> matrix_axis_values({}, "os")
    []
    """
    strategy = job.get("strategy")
    matrix = strategy.get("matrix") if isinstance(strategy, dict) else None
    if not isinstance(matrix, dict):
        return []
    wanted = axis.lower()
    values: list[str] = []
    for leg in _matrix_legs(matrix):
        for name, value in leg.items():
            if name.lower() == wanted and value not in values:
                values.append(value)
    return values


def selected_labels(value: str, job: dict[str, object]) -> list[str]:
    """Return the labels one ``runs-on`` scalar can select.

    Parameters
    ----------
    value
        A whole ``runs-on`` scalar: a label, or a whole expression.
    job
        The job that declares the value, whose matrix a selector reads.

    Returns
    -------
    list[str]
        The labels the scalar can select, never empty; an unresolvable
        selector is returned as itself so the caller counts it as unread.

    Examples
    --------
    >>> selected_labels("ubuntu-latest", {})
    ['ubuntu-latest']
    >>> selected_labels("${{ 'private-runner' }}", {})
    ['private-runner']
    >>> selected_labels("${{ matrix.os }}", {"strategy": {"matrix": {}}})
    ['${{ matrix.os }}']
    """
    fixed = static_label(value)
    if fixed is not None:
        return [fixed]
    axis = matrix_axis(value)
    if axis is None:
        return [value]
    # Returning the empty list here would drop the lane's labels from the
    # inventory instead of reporting the selector nobody could read.
    return matrix_axis_values(job, axis) or [value]


def caller_runner_labels(job: dict[str, object]) -> list[str]:
    """Return the runner labels a local reusable-workflow call passes on.

    A job that calls a local reusable workflow has no ``runs-on``; it passes
    its runner through ``with.runner`` and the callee reads
    ``${{ inputs.runner }}``. Resolving that input against the caller's own
    matrix keeps the callee's label in the inventory, where the registry
    comparison can require it. A remote call is left alone: the workflow file
    is not here, so its runner input cannot be read.

    Returns
    -------
    list[str]
        The labels the call's ``runner`` input selects, or empty when the job
        is not a local reusable-workflow call or passes no runner.

    Examples
    --------
    >>> caller_runner_labels({
    ...     "uses": "./.github/workflows/build.yml",
    ...     "strategy": {"matrix": {"include": [{"runner": "private"}]}},
    ...     "with": {"runner": "${{ matrix.runner }}"},
    ... })
    ['private']
    >>> caller_runner_labels({"with": {"runner": "private"}})
    []
    """
    uses = job.get("uses")
    if not (isinstance(uses, str) and uses.startswith(_LOCAL_WORKFLOWS)):
        return []
    inputs = job.get("with")
    runner = inputs.get("runner") if isinstance(inputs, dict) else None
    return selected_labels(runner, job) if isinstance(runner, str) else []

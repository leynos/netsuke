"""Resolve a matrix-selected ``runs-on`` value to the labels it holds.

A lane may choose its runner with ``${{ matrix.os }}`` rather than naming a
label, and GitHub then takes the value from the job's own matrix: a plain axis
list, or one ``include`` entry per leg. The registry comparison in
``runner_shape_test`` has to see those labels, because a self-hosted runner it
never reads is a runner nobody registers, and an unregistered label is exactly
what that comparison exists to catch.

Only a whole expression reading one axis resolves. A selector whose axis the
job does not declare is returned as itself, so the comparison fails closed
rather than dropping a label it could not read.

Run via ``make test-workflow-contracts``.

Examples
--------
>>> matrix_job = {"strategy": {"matrix": {"os": ["private-runner"]}}}
>>> selected_labels("${{ matrix.os }}", matrix_job)
['private-runner']
>>> selected_labels("${{ matrix.os }}", {})
['${{ matrix.os }}']
"""

from actions_expressions import matrix_axis, static_label


def matrix_axis_values(job: dict[str, object], axis: str) -> list[str]:
    """Return every literal value a job's matrix can give one axis.

    Parameters
    ----------
    job
        A parsed job mapping, which may declare no strategy or no matrix.
    axis
        The matrix axis the selector reads, such as ``"os"``.

    Returns
    -------
    list[str]
        The values from the plain matrix mapping and from every ``include``
        entry that names the axis, in declaration order. Empty when the job
        declares neither.

    Examples
    --------
    >>> matrix_axis_values({"strategy": {"matrix": {"os": ["a", "b"]}}}, "os")
    ['a', 'b']
    >>> matrix_axis_values({"strategy": {"matrix": {"include": [{"os": "c"}]}}}, "os")
    ['c']
    >>> matrix_axis_values({}, "os")
    []
    """
    strategy = job.get("strategy")
    matrix = strategy.get("matrix") if isinstance(strategy, dict) else None
    if not isinstance(matrix, dict):
        return []
    declared = matrix.get(axis)
    values = [str(item) for item in declared] if isinstance(declared, list) else []
    included = matrix.get("include")
    if isinstance(included, list):
        values += [
            str(entry[axis])
            for entry in included
            if isinstance(entry, dict) and axis in entry
        ]
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
        The labels the scalar can select, never empty. A selector the job
        cannot resolve is returned as itself, so the caller counts it as an
        unread label rather than silently dropping it.

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

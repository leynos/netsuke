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

from actions_expressions import matrix_axis, static_label
from matrix_legs import matrix_legs

#: The path prefix that makes a reusable-workflow call local to this repository.
_LOCAL_WORKFLOWS = "./.github/workflows/"


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
    for leg in matrix_legs(matrix):
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

"""Assert generated identifiers stay out of the gate's records and diagnostics.

Split from :mod:`release_admission_test_records` when that module reached the
project's file-size ceiling. The seam is the kind of claim each half makes:
this module decides whether a *value* leaked, while its sibling decides whether
the *records being searched* exist at all. Keep a property calling the sibling
first, because every helper here accepts an empty collection and an exclusion
assertion made on its own is satisfied vacuously by a gate that wrote nothing.
"""

import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc


def assert_identifiers_excluded_from_diagnostics(
    stderr: str, identifiers: set[str], subject: str
) -> None:
    r"""Assert identifiers are absent from everything the gate wrote to stderr.

    Every diagnostic the gate prints is a fixed sentence, so an identifier
    reaching standard error is the same cardinality hazard as one reaching a
    metric label: the runner log is retained with the release. The command's own
    stderr is inherited, so an adapter's message reaches this stream too, which
    is exactly why the check belongs here rather than beside the records.

    Examples
    --------
    Check a diagnostic stream that carries no identifier::

        assert_identifiers_excluded_from_diagnostics(
            "release-admission trace sink failed\n",
            {"a" * 40},
            "generated identifiers",
        )
    """
    assert_identifiers_excluded_from_values(
        stderr.splitlines(),
        identifiers,
        f"{subject} must never become a diagnostic on standard error",
    )


def assert_identifiers_excluded_from_values(
    values: cabc.Iterable[object], identifiers: set[str], message: str
) -> None:
    """Assert generated identifiers are absent from string values.

    Parameters
    ----------
    values
        Metric-label or trace-field values to inspect; non-string values are ignored.
    identifiers
        Identifiers that must not occur as substrings of any value.
    message
        Failure message prefix retained in the assertion diagnostic.

    Notes
    -----
    Use this helper for one metric-label or trace-field value collection at a
    time so callers can retain field-specific failure messages.

    Examples
    --------
    Ignore non-string values while checking labels and trace fields::

        assert_identifiers_excluded_from_values(
            ["fixed-label", 17], {"run-7"}, "identifier leaked"
        )
    """
    matches = [
        (identifier, value)
        for value in values
        if isinstance(value, str)
        for identifier in sorted(identifiers)
        if identifier in value
    ]
    assert not matches, f"{message}; matching identifier/value pairs: {matches!r}"


def assert_identifiers_excluded_from_records(
    metrics: cabc.Iterable[dict[str, object]],
    traces: cabc.Iterable[dict[str, object]],
    identifiers: set[str],
    subject: str,
) -> None:
    """Assert identifiers are absent from metric labels and trace fields.

    Parameters
    ----------
    metrics
        Parsed release-admission metric records to inspect.
    traces
        Parsed release-admission trace records to inspect.
    identifiers
        Identifiers that must not occur as substrings in string values.
    subject
        Noun phrase used in metric-label and trace-field failure messages.

    Notes
    -----
    Keep record traversal and the metric-label shape check shared across the
    release-admission runtime tests.

    Examples
    --------
    Check both telemetry channels in one call::

        assert_identifiers_excluded_from_records(
            [{"labels": {"operation": "resolve_tag_commit"}}],
            [{"event": "operation_complete"}],
            {"run-7"},
            "generated identifiers",
        )
    """
    for record in metrics:
        labels = record["labels"]
        assert isinstance(labels, dict), "every emitted metric must retain labels"
        assert_identifiers_excluded_from_values(
            labels.values(),
            identifiers,
            f"{subject} must never become metric label values",
        )
    for trace in traces:
        assert_identifiers_excluded_from_values(
            trace.values(),
            identifiers,
            f"{subject} must never become trace field values",
        )

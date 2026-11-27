"""Shared accessors for the mutation-testing caller's ``with:`` block.

Two workflow contracts read the same caller configuration: ``mutation_testing_test``
pins the whole ``with:`` block, and ``mutation_exclusion_scope_test`` pins which
paths the ``exclude-globs`` value may name. Both need the block parsed and both
need a value narrowed to ``str`` with a diagnostic that explains why the type
matters, so those two operations live here rather than in either test module.
Duplicating them would let the two contracts disagree about where
``exclude-globs`` is read from.

Importing one collected test module from another would work but is not how this
suite is organised: a test module owns its own helpers, and a second consumer
promotes them to shared support.

Run via ``make test-workflow-contracts``.
"""

import pytest
from workflow_loading import (
    MUTATION_TESTING_WORKFLOW_PATH,
    load_workflow,
    require_mapping,
)


def _mutation_job() -> dict[str, object]:
    """Return the single calling job.

    Returns
    -------
    dict[str, object]
        The parsed ``jobs.mutation`` mapping.
    """
    jobs = require_mapping(
        load_workflow(MUTATION_TESTING_WORKFLOW_PATH).get("jobs"), "jobs"
    )
    return require_mapping(jobs["mutation"], "jobs.mutation")


def mutation_with_block() -> dict[str, object]:
    """Return the caller's ``with:`` block.

    Returns
    -------
    dict[str, object]
        The parsed ``jobs.mutation.with`` mapping.
    """
    return require_mapping(_mutation_job().get("with"), "jobs.mutation.with")


def require_str(value: object, description: str, consequence: str = "") -> str:
    """Return ``value`` as a string, failing the test when it is not one.

    The caller supplies the consequence rather than this helper, because only
    the caller knows *why* the type matters: for ``install-mold`` a YAML
    boolean is an argument-type error at workflow-call time, whereas for
    ``setup-commands`` any non-string is simply the wrong shape.

    Parameters
    ----------
    value
        The parsed YAML value to narrow.
    description
        How to name the value in a failure, e.g. the caller's dotted path.
    consequence
        A clause appended to the failure message explaining why the type
        matters. Empty when the wrong shape is self-evidently wrong.

    Returns
    -------
    str
        ``value`` itself, narrowed to ``str``.
    """
    match value:
        case str() as text:
            return text
        case other:
            pytest.fail(
                f"{description} must be a string, got {other!r} "
                f"({type(other).__name__}){consequence}"
            )

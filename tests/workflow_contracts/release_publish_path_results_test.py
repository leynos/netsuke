"""Contract tests for the ordered job results derived from the release graph.

Every scenario's results come from ``dependency_order``, which walks the
workflow's own ``needs`` edges before any oracle reads a job's outcome. GitHub
rejects a cyclic ``needs`` in practice, so the cycle branch is unreachable
through a real workflow and cannot be exercised by the scenario table; it is
held here instead. The rule it protects is that an unorderable graph is
reported rather than looped over, and still yields every job name so the caller
reports on the graph it was given rather than on a prefix of it.

Run via ``make test-workflow-contracts``.
"""

from release_publish_path_results import dependency_order


def test_dependency_order_places_each_job_after_the_jobs_it_needs() -> None:
    """Order a chain so a dependency is resolved before the job that reads it."""
    jobs: dict[str, object] = {
        "publish": {"needs": ["package", "admission"]},
        "admission": {"needs": "metadata"},
        "package": {"needs": "metadata"},
        "metadata": {},
    }
    violations: list[str] = []

    order = dependency_order(jobs, violations)

    assert order[0] == "metadata", f"metadata must resolve first, got {order!r}"
    assert order.index("package") < order.index("publish"), (
        f"package must precede publish, got {order!r}"
    )
    assert order.index("admission") < order.index("publish"), (
        f"admission must precede publish, got {order!r}"
    )
    assert not violations, f"an acyclic graph must not be reported, got {violations!r}"


def test_dependency_order_reports_a_cycle_and_still_names_every_job() -> None:
    """Report a cyclic pair and return it after the jobs that could be ordered.

    Returning early with only the prefix would leave the unresolvable jobs
    without a result, so the caller would report a missing job instead of the
    cycle that caused it.
    """
    jobs: dict[str, object] = {
        "alpha": {"needs": "beta"},
        "beta": {"needs": "alpha"},
        "gamma": {},
    }
    violations: list[str] = []

    order = dependency_order(jobs, violations)

    assert order == ["gamma", "alpha", "beta"], (
        f"the orderable job must come first and the cycle follow, got {order!r}"
    )
    assert violations == ["release.jobs: dependency cycle among ['alpha', 'beta']"], (
        f"the cycle must be named once with its members, got {violations!r}"
    )

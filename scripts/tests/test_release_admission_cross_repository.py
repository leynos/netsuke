r"""Prove admission fetches a cross-repository candidate from its own origin.

The admission job checks out two commits that must not be confused. The
candidate sits at the workspace root, so ``origin`` names the repository the
candidate belongs to; the pinned verifier sits in a subdirectory. The gate runs
from the workspace root and fetches ``GITHUB_SHA`` from ``origin``, so the fetch
only reaches a caller's candidate when the working directory is the candidate
checkout. Running the same gate from the verifier checkout fetches from *this*
repository instead, which is the regression a single pinned checkout produced.

No command double can decide this: a cmd-mox invocation records its arguments,
its standard input, and its environment, but not its working directory, which is
exactly the input under test. These cases therefore use the two-repository
layout built by :mod:`release_admission_cross_repository_layout`, point
``NETSUKE_RELEASE_ADMISSION_GIT_ADAPTER`` at the host's own ``git``, and choose
the working directory through
:class:`release_admission_test_harness.GateRunRequest`.

Both admission modes are exercised, because the fetch precedes the enforcement
decision: observation reports the refusal and exits zero, while enforcement
fails closed, and neither outcome can mask a fetch that never reached the
candidate.

The producer-attribution switch is set to ``true`` here. This module is about
where the candidate is fetched from, not about who produced the evidence; the
shipped block that refuses unattributed evidence is pinned separately in
:mod:`test_release_admission_producer_trust`.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with hypothesis==6.151.9 \
        --with 'cmd-mox==0.2.0' --with 'cuprum==0.1.0' --with 'cyclopts==4.25.3' \
        python -m pytest \
        scripts/tests/test_release_admission_cross_repository.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import dataclasses
import typing as typ

import pytest
from release_admission_cross_repository_layout import (
    Repositories,
    build_layout,
    git_binary,
)
from release_admission_test_support import (
    CANARY_BY_OPERATION,
    METRICS_VALIDATOR,
    RELEASE_TAG,
    GateRun,
    GateRunRequest,
    assert_failure_trace_sequence,
    expected_gate_labels,
    expected_operation_labels,
    install_adapted,
    operation_records,
    run_gate_request,
)

pytest_plugins = ("cmd_mox.pytest_plugin",)

if typ.TYPE_CHECKING:
    import pathlib

    from cmd_mox import CmdMox

ENFORCEMENT_VARIABLE = "NETSUKE_RELEASE_ADMISSION_ENFORCE"
GIT_ADAPTER_VARIABLE = "NETSUKE_RELEASE_ADMISSION_GIT_ADAPTER"
PRODUCER_TRUST_VARIABLE = "NETSUKE_RELEASE_ADMISSION_PRODUCER_TRUSTED"
TAG_VARIABLE = "NETSUKE_RELEASE_ADMISSION_TAG"
#: A manifest the verifier admits on its own terms once the fetch has reached
#: the candidate: fresh, matching, and reporting both required canaries.
ADMITTABLE_EVIDENCE = "fresh"
#: The operation whose outcome separates a reached candidate from a refused
#: fetch, named once so the assertions cannot drift apart.
FETCH_OPERATION = "fetch_candidate_revision"


@dataclasses.dataclass(frozen=True, slots=True)
class RunLayout:
    """State the two inputs each case varies.

    Attributes
    ----------
    at_verifier
        Whether the gate runs from the verifier checkout instead of the
        candidate checkout.
    enforce
        Whether the run must fail closed rather than only observe.
    """

    at_verifier: bool
    enforce: bool


def _run(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
    repositories: Repositories,
    layout: RunLayout,
) -> GateRun:
    """Run the gate from the checkout the layout names, with real Git.

    Returns
    -------
    GateRun
        The completed process and the records it wrote.
    """
    adapted = install_adapted(cmd_mox)
    environment = {
        **adapted.variables(cmd_mox),
        GIT_ADAPTER_VARIABLE: git_binary(),
        ENFORCEMENT_VARIABLE: str(layout.enforce).lower(),
        PRODUCER_TRUST_VARIABLE: "true",
        TAG_VARIABLE: RELEASE_TAG,
        "GITHUB_SHA": repositories.candidate,
    }
    return run_gate_request(
        cmd_mox,
        GateRunRequest(
            tmp_path,
            evidence_state=ADMITTABLE_EVIDENCE,
            extra_environment=environment,
            cwd=repositories.working_directory(at_verifier=layout.at_verifier),
        ),
    )


def _assert_fetch_succeeded(run: GateRun, *, enforce: bool) -> None:
    """Require the run to reach the candidate and admit it."""
    assert run.result.returncode == 0, (
        f"a candidate reachable from the working directory's origin must be "
        f"admitted; {run.result.stderr}"
    )
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    assert operation_records(run.metrics, FETCH_OPERATION)[-1]["labels"] == (
        expected_operation_labels(
            CANARY_BY_OPERATION[FETCH_OPERATION], FETCH_OPERATION, "success", "none"
        )
    ), "the fetch must have reached the caller's repository"
    assert run.metrics[-1]["labels"] == expected_gate_labels("success", "none"), (
        "an admitted cross-repository candidate must leave a successful gate record"
    )
    assert run.outputs["gate-outcome"] == "success", (
        "the gate must publish the outcome the write jobs read"
    )
    assert run.outputs["enforcement"] == str(enforce).lower(), (
        "the gate must publish the mode it actually ran in"
    )


def _assert_fetch_refused(run: GateRun, *, enforce: bool) -> None:
    """Require the run to refuse the fetch performed from the verifier."""
    assert run.result.returncode == (1 if enforce else 0), (
        "enforcement must fail closed on a missing candidate while observation "
        f"retains the diagnostic; {run.result.stderr}"
    )
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    assert operation_records(run.metrics, FETCH_OPERATION)[-1]["labels"] == (
        expected_operation_labels(
            CANARY_BY_OPERATION[FETCH_OPERATION],
            FETCH_OPERATION,
            "failure",
            "fetch_error",
        )
    ), "fetching from the verifier's repository must be refused as a fetch error"
    assert run.metrics[-1]["labels"] == expected_gate_labels(
        "failure", "fetch_error"
    ), "the gate record must carry the fetch's own category"
    assert run.outputs["gate-error-category"] == "fetch_error", (
        "the published category must name the refused fetch"
    )
    assert_failure_trace_sequence(run.traces, FETCH_OPERATION, "fetch_error")


@pytest.mark.parametrize(
    ("at_verifier", "enforce"),
    [
        pytest.param(False, False, id="candidate-checkout-observation"),
        pytest.param(False, True, id="candidate-checkout-enforcement"),
        pytest.param(True, False, id="verifier-checkout-observation"),
        pytest.param(True, True, id="verifier-checkout-enforcement"),
    ],
)
def test_candidate_fetch_follows_the_working_directory(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
    *,
    at_verifier: bool,
    enforce: bool,
) -> None:
    """Verify only the candidate checkout's ``origin`` supplies the candidate.

    Notes
    -----
    The two checkouts differ in nothing else the gate reads: the candidate
    commit, the evidence manifest, the adapters, and the enforcement mode are
    identical, so a different verdict is attributable to the working directory
    and therefore to which repository ``origin`` names.
    """
    repositories = build_layout(tmp_path)
    run = _run(
        cmd_mox,
        tmp_path,
        repositories,
        RunLayout(at_verifier=at_verifier, enforce=enforce),
    )

    if at_verifier:
        _assert_fetch_refused(run, enforce=enforce)
    else:
        _assert_fetch_succeeded(run, enforce=enforce)

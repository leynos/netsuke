"""Name every controlled admission failure the runtime tests exercise.

The cases live apart from the modules that run them for the reason
:mod:`release_admission_test_scenarios` gives for the wrappers: a table of
data reads as specification, and a test module that also carried it would mix
the statement of what must happen with the machinery that makes it happen.

Each case maps to one refusal class. ``api_error`` and ``fetch_error`` are the
adapter failures RFC 0005 expects the gate to survive, ``mismatch`` is the
exact-commit binding, ``stale_evidence`` and ``missing_evidence`` are the
freshness refusals, ``unknown`` is the state the gate refuses to guess at, and
``timeout`` is the bound it places on its own adapters.

Deferred refusals
-----------------
Four RFC 0005 refusals have no case here, because neither half of the split RFC
0005 places them on is implemented yet:

- a missing or malformed evidence manifest,
- an archive whose digest disagrees with its sidecar,
- a failing advisory or licence scan, and
- a breached performance budget.

They are deferred *evidence-production and admission-validation integration*,
not a producer's exclusive property. Producers supply the manifest, the scan
results, and the measurements; admission then verifies what they supplied. RFC
0005 requires admission to "verify provenance rather than trusting declared
fields": it recomputes each archive's SHA-256 from its actual bytes and
compares that with both the sidecar and the digest the manifest records,
rejecting a mismatch. So the admission-side half is unimplemented here too, and
calling the set producer-owned would understate what this gate still owes.

The gate's own ``verify_evidence`` operation is covered, and its
``missing_evidence`` category with it, but only when the four operations before
it succeed: :meth:`gate.Gate.run_operations` stops at the first failed
operation, so a missing, stale, or unknown freshness state, or an earlier
adapter failure, prevents it from running at all. When it does run, its
``fresh`` handling is the contradiction :mod:`gate` documents and the suite
freezes -- ``check_scan_freshness`` admits a fresh state, but
``verify_evidence`` refuses it unless the state is *not* fresh and a workflow
run identifier is present. A passing freshness check is therefore never
sufficient for admission.
"""

import pytest
from release_admission_test_scenarios import (
    MISMATCHED_REVISION,
    FailureCase,
    answer_no_workflow_run,
    answer_other_commit,
    delay_commit_query,
    delay_workflow_run_query,
    fail_commit_query,
    fail_git,
    fail_workflow_run_query,
)

#: The bound every timeout case configures, and the delay it must exceed. Both
#: stay well inside the shim's own client timeout, so a timeout case measures
#: the gate's bound rather than cmd-mox's transport.
TIMEOUT_ENVIRONMENT = {
    "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS": "1",
}
DELAYED_SECONDS = 2.0

FAILURE_CASES = [
    pytest.param(
        FailureCase(
            evidence_state="missing",
            operation="resolve_tag_commit",
            error_category="api_error",
            adapters={"gh": fail_commit_query()},
        ),
        id="api-error",
    ),
    pytest.param(
        FailureCase(
            evidence_state="missing",
            operation="resolve_tag_commit",
            error_category="mismatch",
            adapters={"gh": answer_other_commit(MISMATCHED_REVISION)},
        ),
        id="candidate-mismatch",
    ),
    pytest.param(
        FailureCase(
            evidence_state="missing",
            operation="fetch_candidate_revision",
            error_category="fetch_error",
            adapters={"git": fail_git},
        ),
        id="fetch-error",
    ),
    pytest.param(
        FailureCase(
            evidence_state="stale",
            operation="check_scan_freshness",
            error_category="stale_evidence",
        ),
        id="stale-evidence",
    ),
    pytest.param(
        FailureCase(
            evidence_state="missing",
            operation="check_scan_freshness",
            error_category="missing_evidence",
        ),
        id="missing-evidence",
    ),
    pytest.param(
        FailureCase(
            evidence_state="something-else",
            operation="check_scan_freshness",
            error_category="unknown",
        ),
        id="unknown-evidence",
    ),
    pytest.param(
        FailureCase(
            evidence_state="fresh",
            operation="verify_evidence",
            error_category="missing_evidence",
        ),
        id="enforcement-rejects-environment-freshness",
    ),
    pytest.param(
        FailureCase(
            evidence_state="missing",
            operation="resolve_tag_commit",
            error_category="timeout",
            adapters={"gh": delay_commit_query(DELAYED_SECONDS)},
            environment=TIMEOUT_ENVIRONMENT,
        ),
        id="operation-timeout",
    ),
    pytest.param(
        FailureCase(
            evidence_state="fresh",
            operation="verify_evidence",
            error_category="missing_evidence",
            adapters={"gh": answer_no_workflow_run()},
            run_lookup_succeeds=True,
        ),
        id="missing-workflow-run-observation",
    ),
    pytest.param(
        FailureCase(
            evidence_state="fresh",
            operation="fetch_workflow_run",
            error_category="api_error",
            adapters={"gh": fail_workflow_run_query()},
            run_lookup_succeeds=False,
        ),
        id="workflow-run-api-error-enforcement",
    ),
    pytest.param(
        FailureCase(
            evidence_state="fresh",
            operation="fetch_workflow_run",
            error_category="api_error",
            adapters={"gh": fail_workflow_run_query()},
            enforce=False,
            run_lookup_succeeds=False,
        ),
        id="workflow-run-api-error-observation",
    ),
    pytest.param(
        FailureCase(
            evidence_state="fresh",
            operation="fetch_workflow_run",
            error_category="timeout",
            adapters={"gh": delay_workflow_run_query(DELAYED_SECONDS)},
            environment=TIMEOUT_ENVIRONMENT,
            run_lookup_succeeds=False,
        ),
        id="workflow-run-timeout-enforcement",
    ),
    pytest.param(
        FailureCase(
            evidence_state="fresh",
            operation="fetch_workflow_run",
            error_category="timeout",
            adapters={"gh": delay_workflow_run_query(DELAYED_SECONDS)},
            environment=TIMEOUT_ENVIRONMENT,
            enforce=False,
            run_lookup_succeeds=False,
        ),
        id="workflow-run-timeout-observation",
    ),
]

__all__ = ("DELAYED_SECONDS", "FAILURE_CASES", "TIMEOUT_ENVIRONMENT")

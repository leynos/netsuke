"""Name every controlled admission failure the runtime tests exercise.

The cases live apart from the modules that run them for the reason
:mod:`release_admission_test_scenarios` gives for the wrappers: a table of
data reads as specification, and a test module that also carried it would mix
the statement of what must happen with the machinery that makes it happen.

Each case maps to one refusal class. ``api_error``, ``fetch_error`` and
``mismatch`` are the adapter failures and the exact-commit binding;
``stale_evidence``, ``missing_evidence`` and ``malformed_evidence`` are the
three ways a manifest can fail to arrive; ``candidate_mismatch`` is a manifest
that arrived and describes another release; ``unknown`` is a manifest whose
canaries did not all succeed; and ``timeout`` is the bound the gate places on
its own adapters.

Deferred refusals
-----------------
RFC 0005 also requires admission to *recompute* what a producer recorded:
an archive whose digest disagrees with its sidecar, a failing advisory or
licence scan, and a breached performance budget. None of those has a case
here, because neither half of the split RFC 0005 places them on is
implemented yet.

They are deferred *evidence-production and admission-validation integration*,
not a producer's exclusive property. Producers supply the artefacts and the
measurements; admission then verifies what they supplied. RFC 0005 requires
admission to "verify provenance rather than trusting declared fields": it
recomputes each archive's SHA-256 from its actual bytes and compares that with
both the sidecar and the digest the manifest records, rejecting a mismatch. So
the admission-side half is unimplemented here too, and calling the set
producer-owned would understate what this gate still owes.

What *is* covered is the manifest itself. Every case below reaches its refusal
through the production verifier, and the four operations before
``verify_evidence`` stop the chain when they fail -- so an evidence refusal is
only observable once the tag resolution, the fetch, the run lookup, and the
freshness check have all succeeded.
"""

import pytest
from release_admission_test_scenarios import (
    MISMATCHED_REVISION,
    FailureCase,
    answer_no_run_head,
    answer_other_commit,
    answer_run_head,
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

#: One case per refusal the gate can reach through its own operations. The
#: evidence states name the manifest each case writes, and every enforcing
#: case also names the tag, because a publishing run must.
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
        id="tag-resolves-elsewhere",
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
            evidence_state="missing",
            operation="fetch_workflow_run",
            error_category="api_error",
            adapters={"gh": answer_no_run_head()},
        ),
        id="empty-run-head",
    ),
    pytest.param(
        FailureCase(
            evidence_state="fresh",
            operation="fetch_workflow_run",
            error_category="candidate_mismatch",
            adapters={"gh": answer_run_head(MISMATCHED_REVISION)},
        ),
        id="run-head-mismatch",
    ),
    pytest.param(
        FailureCase(
            evidence_state="missing",
            operation="fetch_workflow_run",
            error_category="api_error",
            adapters={"gh": fail_workflow_run_query()},
        ),
        id="workflow-run-api-error-enforcement",
    ),
    pytest.param(
        FailureCase(
            evidence_state="missing",
            operation="fetch_workflow_run",
            error_category="api_error",
            adapters={"gh": fail_workflow_run_query()},
            enforce=False,
        ),
        id="workflow-run-api-error-observation",
    ),
    pytest.param(
        FailureCase(
            evidence_state="missing",
            operation="fetch_workflow_run",
            error_category="timeout",
            adapters={"gh": delay_workflow_run_query(DELAYED_SECONDS)},
            environment=TIMEOUT_ENVIRONMENT,
        ),
        id="workflow-run-timeout-enforcement",
    ),
    pytest.param(
        FailureCase(
            evidence_state="missing",
            operation="fetch_workflow_run",
            error_category="timeout",
            adapters={"gh": delay_workflow_run_query(DELAYED_SECONDS)},
            environment=TIMEOUT_ENVIRONMENT,
            enforce=False,
        ),
        id="workflow-run-timeout-observation",
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
            evidence_state="missing",
            operation="check_scan_freshness",
            error_category="missing_evidence",
        ),
        id="missing-evidence",
    ),
    pytest.param(
        FailureCase(
            evidence_state="malformed",
            operation="check_scan_freshness",
            error_category="malformed_evidence",
        ),
        id="malformed-evidence",
    ),
    pytest.param(
        FailureCase(
            evidence_state="oversized",
            operation="check_scan_freshness",
            error_category="malformed_evidence",
        ),
        id="oversized-evidence",
    ),
    pytest.param(
        FailureCase(
            evidence_state="unknown-schema",
            operation="check_scan_freshness",
            error_category="malformed_evidence",
        ),
        id="unknown-schema",
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
            evidence_state="future",
            operation="check_scan_freshness",
            error_category="stale_evidence",
        ),
        id="future-evidence",
    ),
    pytest.param(
        FailureCase(
            evidence_state="mismatched",
            operation="verify_evidence",
            error_category="candidate_mismatch",
        ),
        id="other-commit",
    ),
    pytest.param(
        FailureCase(
            evidence_state="wrong-run",
            operation="verify_evidence",
            error_category="candidate_mismatch",
        ),
        id="other-run",
    ),
    pytest.param(
        FailureCase(
            evidence_state="wrong-repository",
            operation="verify_evidence",
            error_category="candidate_mismatch",
        ),
        id="other-repository",
    ),
    pytest.param(
        FailureCase(
            evidence_state="wrong-tag",
            operation="verify_evidence",
            error_category="candidate_mismatch",
        ),
        id="other-tag",
    ),
    pytest.param(
        FailureCase(
            evidence_state="incomplete",
            operation="verify_evidence",
            error_category="missing_evidence",
        ),
        id="incomplete-results",
    ),
    pytest.param(
        FailureCase(
            evidence_state="unsuccessful",
            operation="verify_evidence",
            error_category="unknown",
        ),
        id="unsuccessful-results",
    ),
]

__all__ = ("DELAYED_SECONDS", "FAILURE_CASES", "TIMEOUT_ENVIRONMENT")

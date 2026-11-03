"""Hold exact composite-action outputs and step order for workflow contracts.

The values here are the test oracle for the publisher's external action shape;
the workflow contract checks the parsed action against them.
"""

ACTION_OUTPUTS = {
    "action": "${{ steps.decision.outputs.action }}",
    "crate-name": "${{ steps.candidate.outputs.crate-name }}",
    "crate-version": "${{ inputs.version }}",
    "registry-state": "${{ steps.decision.outputs.state }}",
    "verification-verdict": "${{ steps.verify.outcome }}",
    "sha256": "${{ steps.candidate.outputs.sha256 }}",
    "verified-sha256": "${{ steps.verify.outputs.sha256 }}",
    "crate-candidate-validation-outcome": "${{ steps.candidate.outcome }}",
    "crate-candidate-validation-started-ns": (
        "${{ steps.candidate.outputs.started_ns }}"
    ),
    "crate-candidate-validation-ended-ns": "${{ steps.candidate.outputs.ended_ns }}",
    "registry-probe-outcome": "${{ steps.probe.outcome }}",
    "registry-probe-started-ns": "${{ steps.probe.outputs.started_ns }}",
    "registry-probe-ended-ns": "${{ steps.probe.outputs.ended_ns }}",
    "registry-decision-outcome": "${{ steps.decision.outcome }}",
    "registry-decision-started-ns": "${{ steps.decision.outputs.started_ns }}",
    "registry-decision-ended-ns": "${{ steps.decision.outputs.ended_ns }}",
    "trusted-publishing-authentication-outcome": "${{ steps.auth.outcome }}",
    "trusted-publishing-authentication-started-ns": (
        "${{ steps.auth_start.outputs.started_ns }}"
    ),
    "trusted-publishing-authentication-ended-ns": (
        "${{ steps.auth_end.outputs.ended_ns }}"
    ),
    "registry-metadata-derivation-outcome": "${{ steps.metadata.outcome }}",
    "registry-metadata-derivation-started-ns": (
        "${{ steps.metadata.outputs.started_ns }}"
    ),
    "registry-metadata-derivation-ended-ns": "${{ steps.metadata.outputs.ended_ns }}",
    "registry-upload-outcome": "${{ steps.upload.outcome }}",
    "registry-upload-started-ns": "${{ steps.upload.outputs.started_ns }}",
    "registry-upload-ended-ns": "${{ steps.upload.outputs.ended_ns }}",
    "registry-download-outcome": "${{ steps.download.outcome }}",
    "registry-download-started-ns": "${{ steps.download.outputs.started_ns }}",
    "registry-download-ended-ns": "${{ steps.download.outputs.ended_ns }}",
    "registry-download-attempts": "${{ steps.download.outputs.attempts }}",
    "registry-download-status-class": "${{ steps.download.outputs.status-class }}",
    "registry-download-elapsed-ms": "${{ steps.download.outputs.elapsed-ms }}",
    "registry-byte-verification-outcome": "${{ steps.verify.outcome }}",
    "registry-byte-verification-started-ns": "${{ steps.verify.outputs.started_ns }}",
    "registry-byte-verification-ended-ns": "${{ steps.verify.outputs.ended_ns }}",
}
ACTION_STEP_ORDER = (
    "candidate",
    "probe",
    "decision",
    "auth_start",
    "auth",
    "auth_end",
    "metadata",
    "upload",
    "download",
    "verify",
)

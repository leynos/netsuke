"""Hold exact composite-action outputs and step order for workflow contracts.

The values here are the test oracle for the publisher's external action shape;
the workflow contract checks the parsed action against them.
"""

ACTION_OUTPUTS = {
    "action": "${{ steps.decision.outputs.action }}",
    "crate-name": "${{ steps.candidate.outputs.crate-name }}",
    "package-name": "${{ steps.candidate.outputs.package-name }}",
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


def classification_outputs_are_fail_closed(decision_run: str) -> bool:
    """Require bounded decision outputs before a failed classifier returns.

    Returns
    -------
    bool
        Whether the action exports the classifier decision before propagating failure.

    Examples
    --------
    >>> classification_outputs_are_fail_closed("")
    False
    """
    action_output = 'printf \'action=%s\\n\' "$action" >>"$GITHUB_OUTPUT"'
    state_output = 'printf \'state=%s\\n\' "$state" >>"$GITHUB_OUTPUT"'
    exit_status = 'exit "$classification_status"'
    required_fragments = (
        "if uv run --no-project --python 3.14 scripts/publish_crates_io_cli.py",
        "classification_status=0",
        "classification_status=$?",
        "action=fail",
        "state=indeterminate",
        'if [[ "$classification_status" -ne 0 ]]',
        exit_status,
    )
    exit_position = decision_run.find(exit_status)
    outputs_precede_failure = all(
        0 <= decision_run.find(output) < exit_position
        for output in (action_output, state_output)
    )
    return all(fragment in decision_run for fragment in required_fragments) and (
        outputs_precede_failure
    )

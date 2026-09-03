//! Trust-aware reconciliation for project manifest-budget requests.
//!
//! Keeps generic configuration precedence from granting an untrusted primary
//! project configuration authority to raise the invoking operator's ceilings.

use tracing::debug;

use super::config::CliConfig;
use super::discovery::ProjectManifestBudgetRequest;

/// Reconcile project budget requests with merged operator ceilings.
///
/// Project configuration may narrow a limit, but never widen a limit supplied
/// by defaults, user configuration, the environment, or the command line.
pub(super) fn reconcile_manifest_budget(
    mut operator_limits: CliConfig,
    project_request: &ProjectManifestBudgetRequest,
) -> CliConfig {
    clamp_limit(
        &mut operator_limits.manifest_evaluation_fuel,
        project_request.evaluation_fuel,
        "manifest_evaluation_fuel",
    );
    clamp_limit(
        &mut operator_limits.manifest_fuel,
        project_request.manifest_fuel,
        "manifest_fuel",
    );
    clamp_limit(
        &mut operator_limits.manifest_rendered_value_bytes,
        project_request.rendered_value_bytes,
        "manifest_rendered_value_bytes",
    );
    clamp_limit(
        &mut operator_limits.manifest_rendered_manifest_bytes,
        project_request.rendered_manifest_bytes,
        "manifest_rendered_manifest_bytes",
    );
    clamp_limit(
        &mut operator_limits.manifest_source_bytes,
        project_request.source_bytes,
        "manifest_source_bytes",
    );
    clamp_limit(
        &mut operator_limits.manifest_foreach_cardinality,
        project_request.foreach_cardinality,
        "manifest_foreach_cardinality",
    );
    clamp_limit(
        &mut operator_limits.manifest_expanded_entries,
        project_request.expanded_entries,
        "manifest_expanded_entries",
    );
    operator_limits
}

/// Clamp one project request to an operator-controlled ceiling.
fn clamp_limit<T>(operator_limit: &mut T, project_request: Option<T>, field: &'static str)
where
    T: Ord + Copy,
{
    let Some(project_limit) = project_request else {
        return;
    };
    if project_limit > *operator_limit {
        debug!(field, "clamped project manifest budget request");
    }
    *operator_limit = (*operator_limit).min(project_limit);
}

#[cfg(test)]
mod tests {
    //! Unit tests for project manifest-budget reconciliation.

    use super::*;

    /// Verify that a project cannot raise an operator ceiling.
    #[test]
    fn project_cannot_widen_an_operator_limit() {
        let reconciled = reconcile_manifest_budget(
            CliConfig {
                manifest_fuel: 16,
                ..CliConfig::default()
            },
            &ProjectManifestBudgetRequest {
                manifest_fuel: Some(17),
                ..ProjectManifestBudgetRequest::default()
            },
        );

        assert_eq!(reconciled.manifest_fuel, 16);
    }

    /// Verify that a project can narrow an operator ceiling.
    #[test]
    fn project_can_narrow_an_operator_limit() {
        let reconciled = reconcile_manifest_budget(
            CliConfig {
                manifest_fuel: 16,
                ..CliConfig::default()
            },
            &ProjectManifestBudgetRequest {
                manifest_fuel: Some(15),
                ..ProjectManifestBudgetRequest::default()
            },
        );

        assert_eq!(reconciled.manifest_fuel, 15);
    }
}

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
    use rstest::{fixture, rstest};

    /// Supply operator ceilings and an empty project request for each case.
    #[fixture]
    fn budget_request() -> (CliConfig, ProjectManifestBudgetRequest) {
        (
            CliConfig {
                manifest_fuel: 16,
                ..CliConfig::default()
            },
            ProjectManifestBudgetRequest::default(),
        )
    }

    #[rstest]
    #[case::cannot_widen(17, 16)]
    #[case::can_narrow(15, 15)]
    fn project_budget_only_narrows_operator_limit(
        budget_request: (CliConfig, ProjectManifestBudgetRequest),
        #[case] requested: u64,
        #[case] expected: u64,
    ) {
        let (operator_limits, mut request) = budget_request;
        request.manifest_fuel = Some(requested);
        let reconciled = reconcile_manifest_budget(operator_limits, &request);
        assert_eq!(reconciled.manifest_fuel, expected);
    }
}

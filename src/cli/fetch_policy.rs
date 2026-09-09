//! Adapt merged configuration to the pure network-policy reconciliation domain.

use super::config::CliConfig;
use super::discovery::ProjectFetchPolicyRequest;
use crate::stdlib::FetchPolicyReconciliationOutcome;
use crate::stdlib::reconciliation::{OperatorFetchPolicy, ProjectFetchPolicy, reconcile};

/// Reconcile fetch grants, preserving blocks and unrelated configuration.
///
/// Discovery supplies the primary quarantined request. The domain owns the trust
/// decision and returns value-free outcome data for the merge observer.
pub(super) fn reconcile_fetch_policy(
    mut config: CliConfig,
    project_request: Option<ProjectFetchPolicyRequest>,
) -> (CliConfig, FetchPolicyReconciliationOutcome) {
    let operator = OperatorFetchPolicy {
        default_deny: config.fetch_default_deny,
        allow_scheme: std::mem::take(&mut config.fetch_allow_scheme),
        allow_host: std::mem::take(&mut config.fetch_allow_host),
        trust_project_policy: config.trust_project_fetch_policy,
    };
    let reconciled = reconcile(operator, project_request.map(domain_request));
    config.fetch_default_deny = reconciled.default_deny;
    config.fetch_allow_scheme = reconciled.allow_scheme;
    config.fetch_allow_host = reconciled.allow_host;
    (config, reconciled.outcome)
}

/// Translate quarantined discovery fields without carrying provenance inward.
fn domain_request(request: ProjectFetchPolicyRequest) -> ProjectFetchPolicy {
    ProjectFetchPolicy {
        default_deny: request.default_deny,
        allow_scheme: request.allow_scheme,
        allow_host: request.allow_host,
    }
}

#[cfg(test)]
mod tests {
    //! Verify the adapter preserves configuration outside reconciled grants.

    use super::*;
    use crate::host_pattern::HostPattern;

    #[test]
    fn adapter_preserves_every_unrelated_configuration_field() {
        let config = CliConfig {
            json: true,
            fetch_block_host: vec![HostPattern::parse("blocked.example.org").expect("valid host")],
            ..CliConfig::default()
        };
        let expected = serde_json::to_value(&config).expect("serialize config");
        let (reconciled, _) = reconcile_fetch_policy(
            config,
            Some(ProjectFetchPolicyRequest {
                default_deny: Some(true),
                ..ProjectFetchPolicyRequest::default()
            }),
        );
        let mut actual = serde_json::to_value(reconciled).expect("serialize reconciled config");
        *actual
            .get_mut("fetch_default_deny")
            .expect("serialized config includes default-deny") = expected
            .get("fetch_default_deny")
            .expect("original config includes default-deny")
            .clone();
        assert_eq!(actual, expected);
    }
}

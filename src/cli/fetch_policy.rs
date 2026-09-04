//! Trust-aware reconciliation for project fetch-policy requests.
//!
//! Keeps generic configuration precedence from granting an untrusted primary
//! project configuration authority to widen the invoking operator's policy.

use super::config::CliConfig;
use super::discovery::ProjectFetchPolicyRequest;

/// Reconcile project fetch-policy restrictions with merged operator policy.
///
/// Project configuration is allowed to make outbound access stricter by
/// enabling default-deny. It cannot grant additional schemes or hosts unless
/// an operator enabled `trust_project_fetch_policy` from a trusted layer.
pub(super) fn reconcile_fetch_policy(
    mut operator_policy: CliConfig,
    project_request: ProjectFetchPolicyRequest,
) -> CliConfig {
    if operator_policy.trust_project_fetch_policy {
        operator_policy
            .fetch_allow_scheme
            .extend(project_request.allow_scheme);
        operator_policy
            .fetch_allow_host
            .extend(project_request.allow_host);
        if let Some(default_deny) = project_request.default_deny {
            operator_policy.fetch_default_deny = default_deny;
        }
    } else {
        operator_policy.fetch_default_deny |= project_request.default_deny == Some(true);
    }
    operator_policy
}

#[cfg(test)]
mod tests {
    //! Unit tests for trust-aware fetch-policy reconciliation.

    use super::*;
    use crate::host_pattern::HostPattern;
    use proptest::prelude::*;

    /// Build a project request from compact test inputs.
    fn project_request(
        default_deny: Option<bool>,
        allow_scheme: &[&str],
        allow_host: Vec<HostPattern>,
    ) -> ProjectFetchPolicyRequest {
        ProjectFetchPolicyRequest {
            default_deny,
            allow_scheme: allow_scheme.iter().map(ToString::to_string).collect(),
            allow_host,
        }
    }

    #[test]
    fn project_false_cannot_clear_operator_default_deny() {
        let reconciled = reconcile_fetch_policy(
            CliConfig {
                fetch_default_deny: true,
                ..CliConfig::default()
            },
            project_request(Some(false), &[], vec![]),
        );

        assert!(reconciled.fetch_default_deny);
    }

    #[test]
    fn project_true_tightens_operator_allow_by_default() {
        let reconciled = reconcile_fetch_policy(
            CliConfig::default(),
            project_request(Some(true), &[], vec![]),
        );

        assert!(reconciled.fetch_default_deny);
    }

    #[test]
    fn project_grants_are_dropped_without_opt_in() {
        let reconciled = reconcile_fetch_policy(
            CliConfig {
                fetch_allow_scheme: vec!["https".to_owned()],
                fetch_allow_host: vec![
                    HostPattern::parse("downloads.example.org")
                        .expect("parse operator host pattern"),
                ],
                ..CliConfig::default()
            },
            project_request(
                Some(false),
                &["http"],
                vec![HostPattern::parse("metadata.internal").expect("parse project host pattern")],
            ),
        );

        assert_eq!(reconciled.fetch_allow_scheme, ["https"]);
        assert_eq!(
            reconciled.fetch_allow_host,
            [HostPattern::parse("downloads.example.org").expect("parse expected host pattern")]
        );
    }

    #[test]
    fn project_grants_are_appended_with_operator_opt_in() {
        let reconciled = reconcile_fetch_policy(
            CliConfig {
                trust_project_fetch_policy: true,
                fetch_allow_scheme: vec!["https".to_owned()],
                fetch_allow_host: vec![
                    HostPattern::parse("downloads.example.org")
                        .expect("parse operator host pattern"),
                ],
                ..CliConfig::default()
            },
            project_request(
                Some(true),
                &["http"],
                vec![HostPattern::parse("metadata.internal").expect("parse project host pattern")],
            ),
        );

        assert!(reconciled.fetch_default_deny);
        assert_eq!(reconciled.fetch_allow_scheme, ["https", "http"]);
        assert_eq!(
            reconciled.fetch_allow_host,
            [
                HostPattern::parse("downloads.example.org").expect("parse expected host pattern"),
                HostPattern::parse("metadata.internal").expect("parse expected host pattern"),
            ]
        );
    }

    #[test]
    fn reconciliation_leaves_cumulative_blocks_untouched() {
        let block = HostPattern::parse("metadata.internal").expect("parse block host pattern");
        let reconciled = reconcile_fetch_policy(
            CliConfig {
                fetch_block_host: vec![block.clone()],
                ..CliConfig::default()
            },
            project_request(Some(true), &[], vec![]),
        );

        assert_eq!(reconciled.fetch_block_host, [block]);
    }

    proptest! {
        /// Reconcile every generated request according to the trust contract.
        #[test]
        fn reconciliation_preserves_trust_boundary_for_generated_policies(
            operator_default_deny in any::<bool>(),
            project_default_deny in prop::option::of(any::<bool>()),
            trust_project_policy in any::<bool>(),
            operator_schemes in prop::collection::vec("[a-z]{1,8}", 0..5),
            project_schemes in prop::collection::vec("[a-z]{1,8}", 0..5),
            operator_host_indices in prop::collection::vec(0u8..16, 0..5),
            project_host_indices in prop::collection::vec(16u8..32, 0..5),
            block_host_indices in prop::collection::vec(32u8..48, 0..5),
        ) {
            let operator_hosts = operator_host_indices
                .iter()
                .map(|index| HostPattern::parse(&format!("operator{index}.example.org"))
                    .expect("strategy constructs valid operator host patterns"))
                .collect::<Vec<_>>();
            let project_hosts = project_host_indices
                .iter()
                .map(|index| HostPattern::parse(&format!("project{index}.example.org"))
                    .expect("strategy constructs valid project host patterns"))
                .collect::<Vec<_>>();
            let blocked_hosts = block_host_indices
                .iter()
                .map(|index| HostPattern::parse(&format!("blocked{index}.example.org"))
                    .expect("strategy constructs valid blocked host patterns"))
                .collect::<Vec<_>>();
            let expected_default_deny = if trust_project_policy {
                project_default_deny.unwrap_or(operator_default_deny)
            } else {
                operator_default_deny || project_default_deny == Some(true)
            };
            let expected_schemes = if trust_project_policy {
                operator_schemes
                    .iter()
                    .chain(project_schemes.iter())
                    .cloned()
                    .collect()
            } else {
                operator_schemes.clone()
            };
            let expected_hosts = if trust_project_policy {
                operator_hosts
                    .iter()
                    .chain(project_hosts.iter())
                    .cloned()
                    .collect()
            } else {
                operator_hosts.clone()
            };
            let reconciled = reconcile_fetch_policy(
                CliConfig {
                    fetch_default_deny: operator_default_deny,
                    trust_project_fetch_policy: trust_project_policy,
                    fetch_allow_scheme: operator_schemes,
                    fetch_allow_host: operator_hosts,
                    fetch_block_host: blocked_hosts.clone(),
                    ..CliConfig::default()
                },
                ProjectFetchPolicyRequest {
                    default_deny: project_default_deny,
                    allow_scheme: project_schemes,
                    allow_host: project_hosts,
                },
            );

            prop_assert_eq!(reconciled.fetch_default_deny, expected_default_deny);
            prop_assert_eq!(reconciled.fetch_allow_scheme, expected_schemes);
            prop_assert_eq!(reconciled.fetch_allow_host, expected_hosts);
            prop_assert_eq!(reconciled.fetch_block_host, blocked_hosts);
        }
    }
}

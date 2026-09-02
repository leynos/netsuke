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
}

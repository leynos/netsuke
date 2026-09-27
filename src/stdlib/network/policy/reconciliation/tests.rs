//! Exercise the trust truth table and an independent ordered-layer model.

use super::*;
use proptest::prelude::*;
use rstest::rstest;

#[rstest]
#[case(false, false, None, (false, DefaultDenyDecision::OperatorRetained))]
#[case(false, true, None, (true, DefaultDenyDecision::OperatorRetained))]
#[case(true, false, None, (false, DefaultDenyDecision::OperatorRetained))]
#[case(true, true, None, (true, DefaultDenyDecision::OperatorRetained))]
#[case(
    false,
    false,
    Some(false),
    (false, DefaultDenyDecision::ProjectDowngradeIgnored)
)]
#[case(
    false,
    true,
    Some(false),
    (true, DefaultDenyDecision::ProjectDowngradeIgnored)
)]
#[case(false, false, Some(true), (true, DefaultDenyDecision::ProjectTightened))]
#[case(
    false,
    true,
    Some(true),
    (true, DefaultDenyDecision::ProjectRestrictionRetained)
)]
#[case(
    true,
    false,
    Some(false),
    (false, DefaultDenyDecision::TrustedProjectOverride)
)]
#[case(
    true,
    true,
    Some(false),
    (false, DefaultDenyDecision::TrustedProjectOverride)
)]
#[case(
    true,
    false,
    Some(true),
    (true, DefaultDenyDecision::TrustedProjectOverride)
)]
#[case(
    true,
    true,
    Some(true),
    (true, DefaultDenyDecision::TrustedProjectOverride)
)]
fn default_deny_truth_table(
    #[case] trust: bool,
    #[case] operator_deny: bool,
    #[case] project_deny: Option<bool>,
    #[case] expected: (bool, DefaultDenyDecision),
) {
    let (expected_deny, decision) = expected;
    let result = reconcile(
        OperatorFetchPolicy {
            default_deny: operator_deny,
            trust_project_policy: trust,
            ..OperatorFetchPolicy::default()
        },
        Some(ProjectFetchPolicy {
            default_deny: project_deny,
            ..ProjectFetchPolicy::default()
        }),
    );
    assert_eq!(result.default_deny, expected_deny);
    assert_eq!(result.outcome.default_deny_decision, decision);
    assert!(result.outcome.project_request_present);
}

#[test]
fn absence_differs_from_an_empty_project_layer() {
    let absent = reconcile(OperatorFetchPolicy::default(), None);
    let present = reconcile(
        OperatorFetchPolicy::default(),
        Some(ProjectFetchPolicy::default()),
    );
    assert!(!absent.outcome.project_request_present);
    assert!(present.outcome.project_request_present);
    assert_eq!(absent.default_deny, present.default_deny);
    assert_eq!(absent.allow_scheme, present.allow_scheme);
    assert_eq!(absent.allow_host, present.allow_host);
}

#[rstest]
#[case(false, (false, 1, 0, 1))]
#[case(true, (false, 2, 1, 0))]
fn project_grants_require_trusted_operator_opt_in(
    #[case] trust: bool,
    #[case] expected: (bool, usize, usize, usize),
) {
    let (expected_deny, expected_grants, accepted, ignored) = expected;
    let result = reconcile(
        OperatorFetchPolicy {
            trust_project_policy: trust,
            allow_scheme: vec!["https".to_owned()],
            allow_host: hosts(&[0]).expect("valid operator host"),
            ..OperatorFetchPolicy::default()
        },
        Some(ProjectFetchPolicy {
            default_deny: Some(false),
            allow_scheme: vec!["http".to_owned()],
            allow_host: hosts(&[1]).expect("valid project host"),
        }),
    );
    assert_eq!(result.default_deny, expected_deny);
    assert_eq!(result.allow_scheme.len(), expected_grants);
    assert_eq!(result.allow_host.len(), expected_grants);
    assert_eq!(result.outcome.accepted_scheme_grants, accepted);
    assert_eq!(result.outcome.accepted_host_grants, accepted);
    assert_eq!(result.outcome.ignored_scheme_grants, ignored);
    assert_eq!(result.outcome.ignored_host_grants, ignored);
}

/// Construct only valid host patterns from generated indices.
fn hosts(indices: &[u8]) -> Result<Vec<HostPattern>, crate::host_pattern::HostPatternError> {
    indices
        .iter()
        .map(|index| HostPattern::parse(&format!("host{index}.example.org")))
        .collect()
}

/// Independently apply the project request to a simple operator model.
fn model_default_deny(operator: bool, trust: bool, request: Option<bool>) -> bool {
    match request {
        Some(project) if trust => project,
        Some(true) => true,
        Some(false) | None => operator,
    }
}

proptest! {
    #[test]
    fn reconciliation_matches_independent_policy_model(
        operator_default in any::<bool>(),
        trust in any::<bool>(),
        operator_schemes in prop::collection::vec("[a-z]{1,8}", 0..5),
        operator_indices in prop::collection::vec(0u8..16, 0..5),
        project_default in prop::option::of(any::<bool>()),
        project_schemes in prop::collection::vec("[a-z]{1,8}", 0..5),
        project_indices in prop::collection::vec(16u8..32, 0..5),
    ) {
        let operator_hosts = hosts(&operator_indices).expect("valid generated operator hosts");
        let project_hosts = hosts(&project_indices).expect("valid generated project hosts");
        let expected_deny = model_default_deny(operator_default, trust, project_default);
        let scheme_count = project_schemes.len();
        let host_count = project_hosts.len();
        let expected_schemes = operator_schemes.iter().chain(trust.then_some(&project_schemes).into_iter().flatten()).cloned().collect::<Vec<_>>();
        let expected_hosts = operator_hosts.iter().chain(trust.then_some(&project_hosts).into_iter().flatten()).cloned().collect::<Vec<_>>();
        let result = reconcile(OperatorFetchPolicy {
            default_deny: operator_default,
            trust_project_policy: trust,
            allow_scheme: operator_schemes,
            allow_host: operator_hosts,
        }, Some(ProjectFetchPolicy {
            default_deny: project_default,
            allow_scheme: project_schemes,
            allow_host: project_hosts,
        }));
        prop_assert_eq!(result.default_deny, expected_deny);
        prop_assert_eq!(result.allow_scheme, expected_schemes);
        prop_assert_eq!(result.allow_host, expected_hosts);
        prop_assert!(result.outcome.project_request_present);
        prop_assert_eq!(result.outcome.trusted_project_policy, trust);
        prop_assert_eq!(result.outcome.requested_scheme_grants, scheme_count);
        prop_assert_eq!(result.outcome.requested_host_grants, host_count);
        prop_assert_eq!(result.outcome.accepted_scheme_grants, usize::from(trust) * scheme_count);
        prop_assert_eq!(result.outcome.accepted_host_grants, usize::from(trust) * host_count);
        prop_assert_eq!(result.outcome.ignored_scheme_grants, usize::from(!trust) * scheme_count);
        prop_assert_eq!(result.outcome.ignored_host_grants, usize::from(!trust) * host_count);
    }
}

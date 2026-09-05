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
        [ProjectFetchPolicy {
            default_deny: project_deny,
            ..ProjectFetchPolicy::default()
        }],
    );
    assert_eq!(result.default_deny, expected_deny);
    assert_eq!(result.outcome.default_deny_decision, decision);
    assert!(result.outcome.project_request_present);
}

#[test]
fn absence_differs_from_an_empty_project_layer() {
    let absent = reconcile(OperatorFetchPolicy::default(), []);
    let present = reconcile(
        OperatorFetchPolicy::default(),
        [ProjectFetchPolicy::default()],
    );
    assert!(!absent.outcome.project_request_present);
    assert!(present.outcome.project_request_present);
    assert_eq!(absent.default_deny, present.default_deny);
    assert_eq!(absent.allow_scheme, present.allow_scheme);
    assert_eq!(absent.allow_host, present.allow_host);
}

#[rstest]
#[case(false, (true, 1, 0, 2))]
#[case(true, (false, 3, 2, 0))]
fn ordered_project_layers_require_trust_to_undo_restrictions(
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
        [
            ProjectFetchPolicy {
                default_deny: Some(true),
                allow_scheme: vec!["http".to_owned()],
                allow_host: hosts(&[1]).expect("valid first project host"),
            },
            ProjectFetchPolicy {
                default_deny: Some(false),
                allow_scheme: vec!["ftp".to_owned()],
                allow_host: hosts(&[2]).expect("valid second project host"),
            },
        ],
    );
    assert_eq!(result.default_deny, expected_deny);
    assert_eq!(result.allow_scheme.len(), expected_grants);
    assert_eq!(result.allow_host.len(), expected_grants);
    assert_eq!(result.outcome.accepted_scheme_grant_count, accepted);
    assert_eq!(result.outcome.accepted_host_grant_count, accepted);
    assert_eq!(result.outcome.ignored_scheme_grant_count, ignored);
    assert_eq!(result.outcome.ignored_host_grant_count, ignored);
}

/// Construct only valid host patterns from generated indices.
fn hosts(indices: &[u8]) -> Result<Vec<HostPattern>, crate::host_pattern::HostPatternError> {
    indices
        .iter()
        .map(|index| HostPattern::parse(&format!("host{index}.example.org")))
        .collect()
}

/// Independently select the highest-priority applicable restriction.
fn model_default_deny(operator: bool, trust: bool, requests: &[ProjectFetchPolicy]) -> bool {
    if trust {
        requests
            .iter()
            .rev()
            .find_map(|request| request.default_deny)
            .unwrap_or(operator)
    } else {
        operator
            || requests
                .iter()
                .any(|request| request.default_deny == Some(true))
    }
}

proptest! {
    #[test]
    fn ordered_layers_match_independent_policy_model(
        operator_default in any::<bool>(),
        trust in any::<bool>(),
        operator_schemes in prop::collection::vec("[a-z]{1,8}", 0..5),
        operator_indices in prop::collection::vec(0u8..16, 0..5),
        layers in prop::collection::vec((
            prop::option::of(any::<bool>()),
            prop::collection::vec("[a-z]{1,8}", 0..5),
            prop::collection::vec(16u8..32, 0..5),
        ), 0..8),
    ) {
        let operator_hosts = hosts(&operator_indices).expect("valid generated operator hosts");
        let requests: Vec<_> = layers.into_iter().map(|(default_deny, allow_scheme, indices)| {
            ProjectFetchPolicy { default_deny, allow_scheme, allow_host: hosts(&indices).expect("valid generated project hosts") }
        }).collect();
        let expected_deny = model_default_deny(operator_default, trust, &requests);
        let scheme_count = requests.iter().map(|request| request.allow_scheme.len()).sum::<usize>();
        let host_count = requests.iter().map(|request| request.allow_host.len()).sum::<usize>();
        let expected_schemes = operator_schemes.iter().chain(
            requests.iter().filter(|_| trust).flat_map(|request| &request.allow_scheme)
        ).cloned().collect::<Vec<_>>();
        let expected_hosts = operator_hosts.iter().chain(
            requests.iter().filter(|_| trust).flat_map(|request| &request.allow_host)
        ).cloned().collect::<Vec<_>>();
        let request_present = !requests.is_empty();
        let result = reconcile(OperatorFetchPolicy {
            default_deny: operator_default,
            trust_project_policy: trust,
            allow_scheme: operator_schemes,
            allow_host: operator_hosts,
        }, requests);
        prop_assert_eq!(result.default_deny, expected_deny);
        prop_assert_eq!(result.allow_scheme, expected_schemes);
        prop_assert_eq!(result.allow_host, expected_hosts);
        prop_assert_eq!(result.outcome.project_request_present, request_present);
        prop_assert_eq!(result.outcome.trust_enabled, trust);
        prop_assert_eq!(result.outcome.requested_scheme_grant_count, scheme_count);
        prop_assert_eq!(result.outcome.requested_host_grant_count, host_count);
        prop_assert_eq!(result.outcome.accepted_scheme_grant_count, usize::from(trust) * scheme_count);
        prop_assert_eq!(result.outcome.accepted_host_grant_count, usize::from(trust) * host_count);
        prop_assert_eq!(result.outcome.ignored_scheme_grant_count, usize::from(!trust) * scheme_count);
        prop_assert_eq!(result.outcome.ignored_host_grant_count, usize::from(!trust) * host_count);
    }
}

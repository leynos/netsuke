//! Reconcile ordered project requests against an operator's fetch-policy ceiling.
//!
//! Discovery owns provenance; this module owns only the pure trust decision.
//! Blocklists are deliberately absent because their cumulative merge is already
//! restrictive. Outcomes contain counts and closed decisions, never policy data.

use crate::host_pattern::HostPattern;

/// Supply the merged operator grants and explicit project trust decision.
#[derive(Debug, Default)]
pub(crate) struct OperatorFetchPolicy {
    /// Whether unmatched hosts are denied.
    pub(crate) default_deny: bool,
    /// Operator-selected scheme grants.
    pub(crate) allow_scheme: Vec<String>,
    /// Operator-selected host grants.
    pub(crate) allow_host: Vec<HostPattern>,
    /// Whether project requests may widen the operator policy.
    pub(crate) trust_project_policy: bool,
}

/// Describe one project layer, supplied in increasing precedence order.
#[derive(Debug, Default)]
pub(crate) struct ProjectFetchPolicy {
    /// Optional default-deny request from this layer.
    pub(crate) default_deny: Option<bool>,
    /// Scheme grants requested by this layer.
    pub(crate) allow_scheme: Vec<String>,
    /// Host grants requested by this layer.
    pub(crate) allow_host: Vec<HostPattern>,
}

/// Explain the default-deny reconciliation without exposing configuration data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefaultDenyDecision {
    /// No project layer requested a default-deny change.
    OperatorRetained,
    /// A project restriction enabled default-deny.
    ProjectTightened,
    /// A project restriction agreed with an existing operator restriction.
    ProjectRestrictionRetained,
    /// An untrusted request to disable default-deny was ignored.
    ProjectDowngradeIgnored,
    /// The last explicit project request applied under operator opt-in.
    TrustedProjectOverride,
}

impl DefaultDenyDecision {
    /// Return a fixed diagnostic label independent of configuration values.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OperatorRetained => "operator_retained",
            Self::ProjectTightened => "project_tightened",
            Self::ProjectRestrictionRetained => "project_restriction_retained",
            Self::ProjectDowngradeIgnored => "project_downgrade_ignored",
            Self::TrustedProjectOverride => "trusted_project_override",
        }
    }
}

/// Report only bounded reconciliation state and aggregate grant counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FetchPolicyReconciliationOutcome {
    /// Whether the operator authorized project widening.
    pub trust_enabled: bool,
    /// Whether at least one project layer was supplied.
    pub project_request_present: bool,
    /// Closed explanation of the default-deny decision.
    pub default_deny_decision: DefaultDenyDecision,
    /// Number of scheme grants requested across project layers.
    pub requested_scheme_grant_count: usize,
    /// Number of project scheme grants accepted.
    pub accepted_scheme_grant_count: usize,
    /// Number of project scheme grants ignored.
    pub ignored_scheme_grant_count: usize,
    /// Number of host grants requested across project layers.
    pub requested_host_grant_count: usize,
    /// Number of project host grants accepted.
    pub accepted_host_grant_count: usize,
    /// Number of project host grants ignored.
    pub ignored_host_grant_count: usize,
}

/// Return effective grants separately from their bounded decision summary.
#[derive(Debug)]
pub(crate) struct ReconciledFetchPolicy {
    /// Effective default-deny restriction.
    pub(crate) default_deny: bool,
    /// Effective scheme grants.
    pub(crate) allow_scheme: Vec<String>,
    /// Effective host grants.
    pub(crate) allow_host: Vec<HostPattern>,
    /// Value-free outcome for observation at the application boundary.
    pub(crate) outcome: FetchPolicyReconciliationOutcome,
}

/// Reconcile all requests without I/O, configuration adapters, or telemetry.
///
/// Untrusted requests may only tighten default-deny; a later `false` cannot
/// erase an earlier restriction. Trusted requests append grants in layer order
/// and the last present default-deny request wins.
pub(crate) fn reconcile(
    mut operator: OperatorFetchPolicy,
    requests: impl IntoIterator<Item = ProjectFetchPolicy>,
) -> ReconciledFetchPolicy {
    let mut outcome = FetchPolicyReconciliationOutcome {
        trust_enabled: operator.trust_project_policy,
        project_request_present: false,
        default_deny_decision: DefaultDenyDecision::OperatorRetained,
        requested_scheme_grant_count: 0,
        accepted_scheme_grant_count: 0,
        ignored_scheme_grant_count: 0,
        requested_host_grant_count: 0,
        accepted_host_grant_count: 0,
        ignored_host_grant_count: 0,
    };
    let mut last_default_deny = None;
    let mut project_restricts = false;
    for request in requests {
        outcome.project_request_present = true;
        outcome.requested_scheme_grant_count += request.allow_scheme.len();
        outcome.requested_host_grant_count += request.allow_host.len();
        if let Some(default_deny) = request.default_deny {
            last_default_deny = Some(default_deny);
            project_restricts |= default_deny;
        }
        if operator.trust_project_policy {
            operator.allow_scheme.extend(request.allow_scheme);
            operator.allow_host.extend(request.allow_host);
        }
    }
    outcome.default_deny_decision =
        default_deny_decision(&operator, last_default_deny, project_restricts);
    if operator.trust_project_policy {
        operator.default_deny = last_default_deny.unwrap_or(operator.default_deny);
        outcome.accepted_scheme_grant_count = outcome.requested_scheme_grant_count;
        outcome.accepted_host_grant_count = outcome.requested_host_grant_count;
    } else {
        operator.default_deny |= project_restricts;
        outcome.ignored_scheme_grant_count = outcome.requested_scheme_grant_count;
        outcome.ignored_host_grant_count = outcome.requested_host_grant_count;
    }
    ReconciledFetchPolicy {
        default_deny: operator.default_deny,
        allow_scheme: operator.allow_scheme,
        allow_host: operator.allow_host,
        outcome,
    }
}

/// Classify the restriction decision before applying it to the operator policy.
const fn default_deny_decision(
    operator: &OperatorFetchPolicy,
    last_request: Option<bool>,
    project_restricts: bool,
) -> DefaultDenyDecision {
    match (
        last_request,
        operator.trust_project_policy,
        project_restricts,
    ) {
        (None, _, _) => DefaultDenyDecision::OperatorRetained,
        (Some(_), true, _) => DefaultDenyDecision::TrustedProjectOverride,
        (Some(_), false, true) if operator.default_deny => {
            DefaultDenyDecision::ProjectRestrictionRetained
        }
        (Some(_), false, true) => DefaultDenyDecision::ProjectTightened,
        (Some(_), false, false) => DefaultDenyDecision::ProjectDowngradeIgnored,
    }
}

#[cfg(test)]
#[path = "reconciliation_tests.rs"]
mod tests;

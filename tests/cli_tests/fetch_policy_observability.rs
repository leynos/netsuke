//! Verify bounded fetch-policy outcomes at the merge and tracing seams.

use super::merge_logging::{TestEnv, merge_and_observe};
use anyhow::{Context, Result, ensure};
use netsuke::cli::{MergeEvent, MergeObserver, TracingMergeObserver};
use netsuke::stdlib::{DefaultDenyDecision, FetchPolicyReconciliationOutcome};
use rstest::rstest;
use test_support::tracing_capture::with_test_subscriber;
use tracing_subscriber::filter::LevelFilter;

/// Merge a quarantined project request using optional operator trust.
fn reconcile_project(
    trusted: bool,
    project_default: bool,
    operator_default: bool,
) -> Result<(Vec<MergeEvent>, String)> {
    let project = tempfile::tempdir().context("create project for observer test")?;
    let config = format!(
        concat!(
            "fetch_default_deny = {}\n",
            "fetch_allow_scheme = [\"private-scheme\", \"secret-scheme\"]\n",
            "fetch_allow_host = [\"private.example\"]\n",
            "file = \"private-manifest.yml\"\n",
        ),
        project_default
    );
    test_support::fs::write(project.path().join(".netsuke.toml"), &config)
        .context("write project request for observer test")?;
    let directory = project.path().to_string_lossy().into_owned();
    let mut args = vec!["netsuke", "--directory", &directory];
    if trusted {
        args.push("--trust-project-fetch-policy");
    }
    if operator_default {
        args.push("--fetch-default-deny");
    }
    let (events, merge_ok) = merge_and_observe(&args, &TestEnv::default())?;
    ensure!(merge_ok, "valid fetch-policy request must merge");
    Ok((events, directory))
}

/// Extract the sole reconciliation outcome from a completed merge.
fn sole_outcome(events: &[MergeEvent]) -> Result<&FetchPolicyReconciliationOutcome> {
    let outcomes: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            MergeEvent::FetchPolicyReconciled { outcome } => Some(outcome),
            _ => None,
        })
        .collect();
    ensure!(outcomes.len() == 1, "expected exactly one outcome");
    outcomes.first().copied().context("missing outcome")
}

#[rstest]
#[case::untrusted_tightens(false, true, false, DefaultDenyDecision::ProjectTightened)]
#[case::untrusted_retains(false, true, true, DefaultDenyDecision::ProjectRestrictionRetained)]
#[case::untrusted_downgrade(false, false, true, DefaultDenyDecision::ProjectDowngradeIgnored)]
#[case::trusted_override(true, false, true, DefaultDenyDecision::TrustedProjectOverride)]
fn merge_returns_one_bounded_reconciliation_outcome(
    #[case] trusted: bool,
    #[case] project_default: bool,
    #[case] operator_default: bool,
    #[case] decision: DefaultDenyDecision,
) -> Result<()> {
    let (events, _) = reconcile_project(trusted, project_default, operator_default)?;
    let outcome = sole_outcome(&events)?;
    let expected = FetchPolicyReconciliationOutcome {
        trust_enabled: trusted,
        project_request_present: true,
        default_deny_decision: decision,
        requested_scheme_grant_count: 2,
        requested_host_grant_count: 1,
        accepted_scheme_grant_count: if trusted { 2 } else { 0 },
        ignored_scheme_grant_count: if trusted { 0 } else { 2 },
        accepted_host_grant_count: usize::from(trusted),
        ignored_host_grant_count: usize::from(!trusted),
    };
    ensure!(
        outcome == &expected,
        "unexpected reconciliation: {outcome:?}"
    );
    ensure!(
        matches!(
            events.last(),
            Some(MergeEvent::FetchPolicyReconciled { .. })
        ),
        "reconciliation must follow the generic merge events"
    );
    Ok(())
}

#[rstest]
#[case::untrusted(false, "project_tightened", (0, 2), (0, 1))]
#[case::trusted(true, "trusted_project_override", (2, 0), (1, 0))]
fn tracing_records_only_bounded_reconciliation_fields(
    #[case] trusted: bool,
    #[case] decision: &str,
    #[case] scheme_counts: (usize, usize),
    #[case] host_counts: (usize, usize),
) -> Result<()> {
    let (accepted_schemes, ignored_schemes) = scheme_counts;
    let (accepted_hosts, ignored_hosts) = host_counts;
    let (events, directory) = reconcile_project(trusted, true, false)?;
    let captured = with_test_subscriber(LevelFilter::DEBUG, |captured| {
        let mut observer = TracingMergeObserver;
        for event in events {
            observer.observe(event);
        }
        captured.snapshot()
    });
    let reconciliation: Vec<_> = captured
        .iter()
        .filter(|event| event.contains("reconciled fetch policy"))
        .collect();
    ensure!(
        reconciliation.len() == 1,
        "trace exactly one reconciliation"
    );
    let event = reconciliation
        .first()
        .context("missing reconciliation trace")?;
    let expected = format!(
        concat!(
            "message=reconciled fetch policy trust_enabled={} ",
            "project_request_present=true default_deny_decision={:?} ",
            "requested_scheme_grant_count=2 accepted_scheme_grant_count={} ",
            "ignored_scheme_grant_count={} requested_host_grant_count=1 ",
            "accepted_host_grant_count={} ignored_host_grant_count={}"
        ),
        trusted, decision, accepted_schemes, ignored_schemes, accepted_hosts, ignored_hosts
    );
    ensure!(
        event.as_str() == expected,
        "unexpected reconciliation trace: {event}"
    );
    for private_value in [
        "private-scheme",
        "secret-scheme",
        "private.example",
        "private-manifest.yml",
        &directory,
    ] {
        ensure!(
            captured.iter().all(|entry| !entry.contains(private_value)),
            "observer must not disclose private configuration"
        );
    }
    Ok(())
}

#[rstest]
fn failed_generic_merge_has_no_reconciliation_event() -> Result<()> {
    let project = tempfile::tempdir().context("create invalid merge directory")?;
    test_support::fs::write(project.path().join(".netsuke.toml"), "jobs = 0\n")
        .context("write invalid merge setting")?;
    let directory = project.path().to_string_lossy().into_owned();
    let (events, merge_ok) =
        merge_and_observe(&["netsuke", "--directory", &directory], &TestEnv::default())?;
    ensure!(!merge_ok, "invalid jobs must fail generic merging");
    ensure!(
        events
            .iter()
            .all(|event| !matches!(event, MergeEvent::FetchPolicyReconciled { .. })),
        "failed merging must not reconcile policy"
    );
    Ok(())
}

//! Focused resource-budget regression tests for manifest evaluation.

use super::super::{
    ManifestBudgetLimits, from_path_for_manifest_query_with_limits, from_str_with_limits,
};
use anyhow::{Context, Result, ensure};
use metrics_util::{
    MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
use tempfile::tempdir;
use test_support::fs as test_fs;

/// Return deliberately small limits while preserving room for fixture YAML.
fn small_limits() -> ManifestBudgetLimits {
    ManifestBudgetLimits {
        evaluation_fuel: 64,
        manifest_fuel: 256,
        rendered_value_bytes: 16,
        rendered_manifest_bytes: 128,
        source_bytes: 1_024,
        foreach_cardinality: 2,
        expanded_entries: 4,
    }
}

#[test]
fn rendered_value_at_limit_succeeds() -> Result<()> {
    let yaml = concat!(
        "netsuke_version: 1.0.0\n",
        "targets:\n",
        "  - name: exact\n",
        "    command: '{{ \"x\" * 16 }}'\n",
    );
    from_str_with_limits(yaml, small_limits())?;
    Ok(())
}

#[test]
fn rendered_value_one_byte_over_fails_without_output_growth() {
    let yaml = concat!(
        "netsuke_version: 1.0.0\n",
        "targets:\n",
        "  - name: over\n",
        "    command: '{{ \"x\" * 17 }}'\n",
    );
    let error = from_str_with_limits(yaml, small_limits()).expect_err("value exceeds budget");
    assert!(
        format!("{error:#}").contains("resource budget exhausted"),
        "unexpected error: {error:#}"
    );
    assert!(!format!("{error:#}").contains("xxxxxxxxxxxxxxxxx"));
}

#[test]
fn aggregate_rendered_bytes_allow_the_exact_limit_and_reject_one_more() -> Result<()> {
    let exact = concat!(
        "netsuke_version: 1.0.0\n",
        "targets:\n",
        "  - name: a\n",
        "    command: '{{ \"x\" * 16 }}'\n",
        "  - name: b\n",
        "    command: '{{ \"x\" * 16 }}'\n",
    );
    let limits = ManifestBudgetLimits {
        rendered_manifest_bytes: 34,
        ..small_limits()
    };
    from_str_with_limits(exact, limits)?;

    let one_more = concat!(
        "netsuke_version: 1.0.0\n",
        "targets:\n",
        "  - name: a\n",
        "    command: '{{ \"x\" * 16 }}'\n",
        "  - name: b\n",
        "    command: '{{ \"x\" * 16 }}'\n",
        "  - name: c\n",
        "    command: x\n",
    );
    let error = from_str_with_limits(one_more, limits)
        .expect_err("one byte above the aggregate budget must fail");
    ensure!(
        format!("{error:#}").contains("resource budget exhausted"),
        "unexpected error: {error:#}"
    );
    Ok(())
}

#[test]
fn foreach_stops_at_the_configured_cardinality() {
    let yaml = concat!(
        "netsuke_version: 1.0.0\n",
        "targets:\n",
        "  - name: '{{ item }}'\n",
        "    foreach: [one, two, three]\n",
        "    command: echo ok\n",
    );
    let error = from_str_with_limits(yaml, small_limits()).expect_err("foreach exceeds budget");
    assert!(
        format!("{error:#}").contains("resource budget exhausted"),
        "unexpected error: {error:#}"
    );
    assert!(!format!("{error:#}").contains("three"));
}

#[test]
fn foreach_at_the_configured_cardinality_succeeds() -> Result<()> {
    let yaml = concat!(
        "netsuke_version: 1.0.0\n",
        "targets:\n",
        "  - name: '{{ item }}'\n",
        "    foreach: [one, two]\n",
        "    command: echo ok\n",
    );
    from_str_with_limits(yaml, small_limits())?;
    Ok(())
}

#[test]
fn aggregate_expansion_allows_the_exact_limit_and_rejects_one_more() -> Result<()> {
    let exact = concat!(
        "netsuke_version: 1.0.0\n",
        "targets:\n",
        "  - name: '{{ item }}'\n",
        "    foreach: [one, two]\n",
        "    command: echo ok\n",
    );
    let limits = ManifestBudgetLimits {
        foreach_cardinality: 3,
        expanded_entries: 2,
        ..small_limits()
    };
    from_str_with_limits(exact, limits)?;

    let one_more = concat!(
        "netsuke_version: 1.0.0\n",
        "targets:\n",
        "  - name: '{{ item }}'\n",
        "    foreach: [one, two, three]\n",
        "    command: echo ok\n",
    );
    let error = from_str_with_limits(one_more, limits)
        .expect_err("third expansion must exhaust the aggregate limit");
    ensure!(
        format!("{error:#}").contains("resource budget exhausted"),
        "unexpected error: {error:#}"
    );
    Ok(())
}

#[test]
fn compact_loop_runs_out_of_fuel_before_allocating_requested_output() {
    let yaml = concat!(
        "netsuke_version: 1.0.0\n",
        "targets:\n",
        "  - name: loop\n",
        "    command: '{% for _ in range(50000) %}x{% endfor %}'\n",
    );
    let error = from_str_with_limits(yaml, small_limits()).expect_err("loop exceeds fuel");
    assert!(
        format!("{error:#}").contains("resource budget exhausted"),
        "unexpected error: {error:#}"
    );
    assert!(!format!("{error:#}").contains("50000"));
}

#[test]
fn macro_and_when_evaluations_share_the_fuel_budget() {
    let macro_yaml = concat!(
        "netsuke_version: 1.0.0\n",
        "macros:\n",
        "  - signature: repeat()\n",
        "    body: '{% for _ in range(50000) %}x{% endfor %}'\n",
        "targets:\n",
        "  - name: macro\n",
        "    command: '{{ repeat() }}'\n",
    );
    let macro_error = from_str_with_limits(macro_yaml, small_limits())
        .expect_err("macro loop must consume the shared fuel allowance");
    assert!(
        format!("{macro_error:#}").contains("resource budget exhausted"),
        "unexpected macro error: {macro_error:#}"
    );

    let when_yaml = concat!(
        "netsuke_version: 1.0.0\n",
        "targets:\n",
        "  - name: first-conditional\n",
        "    when: 'true'\n",
        "    command: echo ok\n",
        "  - name: second-conditional\n",
        "    when: 'true'\n",
        "    command: echo ok\n",
    );
    let when_limits = ManifestBudgetLimits {
        manifest_fuel: small_limits().evaluation_fuel,
        ..small_limits()
    };
    let when_error = from_str_with_limits(when_yaml, when_limits)
        .expect_err("when expressions must consume the shared fuel allowance");
    let rendered = format!("{when_error:#}");
    assert!(rendered.contains("resource budget exhausted"));
    assert!(!rendered.contains("second-conditional"));
}

#[test]
fn manifest_query_rendering_uses_the_same_budget() -> Result<()> {
    let workspace = tempdir().context("create query workspace")?;
    let path = workspace.path().join("Netsukefile");
    test_fs::write(
        &path,
        concat!(
            "netsuke_version: 1.0.0\n",
            "targets:\n",
            "  - name: metadata\n",
            "    description: '{{ \"q\" * 17 }}'\n",
            "    command: echo ignored\n",
        ),
    )?;

    let error = from_path_for_manifest_query_with_limits(&path, small_limits(), None)
        .expect_err("query rendering must enforce the value budget");
    ensure!(
        format!("{error:#}").contains("resource budget exhausted"),
        "unexpected error: {error:#}"
    );
    Ok(())
}

#[test]
fn budget_telemetry_uses_only_closed_labels_and_redacted_errors() {
    let yaml = concat!(
        "netsuke_version: 1.0.0\n",
        "targets:\n",
        "  - name: telemetry\n",
        "    command: '{{ \"s3cr3t\" * 3 }}'\n",
    );
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let error = metrics::with_local_recorder(&recorder, || {
        from_str_with_limits(yaml, small_limits()).expect_err("render must exceed value budget")
    });
    let snapshot = snapshotter.snapshot().into_vec();
    let counters = snapshot
        .iter()
        .filter_map(|(key, _, _, value)| {
            (key.kind() == MetricKind::Counter
                && key.key().name() == "netsuke_manifest_budget_exhausted_total")
                .then_some((key.key().labels().collect::<Vec<_>>(), value))
        })
        .collect::<Vec<_>>();
    assert_eq!(counters.len(), 1, "budget exhaustion should count once");
    let (labels, value) = counters
        .first()
        .expect("the asserted budget counter must supply labels and a value");
    assert!(
        labels
            .iter()
            .any(|label| label.key() == "stage" && label.value() == "render")
    );
    assert!(
        labels
            .iter()
            .any(|label| label.key() == "budget" && label.value() == "value_bytes")
    );
    assert!(matches!(value, DebugValue::Counter(1)));
    let rendered = format!("{error:#}");
    assert!(!rendered.contains("s3cr3t"));
    assert!(!labels.iter().any(|label| label.value().contains("s3cr3t")));
}

//! Verify fuel refunds, per-evaluation fuel reporting, source-byte boundaries,
//! and side-effect-free query budget failures.

use crate::manifest::{
    ManifestBudgetLimits, from_path_for_manifest_query_with_limits, from_str_with_limits,
};
use anyhow::{Context, Result, ensure};
use metrics_util::debugging::DebuggingRecorder;
use rstest::rstest;
use std::fmt::Write as _;
use tempfile::tempdir;
use test_support::fluent::normalize_fluent_isolates;

#[test]
fn cheap_when_conditions_refund_unused_evaluation_fuel() -> Result<()> {
    const COUNT: usize = 20;
    let mut yaml = String::from("netsuke_version: 1.0.0\ntargets:\n");
    for index in 0..COUNT {
        writeln!(
            yaml,
            "  - name: t{index}\n    when: 'true'\n    command: echo ok"
        )?;
    }
    let limits = ManifestBudgetLimits {
        evaluation_fuel: 1_000,
        manifest_fuel: 1_000,
        ..ManifestBudgetLimits::default()
    };
    let manifest = from_str_with_limits(&yaml, limits)?;
    ensure!(
        manifest.targets.len() == COUNT,
        "all cheap conditions must succeed"
    );
    Ok(())
}

#[rstest]
#[case::target_command("  - name: loop\n    command: '{% for _ in range(50000) %}x{% endfor %}'\n")]
#[case::when_template(
    "  - name: loop\n    when: '{% for _ in range(50000) %}x{% endfor %}'\n    command: echo ok\n"
)]
fn template_work_exceeding_evaluation_fuel_reports_the_per_evaluation_limit(#[case] entry: &str) {
    let yaml = format!("netsuke_version: 1.0.0\ntargets:\n{entry}");
    // The aggregate allowance is ample, so only the per-evaluation cap can
    // fail here; reporting `manifest_fuel` instead would expose an ignored
    // per-evaluation limit.
    let limits = ManifestBudgetLimits {
        evaluation_fuel: 64,
        manifest_fuel: 1_000_000,
        ..ManifestBudgetLimits::default()
    };

    let error = from_str_with_limits(&yaml, limits)
        .expect_err("template work must exhaust the per-evaluation fuel limit");
    let rendered = normalize_fluent_isolates(&format!("{error:#}"));
    assert!(
        rendered.contains("resource budget exhausted"),
        "unexpected error: {rendered}"
    );
    assert!(
        rendered.contains("after reaching 64"),
        "the reported limit must be evaluation_fuel, not manifest_fuel: {rendered}"
    );
}

#[test]
fn manifest_source_exactly_at_the_limit_loads_and_one_byte_more_fails() -> Result<()> {
    // The document is charged once, then every rendered field is charged again
    // as template source, so the exact total cannot be read off the file size.
    // Keep both field templates free of Jinja syntax so each stays charged
    // exactly once; a template containing braces would be charged twice.
    let yaml = concat!(
        "netsuke_version: 1.0.0\n",
        "targets:\n",
        "  - name: marker\n",
        "    command: echo ok\n",
    );
    let charged_source = yaml.len() + "marker".len() + "echo ok".len();

    let exact = from_str_with_limits(
        yaml,
        ManifestBudgetLimits {
            source_bytes: charged_source,
            ..ManifestBudgetLimits::default()
        },
    );
    exact.context("templates whose source exactly meets the limit must load")?;

    let over = from_str_with_limits(
        yaml,
        ManifestBudgetLimits {
            source_bytes: charged_source - 1,
            ..ManifestBudgetLimits::default()
        },
    );
    let error = over.expect_err("one byte above the charged source must fail");
    let rendered = normalize_fluent_isolates(&format!("{error:#}"));
    ensure!(
        rendered.contains("resource budget exhausted") && rendered.contains("during source"),
        "the failure must report the source stage: {rendered}"
    );
    Ok(())
}

#[test]
fn query_budget_failure_does_not_emit_budget_metrics() -> Result<()> {
    let workspace = tempdir().context("create query budget workspace")?;
    let path = workspace.path().join("Netsukefile");
    test_support::fs::write(
        &path,
        concat!(
            "netsuke_version: 1.0.0\ntargets:\n",
            "  - name: query\n    description: '{{ \"secret\" * 3 }}'\n    command: echo ok\n",
        ),
    )?;
    let limits = ManifestBudgetLimits {
        rendered_value_bytes: 16,
        ..ManifestBudgetLimits::default()
    };
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let error = metrics::with_local_recorder(&recorder, || {
        from_path_for_manifest_query_with_limits(&path, limits, None)
            .expect_err("query description must exhaust its budget")
    });
    ensure!(format!("{error:#}").contains("resource budget exhausted"));
    ensure!(
        snapshotter
            .snapshot()
            .into_vec()
            .iter()
            .all(|(key, _, _, _)| {
                key.key().name() != "netsuke_manifest_budget_exhausted_total"
            }),
        "query failure must not emit budget telemetry"
    );
    Ok(())
}

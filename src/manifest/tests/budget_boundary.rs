//! Verify fuel refunds and side-effect-free query budget failures.

use crate::manifest::{
    ManifestBudgetLimits, from_path_for_manifest_query_with_limits, from_str_with_limits,
};
use anyhow::{Context, Result, ensure};
use metrics_util::debugging::DebuggingRecorder;
use std::fmt::Write as _;
use tempfile::tempdir;

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

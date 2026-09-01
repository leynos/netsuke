//! Runner-boundary coverage for bounded legacy-recipe operation telemetry.
//!
//! Each case invokes [`netsuke::runner::run_with_ninja_program`] with an
//! injected executable and local metrics recorder. This verifies the complete
//! build or Ninja-tool orchestration boundary, not merely its helper.

use anyhow::{Context, Result, ensure};
use metrics_util::{
    CompositeKey, MetricKind,
    debugging::{DebugValue, DebuggingRecorder},
};
use netsuke::cli::{Cli, Commands};
use netsuke::output_prefs;
use netsuke::runner::{
    LEGACY_RECIPE_EXECUTION_DURATION, LEGACY_RECIPE_EXECUTIONS_TOTAL, run_with_ninja_program,
};
use std::path::{Path, PathBuf};
use test_support::check_ninja::{ToolName, fake_ninja_check_build_file, fake_ninja_expect_tool};
use test_support::fs as test_fs;

/// Represent one drained local metrics snapshot.
type Snapshot = Vec<(
    CompositeKey,
    Option<metrics::Unit>,
    Option<metrics::SharedString>,
    DebugValue,
)>;

/// Return the bounded label selected by the host's default recipe shell.
const fn host_recipe_shell_label() -> &'static str {
    if cfg!(windows) { "powershell" } else { "posix" }
}

/// Capture one actual runner operation under an isolated metrics recorder.
fn capture_operation(cli: &Cli, ninja_program: &Path) -> (Result<()>, Snapshot) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let result = metrics::with_local_recorder(&recorder, || {
        run_with_ninja_program(cli, output_prefs::resolve(None), ninja_program)
    });
    (result, snapshotter.snapshot().into_vec())
}

/// Construct a CLI that invokes one command from a manifest in `directory`.
fn runner_cli(directory: &Path, manifest: PathBuf, command: Option<Commands>) -> Cli {
    Cli {
        file: manifest,
        directory: Some(directory.to_path_buf()),
        command,
        ..Cli::default()
    }
}

/// Write one manifest into a fresh temporary project and return its owner and path.
fn temporary_manifest(content: &str) -> Result<(tempfile::TempDir, PathBuf)> {
    let directory = tempfile::tempdir().context("create temporary manifest directory")?;
    let manifest = directory.path().join("Netsukefile");
    test_fs::write(&manifest, content).context("write temporary Netsukefile")?;
    Ok((directory, manifest))
}

/// Assert one runner invocation emits exactly one bounded counter and histogram pair.
fn assert_operation(snapshot: &Snapshot, operation: &str, outcome: &str, failure_category: &str) {
    let labels = [
        ("operation", operation),
        ("recipe_shell", host_recipe_shell_label()),
        ("outcome", outcome),
        ("failure_category", failure_category),
    ];
    assert_metric(
        snapshot,
        MetricKind::Counter,
        LEGACY_RECIPE_EXECUTIONS_TOTAL,
        &labels,
    );
    assert_metric(
        snapshot,
        MetricKind::Histogram,
        LEGACY_RECIPE_EXECUTION_DURATION,
        &labels,
    );
}

/// Assert one metric has the exact bounded labels and one recorded observation.
fn assert_metric(snapshot: &Snapshot, kind: MetricKind, name: &str, labels: &[(&str, &str)]) {
    let matching = snapshot
        .iter()
        .filter(|(key, _, _, _)| key.kind() == kind && key.key().name() == name)
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one {kind:?} {name}: {snapshot:?}"
    );
    assert!(
        matching.first().is_some_and(|(key, _, _, debug_value)| {
            let actual = key
                .key()
                .labels()
                .map(|label| (label.key(), label.value()))
                .collect::<Vec<_>>();
            let has_observation = match debug_value {
                DebugValue::Counter(counter_value) => *counter_value == 1,
                DebugValue::Histogram(values) => values.len() == 1,
                DebugValue::Gauge(_) => false,
            };
            actual.len() == labels.len()
                && labels.iter().all(|label| actual.contains(label))
                && has_observation
        }),
        "expected one bounded {kind:?} {name}: {snapshot:?}"
    );
}

/// Verify the build dispatch path records one successful operation.
#[test]
fn build_dispatch_records_successful_legacy_recipe_operation() -> Result<()> {
    let (_ninja_directory, ninja_program) = fake_ninja_check_build_file()?;
    let (directory, manifest) = temporary_manifest(include_str!("data/minimal.yml"))?;
    let cli = runner_cli(directory.path(), manifest, None);

    let (result, snapshot) = capture_operation(&cli, &ninja_program);
    result.context("the injected Ninja build should succeed")?;
    assert_operation(&snapshot, "build", "success", "none");
    Ok(())
}

/// Verify the Ninja-tool dispatch path records one successful operation.
#[test]
fn ninja_tool_dispatch_records_successful_legacy_recipe_operation() -> Result<()> {
    let (_ninja_directory, ninja_program) = fake_ninja_expect_tool(ToolName::new("clean"))?;
    let (directory, manifest) = temporary_manifest(include_str!("data/minimal.yml"))?;
    let cli = runner_cli(directory.path(), manifest, Some(Commands::Clean));

    let (result, snapshot) = capture_operation(&cli, &ninja_program);
    result.context("the injected Ninja tool should succeed")?;
    assert_operation(&snapshot, "ninja_tool", "success", "none");
    Ok(())
}

/// Verify build failures retain their fixed stage category through the runner boundary.
#[test]
fn build_dispatch_records_bounded_legacy_recipe_failure_categories() -> Result<()> {
    let temporary_directory = tempfile::tempdir().context("create missing-manifest directory")?;
    let missing_manifest = temporary_directory.path().join("Netsukefile");
    let missing_cli = runner_cli(temporary_directory.path(), missing_manifest, None);
    let (missing_result, missing_snapshot) = capture_operation(&missing_cli, Path::new("ninja"));
    ensure!(missing_result.is_err(), "the missing manifest should fail");
    assert_operation(&missing_snapshot, "build", "error", "manifest");

    let (graph_directory, graph_manifest) = temporary_manifest(include_str!("data/circular.yml"))?;
    let graph_cli = runner_cli(graph_directory.path(), graph_manifest, None);
    let (graph_result, graph_snapshot) = capture_operation(&graph_cli, Path::new("ninja"));
    ensure!(graph_result.is_err(), "the circular graph should fail");
    assert_operation(&graph_snapshot, "build", "error", "graph");

    let unsafe_manifest = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: out\n",
        "    sources: input|file\n",
        "    command: \"echo hi\"\n",
    );
    let (generation_directory, generation_manifest) = temporary_manifest(unsafe_manifest)?;
    let generation_cli = runner_cli(generation_directory.path(), generation_manifest, None);
    let (generation_result, generation_snapshot) =
        capture_operation(&generation_cli, Path::new("ninja"));
    ensure!(
        generation_result.is_err(),
        "the unsafe Ninja path should fail"
    );
    assert_operation(&generation_snapshot, "build", "error", "ninja_generation");

    let (io_directory, io_manifest) = temporary_manifest(include_str!("data/minimal.yml"))?;
    let missing_ninja = io_directory.path().join("missing-ninja");
    let io_cli = runner_cli(io_directory.path(), io_manifest, None);
    let (io_result, io_snapshot) = capture_operation(&io_cli, &missing_ninja);
    ensure!(
        io_result.is_err(),
        "the missing Ninja executable should fail"
    );
    assert_operation(&io_snapshot, "build", "error", "ninja_io");
    Ok(())
}

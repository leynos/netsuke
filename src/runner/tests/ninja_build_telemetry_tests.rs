//! Check generation and telemetry boundaries around runner-owned Ninja builds.
//!
//! Both tests sit on the same seam: what the runner hands to Ninja, and what it
//! records about asking. Keeping them together leaves `tests/mod.rs` under the
//! workspace file-length limit as the runner's own tests grow.

use super::*;
use crate::ir::{BuildEdge, BuildGraph, DependencyOrder};
use crate::ninja_gen::NinjaGenError;
use crate::recipe_shell::RecipeShell;
use crate::runner::{NINJA_BUILD_EXECUTIONS_TOTAL, run_with_ninja_program};
use anyhow::{Result, ensure};
use camino::Utf8PathBuf;
use metrics_util::debugging::DebuggingRecorder;
use rstest::rstest;
use test_support::{localizer_test_lock, set_en_localizer};

#[rstest]
#[case::progress_enabled(crate::cli::ProgressPolicy::Auto)]
#[case::progress_disabled(crate::cli::ProgressPolicy::Never)]
fn invalid_job_options_do_not_record_a_ninja_build(
    #[case] progress: crate::cli::ProgressPolicy,
) -> Result<()> {
    let _lock = localizer_test_lock().map_err(|error| anyhow::anyhow!("{error}"))?;
    let _guard = set_en_localizer();
    let (_temp, manifest_path) = write_manifest(MINIMAL_MANIFEST)?;
    let cli = Cli {
        file: manifest_path,
        jobs: Some(0),
        progress,
        ..Cli::default()
    };
    let plan = crate::display_plan::resolve_display_plan(
        cli.display_policies(),
        crate::display_plan::TerminalFacts::default(),
    );
    let program = camino::Utf8Path::new("ninja");
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let (result, metrics) = metrics::with_local_recorder(&recorder, || {
        let result = run_with_ninja_program(&cli, plan, program);
        (result, snapshotter.snapshot().into_vec())
    });

    ensure!(
        result.is_err(),
        "an invalid job count must fail before Ninja runs"
    );
    ensure!(
        !metrics
            .iter()
            .any(|(key, _, _, _)| key.key().name() == NINJA_BUILD_EXECUTIONS_TOTAL),
        "option validation must not record a Ninja build: {metrics:?}"
    );
    Ok(())
}

#[test]
fn ninja_text_propagates_typed_generation_errors() {
    let mut graph = BuildGraph::default();
    graph
        .insert_edge(BuildEdge {
            action_id: "missing".into(),
            inputs: Vec::new(),
            implicit_deps: Vec::new(),
            dependency_order: DependencyOrder::Parallel,
            explicit_outputs: vec![Utf8PathBuf::from("hello")],
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            progress_label: None,
            phony: false,
            always: false,
            output_visibility: crate::ir::EdgeOutputVisibility::default(),
        })
        .expect("test graph output aliases must be unique");

    let error =
        crate::runner::generation::ninja_text_for_shell(&graph, RecipeShell::host_default())
            .expect_err("missing action should fail generation");
    assert!(matches!(
        error,
        NinjaGenError::MissingAction { ref id, .. } if id == "missing"
    ));
}

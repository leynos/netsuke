//! Unit tests for runner path resolution, predicate helpers, and core helpers.

use super::*;
use crate::cli::{HelpArgs, HelpTopic};
use crate::ir::{BuildEdge, BuildGraph, DependencyOrder};
use crate::manifest::ManifestLoadStage;
use crate::ninja_gen::NinjaGenError;
use crate::status::{LocalizationKey, StageNumber, StatusReporter};
use anyhow::{Result, ensure};
use camino::Utf8PathBuf;
use rstest::rstest;
use std::cell::Cell;
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use test_support::{localizer_test_lock, set_en_localizer};

const MINIMAL_MANIFEST: &str = concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: hello\n",
    "    command: echo hi\n",
);

/// Write a manifest and return a UTF-8 path suitable for runner generation.
fn write_manifest(manifest: &str) -> Result<(tempfile::TempDir, Utf8PathBuf)> {
    let temp = tempfile::tempdir()?;
    let manifest_path = temp.path().join("Netsukefile");
    test_support::fs::write(&manifest_path, manifest)?;
    let utf8_path = Utf8PathBuf::from_path_buf(manifest_path)
        .map_err(|path| anyhow::anyhow!("non-UTF-8 temp path: {}", path.display()))?;
    Ok((temp, utf8_path))
}

/// Record runner pipeline stages without coupling the test to rendered text.
#[derive(Default)]
struct StageRecordingReporter {
    stages: Mutex<Vec<u32>>,
}

impl StageRecordingReporter {
    fn stages(&self) -> Vec<u32> {
        self.stages
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl StatusReporter for StageRecordingReporter {
    fn report_stage(&self, current: StageNumber, _total: StageNumber, _description: &str) {
        self.stages
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(current.get());
    }

    fn report_complete(&self, _tool_key: LocalizationKey) {}
}

#[rstest]
#[case(None, "out.ninja", "out.ninja")]
#[case(Some("work"), "out.ninja", "work/out.ninja")]
#[case(Some("work"), "/tmp/out.ninja", "/tmp/out.ninja")]
fn resolve_output_path_respects_directory(
    #[case] directory: Option<&str>,
    #[case] input: &str,
    #[case] expected: &str,
) {
    let cli = Cli {
        directory: directory.map(PathBuf::from),
        ..Cli::default()
    };
    let resolved = resolve_output_path(&cli, Path::new(input));
    assert_eq!(resolved.as_ref(), Path::new(expected));
}

#[rstest]
fn generation_steps_run_without_reporter() -> anyhow::Result<()> {
    let (_temp, manifest_path) = write_manifest(MINIMAL_MANIFEST)?;
    let mut stages = Vec::new();

    // The pure pipeline composes without a runner status reporter.
    let manifest =
        generation::load_manifest(&manifest_path, Some(&mut |stage| stages.push(stage)))?;
    let graph = generation::build_graph(&manifest)?;
    let (ninja_text, _) = generation::ninja_text(&graph)?.into_parts();
    ensure!(
        stages
            == vec![
                ManifestLoadStage::ManifestIngestion,
                ManifestLoadStage::InitialYamlParsing,
                ManifestLoadStage::TemplateExpansion,
                ManifestLoadStage::FinalRendering,
            ],
        "unexpected query-loader stage sequence: {stages:?}"
    );
    anyhow::ensure!(
        ninja_text.contains("build hello:"),
        "expected generated Ninja to contain the hello build edge:\n{}",
        ninja_text
    );
    Ok(())
}

#[rstest]
#[case::fetch("{{ fetch('https://example.invalid', cache=true) }}", "fetch")]
#[case::shell("{{ 'ignored' | shell('printf side-effect') }}", "shell")]
fn query_loader_rejects_effectful_template_helpers(
    #[case] expression: &str,
    #[case] helper: &str,
) -> Result<()> {
    let manifest = format!(
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  - name: hello\n",
            "    description: >-\n",
            "      {}\n",
            "    command: echo hi\n",
        ),
        expression
    );
    let (temp, manifest_path) = write_manifest(&manifest)?;

    let error = generation::load_manifest(&manifest_path, None)
        .expect_err("query loader should reject effectful template helpers");
    ensure!(
        error
            .chain()
            .any(|cause| cause.to_string().contains(helper)),
        "query loader should name the rejected helper: {error:?}"
    );
    ensure!(
        !temp.path().join(".netsuke").exists(),
        "query loader must not create an effectful template cache"
    );
    Ok(())
}

#[test]
fn query_loader_preserves_load_error_context() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let manifest_path = Utf8PathBuf::from_path_buf(temp.path().join("Netsukefile"))
        .map_err(|path| anyhow::anyhow!("non-UTF-8 temp path: {}", path.display()))?;

    let error = generation::load_manifest(&manifest_path, None)
        .expect_err("missing manifest should fail to load");
    ensure!(
        error.to_string().contains(manifest_path.as_str()),
        "load context should name the manifest path: {error:?}"
    );
    Ok(())
}

#[test]
fn build_graph_preserves_graph_error_context() -> Result<()> {
    let _lock = localizer_test_lock().map_err(|error| anyhow::anyhow!("{error}"))?;
    let _guard = set_en_localizer();
    let (_temp, manifest_path) = write_manifest(include_str!("../../tests/data/circular.yml"))?;
    let manifest = generation::load_manifest(&manifest_path, None)?;

    let error = generation::build_graph(&manifest).expect_err("cycle should fail graph building");
    let expected = localization::message(keys::RUNNER_CONTEXT_BUILD_GRAPH).to_string();
    ensure!(
        error.to_string().contains(&expected),
        "graph context should be retained: {error:?}"
    );
    Ok(())
}

#[test]
fn ninja_text_propagates_typed_generation_errors() {
    let mut graph = BuildGraph::default();
    graph.targets.insert(
        Utf8PathBuf::from("hello"),
        BuildEdge {
            action_id: "missing".into(),
            inputs: Vec::new(),
            implicit_deps: Vec::new(),
            dependency_order: DependencyOrder::Parallel,
            explicit_outputs: vec![Utf8PathBuf::from("hello")],
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            phony: false,
            always: false,
        },
    );

    let error = generation::ninja_text(&graph).expect_err("missing action should fail generation");
    assert!(matches!(
        error,
        NinjaGenError::MissingAction { ref id, .. } if id == "missing"
    ));
}

#[test]
fn runner_reports_the_complete_generation_stage_sequence() -> Result<()> {
    let (temp, manifest_path) = write_manifest(MINIMAL_MANIFEST)?;
    let cli = Cli {
        file: manifest_path.into_std_path_buf(),
        directory: Some(temp.path().to_path_buf()),
        command: Some(Commands::Generate { output: None }),
        ..Cli::default()
    };
    let reporter = StageRecordingReporter::default();

    let generated = generate_ninja(&cli, &reporter, None)?;
    let (ninja_text, _) = generated.into_parts();

    let stages = reporter.stages();
    ensure!(
        stages == (1..=6).collect::<Vec<_>>(),
        "unexpected runner stage sequence: {stages:?}"
    );
    ensure!(
        ninja_text.contains("build hello:"),
        "runner generation should produce the hello build edge: {ninja_text}"
    );
    Ok(())
}

#[test]
fn help_targets_bypasses_ninja_program_resolution() -> Result<()> {
    let _lock = localizer_test_lock().map_err(|error| anyhow::anyhow!("{error}"))?;
    let _guard = set_en_localizer();
    let cli = Cli {
        file: PathBuf::from("missing-help-targets-manifest.yml"),
        command: Some(Commands::Help(HelpArgs {
            topic: Some(HelpTopic::Targets),
        })),
        ..Cli::default()
    };
    let resolver_called = Cell::new(false);

    let result =
        run_with_ninja_program_resolver(&cli, crate::output_prefs::resolve(None), None, || {
            resolver_called.set(true);
            PathBuf::from("ninja")
        });

    ensure!(result.is_err(), "missing help manifest should fail");
    ensure!(
        !resolver_called.get(),
        "help targets must not resolve the Ninja program"
    );
    Ok(())
}

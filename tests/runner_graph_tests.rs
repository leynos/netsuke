//! Integration tests for the in-process `graph` subcommand.
//!
//! The `graph` subcommand no longer shells out to `ninja -t graph`; it builds
//! the [`BuildGraph`] in-process and renders DOT directly. These tests verify
//! the dispatch works without Ninja installed and writes the artefact through
//! the shared file/stdout sinks.

use anyhow::{Context, Result, bail, ensure};
use netsuke::cli::{Cli, Commands, GraphArgs};
use netsuke::output_prefs;
use netsuke::runner::run;
use rstest::rstest;
use serde_json::Value;
use test_support::{localizer_test_lock, set_en_localizer};

mod fixtures;
use fixtures::create_test_manifest;

fn run_graph(cli: &Cli) -> Result<()> {
    let _lock = localizer_test_lock()
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("localizer test lock poisoned")?;
    let _guard = set_en_localizer();
    run(cli, output_prefs::resolve(None)).context("running graph subcommand")
}

#[rstest]
fn graph_with_output_writes_dot_file() -> Result<()> {
    let (temp, manifest_path) = create_test_manifest()?;
    let dot_path = temp.path().join("graph.dot");
    let cli = Cli {
        file: manifest_path,
        directory: Some(temp.path().to_path_buf()),
        command: Some(Commands::Graph(GraphArgs {
            html: false,
            output: Some("graph.dot".into()),
        })),
        ..Cli::default()
    };
    run_graph(&cli)?;
    let written = std::fs::read_to_string(&dot_path)
        .with_context(|| format!("read graph.dot at {}", dot_path.display()))?;
    ensure!(
        written.starts_with("digraph netsuke {"),
        "DOT artefact should start with digraph header; got: {written:.80}"
    );
    ensure!(
        written.contains("\"hello\""),
        "DOT artefact should mention the `hello` target; got: {written}"
    );
    Ok(())
}

#[rstest]
fn json_graph_with_output_writes_file_and_result_document() -> Result<()> {
    let (temp, manifest_path) = create_test_manifest()?;
    let dot_path = temp.path().join("graph-json.dot");
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .arg("--json")
        .arg("--file")
        .arg(&manifest_path)
        .args(["graph", "--output", "graph-json.dot"])
        .output()
        .context("run netsuke --json graph --output graph-json.dot")?;

    ensure!(
        output.status.success(),
        "JSON graph file output should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        output.stderr.is_empty(),
        "JSON graph file output should keep stderr empty"
    );
    let dot = std::fs::read_to_string(&dot_path)
        .with_context(|| format!("read graph output at {}", dot_path.display()))?;
    ensure!(
        dot.contains("digraph netsuke"),
        "graph file should contain DOT output"
    );

    let stdout = String::from_utf8(output.stdout).context("stdout should be valid UTF-8")?;
    let result: Value =
        serde_json::from_str(&stdout).context("stdout should be one JSON result document")?;
    ensure!(
        result.pointer("/result/command").and_then(Value::as_str) == Some("graph"),
        "JSON result should identify the graph command: {result}"
    );
    ensure!(
        result
            .pointer("/result/content")
            .is_some_and(Value::is_null),
        "JSON result should omit inline graph content: {result}"
    );
    ensure!(
        !stdout.contains("digraph netsuke"),
        "JSON result should not duplicate the DOT graph"
    );
    Ok(())
}

#[rstest]
fn graph_with_invalid_manifest_reports_error() -> Result<()> {
    let temp = tempfile::tempdir().context("temp dir")?;
    let manifest_path = temp.path().join("Netsukefile");
    std::fs::copy("tests/data/invalid_version.yml", &manifest_path)
        .with_context(|| format!("copy invalid manifest to {}", manifest_path.display()))?;
    let cli = Cli {
        file: manifest_path,
        command: Some(Commands::Graph(GraphArgs::default())),
        ..Cli::default()
    };
    let Err(_) = run_graph(&cli) else {
        bail!("expected graph to fail with invalid manifest");
    };
    Ok(())
}

#[rstest]
fn graph_html_writes_self_contained_document() -> Result<()> {
    let (temp, manifest_path) = create_test_manifest()?;
    let html_path = temp.path().join("graph.html");
    let cli = Cli {
        file: manifest_path,
        directory: Some(temp.path().to_path_buf()),
        command: Some(Commands::Graph(GraphArgs {
            html: true,
            output: Some("graph.html".into()),
        })),
        ..Cli::default()
    };
    run_graph(&cli)?;
    let html = std::fs::read_to_string(&html_path).context("read graph.html")?;
    ensure!(html.starts_with("<!doctype html>"), "should be HTML doc");
    ensure!(html.contains("<svg"), "should contain SVG");
    ensure!(
        html.contains("<noscript>"),
        "should contain noscript fallback"
    );
    ensure!(
        !html.contains("href=\"http") && !html.contains("src=\"http"),
        "no external references"
    );
    Ok(())
}

#[rstest]
fn graph_with_output_dash_writes_to_stdout_and_not_file() -> Result<()> {
    // Subprocess-driven test: the `-` sentinel must route DOT to stdout and
    // not create any artefact on disk. Existing project precedent is
    // `manifest_subcommand_streams_to_stdout_when_dash` in assert_cmd_tests,
    // which uses `assert_cmd::Command` for stdout capture.
    let (temp, manifest_path) = create_test_manifest()?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .arg("--file")
        .arg(&manifest_path)
        .arg("graph")
        .arg("--output")
        .arg("-")
        .env("PATH", "")
        .output()
        .context("run netsuke graph --output -")?;
    ensure!(
        output.status.success(),
        "graph --output - should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        stdout.starts_with("digraph netsuke {"),
        "stdout should carry DOT header; got: {stdout:.80}"
    );
    ensure!(
        !temp.path().join("-").exists(),
        "no file named '-' should be created"
    );
    Ok(())
}

#[rstest]
fn graph_is_deterministic_across_repeated_runs() -> Result<()> {
    let (temp, manifest_path) = create_test_manifest()?;
    let mut outputs: Vec<String> = Vec::new();
    for tag in ["first", "second"] {
        let dot_path = temp.path().join(format!("graph-{tag}.dot"));
        let cli = Cli {
            file: manifest_path.clone(),
            directory: Some(temp.path().to_path_buf()),
            command: Some(Commands::Graph(GraphArgs {
                html: false,
                output: Some(format!("graph-{tag}.dot").into()),
            })),
            ..Cli::default()
        };
        run_graph(&cli)?;
        outputs.push(std::fs::read_to_string(&dot_path).context("read DOT artefact")?);
    }
    let mut iter = outputs.iter();
    let first = iter.next().context("first DOT output")?;
    let second = iter.next().context("second DOT output")?;
    ensure!(
        first == second,
        "repeated DOT renders should be byte-identical"
    );
    Ok(())
}

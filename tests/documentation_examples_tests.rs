//! Executable contracts for examples in the public user documentation.

mod documentation_examples;

use anyhow::{Context, Result, ensure};
use documentation_examples::{
    assert_success, documented_example, load_documented_examples, manifest_workspace,
};
use rstest::rstest;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;
use test_support::check_ninja;
use test_support::fluent::normalize_fluent_isolates;
use test_support::fs as test_fs;
use test_support::netsuke::{NetsukeRun, run_netsuke_in, run_netsuke_in_with_env};

const EXPECTED_EXAMPLE_IDS: &[&str] = &[
    "guide-accessible-output",
    "guide-binstall-install",
    "guide-cli-usage",
    "guide-command-available-manifest",
    "guide-command-list",
    "guide-complete-manifest",
    "guide-crates-io-install",
    "guide-direct-command-list",
    "guide-env-reader-snippet",
    "guide-first-build-commands",
    "guide-first-build-manifest",
    "guide-foreach-manifest",
    "guide-help-targets",
    "guide-json-command",
    "guide-json-output",
    "guide-macro-manifest",
    "guide-ninja-request-snippet",
    "guide-output-streams",
    "guide-project-anchor",
    "guide-project-config",
    "guide-source-install",
    "guide-utility-commands",
    "guide-windows-help",
    "guide-windows-help-install",
    "guide-windows-path",
    "readme-binstall-install",
    "readme-crates-io-install",
    "readme-first-build-commands",
    "readme-first-build-manifest",
    "readme-source-install",
    "stdlib-fetch-expression",
    "stdlib-file-tests-manifest",
    "stdlib-host-context-manifest",
    "stdlib-jinja-syntax-manifest",
    "stdlib-path-and-collection-manifest",
    "stdlib-time-manifest",
    "stdlib-yaml-syntax-manifest",
];

/// The guide's env-reader snippet must stay in step with the API it mirrors.
///
/// The snippet is Rust and is executed as the doctest on `from_str_with_env`;
/// this pins the guide copy to the same entry points so the two cannot drift
/// silently.
#[test]
fn env_reader_snippet_mirrors_the_doctest() -> Result<()> {
    let example = documented_example("guide-env-reader-snippet")?;
    ensure!(
        example.language == "rust",
        "the env-reader snippet should be a Rust fence"
    );
    for needle in ["from_str_with_env", "EnvReader", "env('PROFILE')"] {
        ensure!(
            example.body.contains(needle),
            "the env-reader snippet should mention {needle}"
        );
    }
    Ok(())
}
/// The guide's Ninja-request snippet must name the API it documents.
///
/// The snippet is the only place the guide constructs the request bundles, so
/// pinning the identifiers keeps it from drifting into prose about types the
/// crate no longer exports.
#[test]
fn ninja_request_snippet_names_both_request_types() -> Result<()> {
    let example = documented_example("guide-ninja-request-snippet")?;
    ensure!(
        example.language == "rust",
        "the Ninja-request snippet should be a Rust fence"
    );
    for needle in [
        "NinjaBuildRequest",
        "NinjaToolRequest",
        "run_ninja_with",
        "run_ninja_tool_with",
        "CommandEnv::inherit",
    ] {
        ensure!(
            example.body.contains(needle),
            "the Ninja-request snippet should mention {needle}"
        );
    }
    Ok(())
}

fn assert_default_edges_exist(ninja: &str, context: &str) -> Result<()> {
    for default in ninja
        .lines()
        .filter_map(|line| line.strip_prefix("default "))
        .flat_map(str::split_whitespace)
    {
        let edge = format!("build {default}:");
        ensure!(
            ninja.lines().any(|line| line.starts_with(&edge)),
            "{context} default '{default}' should have a generated build edge"
        );
    }
    Ok(())
}

fn assert_generates_valid_ninja(run: &NetsukeRun, context: &str) -> Result<()> {
    assert_success(run, context)?;
    ensure!(
        run.stdout.contains("rule ") && run.stdout.contains("build "),
        "{context} should generate a Ninja manifest"
    );
    assert_default_edges_exist(&run.stdout, context)
}

fn run_with_fake_ninja(workspace: &Path, args: &[&str]) -> Result<NetsukeRun> {
    let (_ninja_dir, ninja_path) = check_ninja::fake_ninja_check_build_file()?;
    let ninja = ninja_path
        .to_str()
        .context("fake Ninja path should be valid UTF-8")?;
    run_netsuke_in_with_env(workspace, args, &[(netsuke::runner::NINJA_ENV, ninja)])
}

#[test]
fn every_documented_fence_has_a_known_unique_identifier() -> Result<()> {
    let examples = load_documented_examples()?;
    let actual = examples
        .iter()
        .map(|example| example.id.as_str())
        .collect::<BTreeSet<_>>();
    let expected = EXPECTED_EXAMPLE_IDS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    ensure!(
        actual == expected,
        "documented example registry drifted\nexpected: {expected:#?}\nactual: {actual:#?}"
    );
    Ok(())
}

#[rstest]
#[case("readme-first-build-manifest")]
#[case("guide-first-build-manifest")]
#[case("guide-complete-manifest")]
#[case("guide-foreach-manifest")]
#[case("guide-macro-manifest")]
#[case("guide-command-list")]
#[case("guide-direct-command-list")]
#[case("guide-command-available-manifest")]
#[case("stdlib-yaml-syntax-manifest")]
#[case("stdlib-jinja-syntax-manifest")]
fn documented_manifest_generates_ninja(#[case] example_id: &str) -> Result<()> {
    let workspace = manifest_workspace(example_id)?;
    let run = run_netsuke_in(workspace.path(), &["--progress", "never", "generate"])?;

    assert_generates_valid_ninja(&run, example_id)
}

#[test]
fn documented_fetch_expression_is_registered_but_not_executed() -> Result<()> {
    let example = documented_example("stdlib-fetch-expression")?;
    ensure!(
        example.language == "jinja",
        "fetch example should remain an expression, not an executable manifest"
    );
    ensure!(
        example.body == "{{ fetch('https://example.com/toolchain.json', cache=true) }}\n",
        "fetch expression drifted"
    );
    Ok(())
}

#[rstest]
#[case("readme-first-build-manifest", "readme-first-build-commands")]
#[case("guide-first-build-manifest", "guide-first-build-commands")]
fn documented_first_run_flow_builds(
    #[case] manifest_id: &str,
    #[case] commands_id: &str,
) -> Result<()> {
    let commands = documented_example(commands_id)?;
    ensure!(
        commands.body == "netsuke\ncat hello.txt\n",
        "{commands_id} should describe the tested first-run flow"
    );
    let workspace = manifest_workspace(manifest_id)?;
    let run = run_with_fake_ninja(workspace.path(), &[])?;

    assert_success(&run, commands_id)?;
    ensure!(
        normalize_fluent_isolates(&run.stderr).contains("Build complete."),
        "{commands_id} should finish the documented build"
    );
    Ok(())
}

#[test]
fn documented_cli_shape_matches_live_help() -> Result<()> {
    let example = documented_example("guide-cli-usage")?;
    ensure!(
        example.body == "netsuke [OPTIONS] [COMMAND]\nnetsuke [OPTIONS] build [TARGETS]...\n",
        "CLI synopsis example drifted"
    );
    let run = run_netsuke_in(Path::new("."), &["--locale", "en-US", "--help"])?;
    assert_success(&run, "top-level help")?;
    ensure!(
        run.stdout.contains("Usage: netsuke [OPTIONS] [COMMAND]"),
        "top-level help should expose the documented command shape"
    );
    let build = run_netsuke_in(Path::new("."), &["--locale", "en-US", "build", "--help"])?;
    assert_success(&build, "build help")?;
    ensure!(
        build.stdout.contains("Usage: netsuke build [TARGETS]..."),
        "build help should expose the documented target shape"
    );
    Ok(())
}

#[test]
fn directory_and_utility_command_examples_run() -> Result<()> {
    let anchor = documented_example("guide-project-anchor")?;
    ensure!(
        anchor.body == "netsuke --directory /path/to/project build\n",
        "directory example drifted"
    );
    let utility = documented_example("guide-utility-commands")?;
    ensure!(
        utility.body
            == concat!(
                "netsuke clean\n",
                "netsuke graph --output build.dot\n",
                "netsuke graph --html --output graph.html\n",
                "netsuke generate\n",
                "netsuke generate --output build.ninja\n"
            ),
        "utility command example drifted"
    );

    let workspace = manifest_workspace("guide-first-build-manifest")?;
    let directory = workspace
        .path()
        .to_str()
        .context("temporary workspace path should be UTF-8")?;
    let anchored = run_with_fake_ninja(Path::new("."), &["--directory", directory, "build"])?;
    assert_success(&anchored, "directory build")?;

    assert_success(
        &run_with_fake_ninja(workspace.path(), &["clean"])?,
        "clean example",
    )?;
    assert_success(
        &run_netsuke_in(workspace.path(), &["graph", "--output", "build.dot"])?,
        "DOT graph example",
    )?;
    assert_success(
        &run_netsuke_in(
            workspace.path(),
            &["graph", "--html", "--output", "graph.html"],
        )?,
        "HTML graph example",
    )?;
    assert_success(
        &run_netsuke_in(workspace.path(), &["generate"])?,
        "generate stdout example",
    )?;
    assert_success(
        &run_netsuke_in(workspace.path(), &["generate", "--output", "build.ninja"])?,
        "generate file example",
    )?;
    for output in ["build.dot", "graph.html", "build.ninja"] {
        ensure!(
            workspace.path().join(output).is_file(),
            "{output} should be created"
        );
    }
    Ok(())
}

#[test]
fn help_targets_example_lists_described_targets() -> Result<()> {
    let example = documented_example("guide-help-targets")?;
    ensure!(
        example.body == "netsuke help targets\n",
        "help targets example drifted"
    );
    let workspace = manifest_workspace("guide-first-build-manifest")?;
    let run = run_netsuke_in(workspace.path(), &["help", "targets"])?;
    assert_success(&run, "help targets example")?;
    ensure!(
        normalize_fluent_isolates(&run.stdout).contains("Targets:"),
        "help targets should print the Targets section"
    );
    Ok(())
}
#[test]
fn project_configuration_example_is_accepted() -> Result<()> {
    let example = documented_example("guide-project-config")?;
    let workspace = manifest_workspace("guide-first-build-manifest")?;
    let config_path = workspace.path().join("example.toml");
    test_fs::write(&config_path, example.body).context("write documented config")?;
    let config = config_path
        .to_str()
        .context("temporary config path should be UTF-8")?;
    let run = run_netsuke_in(
        workspace.path(),
        &["--config", config, "--progress", "never", "generate"],
    )?;
    assert_success(&run, "project configuration example")
}

#[test]
fn output_stream_and_accessibility_examples_match_live_output() -> Result<()> {
    let streams = documented_example("guide-output-streams")?;
    ensure!(
        streams.body
            == concat!(
                "netsuke graph > build.dot\n",
                "netsuke --progress never build\n",
                "netsuke generate > build.ninja\n"
            ),
        "output stream example drifted"
    );
    let workspace = manifest_workspace("guide-first-build-manifest")?;
    let graph = run_netsuke_in(workspace.path(), &["graph"])?;
    assert_success(&graph, "graph stdout example")?;
    ensure!(graph.stdout.contains("digraph"), "graph should use stdout");
    let manifest = run_netsuke_in(workspace.path(), &["generate"])?;
    assert_success(&manifest, "generate stdout example")?;
    ensure!(
        manifest.stdout.contains("rule ") && manifest.stdout.contains("build "),
        "generate should use stdout"
    );
    let quiet = run_with_fake_ninja(workspace.path(), &["--progress", "never", "build"])?;
    assert_success(&quiet, "progress-disabled build")?;

    let expected = documented_example("guide-accessible-output")?;
    let accessible = run_with_fake_ninja(workspace.path(), &["--accessibility", "on", "build"])?;
    assert_success(&accessible, "accessible build")?;
    let stderr = normalize_fluent_isolates(&accessible.stderr);
    for line in expected.body.lines() {
        ensure!(
            stderr.contains(line),
            "accessible output should contain '{line}', got:\n{stderr}"
        );
    }
    Ok(())
}

#[test]
fn json_diagnostic_example_matches_live_schema() -> Result<()> {
    let command = documented_example("guide-json-command")?;
    ensure!(
        command.body == "netsuke --json --no-input --file missing.yml build\n",
        "JSON command example drifted"
    );
    let expected: Value = serde_json::from_str(&documented_example("guide-json-output")?.body)
        .context("parse documented JSON diagnostic")?;
    let workspace = tempfile::tempdir().context("create JSON diagnostic workspace")?;
    let run = run_netsuke_in(
        workspace.path(),
        &["--json", "--no-input", "--file", "missing.yml", "build"],
    )?;
    ensure!(!run.success, "missing manifest invocation should fail");
    ensure!(
        run.stdout.is_empty(),
        "JSON failure should leave stdout empty"
    );
    let actual: Value = serde_json::from_str(&normalize_fluent_isolates(&run.stderr))
        .context("parse live JSON diagnostic")?;
    ensure!(
        actual == expected,
        "documented JSON diagnostic drifted\nexpected: {expected:#}\nactual: {actual:#}"
    );
    Ok(())
}

#[rstest]
#[case("examples/basic_c.yml")]
#[case("examples/photo_edit.yml")]
#[case("examples/visual_design.yml")]
#[case("examples/website.yml")]
#[case("examples/writing.yml")]
#[case("examples/hello-world/Netsukefile")]
fn linked_repository_example_generates_ninja(#[case] path: &str) -> Result<()> {
    let run = run_netsuke_in(
        Path::new("."),
        &["--progress", "never", "--file", path, "generate"],
    )?;
    assert_generates_valid_ninja(&run, path)
}

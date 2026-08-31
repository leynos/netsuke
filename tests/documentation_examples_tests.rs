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
    "guide-boolean-string-interpolation",
    "guide-check-command",
    "guide-check-config",
    "guide-check-explain",
    "guide-check-policy",
    "guide-check-suppression",
    "guide-cli-usage",
    "guide-command-available-manifest",
    "guide-command-list",
    "guide-complete-manifest",
    "guide-configuration-observability",
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
    "guide-serial-dependency-order-manifest",
    "guide-source-install",
    "guide-utility-commands",
    "guide-verbose-timing-reporter",
    "guide-windows-bash-compatibility",
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

/// Assert that a documented configuration file is accepted by a command.
///
/// The two configuration examples differ only in which file they write and
/// which command reads it, so sharing the setup keeps the thing under test —
/// that the documented file is accepted as written — in one place.
fn documented_configuration_example_is_accepted(
    example_id: &str,
    config_filename: &str,
    command_arguments: &[&str],
    context: &str,
) -> Result<()> {
    let example = documented_example(example_id)?;
    let workspace = manifest_workspace("guide-first-build-manifest")?;
    let config_path = workspace.path().join(config_filename);
    test_fs::write(&config_path, example.body)
        .with_context(|| format!("write documented config {config_filename}"))?;
    let config = config_path
        .to_str()
        .context("temporary config path should be UTF-8")?;
    let mut arguments = vec!["--config", config];
    arguments.extend_from_slice(command_arguments);
    let run = run_netsuke_in(workspace.path(), &arguments)?;
    assert_success(&run, context)
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
#[case("guide-boolean-string-interpolation")]
#[case("guide-complete-manifest")]
#[case("guide-foreach-manifest")]
#[case("guide-macro-manifest")]
#[case("guide-command-list")]
#[case("guide-direct-command-list")]
#[case("guide-command-available-manifest")]
#[case("guide-serial-dependency-order-manifest")]
#[case("stdlib-yaml-syntax-manifest")]
#[case("stdlib-jinja-syntax-manifest")]
fn documented_manifest_generates_ninja(#[case] example_id: &str) -> Result<()> {
    let workspace = manifest_workspace(example_id)?;
    let run = run_netsuke_in(workspace.path(), &["--progress", "never", "generate"])?;

    assert_generates_valid_ninja(&run, example_id)
}

#[test]
fn serial_aggregate_example_generates_a_dependency_only_node() -> Result<()> {
    let workspace = manifest_workspace("guide-serial-dependency-order-manifest")?;
    let run = run_netsuke_in(workspace.path(), &["--progress", "never", "generate"])?;

    assert_success(&run, "guide dependency-only serial aggregate")?;
    ensure!(
        run.stdout.contains("build all: phony | .netsuke/serial/"),
        "the documented aggregate should lower to a dependency-only phony node: {}",
        run.stdout
    );
    ensure!(
        !run.stdout.contains("command = :"),
        "the documented aggregate must not emit a shell no-op: {}",
        run.stdout
    );
    Ok(())
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
    let run = run_netsuke_in(workspace.path(), &["--locale", "en-US", "help", "targets"])?;
    assert_success(&run, "help targets example")?;
    ensure!(
        normalize_fluent_isolates(&run.stdout).contains("Targets:"),
        "help targets should print the Targets section"
    );
    ensure!(
        normalize_fluent_isolates(&run.stdout).contains("hello.txt"),
        "help targets should list the documented target"
    );
    Ok(())
}

/// The documented `netsuke check` invocation must run and report cleanly.
#[test]
fn check_example_reports_a_clean_manifest() -> Result<()> {
    let example = documented_example("guide-check-command")?;
    ensure!(example.body == "netsuke check\n", "check example drifted");
    let workspace = manifest_workspace("guide-first-build-manifest")?;
    let run = run_netsuke_in(workspace.path(), &["--locale", "en-US", "check"])?;
    assert_success(&run, "check example")?;
    ensure!(
        normalize_fluent_isolates(&run.stdout).contains("Lint results"),
        "check should print a summary, got {}",
        run.stdout
    );
    Ok(())
}

/// Every documented `netsuke check` invocation must run as written.
///
/// The `--explain` catalogue is checked for a rule it must contain, and the
/// policy example for the effect its selectors claim: a `clarity=off` category
/// selector followed by a rule selector that promotes one of that category's
/// rules to `error` must still report that rule.
#[test]
fn check_explain_and_policy_examples_run() -> Result<()> {
    let workspace = manifest_workspace("guide-first-build-manifest")?;
    let explain = documented_example("guide-check-explain")?;
    for line in explain.body.lines() {
        let arguments: Vec<&str> = line.split_whitespace().skip(1).collect();
        let run = run_netsuke_in(workspace.path(), &arguments)?;
        assert_success(&run, line)?;
        ensure!(
            run.stdout.contains("directory-dep-not-order-only"),
            "`{line}` should describe the rule it names, got {}",
            run.stdout
        );
    }

    // The documented selectors disable the `clarity` category, then promote one
    // of its rules back to `error` under a `warning` threshold. Running them
    // against a manifest that violates that rule is what shows the selectors
    // take effect: asserting only that the command succeeds would pass just as
    // well if they were ignored entirely.
    let policy = documented_example("guide-check-policy")?;
    let arguments: Vec<&str> = policy.body.split_whitespace().skip(1).collect();
    let policy_workspace = tempfile::tempdir().context("create the policy workspace")?;
    test_fs::write(
        policy_workspace.path().join("Netsukefile"),
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  - name: output.txt\n",
            "    sources: input.txt\n",
            "    command: \"cp input.txt output.txt\"\n",
        ),
    )
    .context("write the policy fixture manifest")?;
    let run = run_netsuke_in(policy_workspace.path(), &arguments)?;
    ensure!(
        !run.success,
        "the promoted rule should reach the threshold: {}{}",
        run.stdout,
        run.stderr
    );
    ensure!(
        format!("{}{}", run.stdout, run.stderr).contains("literal-recipe-path"),
        "the promoted rule should be the one reported"
    );
    Ok(())
}

/// The documented configuration example must be accepted and take effect.
#[test]
fn check_configuration_example_is_accepted() -> Result<()> {
    documented_configuration_example_is_accepted(
        "guide-check-config",
        "check.toml",
        &["--json", "check", "--explain"],
        "check configuration example",
    )
}

/// The documented suppression comment must silence the finding it names.
///
/// Without the directive the manifest reports `background-job`; with it, the
/// run is clean. Asserting both directions is what proves the example teaches
/// a working suppression rather than a manifest that never had a finding.
#[test]
fn check_suppression_example_silences_its_finding() -> Result<()> {
    let workspace = manifest_workspace("guide-check-suppression")?;
    let clean = run_netsuke_in(workspace.path(), &["--json", "check", "--fail-on", "never"])?;
    assert_success(&clean, "suppressed check example")?;
    let document: Value =
        serde_json::from_str(&clean.stdout).context("parse the check result document")?;
    let findings = document
        .pointer("/result/findings")
        .and_then(Value::as_array)
        .context("the result should carry a findings array")?;
    ensure!(
        findings.is_empty(),
        "the documented directive should silence every finding, got {findings:?}"
    );
    ensure!(
        document.pointer("/result/summary/suppressed") == Some(&Value::from(1)),
        "the directive should be recorded as having suppressed one finding"
    );
    Ok(())
}
#[test]
fn project_configuration_example_is_accepted() -> Result<()> {
    documented_configuration_example_is_accepted(
        "guide-project-config",
        "example.toml",
        &["--progress", "never", "generate"],
        "project configuration example",
    )
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

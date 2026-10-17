//! Check documented CLI examples against live command behaviour.

use super::*;

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

#[test]
fn help_targets_edges_example_matches_live_schema() -> Result<()> {
    let mut expected: Value =
        serde_json::from_str(&documented_example("guide-help-targets-edges-json")?.body)
            .context("parse documented help-targets graph JSON")?;
    let workspace = tempfile::tempdir().context("create graph JSON example workspace")?;
    test_fs::write(
        workspace.path().join("Netsukefile"),
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  - name: hello.txt\n",
            "    description: Build hello.txt\n",
            "    command: touch hello.txt\n",
            "defaults: [hello.txt]\n"
        ),
    )
    .context("write graph JSON example manifest")?;
    let run = run_netsuke_in(workspace.path(), &["--json", "help", "targets", "--edges"])?;
    assert_success(&run, "help-targets graph JSON example")?;
    let mut actual: Value =
        serde_json::from_str(&run.stdout).context("parse live help-targets graph JSON")?;
    *actual
        .pointer_mut("/generator/version")
        .context("live graph JSON generator version missing")? =
        Value::String("<version>".to_owned());
    *expected
        .pointer_mut("/generator/version")
        .context("documented graph JSON generator version missing")? =
        Value::String("<version>".to_owned());
    ensure!(
        actual == expected,
        "documented help-targets graph JSON drifted\nexpected: {expected:#}\nactual: {actual:#}"
    );
    Ok(())
}

//! Executable contracts for examples in the project documentation.

mod documentation_examples;

#[path = "documentation_examples_tests/cli_examples.rs"]
mod cli_examples;

#[cfg(feature = "lint")]
#[path = "documentation_examples_tests/lint_examples.rs"]
mod lint_examples;

#[path = "documentation_examples_tests/release_wix_extension.rs"]
mod release_wix_extension;

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
    "devguide-clock-snippet",
    "devguide-env-reader-snippet",
    "devguide-ninja-request-snippet",
    "devguide-verbose-timing-reporter",
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
    "guide-file-follow-symlinks-expression",
    "guide-file-max-bytes-expression",
    "guide-first-build-commands",
    "guide-first-build-manifest",
    "guide-foreach-manifest",
    "guide-help-targets",
    "guide-help-targets-edges-json",
    "guide-json-command",
    "guide-json-output",
    "guide-macro-manifest",
    "guide-manifest-environment-policy",
    "guide-manifest-environment-policy-cli",
    "guide-manifest-environment-policy-env",
    "guide-output-streams",
    "guide-project-anchor",
    "guide-project-config",
    "guide-release-wix-extension-version",
    "guide-serial-dependency-order-manifest",
    "guide-source-install",
    "guide-utility-commands",
    "guide-windows-bash-compatibility",
    "guide-windows-help",
    "guide-windows-help-install",
    "guide-windows-path",
    "readme-backtick-rejection-manifest",
    "readme-binstall-install",
    "readme-crates-io-install",
    "readme-first-build-commands",
    "readme-first-build-manifest",
    "readme-quoted-path-manifest",
    "readme-safe-placeholder-manifest",
    "readme-source-install",
    "stdlib-fetch-expression",
    "stdlib-file-tests-manifest",
    "stdlib-host-context-manifest",
    "stdlib-jinja-syntax-manifest",
    "stdlib-optional-rustflags-manifest",
    "stdlib-path-and-collection-manifest",
    "stdlib-time-manifest",
    "stdlib-yaml-syntax-manifest",
];

/// Assert that a developers' guide snippet is a Rust fence naming the given APIs.
///
/// Several guide snippets duplicate an executable doctest, and pinning the copy
/// to the same identifiers is what keeps the two from drifting silently. The
/// label names the snippet so a failure reads in the guide's own vocabulary.
fn assert_snippet_names(example_id: &str, label: &str, needles: &[&str]) -> Result<()> {
    let example = documented_example(example_id)?;
    ensure!(
        example.language == "rust",
        "the {label} snippet should be a Rust fence"
    );
    for needle in needles {
        ensure!(
            example.body.contains(needle),
            "the {label} snippet should mention {needle}"
        );
    }
    Ok(())
}

/// The developers' guide env-reader snippet must track the API it mirrors.
///
/// The snippet is Rust and is executed as the doctest on `from_str_with_env`.
#[test]
fn env_reader_snippet_mirrors_the_doctest() -> Result<()> {
    assert_snippet_names(
        "devguide-env-reader-snippet",
        "env-reader",
        &["from_str_with_env", "EnvReader", "env('PROFILE')"],
    )
}

/// The developers' guide clock snippet must track the API it mirrors.
///
/// The snippet is Rust and is executed as the doctest on `with_clock`.
#[test]
fn clock_snippet_mirrors_the_doctest() -> Result<()> {
    assert_snippet_names(
        "devguide-clock-snippet",
        "clock",
        &["with_clock", "fixed_clock", "StdlibConfig"],
    )
}

/// The developers' guide Ninja-request snippet must name the API it documents.
///
/// The snippet is the only place the developers' guide constructs the request
/// bundles, so pinning the identifiers keeps it from drifting into prose about
/// types the crate no longer exports.
#[test]
fn ninja_request_snippet_names_both_request_types() -> Result<()> {
    assert_snippet_names(
        "devguide-ninja-request-snippet",
        "Ninja-request",
        &[
            "NinjaBuildRequest",
            "NinjaToolRequest",
            "run_ninja_with",
            "run_ninja_tool_with",
            "CommandEnv::inherit",
        ],
    )
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
#[case("stdlib-optional-rustflags-manifest")]
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

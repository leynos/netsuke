//! CLI contracts for a heredoc delimiter marker after an active backslash.
//!
//! IR lowering must reject the declaration before the configured Ninja program
//! starts, because a rendered path could otherwise escape the encoding's
//! opening quote and become shell syntax inside the executable script. These
//! cases drive the real CLI with a sentinel Ninja so the ordering is observable.

#![cfg(unix)]

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use test_support::fluent::normalize_fluent_isolates;
use test_support::fs as test_fs;
use test_support::netsuke::run_netsuke_in_with_env;
use test_support::write_exec_with_content;

/// The quote-leading path bound to the marker in each scenario.
const INJECTING_PATH: &str = "\"; touch injected; #";

/// One CLI rejection scenario.
struct EscapeCase {
    /// Target body carrying the escaped heredoc declaration.
    recipe: &'static str,
    /// Target name, which also supplies the output binding.
    target: &'static str,
    /// Whether the scenario binds the hostile path as a source.
    has_hostile_source: bool,
    /// Internal marker text the diagnostic must name.
    fragment: &'static str,
}

/// Render a Netsukefile around one scenario's target body.
fn manifest_text(case: &EscapeCase) -> String {
    let sources = if case.has_hostile_source {
        format!("    sources: '{INJECTING_PATH}'\n")
    } else {
        String::new()
    };
    format!(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: '{}'\n{}{}",
        case.target, sources, case.recipe
    )
}

/// Run the build with a sentinel Ninja and assert lowering rejected the recipe.
fn assert_rejected_before_ninja(case: &EscapeCase) -> Result<()> {
    let workspace = tempfile::tempdir().context("create heredoc delimiter workspace")?;
    test_fs::write(workspace.path().join("Netsukefile"), manifest_text(case))
        .context("write heredoc delimiter manifest")?;
    if case.has_hostile_source {
        test_fs::write(workspace.path().join(INJECTING_PATH), "input contents")
            .context("write hostile source file")?;
    }

    let sentinel = workspace.path().join("ninja-started");
    let fake_ninja = write_exec_with_content(
        workspace.path(),
        "fake-ninja",
        "#!/bin/sh\nset -eu\nprintf started > \"$NETSUKE_NINJA_SENTINEL\"\n",
    )
    .context("write sentinel fake Ninja")?;
    let ninja = fake_ninja
        .to_str()
        .context("fake Ninja path should be UTF-8")?;
    let sentinel_path = sentinel
        .to_str()
        .context("Ninja sentinel path should be UTF-8")?;

    // The human renderer reports only the runner's graph context and drops the
    // inner interpolation diagnostic, so ask for the JSON report, whose cause
    // chain carries the localized rejection the assertion below names.
    let output = run_netsuke_in_with_env(
        workspace.path(),
        &["--json", "--locale", "en-US", "--no-input", "build"],
        &[
            ("NETSUKE_NINJA", ninja),
            ("NETSUKE_NINJA_SENTINEL", sentinel_path),
        ],
    )
    .context("run Netsuke build with sentinel Ninja")?;

    let stderr = normalize_fluent_isolates(&output.stderr);
    ensure!(
        !output.success,
        "an escaped delimiter marker must fail the build: {stderr}"
    );
    ensure!(
        stderr.contains("Invalid command interpolation:"),
        "the diagnostic must name the interpolation failure: {stderr}"
    );
    ensure!(
        stderr.contains(case.fragment),
        "the diagnostic must name the escaped marker {}: {stderr}",
        case.fragment
    );
    ensure!(
        !sentinel.exists(),
        "Ninja must not start when delimiter lowering rejects the recipe"
    );
    ensure!(
        !workspace.path().join("injected").exists(),
        "the rejected path must not reach a shell that executes it"
    );
    Ok(())
}

/// Reject an escaped input marker in both script and command recipes.
#[rstest]
#[case::script(EscapeCase {
    recipe: r#"    script: |
      cat <<\{{ ins }} > out
      body
      \"; touch injected; #
"#,
    target: "out",
    has_hostile_source: true,
    fragment: r"\\__NETSUKE_INS_PLACEHOLDER__",
})]
#[case::command(EscapeCase {
    recipe: "    command: 'cat <<\\{{ ins }} > out'\n",
    target: "out",
    has_hostile_source: true,
    fragment: r"\\__NETSUKE_INS_PLACEHOLDER__",
})]
fn escaped_input_delimiter_marker_is_rejected(#[case] case: EscapeCase) -> Result<()> {
    assert_rejected_before_ninja(&case)
}

/// Reject an escaped output marker whose hostile target names the path.
#[rstest]
#[case::script(EscapeCase {
    recipe: r#"    script: |
      cat <<\{{ outs }} > /dev/null
      body
      \"; touch injected; #
"#,
    target: INJECTING_PATH,
    has_hostile_source: false,
    fragment: r"\\__NETSUKE_OUTS_PLACEHOLDER__",
})]
#[case::command(EscapeCase {
    recipe: "    command: 'cat <<\\{{ outs }} > /dev/null'\n",
    target: INJECTING_PATH,
    has_hostile_source: false,
    fragment: r"\\__NETSUKE_OUTS_PLACEHOLDER__",
})]
fn escaped_output_delimiter_marker_is_rejected(#[case] case: EscapeCase) -> Result<()> {
    assert_rejected_before_ninja(&case)
}

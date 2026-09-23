//! Pin the README's POSIX command-interpolation contract to executable examples.

#![cfg(unix)]

mod documentation_examples;

use anyhow::{Context, Result};
use documentation_examples::{assert_success, documented_example, manifest_workspace};
use googletest::{assert_that, matchers::contains_substring};
use mockable::{DefaultEnv, Env};
use netsuke::{
    ir::BuildGraph,
    manifest,
    ninja_gen::{RecipeShell, generate_with_shell},
};
use pretty_assertions::assert_eq;
use rstest::{fixture, rstest};
use test_support::{fs as test_fs, netsuke::run_netsuke_in_with_env};

/// Load the published accepted manifest for table and boundary controls.
///
/// # Errors
/// Return an error if the README example is absent or malformed.
#[fixture]
fn safe_manifest() -> Result<String> {
    Ok(documented_example("readme-safe-placeholder-manifest")?.body)
}

/// Compile a manifest through rendering, lowering, and the POSIX Ninja backend.
///
/// Keep this helper local: these tests inspect published examples rather than
/// constructing IR actions that would bypass marker rendering.
///
/// # Errors
/// Return the original manifest, lowering, or backend error.
fn generate_posix(source: &str) -> Result<String> {
    let manifest = manifest::from_str(source)?;
    let graph = BuildGraph::from_manifest_for_shell(&manifest, RecipeShell::Posix)?;
    generate_with_shell(&graph, RecipeShell::Posix).map_err(Into::into)
}

#[rstest]
fn documented_safe_placeholder_manifest_builds(safe_manifest: Result<String>) -> Result<()> {
    let source = safe_manifest?;
    let ninja = generate_posix(&source)?;
    assert_that!(ninja, contains_substring("$$PATH"));
    let workspace = manifest_workspace("readme-safe-placeholder-manifest")?;
    test_fs::write(workspace.path().join("input.txt"), "README contract\n")?;
    let path = DefaultEnv
        .string("PATH")
        .context("host PATH is required for Ninja")?;
    let run = run_netsuke_in_with_env(
        workspace.path(),
        &[],
        &[("PATH", &path), ("NETSUKE_NINJA", "ninja")],
    )?;
    assert_success(&run, "documented safe placeholder manifest")?;
    assert_eq!(
        test_fs::read_to_string(workspace.path().join("output.txt"))?,
        "README contract\n"
    );
    Ok(())
}

#[rstest]
#[case::inputs("{{ ins }}", "input.txt")]
#[case::outputs("{{ outs }}", "output.txt")]
#[case::in_variable("$in", "$$in")]
#[case::out_variable("$out", "$$out")]
#[case::ins_variable("$ins", "$$ins")]
#[case::outs_variable("$outs", "$$outs")]
#[case::input_variable("$input", "$$input")]
#[case::output_variable("$output", "$$output")]
#[case::path_variable("$PATH", "$$PATH")]
fn dollar_forms_are_shell_variables_in_both_recipe_kinds(
    safe_manifest: Result<String>,
    #[case] form: &str,
    #[case] expected: &str,
    #[values("command", "script")] kind: &str,
) -> Result<()> {
    let source = safe_manifest?.replace(
        "command: 'cat {{ ins }} > {{ outs }} && test -n \"$PATH\"'",
        &format!("{kind}: 'printf %s {form}'"),
    );
    let ninja = generate_posix(&source)?;
    let encoded_expected = if kind == "script" && form.starts_with('$') {
        format!("\\{expected}")
    } else {
        expected.to_owned()
    };
    assert_that!(
        ninja,
        contains_substring(format!("printf %s {encoded_expected}"))
    );
    Ok(())
}

#[rstest]
fn netsuke_owned_path_substitutions_are_quoted() -> Result<()> {
    let example = documented_example("readme-quoted-path-manifest")?;
    let ninja = generate_posix(&example.body)?;
    assert_that!(
        ninja,
        contains_substring("cat input' file.txt' > output.txt")
    );
    Ok(())
}

#[rstest]
fn documented_backtick_manifest_is_rejected() -> Result<()> {
    let workspace = manifest_workspace("readme-backtick-rejection-manifest")?;
    let run = run_netsuke_in_with_env(
        workspace.path(),
        &["--json", "--locale", "en-GB"],
        &[
            ("NETSUKE_NINJA", "/no-such-readme-ninja"),
            ("LANG", "en_GB.UTF-8"),
        ],
    )?;
    assert_eq!(run.success, false);
    assert_that!(
        run.stderr,
        contains_substring("Invalid command interpolation:")
    );
    Ok(())
}

#[rstest]
fn balanced_author_backticks_are_accepted(safe_manifest: Result<String>) -> Result<()> {
    let source = safe_manifest?.replace("cat {{ ins }}", "echo `printf authored`");
    let ninja = generate_posix(&source)?;
    assert_that!(ninja, contains_substring("`printf authored`"));
    Ok(())
}

#[rstest]
fn odd_backtick_count_without_markers_is_rejected(safe_manifest: Result<String>) -> Result<()> {
    let source = safe_manifest?.replace(
        "cat {{ ins }} > {{ outs }} && test -n \"$PATH\"",
        "echo `authored",
    );
    let error = generate_posix(&source).expect_err("an odd backtick count must be rejected");
    assert_that!(
        error.to_string(),
        contains_substring("Invalid command interpolation:")
    );
    Ok(())
}

#[rstest]
#[case::command("command", false)]
#[case::script("script", true)]
fn shlex_gate_applies_to_commands_not_scripts(
    safe_manifest: Result<String>,
    #[case] kind: &str,
    #[case] accepted: bool,
) -> Result<()> {
    let source = safe_manifest?.replace(
        "command: 'cat {{ ins }} > {{ outs }} && test -n \"$PATH\"'",
        &format!("{kind}: |\n      echo 'unterminated"),
    );
    let result = generate_posix(&source);
    if accepted {
        assert_that!(result?, contains_substring("unterminated"));
    } else {
        let error = result.expect_err("command quoting must be checked");
        assert_that!(
            error.to_string(),
            contains_substring("Invalid command interpolation:")
        );
    }
    Ok(())
}

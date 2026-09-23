//! Pin the README's POSIX command-interpolation contract to executable examples.

#![cfg(unix)]

mod documentation_examples;

use anyhow::{Context, Result, bail, ensure};
use documentation_examples::{assert_success, documented_example, manifest_workspace};
use googletest::{assert_that, matchers::contains_substring};
use mockable::{DefaultEnv, Env};
use netsuke::{
    ast::Recipe,
    ir::{BuildGraph, INS_TOKEN, OUTS_TOKEN},
    manifest,
    ninja_gen::{NinjaGenError, RecipeShell, generate_with_shell},
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

/// Return a lowered recipe and graph for the selected shell.
///
/// Keep this helper local: these tests inspect published examples rather than
/// constructing IR actions that would bypass marker rendering.
///
/// # Errors
/// Return the original manifest, lowering, or backend error, or report an
/// unexpected action shape.
fn lower_recipe_for_shell(source: &str, shell: RecipeShell) -> Result<(String, BuildGraph)> {
    let manifest = manifest::from_str(source)?;
    let graph = BuildGraph::from_manifest_for_shell(&manifest, shell)?;
    let action = graph
        .actions
        .values()
        .next()
        .context("README action is absent")?;
    let recipe = match &action.recipe {
        Recipe::Command { command } => command.to_string_vec().join("\n"),
        Recipe::Script { script } => script.clone(),
        Recipe::Rule { .. } => bail!("README action unexpectedly uses a rule"),
    };
    Ok((recipe, graph))
}

/// Compile a manifest through rendering, lowering, and the POSIX Ninja backend.
///
/// # Errors
/// Return the original manifest, lowering, or backend error.
fn generate_posix(source: &str) -> Result<String> {
    let (_, graph) = lower_recipe_for_shell(source, RecipeShell::Posix)?;
    Ok(generate_with_shell(&graph, RecipeShell::Posix)?)
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
#[case::comment(
    "printf '%s' {{ ins }} > {{ outs }} # {{ ins }} {{ outs }}",
    format!("printf '%s' input.txt > output.txt # {INS_TOKEN} {OUTS_TOKEN}")
)]
#[case::heredoc(
    "cat <<EOF > {{ outs }}\n{{ ins }} {{ outs }}\nEOF\nprintf '%s' {{ ins }} > /dev/null",
    format!(
        "cat <<EOF > output.txt\n{INS_TOKEN} {OUTS_TOKEN}\nEOF\nprintf '%s' input.txt > /dev/null"
    )
)]
fn inert_script_regions_preserve_rendered_markers(
    #[case] recipe: &str,
    #[case] expected_lowered: String,
    #[values("command", "script")] kind: &str,
    #[values(RecipeShell::Posix, RecipeShell::Bash)] shell: RecipeShell,
) -> Result<()> {
    let indented_recipe = recipe.replace('\n', "\n      ");
    let source = safe_manifest()?.replace(
        "command: 'cat {{ ins }} > {{ outs }} && test -n \"$PATH\"'",
        &format!("{kind}: |\n      {indented_recipe}"),
    );
    let (lowered, graph) = lower_recipe_for_shell(&source, shell)?;
    assert_eq!(lowered.trim_end_matches('\n'), expected_lowered);
    if kind == "command" && recipe.contains('\n') {
        let error = generate_with_shell(&graph, shell)
            .expect_err("a multiline command cannot fit one Ninja binding");
        ensure!(
            matches!(error, NinjaGenError::UnsafeNinjaValue),
            "expected unsafe Ninja value, got {error:?}"
        );
    } else {
        let ninja = generate_with_shell(&graph, shell)?;
        assert_that!(ninja.as_str(), contains_substring(INS_TOKEN));
        assert_that!(ninja.as_str(), contains_substring(OUTS_TOKEN));
    }
    Ok(())
}

#[rstest]
fn heredoc_body_marker_remains_literal_when_ninja_runs(
    safe_manifest: Result<String>,
) -> Result<()> {
    let source = safe_manifest?.replace(
        "command: 'cat {{ ins }} > {{ outs }} && test -n \"$PATH\"'",
        "script: |\n      cat <<EOF > {{ outs }}\n      {{ ins }}\n      EOF\n      cat {{ ins }} > /dev/null",
    );
    let workspace = manifest_workspace("readme-safe-placeholder-manifest")?;
    test_fs::write(workspace.path().join("Netsukefile"), source)?;
    test_fs::write(
        workspace.path().join("input.txt"),
        "input path must not appear\n",
    )?;
    let path = DefaultEnv
        .string("PATH")
        .context("host PATH is required for Ninja")?;
    let run = run_netsuke_in_with_env(
        workspace.path(),
        &[],
        &[("PATH", &path), ("NETSUKE_NINJA", "ninja")],
    )?;
    assert_success(&run, "README heredoc body marker")?;
    assert_eq!(
        test_fs::read_to_string(workspace.path().join("output.txt"))?,
        format!("{INS_TOKEN}\n")
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

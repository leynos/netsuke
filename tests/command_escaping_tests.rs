//! Tests for shell quoting of command substitutions.

use anyhow::{Context, Result, bail, ensure};
use netsuke::{ast::Recipe, ir::BuildGraph, manifest};
use rstest::rstest;
use test_support::manifest::manifest_yaml;

/// Extract shell words from the first target's command.
///
/// # Examples
/// ```no_run
/// # use anyhow::Result;
/// # use command_escaping_tests::command_words;
/// # use test_support::manifest::manifest_yaml;
/// # fn demo() -> Result<()> {
/// let words = command_words(
///     "targets:\n  - name: out\n    sources: in\n    command: \"echo hi\"\n",
/// )?;
/// assert_eq!(words, ["echo", "hi"]);
/// # Ok(())
/// # }
/// # demo().unwrap();
/// ```
fn command_words(body: &str) -> Result<Vec<String>> {
    let yaml = manifest_yaml(body);
    let manifest = manifest::from_str(&yaml)?;
    let graph = BuildGraph::from_manifest(&manifest)?;
    let action = graph
        .actions
        .values()
        .next()
        .context("manifest should contain at least one action")?;
    let Recipe::Command { command } = &action.recipe else {
        bail!("expected command recipe, got: {:?}", action.recipe);
    };
    shlex::split(command).context("split command into words")
}

#[rstest]
fn inputs_and_outputs_are_quoted() -> Result<()> {
    let words = command_words(
        "targets:\n  - name: 'out file'\n    sources: 'in file'\n    command: \"cat $in > $out\"\n",
    )?;
    let expected = ["cat", "in file", ">", "out file"].map(ToOwned::to_owned);
    ensure!(
        words == expected,
        "expected shell words to match quoted inputs/outputs"
    );
    Ok(())
}

#[rstest]
fn multiple_inputs_outputs_with_special_chars_are_quoted() -> Result<()> {
    let words = command_words(
        "targets:\n  - name: ['out file', 'out&2']\n    sources: ['in file', 'input$1']\n    command: \"echo $in && echo $out\"\n",
    )?;
    let expected = [
        "echo", "in file", "input$1", "&&", "echo", "out file", "out&2",
    ]
    .map(ToOwned::to_owned);
    ensure!(
        words == expected,
        "expected words to preserve quoting for lists"
    );
    Ok(())
}

#[rstest]
fn variable_name_overlap_not_rewritten() -> Result<()> {
    let words = command_words(
        "targets:\n  - name: 'out file'\n    sources: in\n    command: \"echo $input > $out\"\n",
    )?;
    let expected = ["echo", "$input", ">", "out file"].map(ToOwned::to_owned);
    ensure!(words == expected, "unexpected placeholder rewriting");
    Ok(())
}

#[rstest]
fn output_variable_overlap_not_rewritten() -> Result<()> {
    let words = command_words(
        "targets:\n  - name: out\n    sources: in\n    command: \"echo $output_dir > $out\"\n",
    )?;
    let expected = ["echo", "$output_dir", ">", "out"].map(ToOwned::to_owned);
    ensure!(words == expected, "unexpected output placeholder rewriting");
    Ok(())
}

#[rstest]
fn newline_in_paths_is_quoted() -> Result<()> {
    let words = command_words(
        "targets:\n  - name: \"o'ut\\nfile\"\n    sources: \"-in file\"\n    command: \"printf %s $in > $out\"\n",
    )?;
    let expected = ["printf", "%s", "-in file", ">", "o'ut\nfile"].map(ToOwned::to_owned);
    ensure!(words == expected, "expected newline to be preserved");
    Ok(())
}

#[rstest]
fn command_without_placeholders_remains_valid() -> Result<()> {
    let words =
        command_words("targets:\n  - name: out\n    sources: in\n    command: \"echo hi\"\n")?;
    let expected = ["echo", "hi"].map(ToOwned::to_owned);
    ensure!(
        words == expected,
        "command without placeholders should split literally"
    );
    Ok(())
}

#[rstest]
#[case("echo \"unterminated")]
#[case("echo 'unterminated")]
#[case("echo `unterminated")]
fn invalid_command_errors(#[case] cmd: &str) -> Result<()> {
    let escaped = cmd.replace('\\', "\\\\").replace('"', "\\\"");
    let yaml = manifest_yaml(&format!(
        "targets:\n  - name: out\n    sources: in\n    command: \"{escaped}\"\n"
    ));
    let manifest = manifest::from_str(&yaml)?;
    let Err(err) = BuildGraph::from_manifest(&manifest) else {
        bail!("expected invalid command to fail");
    };
    ensure!(
        err.to_string().contains("Invalid command interpolation"),
        "unexpected error: {err}"
    );
    Ok(())
}

//! Verify runner-selected Bash executables in generated Ninja bundles.

use super::super::{RecipeShell, dyndep, test_support::command_action};
use crate::{
    ast::{CommandIsolation, Recipe, StringOrList},
    ir::BuildGraph,
    recipe_shell::BashExecutable,
};
use anyhow::{Result, ensure};
use camino::Utf8PathBuf;

/// Carry one runner-selected executable into the generated Ninja rule.
#[test]
fn generated_bundle_binds_the_preflighted_bash_executable() -> Result<()> {
    let mut graph = BuildGraph::default();
    graph.actions.insert(
        "run".into(),
        command_action(StringOrList::String("echo ready".into())),
    );
    let workspace = tempfile::tempdir()?;
    let executable_path = Utf8PathBuf::from_path_buf(
        workspace
            .path()
            .join("Git Bash λ $")
            .join("bin")
            .join("bash.exe"),
    )
    .map_err(|_| anyhow::anyhow!("fixture executable path should be UTF-8"))?;
    let executable = BashExecutable::from_absolute_path(executable_path)?;

    let bundle =
        dyndep::generate_bundle_for_shell_with_bash(&graph, RecipeShell::Bash, Some(&executable))?;

    let expected_command = format!(
        "command = \"{}\" -e -c \"echo ready\"",
        executable.as_path().as_str().replace('$', "$$")
    );
    ensure!(
        bundle.build_file().contains(&expected_command),
        "generated command should use the selected Bash executable"
    );
    Ok(())
}

/// Bind the selected Bash path across scalar, script, and list recipe forms.
#[test]
fn every_bash_recipe_form_uses_the_preflighted_executable() -> Result<()> {
    let mut graph = BuildGraph::default();
    graph.actions.insert(
        "scalar".into(),
        command_action(StringOrList::String("echo scalar".into())),
    );
    graph.actions.insert(
        "shared-list".into(),
        command_action(StringOrList::List(vec!["echo shared".into()])),
    );
    let mut isolated_list = command_action(StringOrList::List(vec!["echo isolated".into()]));
    if let Recipe::Command { isolation, .. } = &mut isolated_list.recipe {
        *isolation = CommandIsolation::Isolated;
    }
    graph.actions.insert("isolated-list".into(), isolated_list);
    let mut script = command_action(StringOrList::String(String::new()));
    script.recipe = Recipe::Script {
        script: "echo script".into(),
    };
    graph.actions.insert("script".into(), script);
    let workspace = tempfile::tempdir()?;
    let executable_path = Utf8PathBuf::from_path_buf(
        workspace
            .path()
            .join("Git Bash λ $")
            .join("bin")
            .join("bash.exe"),
    )
    .map_err(|_| anyhow::anyhow!("fixture executable path should be UTF-8"))?;
    let executable = BashExecutable::from_absolute_path(executable_path)?;

    let bundle =
        dyndep::generate_bundle_for_shell_with_bash(&graph, RecipeShell::Bash, Some(&executable))?;
    let binding = format!(
        "command = \"{}\" -e -c ",
        executable.as_path().as_str().replace('$', "$$")
    );

    let binding_count = bundle.build_file().matches(&binding).count();
    ensure!(
        binding_count == 4,
        "expected four Bash bindings, found {binding_count}"
    );
    Ok(())
}

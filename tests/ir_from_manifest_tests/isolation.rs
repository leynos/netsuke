//! Preserve isolated command-list modes through manifest-to-IR lowering.

use super::*;
use netsuke::ast::CommandIsolation;

/// Preserve isolation while interpolating a directly declared command list.
#[test]
fn direct_isolated_command_list_preserves_mode_through_interpolation() -> Result<()> {
    let manifest = manifest::from_str(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out/app\n    sources: src/main.c\n    command_isolation: isolated\n    command:\n      - echo {{ ins }}\n      - echo {{ outs }}\n",
    )?;
    let graph = BuildGraph::from_manifest_for_shell(&manifest, RecipeShell::Posix)?;
    let action = graph
        .actions
        .values()
        .next()
        .context("expected one action")?;
    let Recipe::Command {
        command, isolation, ..
    } = &action.recipe
    else {
        bail!(
            "expected an isolated command recipe, got {:?}",
            action.recipe
        );
    };
    ensure!(*isolation == CommandIsolation::Isolated);
    ensure!(
        command.to_string_vec() == ["echo src/main.c", "echo out/app"],
        "command interpolation should preserve entry order: {command:?}"
    );
    Ok(())
}

/// Preserve an isolated command-list mode inherited from a reusable rule.
#[test]
fn isolated_rule_command_list_preserves_mode_through_interpolation() -> Result<()> {
    let manifest = manifest::from_str(
        "netsuke_version: '1.0.0'\nrules:\n  - name: build\n    command_isolation: isolated\n    command:\n      - echo {{ ins }}\n      - echo {{ outs }}\ntargets:\n  - name: out/app\n    sources: src/main.c\n    rule: build\n",
    )?;
    let graph = BuildGraph::from_manifest_for_shell(&manifest, RecipeShell::Posix)?;
    let action = graph
        .actions
        .values()
        .next()
        .context("expected one action")?;
    let Recipe::Command {
        command, isolation, ..
    } = &action.recipe
    else {
        bail!(
            "expected an isolated rule-backed command, got {:?}",
            action.recipe
        );
    };
    ensure!(*isolation == CommandIsolation::Isolated);
    ensure!(
        command.to_string_vec() == ["echo src/main.c", "echo out/app"],
        "rule interpolation should preserve entry order: {command:?}"
    );
    Ok(())
}

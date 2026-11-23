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

/// Keep an unfinished isolated entry from reclassifying the next entry's marker.
///
/// Isolated entries run in separate PowerShell processes, so entry one's open
/// quote never reaches entry two's marker site.
#[test]
fn isolated_power_shell_entries_interpolate_independently() -> Result<()> {
    let manifest = manifest::from_str(concat!(
        "netsuke_version: '1.0.0'\n",
        "targets:\n",
        "  - name: out.txt\n",
        "    sources: \"; New-Item pwned; Write-Output x\"\n",
        "    command_isolation: isolated\n",
        "    command:\n",
        "      - \"Write-Output 'prefix\"\n",
        "      - Copy-Item {{ ins }} {{ outs }}\n",
    ))?;
    let graph = BuildGraph::from_manifest_for_shell(&manifest, RecipeShell::PowerShell)?;
    ensure_isolated_power_shell_entries(&graph, "a direct isolated list")
}

/// Preserve independent interpolation for an isolated rule-backed list.
#[test]
fn isolated_rule_power_shell_entries_interpolate_independently() -> Result<()> {
    let manifest = manifest::from_str(concat!(
        "netsuke_version: '1.0.0'\n",
        "rules:\n",
        "  - name: emit\n",
        "    command_isolation: isolated\n",
        "    command:\n",
        "      - \"Write-Output 'prefix\"\n",
        "      - Copy-Item {{ ins }} {{ outs }}\n",
        "targets:\n",
        "  - name: out.txt\n",
        "    sources: \"; New-Item pwned; Write-Output x\"\n",
        "    rule: emit\n",
    ))?;
    let graph = BuildGraph::from_manifest_for_shell(&manifest, RecipeShell::PowerShell)?;
    ensure_isolated_power_shell_entries(&graph, "a rule-backed isolated list")
}

/// Assert one isolated PowerShell list keeps its entries independent.
fn ensure_isolated_power_shell_entries(graph: &BuildGraph, description: &str) -> Result<()> {
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
    ensure!(
        *isolation == CommandIsolation::Isolated,
        "{description} should keep its isolation"
    );
    ensure!(
        command.to_string_vec()
            == [
                "Write-Output 'prefix",
                "Copy-Item '; New-Item pwned; Write-Output x' 'out.txt'",
            ],
        "{description} should interpolate each entry at its own boundary: {command:?}"
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

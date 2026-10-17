//! Regression tests for command-list manifest-to-IR lowering.

use super::*;
use crate::ir::cmd_interpolate::{binding_preparations, reset_binding_preparations};
use crate::ir::{INS_TOKEN, OUTS_TOKEN};
use crate::manifest;
use crate::recipe_shell::RecipeShell;
use proptest::prelude::*;

#[test]
fn large_command_list_prepares_path_bindings_once() {
    reset_binding_preparations();
    let entries = (0..64)
        .map(|index| format!("printf {index} {INS_TOKEN} {OUTS_TOKEN}"))
        .collect();
    let mut actions = IrHashMap::default();
    register_action(
        &mut actions,
        Recipe::Command {
            command: StringOrList::List(entries),
            isolation: crate::ast::CommandIsolation::Shared,
        },
        None,
        ActionBindings {
            inputs: &[Utf8PathBuf::from("input")],
            outputs: &[Utf8PathBuf::from("output")],
            shell: RecipeShell::Posix,
        },
    )
    .expect("shell-safe command list should lower");
    assert_eq!(
        binding_preparations(),
        1,
        "all entries in one recipe must reuse one prepared input/output binding set"
    );
}

#[test]
fn power_shell_script_manifest_rejects_quoted_input_marker() {
    let source = concat!(
        "netsuke_version: '1.0.0'\n",
        "targets:\n",
        "  - name: output\n",
        "    sources: \"; New-Item pwned; Write-Output x\"\n",
        "    script: \"Write-Output '{{ ins }}' > {{ outs }}\"\n",
    );
    let manifest = manifest::from_str(source).expect("the script manifest should parse");
    let error = BuildGraph::from_manifest_for_shell(&manifest, RecipeShell::PowerShell)
        .expect_err("quoted PowerShell script markers must fail during IR lowering");

    assert!(matches!(
        error,
        crate::ir::IrGenError::InvalidCommand { .. }
    ));
}

#[test]
fn power_shell_command_manifest_rejects_quoted_input_marker() {
    let source = concat!(
        "netsuke_version: '1.0.0'\n",
        "targets:\n",
        "  - name: output\n",
        "    sources: \"; New-Item pwned; Write-Output x\"\n",
        "    command: \"Write-Output '{{ ins }}' > {{ outs }}\"\n",
    );
    let manifest = manifest::from_str(source).expect("the command manifest should parse");
    let error = BuildGraph::from_manifest_for_shell(&manifest, RecipeShell::PowerShell)
        .expect_err("quoted PowerShell command markers must fail during IR lowering");

    assert!(matches!(
        error,
        crate::ir::IrGenError::InvalidCommand { .. }
    ));
}

proptest! {
    #[test]
    fn command_list_placeholder_interpolation_preserves_entry_order(
        labels in prop::collection::vec("[a-z]{1,10}", 1..9),
    ) {
        let entries: Vec<String> = labels
            .iter()
            .map(|label| format!("echo {label} {INS_TOKEN} {OUTS_TOKEN}"))
            .collect();
        let mut actions = IrHashMap::default();
        let action_id = register_action(
            &mut actions,
            Recipe::Command { command: StringOrList::List(entries), isolation: crate::ast::CommandIsolation::Shared },
            None,
            ActionBindings {
                inputs: &[Utf8PathBuf::from("input")],
                outputs: &[Utf8PathBuf::from("output")],
                shell: RecipeShell::Posix,
            },
        ).expect("shell-safe generated entries should interpolate");
        let action = actions.get(&action_id).expect("registered action should be available");
        let Recipe::Command { command, .. } = &action.recipe else {
            prop_assert!(false, "registered command list should remain a command recipe");
            return Ok(());
        };
        let expected: Vec<String> = labels
            .iter()
            .map(|label| format!("echo {label} input output"))
            .collect();
        prop_assert_eq!(command.to_string_vec(), expected);
    }
}

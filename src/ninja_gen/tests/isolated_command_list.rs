//! Focused tests for isolated POSIX command-list rendering.

use super::*;

/// Render each isolated POSIX entry in its own fail-fast subshell.
#[test]
fn isolated_command_list_wraps_every_entry_in_a_subshell() -> Result<()> {
    let action = Action {
        recipe: Recipe::Command {
            command: StringOrList::List(vec!["cd nested".into(), "pwd".into()]),
            isolation: crate::ast::CommandIsolation::Isolated,
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let mut graph = BuildGraph::default();
    graph.actions.insert("isolated".into(), action);

    let ninja = generate_posix(&graph)?;
    ensure!(ninja.contains("command = ( { _netsuke_background_before="));
    ensure!(ninja.contains("} ) && ( { _netsuke_background_before="));
    ensure!(
        ninja.contains("if [ \"$$_netsuke_command_status\" -ne 0 ]; then"),
        "isolated exit traps must emit markers only for non-zero status:\n{ninja}"
    );
    Ok(())
}

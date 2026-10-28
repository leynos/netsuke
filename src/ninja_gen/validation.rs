//! Validate recipes and metadata before Ninja rendering.

use super::command_list::{CommandListEntry, CommandListEntryError, command_list_entry_error};
use super::{NinjaGenError, RecipeShell, escape::validate_ninja_value};
use crate::ast::{CommandIsolation, Recipe, StringOrList};
use crate::ir::{Action, BuildEdge, BuildGraph};
use crate::localization::{self, keys};

/// Reject console scheduling that conflicts with an edge's action.
///
/// # Errors
///
/// Returns an error when a console edge uses a dependency-only recipe or
/// conflicts with an action's custom pool.
pub(super) fn validate_console_scheduling(graph: &BuildGraph) -> Result<(), NinjaGenError> {
    for edge in graph.edges().filter(|edge| edge.console) {
        if let Some(action) = graph.actions.get(&edge.action_id) {
            validate_console_edge(edge, action)?;
        }
    }
    Ok(())
}

/// Reject a console edge whose action conflicts with console scheduling.
///
/// # Errors
///
/// Returns an error for dependency-only recipes, conflicting pools, or invalid
/// Ninja pool values.
fn validate_console_edge(edge: &BuildEdge, action: &Action) -> Result<(), NinjaGenError> {
    let output = edge
        .explicit_outputs
        .first()
        .map_or_else(|| "<no explicit output>".to_owned(), ToString::to_string);
    if action.recipe.is_dependency_only() {
        return Err(NinjaGenError::ConsoleDependencyOnly {
            message: localization::message(keys::NINJA_GEN_CONSOLE_DEPENDENCY_ONLY)
                .with_arg("output", &output),
            output,
        });
    }
    if let Some(pool) = action.pool.as_deref() {
        validate_ninja_value(pool)?;
        if pool != "console" {
            return Err(NinjaGenError::ConsolePoolConflict {
                message: localization::message(keys::NINJA_GEN_CONSOLE_POOL_CONFLICT)
                    .with_arg("output", &output)
                    .with_arg("pool", pool),
                output,
                pool: pool.to_owned(),
            });
        }
    }
    Ok(())
}

/// Reject recipes the generated shell cannot execute with stable semantics.
pub(super) fn validate_action_recipe(
    action: &crate::ir::Action,
    action_index: usize,
    shell: RecipeShell,
) -> Result<(), NinjaGenError> {
    if let Recipe::Command {
        command, isolation, ..
    } = &action.recipe
    {
        if *isolation == CommandIsolation::Isolated && !matches!(command, StringOrList::List(_)) {
            return Err(NinjaGenError::InvalidCommandIsolation { action_index });
        }
        if command.is_empty_content() {
            return Err(NinjaGenError::EmptyCommandRecipe { action_index });
        }
    }
    if shell == RecipeShell::PowerShell {
        return Ok(());
    }
    match &action.recipe {
        Recipe::Command {
            command: StringOrList::String(command),
            ..
        } => validate_scalar_command(command),
        Recipe::Command {
            command: StringOrList::List(entries),
            ..
        } => validate_posix_command_list(entries, action_index),
        Recipe::Command {
            command: StringOrList::Empty,
            ..
        }
        | Recipe::Script { .. }
        | Recipe::Rule { .. } => Ok(()),
    }
}

/// Reject scalar command text that cannot occupy one Ninja command binding.
fn validate_scalar_command(command: &str) -> Result<(), NinjaGenError> {
    validate_ninja_value(command)
}

/// Reject POSIX command-list entries that violate the generated wrapper contract.
fn validate_posix_command_list(
    entries: &[String],
    action_index: usize,
) -> Result<(), NinjaGenError> {
    for (zero_based_entry_index, entry) in entries.iter().enumerate() {
        let entry_index = zero_based_entry_index + 1;
        match command_list_entry_error(CommandListEntry(entry)) {
            Some(CommandListEntryError::MultipleBackgroundJobs) => {
                return Err(NinjaGenError::MultipleBackgroundJobs {
                    action_index,
                    entry_index,
                });
            }
            Some(CommandListEntryError::UnsupportedExec) => {
                return Err(NinjaGenError::UnsupportedCommandListExec {
                    action_index,
                    entry_index,
                });
            }
            Some(CommandListEntryError::UnanalyzableEval) => {
                return Err(NinjaGenError::UnanalyzableCommandListEval {
                    action_index,
                    entry_index,
                });
            }
            Some(CommandListEntryError::NinjaControlCharacter) => {
                return Err(NinjaGenError::NinjaControlCharacter {
                    action_index,
                    entry_index,
                });
            }
            None => {}
        }
    }
    Ok(())
}
/// Reject metadata values that cannot remain within one Ninja binding.
pub(super) fn validate_action_metadata(action: &crate::ir::Action) -> Result<(), NinjaGenError> {
    for value in [
        action.description.as_ref(),
        action.depfile.as_ref(),
        action.deps_format.as_ref(),
        action.pool.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        validate_ninja_value(value)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Verifies shell-specific recipe validation boundaries.

    use super::{NinjaGenError, RecipeShell, validate_action_recipe, validate_console_scheduling};
    use crate::ast::{CommandIsolation, Recipe, StringOrList};
    use crate::ir::{Action, BuildEdge, BuildGraph, DependencyOrder};
    use crate::ninja_gen::{generate, generate_bundle};
    use camino::Utf8PathBuf;

    /// Construct one action with a supplied command recipe for validation tests.
    fn command_action(command: StringOrList) -> Action {
        Action {
            recipe: Recipe::Command {
                command,
                isolation: CommandIsolation::Shared,
            },
            description: None,
            depfile: None,
            deps_format: None,
            pool: None,
            restat: false,
        }
    }

    /// Build one console edge whose recipe and pool can vary across cases.
    fn console_graph(recipe: Recipe, pool: Option<&str>) -> BuildGraph {
        let mut graph = BuildGraph::default();
        graph.actions.insert(
            "console-action".into(),
            Action {
                recipe,
                description: None,
                depfile: None,
                deps_format: None,
                pool: pool.map(str::to_owned),
                restat: false,
            },
        );
        graph
            .insert_edge(BuildEdge {
                action_id: "console-action".into(),
                inputs: Vec::new(),
                implicit_deps: Vec::new(),
                dependency_order: DependencyOrder::Parallel,
                explicit_outputs: vec![Utf8PathBuf::from("console.out")],
                implicit_outputs: Vec::new(),
                order_only_deps: Vec::new(),
                progress_label: None,
                phony: false,
                always: false,
                console: true,
            })
            .expect("test graph output aliases must be unique");
        graph
    }

    /// Accept multi-line scalar text because PowerShell receives it through its transport.
    #[test]
    fn power_shell_accepts_a_multi_line_scalar_command() {
        let action = command_action(StringOrList::String(
            "Write-Output first\nWrite-Output second".into(),
        ));
        assert!(validate_action_recipe(&action, 1, RecipeShell::PowerShell).is_ok());
    }

    /// Bypass POSIX-only command-list restrictions for PowerShell command lists.
    #[test]
    fn power_shell_bypasses_posix_command_list_validation() {
        let action = command_action(StringOrList::List(vec!["echo one & echo two &".into()]));
        assert!(validate_action_recipe(&action, 1, RecipeShell::PowerShell).is_ok());
        assert!(matches!(
            validate_action_recipe(&action, 1, RecipeShell::Posix),
            Err(NinjaGenError::MultipleBackgroundJobs { .. })
        ));
    }

    /// Reject empty commands before applying the PowerShell validation bypass.
    #[test]
    fn power_shell_rejects_an_empty_command_recipe() {
        let action = command_action(StringOrList::Empty);
        assert!(matches!(
            validate_action_recipe(&action, 1, RecipeShell::PowerShell),
            Err(NinjaGenError::EmptyCommandRecipe { .. })
        ));
    }

    /// Reject programmatic isolated scalar and dependency-only recipes.
    #[rstest::rstest]
    #[case::scalar(StringOrList::String("echo hi".into()))]
    #[case::dependency_marker(StringOrList::Empty)]
    fn isolated_mode_requires_a_non_empty_command_list(#[case] command: StringOrList) {
        let action = Action {
            recipe: Recipe::Command {
                command,
                isolation: CommandIsolation::Isolated,
            },
            description: None,
            depfile: None,
            deps_format: None,
            pool: None,
            restat: false,
        };
        assert!(matches!(
            validate_action_recipe(&action, 1, RecipeShell::PowerShell),
            Err(NinjaGenError::InvalidCommandIsolation { action_index: 1 })
        ));
    }

    /// Reject console scheduling when the action is only a dependency marker.
    #[test]
    fn console_edges_reject_dependency_only_recipes() {
        let graph = console_graph(
            Recipe::Command {
                command: StringOrList::Empty,
                isolation: CommandIsolation::Shared,
            },
            None,
        );
        let error = validate_console_scheduling(&graph)
            .expect_err("dependency-only console edge must fail validation");
        assert!(error.to_string().contains("console.out"));
        assert!(matches!(
            error,
            NinjaGenError::ConsoleDependencyOnly { output, .. } if output == "console.out"
        ));
    }

    /// Reject custom action pools when the edge independently requests console scheduling.
    #[test]
    fn console_edges_reject_conflicting_action_pools() {
        let graph = console_graph(
            Recipe::Command {
                command: "echo ready".into(),
                isolation: CommandIsolation::Shared,
            },
            Some("link_pool"),
        );
        let error = validate_console_scheduling(&graph)
            .expect_err("custom pool conflict must fail validation");
        let rendered = error.to_string();
        assert!(rendered.contains("console.out"));
        assert!(rendered.contains("link_pool"));
        assert!(matches!(
            error,
            NinjaGenError::ConsolePoolConflict { output, pool, .. }
                if output == "console.out" && pool == "link_pool"
        ));
    }

    /// Accept an explicit `console` action pool when it matches the edge selection.
    #[test]
    fn console_edges_accept_matching_action_pools() {
        let graph = console_graph(
            Recipe::Command {
                command: "echo ready".into(),
                isolation: CommandIsolation::Shared,
            },
            Some("console"),
        );
        assert!(validate_console_scheduling(&graph).is_ok());
    }

    /// Return the same localized scheduling errors from direct and bundled generation.
    #[test]
    fn direct_and_bundle_generators_reject_invalid_console_scheduling() {
        let dependency_only = console_graph(
            Recipe::Command {
                command: StringOrList::Empty,
                isolation: CommandIsolation::Shared,
            },
            None,
        );
        let direct_error = generate(&dependency_only)
            .expect_err("direct generation must reject dependency-only console edges");
        assert!(direct_error.to_string().contains("console.out"));
        assert!(matches!(
            direct_error,
            NinjaGenError::ConsoleDependencyOnly { output, .. } if output == "console.out"
        ));
        let bundle_error = generate_bundle(&dependency_only)
            .expect_err("bundle generation must reject dependency-only console edges");
        assert!(bundle_error.to_string().contains("console.out"));
        assert!(matches!(
            bundle_error,
            NinjaGenError::ConsoleDependencyOnly { output, .. } if output == "console.out"
        ));

        let pool_conflict = console_graph(
            Recipe::Command {
                command: "echo ready".into(),
                isolation: CommandIsolation::Shared,
            },
            Some("link_pool"),
        );
        let direct_pool_error = generate(&pool_conflict)
            .expect_err("direct generation must reject console pool conflicts");
        assert!(direct_pool_error.to_string().contains("console.out"));
        assert!(direct_pool_error.to_string().contains("link_pool"));
        assert!(matches!(
            direct_pool_error,
            NinjaGenError::ConsolePoolConflict { output, pool, .. }
                if output == "console.out" && pool == "link_pool"
        ));
        let bundle_pool_error = generate_bundle(&pool_conflict)
            .expect_err("bundle generation must reject console pool conflicts");
        assert!(bundle_pool_error.to_string().contains("console.out"));
        assert!(bundle_pool_error.to_string().contains("link_pool"));
        assert!(matches!(
            bundle_pool_error,
            NinjaGenError::ConsolePoolConflict { output, pool, .. }
                if output == "console.out" && pool == "link_pool"
        ));
    }
}

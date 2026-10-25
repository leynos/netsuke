//! Ninja file generator.
//!
//! This module converts a [`crate::ir::BuildGraph`] into the textual
//! representation expected by the Ninja build system. The generator sorts
//! actions and edges to ensure deterministic output for snapshot tests. The
//! generated Ninja file is written by the runner and `generate` command for
//! downstream execution by the Ninja build system.

pub mod dyndep;
mod path_syntax;
mod rule_utils;

use dyndep::reject_reserved_paths;
pub use dyndep::{GeneratedDyndep, GeneratedNinja, generate_bundle};
pub(crate) use path_syntax::{reject_unsupported_path_characters, validated_ninja_path};
pub(crate) use rule_utils::{
    edge_requires_gates, graph_requires_dyndep, join, path_key, write_action_rules,
};

use crate::ast::{CommandIsolation, Recipe, StringOrList};
use crate::ir::BuildGraph;
use crate::localization::{self, keys};
use crate::recipe_shell::BashExecutable;
use std::fmt::Write;

pub(crate) mod command_list;
mod error;
mod explicit_shell;

mod escape;
mod recipe_shell;
mod validation;

pub use crate::recipe_shell::RecipeShell;
pub use error::NinjaGenError;
use escape::{ShellText, escape_metadata_value};
pub use explicit_shell::generate_with_shell;
use recipe_shell::escape_posix_script;
/// Write `key = value` to a Ninja file when `opt` holds a value.
///
/// The indented assignment is emitted only for present values, so optional
/// fields vanish from the generated file instead of being left blank.
macro_rules! write_kv {
    ($f:expr, $key:expr, $opt:expr) => {
        if let Some(val) = $opt {
            writeln!($f, "  {} = {}", $key, val)?;
        }
    };
}
/// Write `key = 1` to a Ninja file when `cond` is set.
///
/// Boolean flags are rendered as a `1` only when enabled, matching Ninja's
/// convention for switch-like variables.
macro_rules! write_flag {
    ($f:expr, $key:expr, $cond:expr) => {
        if $cond {
            writeln!($f, "  {} = 1", $key)?;
        }
    };
}

mod display_edge;
pub(crate) use display_edge::DisplayEdge;
mod progress_label;
/// Generate a Ninja build file as a string.
///
/// # Examples
/// ```
/// use netsuke::ast::{CommandIsolation, Recipe};
/// use netsuke::ir::{Action, BuildEdge, BuildGraph};
/// use camino::Utf8PathBuf;
/// let mut graph = BuildGraph::default();
/// graph.actions.insert("a".into(), Action {
///     recipe: Recipe::Command {
///         command: "true".into(),
///         isolation: CommandIsolation::Shared,
///     },
///     description: None, depfile: None, deps_format: None,
///     pool: None, restat: false
/// });
/// graph.insert_edge(BuildEdge {
///     action_id: "a".into(), inputs: Vec::new(),
///     implicit_deps: Vec::new(),
///     dependency_order: netsuke::ir::DependencyOrder::Parallel,
///     explicit_outputs: vec![Utf8PathBuf::from("out")],
///     implicit_outputs: Vec::new(), order_only_deps: Vec::new(),
///     progress_label: None, phony: false, always: false
/// }).expect("test graph output aliases must be unique");
/// # let result: Result<(), netsuke::ninja_gen::NinjaGenError> = (|| {
/// let text = netsuke::ninja_gen::generate(&graph)?;
/// assert!(text.contains("rule a"));
/// # Ok(())
/// # })();
/// # assert!(result.is_ok());
/// ```
///
/// # Errors
///
/// Returns [`NinjaGenError`] if a build edge references an unknown action, a
/// programmatic action has an empty command recipe, a command-list entry starts
/// multiple background jobs, a command-list entry uses an unsupported `exec`
/// structure, a command-list `eval` payload cannot be analysed, a command-list
/// entry contains a Ninja control character, isolated mode is attached to a
/// non-list command, a PowerShell recipe exceeds its size limit, an isolated
/// PowerShell entry exceeds Windows' command-line limit, a graph path uses
/// Netsuke's reserved serial-ordering namespace, or writing to the output fails.
pub fn generate(graph: &BuildGraph) -> Result<String, NinjaGenError> {
    let mut out = String::new();
    generate_into(graph, &mut out)?;
    Ok(out)
}

/// Write a Ninja build file to the provided writer.
///
/// # Examples
/// ```
/// use netsuke::ast::{CommandIsolation, Recipe};
/// use netsuke::ir::{Action, BuildEdge, BuildGraph};
/// use camino::Utf8PathBuf;
/// let mut graph = BuildGraph::default();
/// graph.actions.insert("a".into(), Action {
///     recipe: Recipe::Command {
///         command: "true".into(),
///         isolation: CommandIsolation::Shared,
///     },
///     description: None, depfile: None, deps_format: None,
///     pool: None, restat: false
/// });
/// graph.insert_edge(BuildEdge {
///     action_id: "a".into(), inputs: Vec::new(),
///     implicit_deps: Vec::new(),
///     dependency_order: netsuke::ir::DependencyOrder::Parallel,
///     explicit_outputs: vec![Utf8PathBuf::from("out")],
///     implicit_outputs: Vec::new(), order_only_deps: Vec::new(),
///     progress_label: None, phony: false, always: false
/// }).expect("test graph output aliases must be unique");
/// let mut out = String::new();
/// # let result: Result<(), netsuke::ninja_gen::NinjaGenError> = (|| {
/// netsuke::ninja_gen::generate_into(&graph, &mut out)?;
/// assert!(out.contains("build out: a"));
/// # Ok(())
/// # })();
/// # assert!(result.is_ok());
/// ```
///
/// # Errors
///
/// Returns [`NinjaGenError`] if a build edge references an unknown action, a
/// programmatic action has an empty command recipe, a command-list entry starts
/// multiple background jobs, a command-list entry uses an unsupported `exec`
/// structure, a command-list `eval` payload cannot be analysed, a command-list
/// entry contains a Ninja control character, isolated mode is attached to a
/// non-list command, a PowerShell recipe exceeds its size limit, an isolated
/// PowerShell entry exceeds Windows' command-line limit, a graph path uses
/// Netsuke's reserved serial-ordering namespace, or writing to the output fails.
pub fn generate_into<W: Write>(graph: &BuildGraph, out: &mut W) -> Result<(), NinjaGenError> {
    generate_into_with_shell(graph, out, RecipeShell::host_default())
}

/// Write a Ninja build file for one explicit legacy recipe interpreter.
///
/// # Errors
///
/// Returns [`NinjaGenError`] when the graph cannot be represented safely or
/// writing the generated output fails.
pub(crate) fn generate_into_with_shell<W: Write>(
    graph: &BuildGraph,
    out: &mut W,
    shell: RecipeShell,
) -> Result<(), NinjaGenError> {
    let localizer = localization::localizer();
    reject_unsupported_path_characters(graph)?;
    reject_reserved_paths(graph)?;
    if graph_requires_dyndep(graph) {
        return Err(NinjaGenError::DyndepFilesRequired {
            message: localization::message(keys::NINJA_GEN_DYNDEP_FILES_REQUIRED),
        });
    }
    write_action_rules(graph, out, shell, None)?;
    let mut edges: Vec<_> = graph.edges().collect();
    edges.sort_by_key(|a| path_key(&a.explicit_outputs));
    for edge in edges {
        let action =
            graph
                .actions
                .get(&edge.action_id)
                .ok_or_else(|| NinjaGenError::MissingAction {
                    id: edge.action_id.clone(),
                    message: localization::message(keys::NINJA_GEN_MISSING_ACTION)
                        .with_arg("id", &edge.action_id),
                })?;
        let localized_description = progress_label::for_edge(edge, action, localizer.as_ref());
        let ninja_description = escape_metadata_value(localized_description.as_deref())?;
        write!(
            out,
            "{}",
            DisplayEdge {
                edge,
                action_name: if action.recipe.is_dependency_only() {
                    "phony"
                } else {
                    &edge.action_id
                },
                action_restat: action.restat,
                implicit_deps: &edge.implicit_deps,
                description: ninja_description.as_ref(),
            }
        )?;
    }

    if !graph.default_targets.is_empty() {
        let mut defs = graph.default_targets.clone();
        defs.sort();
        writeln!(out, "default {}", join(&defs))?;
    }

    Ok(())
}

/// Wrapper struct to display a rule with its identifier.
pub(crate) struct NamedAction<'a> {
    /// The rule identifier bound to the action.
    id: &'a str,
    /// The IR action whose recipe and metadata are rendered.
    action: &'a crate::ir::Action,
    /// The explicit interpreter receiving this action's legacy recipe text.
    shell: RecipeShell,
}

impl NamedAction<'_> {
    /// Write the action's optional metadata bindings followed by a blank line.
    ///
    /// # Errors
    ///
    /// Returns [`NinjaGenError`] when a metadata binding cannot be escaped or
    /// the output writer fails.
    fn write_metadata<W: Write>(&self, f: &mut W) -> Result<(), NinjaGenError> {
        let description = escape_metadata_value(self.action.description.as_deref())?;
        let depfile = escape_metadata_value(self.action.depfile.as_deref())?;
        let deps_format = escape_metadata_value(self.action.deps_format.as_deref())?;
        let pool = escape_metadata_value(self.action.pool.as_deref())?;
        write_kv!(f, "description", &description);
        write_kv!(f, "depfile", &depfile);
        write_kv!(f, "deps", &deps_format);
        write_kv!(f, "pool", &pool);
        write_flag!(f, "restat", self.action.restat);
        writeln!(f)?;
        Ok(())
    }

    /// Panic in debug builds when `command` is not POSIX-shell parseable.
    fn assert_shell_command(command: &str) {
        // `shlex::split` approximates POSIX shell parsing; keep this debug-only
        // sanity guard to catch obviously malformed commands during development.
        debug_assert!(
            shlex::split(command).is_some(),
            "invalid command: {command}"
        );
    }

    /// Reject a recipe that references another rule.
    #[cold]
    #[expect(
        clippy::panic_in_result_fn,
        reason = "debug builds intentionally panic to expose rule recursion"
    )]
    #[expect(
        clippy::manual_assert,
        reason = "debug-only guard escalates to panic for visibility"
    )]
    fn reject_rule_recipe() -> Result<ShellText, NinjaGenError> {
        if cfg!(debug_assertions) {
            panic!("rules do not reference other rules");
        }
        Err(NinjaGenError::UnsafeNinjaValue)
    }

    /// Reject a command recipe that carries no entries.
    ///
    /// Deserialization rejects empty command recipes, so reaching here means an
    /// earlier stage constructed one directly. `Display::to_string` turns the
    /// returned error into a panic, so the fault still surfaces loudly without
    /// a hand-rolled debug-only panic.
    #[cold]
    const fn reject_empty_command_recipe() -> Result<ShellText, NinjaGenError> {
        Err(NinjaGenError::UnsafeNinjaValue)
    }

    /// Converts the action recipe into shell text before Ninja escaping.
    fn shell_text(&self) -> Result<ShellText, NinjaGenError> {
        let command = match &self.action.recipe {
            Recipe::Command {
                command: StringOrList::String(scalar_command),
                ..
            } => {
                if self.shell != RecipeShell::PowerShell {
                    Self::assert_shell_command(scalar_command);
                }
                ShellText::new(scalar_command.clone())
            }
            Recipe::Command {
                command: StringOrList::List(items),
                isolation,
            } => command_list::command_list_shell_text(self.id, items, *isolation, self.shell)?,
            Recipe::Command {
                command: StringOrList::Empty,
                ..
            } => return Self::reject_empty_command_recipe(),
            Recipe::Script { script } => self.script_shell_text(script),
            Recipe::Rule { .. } => return Self::reject_rule_recipe(),
        };
        Ok(command)
    }

    /// Wraps a multi-line script in a one-line shell command for Ninja.
    fn script_shell_text(&self, script: &str) -> ShellText {
        if self.shell == RecipeShell::PowerShell {
            return ShellText::new(script.to_owned());
        }
        // Ninja commands must be single-line. Encode newlines and reconstruct the
        // original script with `printf %b` piped into a fresh shell to preserve
        // expected expansions.
        let escaped = escape_posix_script(script);
        let cmd = format!("/bin/sh -e -c \"printf %b '{escaped}' | /bin/sh -e\"");
        // Scripts are allowed to contain shell constructs such as heredocs and
        // comments that `shlex` cannot model, so only command recipes use the
        // debug parser guard.
        ShellText::new(cmd)
    }
    /// Writes this action's Ninja rule, escaping only the shell-text boundary.
    fn write_into<W: Write>(&self, output: &mut W) -> Result<(), NinjaGenError> {
        self.write_into_with_bash(output, None)
    }

    /// Write one action using the optional runner-selected Bash executable.
    ///
    /// # Errors
    ///
    /// Returns [`NinjaGenError`] when the recipe, metadata, or output cannot be
    /// rendered safely.
    fn write_into_with_bash<W: Write>(
        &self,
        output: &mut W,
        bash_executable: Option<&BashExecutable>,
    ) -> Result<(), NinjaGenError> {
        let shell_text = self.shell_text()?;
        let command = match (&self.action.recipe, self.shell) {
            (
                Recipe::Command {
                    command: StringOrList::List(_),
                    isolation: CommandIsolation::Isolated,
                },
                RecipeShell::PowerShell,
            ) => self.shell.isolated_command_list_value(&shell_text)?,
            _ => self
                .shell
                .command_value_with_bash(&shell_text, bash_executable)?,
        };
        writeln!(output, "rule {}", self.id)?;
        writeln!(output, "  command = {}", command.command())?;
        if let Some(content) = command.response_file_content() {
            writeln!(output, "  rspfile = $out.netsuke-{}.ps1", self.id)?;
            writeln!(output, "  rspfile_content = {content}")?;
        }
        self.write_metadata(output)
    }
}
#[cfg(test)]
mod property_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

//! Ninja file generator.
//!
//! This module converts a [`crate::ir::BuildGraph`] into the textual
//! representation expected by the Ninja build system. The generator sorts
//! actions and edges to ensure deterministic output for snapshot tests. The
//! generated Ninja file is written by the runner and `generate` command for
//! downstream execution by the Ninja build system.

pub mod dyndep;
mod path_syntax;

use dyndep::reject_reserved_paths;
pub use dyndep::{GeneratedDyndep, GeneratedNinja, generate_bundle};
pub(crate) use path_syntax::{reject_unsupported_path_characters, validated_ninja_path};

use crate::ast::{Recipe, StringOrList};
use crate::ir::{BuildEdge, BuildGraph};
use crate::localization::{self, keys};
use camino::Utf8PathBuf;
use itertools::Itertools;
use std::collections::HashSet;
use std::fmt::Write;

mod explicit_shell;
#[path = "../ninja_gen_command_list.rs"]
pub(crate) mod ninja_gen_command_list;
#[path = "../ninja_gen_error.rs"]
mod ninja_gen_error;

#[path = "../ninja_gen_escape.rs"]
mod ninja_gen_escape;
#[path = "../ninja_gen_recipe_shell.rs"]
mod ninja_gen_recipe_shell;
#[path = "../ninja_gen_validation.rs"]
mod ninja_gen_validation;

pub use crate::recipe_shell::RecipeShell;
pub use explicit_shell::generate_with_shell;
use ninja_gen_command_list::{ActionId, CommandListEntry, command_list_entry};
pub use ninja_gen_error::NinjaGenError;
use ninja_gen_escape::{ShellText, escape_metadata_value};
use ninja_gen_validation::{validate_action_metadata, validate_action_recipe};
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
/// Generate a Ninja build file as a string.
///
/// # Examples
/// ```
/// use netsuke::ast::Recipe;
/// use netsuke::ir::{Action, BuildEdge, BuildGraph};
/// use camino::Utf8PathBuf;
/// let mut graph = BuildGraph::default();
/// graph.actions.insert("a".into(), Action {
///     recipe: Recipe::Command { command: "true".into() },
///     description: None, depfile: None, deps_format: None,
///     pool: None, restat: false
/// });
/// graph.targets.insert(Utf8PathBuf::from("out"), BuildEdge {
///     action_id: "a".into(), inputs: Vec::new(),
///     implicit_deps: Vec::new(),
///     dependency_order: netsuke::ir::DependencyOrder::Parallel,
///     explicit_outputs: vec![Utf8PathBuf::from("out")],
///     implicit_outputs: Vec::new(), order_only_deps: Vec::new(),
///     phony: false, always: false
/// });
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
/// entry contains a Ninja control character, a graph path uses Netsuke's
/// reserved serial-ordering namespace, or writing to the output fails.
pub fn generate(graph: &BuildGraph) -> Result<String, NinjaGenError> {
    let mut out = String::new();
    generate_into(graph, &mut out)?;
    Ok(out)
}

/// Write a Ninja build file to the provided writer.
///
/// # Examples
/// ```
/// use netsuke::ast::Recipe;
/// use netsuke::ir::{Action, BuildEdge, BuildGraph};
/// use camino::Utf8PathBuf;
/// let mut graph = BuildGraph::default();
/// graph.actions.insert("a".into(), Action {
///     recipe: Recipe::Command { command: "true".into() },
///     description: None, depfile: None, deps_format: None,
///     pool: None, restat: false
/// });
/// graph.targets.insert(Utf8PathBuf::from("out"), BuildEdge {
///     action_id: "a".into(), inputs: Vec::new(),
///     implicit_deps: Vec::new(),
///     dependency_order: netsuke::ir::DependencyOrder::Parallel,
///     explicit_outputs: vec![Utf8PathBuf::from("out")],
///     implicit_outputs: Vec::new(), order_only_deps: Vec::new(),
///     phony: false, always: false
/// });
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
/// entry contains a Ninja control character, a graph path uses Netsuke's
/// reserved serial-ordering namespace, or writing to the output fails.
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
    reject_unsupported_path_characters(graph)?;
    reject_reserved_paths(graph)?;
    if graph_requires_dyndep(graph) {
        return Err(NinjaGenError::DyndepFilesRequired {
            message: localization::message(keys::NINJA_GEN_DYNDEP_FILES_REQUIRED),
        });
    }
    write_action_rules(graph, out, shell)?;
    let mut edges: Vec<_> = graph.targets.values().collect();
    edges.sort_by_key(|a| path_key(&a.explicit_outputs));
    let mut seen = HashSet::new();
    for edge in edges {
        let key = path_key(&edge.explicit_outputs);
        if !seen.insert(key.clone()) {
            continue;
        }
        let action =
            graph
                .actions
                .get(&edge.action_id)
                .ok_or_else(|| NinjaGenError::MissingAction {
                    id: edge.action_id.clone(),
                    message: localization::message(keys::NINJA_GEN_MISSING_ACTION)
                        .with_arg("id", &edge.action_id),
                })?;
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

/// Write executable rules in stable action-ID order, omitting dependency-only `phony` nodes.
pub(crate) fn write_action_rules<W: Write>(
    graph: &BuildGraph,
    out: &mut W,
    shell: RecipeShell,
) -> Result<(), NinjaGenError> {
    let mut actions: Vec<_> = graph.actions.iter().collect();
    actions.sort_by_key(|(id, _)| *id);
    for (zero_based_action_index, (id, action)) in actions.into_iter().enumerate() {
        if action.recipe.is_dependency_only() {
            continue;
        }
        validate_action_recipe(action, zero_based_action_index + 1, shell)?;
        validate_action_metadata(action)?;
        NamedAction { id, action, shell }.write_into(out)?;
    }
    Ok(())
}
/// Convert a slice of paths into a space-separated string.
pub(crate) fn join(paths: &[Utf8PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path_syntax::clone_validated_ninja_path(path.as_str()))
        .join(" ")
}

/// Generate a stable key for a list of paths.
pub(crate) fn path_key(paths: &[Utf8PathBuf]) -> String {
    let mut parts: Vec<String> = paths.iter().map(|p| p.as_str().to_owned()).collect();
    parts.sort_unstable();
    parts.join(&char::from(0).to_string())
}

/// Escape a script for the wrapper's single-quoted `printf %b` argument.
///
/// Preserve the script through the double-quoted `sh -c` wrapper so its inner
/// shell receives literal text, including line breaks and apostrophes, before
/// it evaluates intentional shell variables.
fn escape_script(script: &str) -> String {
    script
        .replace('\\', "\\\\")
        .replace('$', "\\$")
        .replace('"', "\\\"")
        .replace('`', "\\`")
        .replace('\'', r"'\''")
        .replace('\n', "\\n")
}
/// Whether the graph contains an edge whose serial list needs dyndep gates.
pub(crate) fn graph_requires_dyndep(graph: &BuildGraph) -> bool {
    graph.targets.values().any(edge_requires_gates)
}

/// Whether one edge's serial dependency list needs staged dyndep gates.
pub(crate) fn edge_requires_gates(edge: &BuildEdge) -> bool {
    edge.dependency_order == crate::ir::DependencyOrder::Serial && edge.implicit_deps.len() > 1
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
            } => {
                if self.shell != RecipeShell::PowerShell {
                    Self::assert_shell_command(scalar_command);
                }
                ShellText::new(scalar_command.clone())
            }
            Recipe::Command {
                command: StringOrList::List(items),
            } => self.command_list_shell_text(items),
            Recipe::Command {
                command: StringOrList::Empty,
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
        let escaped = escape_script(script);
        let cmd = format!("/bin/sh -e -c \"printf %b '{escaped}' | /bin/sh -e\"");
        // Scripts are allowed to contain shell constructs such as heredocs and
        // comments that `shlex` cannot model, so only command recipes use the
        // debug parser guard.
        ShellText::new(cmd)
    }
    /// Write list entries as isolated current-shell groups joined by `&&`.
    fn command_list_shell_text(&self, items: &[String]) -> ShellText {
        if let Some(script) = self.shell.command_list_script(items) {
            return ShellText::new(script);
        }
        // Brace groups keep each entry a distinct shell unit, and `eval`
        // prevents comments or trailing control operators inside an entry
        // consuming its terminator. Braces run in the current shell (unlike
        // `( ... )`), so working directory, environment, and variables set by
        // one entry still carry into the next, and the `&&` chain stays
        // fail-fast.
        let command_line = items
            .iter()
            .enumerate()
            .map(|(entry_index, item)| {
                command_list_entry(CommandListEntry(item), ActionId(self.id), entry_index + 1)
            })
            .join(" && ");
        Self::assert_shell_command(&command_line);
        ShellText::new(command_line)
    }

    /// Writes this action's Ninja rule, escaping only the shell-text boundary.
    fn write_into<W: Write>(&self, output: &mut W) -> Result<(), NinjaGenError> {
        let command = self.shell.command_value(&self.shell_text()?)?;
        writeln!(output, "rule {}", self.id)?;
        writeln!(output, "  command = {}", command.command())?;
        if let Some(content) = command.response_file_content() {
            writeln!(output, "  rspfile = $out.netsuke-{}.rsp", self.id)?;
            writeln!(output, "  rspfile_content = {content}")?;
        }
        self.write_metadata(output)
    }
}
#[cfg(test)]
#[path = "../ninja_gen_property_tests.rs"]
mod property_tests;
#[cfg(test)]
#[path = "../ninja_gen_test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "../ninja_gen_tests.rs"]
mod tests;

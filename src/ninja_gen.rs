//! Ninja file generator.
//!
//! This module converts a [`crate::ir::BuildGraph`] into the textual
//! representation expected by the Ninja build system. The generator sorts
//! actions and edges to ensure deterministic output for snapshot tests. The
//! generated Ninja file is written by the runner and `generate` command for
//! downstream execution by the Ninja build system.

use crate::ast::{Recipe, StringOrList};
use crate::ir::{BuildEdge, BuildGraph};
use crate::localization::{self, LocalizedMessage, keys};
use camino::Utf8PathBuf;
use itertools::Itertools;
use std::collections::HashSet;
use std::fmt::{self, Display, Formatter, Write};
use thiserror::Error;

#[path = "ninja_gen_command_list.rs"]
pub(crate) mod ninja_gen_command_list;
#[path = "ninja_gen_validation.rs"]
mod ninja_gen_validation;

use ninja_gen_command_list::{ActionId, CommandListEntry, command_list_entry};
use ninja_gen_validation::validate_action_recipe;
/// Errors produced while rendering Ninja manifests.
#[derive(Debug, Error)]
pub enum NinjaGenError {
    /// The build graph referenced an action that was not defined.
    #[error("{message}")]
    MissingAction {
        /// Identifier of the missing action referenced by a build edge.
        id: String,
        /// Localized error message.
        message: LocalizedMessage,
    },
    /// An action built outside manifest deserialization has no command entries.
    #[error("command-list action {action_index} has no command entries")]
    EmptyCommandRecipe {
        /// One-based stable position in generated action order.
        action_index: usize,
    },
    /// A list entry starts more than one background job, which cannot be
    /// attributed reliably by a shared POSIX shell.
    #[error(
        "command-list action {action_index}, entry {entry_index} starts multiple background jobs"
    )]
    MultipleBackgroundJobs {
        /// One-based stable position in generated action order.
        action_index: usize,
        /// One-based stable position in the command list.
        entry_index: usize,
    },
    /// A list entry uses `exec` in a shell structure the wrapper cannot
    /// supervise without changing its semantics.
    #[error(
        "command-list action {action_index}, entry {entry_index} has unsupported exec structure"
    )]
    UnsupportedCommandListExec {
        /// One-based stable position in generated action order.
        action_index: usize,
        /// One-based stable position in the command list.
        entry_index: usize,
    },
    /// Formatting the Ninja output failed.
    #[error("{message}")]
    Format {
        /// Underlying formatting error.
        #[source]
        source: fmt::Error,
        /// Localized error message.
        message: LocalizedMessage,
    },
}

impl From<fmt::Error> for NinjaGenError {
    fn from(source: fmt::Error) -> Self {
        Self::Format {
            message: localization::message(keys::NINJA_GEN_FORMAT),
            source,
        }
    }
}
macro_rules! write_kv {
    ($f:expr, $key:expr, $opt:expr) => {
        if let Some(val) = $opt {
            writeln!($f, "  {} = {}", $key, val)?;
        }
    };
}

macro_rules! write_flag {
    ($f:expr, $key:expr, $cond:expr) => {
        if $cond {
            writeln!($f, "  {} = 1", $key)?;
        }
    };
}

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
/// programmatic action has an empty command recipe, or writing to the output
/// fails.
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
/// programmatic action has an empty command recipe, or writing to the output
/// fails.
pub fn generate_into<W: Write>(graph: &BuildGraph, out: &mut W) -> Result<(), NinjaGenError> {
    let mut actions: Vec<_> = graph.actions.iter().collect();
    actions.sort_by_key(|(id, _)| *id);
    for (zero_based_action_index, (id, action)) in actions.into_iter().enumerate() {
        let action_index = zero_based_action_index + 1;
        validate_action_recipe(action, action_index)?;
        write!(out, "{}", NamedAction { id, action })?;
    }

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
                action_restat: action.restat,
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

/// Convert a slice of paths into a space-separated string.
fn join(paths: &[Utf8PathBuf]) -> String {
    paths.iter().map(|p| p.as_str()).join(" ")
}

/// Generate a stable key for a list of paths.
fn path_key(paths: &[Utf8PathBuf]) -> String {
    let mut parts: Vec<String> = paths.iter().map(|p| p.as_str().to_owned()).collect();
    parts.sort_unstable();
    let separator = char::from(0).to_string();
    parts.join(&separator)
}

/// Escape a script for embedding within a single-quoted `printf %b` argument.
///
/// Backslashes, dollar signs, double quotes, backticks, and single quotes are
/// escaped so the outer shell preserves them, while newlines become `\n` to
/// keep the rule on one line. Percent signs are passed through unchanged because
/// the script is an argument rather than a format string, allowing the inner
/// shell to perform variable expansion.
fn escape_script(script: &str) -> String {
    script
        .replace('\\', "\\\\")
        .replace('$', "\\$")
        .replace('"', "\\\"")
        .replace('`', "\\`")
        .replace('\'', "'\"'\"'")
        .replace('\n', "\\n")
}

/// Wrapper struct to display a rule with its identifier.
struct NamedAction<'a> {
    id: &'a str,
    action: &'a crate::ir::Action,
}

impl NamedAction<'_> {
    fn write_recipe(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.action.recipe {
            Recipe::Command {
                command: StringOrList::String(scalar_command),
            } => {
                Self::assert_shell_command(scalar_command);
                writeln!(f, "  command = {scalar_command}")
            }
            Recipe::Command {
                command: StringOrList::List(items),
            } => {
                let command_line =
                    // Brace groups keep each entry a distinct shell unit, and
                    // `eval` prevents comments or trailing control operators
                    // inside an entry consuming its terminator. Braces run in
                    // the current shell (unlike `( ... )`), so working
                    // directory, environment, and variables set by one entry
                    // still carry into the next, and the `&&` chain stays
                    // fail-fast.
                    items.iter()
                        .enumerate()
                        .map(|(entry_index, item)| {
                            command_list_entry(
                                CommandListEntry(item),
                                ActionId(self.id),
                                entry_index + 1,
                            )
                        })
                        .join(" && ");
                Self::assert_shell_command(&command_line);
                writeln!(f, "  command = {command_line}")
            }
            Recipe::Command {
                command: StringOrList::Empty,
            } => Self::reject_empty_command_recipe(),
            Recipe::Script { script } => Self::write_script_command(f, script),
            Recipe::Rule { .. } => Self::reject_rule_recipe(),
        }
    }

    fn write_script_command(f: &mut Formatter<'_>, script: &str) -> fmt::Result {
        // Ninja commands must be single-line. Encode newlines and reconstruct the
        // original script with `printf %b` piped into a fresh shell to preserve
        // expected expansions.
        let escaped = escape_script(script);
        let cmd = format!("/bin/sh -e -c \"printf %b '{escaped}' | /bin/sh -e\"");
        Self::assert_shell_command(&cmd);
        writeln!(f, "  command = {cmd}")
    }

    fn write_metadata(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write_kv!(f, "description", &self.action.description);
        write_kv!(f, "depfile", &self.action.depfile);
        write_kv!(f, "deps", &self.action.deps_format);
        write_kv!(f, "pool", &self.action.pool);
        write_flag!(f, "restat", self.action.restat);
        writeln!(f)
    }

    fn assert_shell_command(command: &str) {
        // `shlex::split` approximates POSIX shell parsing; keep this debug-only
        // sanity guard to catch obviously malformed commands during development.
        debug_assert!(
            shlex::split(command).is_some(),
            "invalid command: {command}"
        );
    }

    #[cold]
    #[expect(
        clippy::panic_in_result_fn,
        reason = "debug builds intentionally panic to expose rule recursion"
    )]
    #[expect(
        clippy::manual_assert,
        reason = "debug-only guard escalates to panic for visibility"
    )]
    fn reject_rule_recipe() -> fmt::Result {
        if cfg!(debug_assertions) {
            panic!("rules do not reference other rules");
        }
        Err(fmt::Error)
    }

    /// Reject a command recipe that carries no entries.
    ///
    /// Deserialization rejects empty command recipes, so reaching here means an
    /// earlier stage constructed one directly. `Display::to_string` turns the
    /// returned error into a panic, so the fault still surfaces loudly without
    /// a hand-rolled debug-only panic.
    #[cold]
    const fn reject_empty_command_recipe() -> fmt::Result {
        Err(fmt::Error)
    }
}

impl Display for NamedAction<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "rule {}", self.id)?;
        self.write_recipe(f)?;
        self.write_metadata(f)
    }
}
/// Wrapper struct to display a build edge.
struct DisplayEdge<'a> {
    edge: &'a BuildEdge,
    action_restat: bool,
}

impl Display for DisplayEdge<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "build {}", join(&self.edge.explicit_outputs))?;
        if !self.edge.implicit_outputs.is_empty() {
            write!(f, " | {}", join(&self.edge.implicit_outputs))?;
        }
        write!(f, ": {}", self.edge.action_id)?;
        if !self.edge.inputs.is_empty() {
            write!(f, " {}", join(&self.edge.inputs))?;
        }
        if !self.edge.implicit_deps.is_empty() {
            write!(f, " | {}", join(&self.edge.implicit_deps))?;
        }
        if !self.edge.order_only_deps.is_empty() {
            write!(f, " || {}", join(&self.edge.order_only_deps))?;
        }
        writeln!(f)?;
        write_flag!(f, "restat", self.edge.always && !self.action_restat);
        writeln!(f)
    }
}
#[cfg(test)]
#[path = "ninja_gen_property_tests.rs"]
mod property_tests;
#[cfg(test)]
#[path = "ninja_gen_tests.rs"]
mod tests;

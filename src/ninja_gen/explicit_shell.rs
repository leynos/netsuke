//! Generates Ninja text with a caller-selected legacy recipe interpreter.

use crate::ir::BuildGraph;
use crate::recipe_shell::InterpreterExecutable;

use super::{NinjaGenError, RecipeShell, generate_into_with_interpreter, generate_into_with_shell};

/// Generate a Ninja build file for one explicit legacy recipe interpreter.
///
/// The Netsuke CLI resolves the interpreter from its host contract before
/// generation. Library consumers can select an explicit renderer when they
/// need deterministic cross-platform Ninja text.
///
/// # Errors
///
/// Returns [`NinjaGenError`] when the graph cannot be represented safely for
/// the selected interpreter or writing the output fails.
pub fn generate_with_shell(
    graph: &BuildGraph,
    shell: RecipeShell,
) -> Result<String, NinjaGenError> {
    let mut out = String::new();
    generate_into_with_shell(graph, &mut out, shell)?;
    Ok(out)
}

/// Generate a Ninja build file bound to one trusted interpreter executable.
///
/// The binding is used verbatim for `PowerShell` and `Bash` recipes. A
/// [`RecipeShell::Posix`] recipe ignores it because Ninja launches the host
/// shell itself.
///
/// # Examples
///
/// ```
/// use netsuke::ast::{CommandIsolation, Recipe};
/// use netsuke::ir::{Action, BuildEdge, BuildGraph};
/// use netsuke::ninja_gen::{
///     InterpreterExecutable, RecipeShell, generate_with_interpreter,
/// };
/// use camino::Utf8PathBuf;
///
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
/// let executable_path = if cfg!(windows) {
///     "C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe"
/// } else {
///     "/bin/sh"
/// };
/// let executable = InterpreterExecutable::from_absolute_path(executable_path.into())
///     .expect("fixture path should be absolute");
/// let ninja = generate_with_interpreter(&graph, RecipeShell::Posix, &executable)
///     .expect("POSIX rendering should ignore the binding");
/// assert!(ninja.contains("command = true"));
/// ```
///
/// # Errors
///
/// Returns [`NinjaGenError`] when the graph cannot be represented safely for
/// the selected interpreter or writing the output fails.
pub fn generate_with_interpreter(
    graph: &BuildGraph,
    shell: RecipeShell,
    interpreter: &InterpreterExecutable,
) -> Result<String, NinjaGenError> {
    let mut out = String::new();
    generate_into_with_interpreter(graph, &mut out, shell, interpreter)?;
    Ok(out)
}

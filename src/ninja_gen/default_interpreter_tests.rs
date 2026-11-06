//! Verifies default interpreter resolution and the public generation entry points.
//!
//! The library entry points resolve a Windows PowerShell binding themselves and
//! bind nothing for other shells. These tests pin that boundary: a locator
//! failure is reported rather than silently downgraded to an unbound render, a
//! dependency-only graph never consults the locator, and an unbound non-default
//! shell still refuses to render a command.
//!
//! The rendered form of an arbitrary absolute interpreter path is covered by
//! the sibling `default_interpreter_properties` module.

use super::{
    NinjaGenError, RecipeShell, generate_with_shell, resolve_default_interpreter_with,
    test_support::{command_action, power_shell_interpreter},
};
use crate::ast::StringOrList;
use crate::ir::{BuildEdge, BuildGraph, DependencyOrder};
use crate::power_shell_location::WindowsPowerShellLocationError;
use camino::Utf8PathBuf;
use rstest::rstest;

/// Build one command action graph that renders an interpreter command.
fn command_graph() -> BuildGraph {
    let mut graph = BuildGraph::default();
    graph
        .actions
        .insert("run".into(), command_action("echo hi".into()));
    graph
        .insert_edge(BuildEdge {
            action_id: "run".into(),
            inputs: Vec::new(),
            implicit_deps: Vec::new(),
            dependency_order: DependencyOrder::Parallel,
            explicit_outputs: vec![Utf8PathBuf::from("out")],
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            progress_label: None,
            phony: false,
            always: false,
        })
        .expect("fixture outputs are unique");
    graph
}

/// Build one dependency-only graph that renders no interpreter command.
fn dependency_only_graph() -> BuildGraph {
    let mut graph = BuildGraph::default();
    graph
        .actions
        .insert("group".into(), command_action(StringOrList::Empty));
    graph
        .insert_edge(BuildEdge {
            action_id: "group".into(),
            inputs: Vec::new(),
            implicit_deps: Vec::new(),
            dependency_order: DependencyOrder::Parallel,
            explicit_outputs: vec![Utf8PathBuf::from("group")],
            implicit_outputs: Vec::new(),
            order_only_deps: Vec::new(),
            progress_label: None,
            phony: true,
            always: false,
        })
        .expect("fixture outputs are unique");
    graph
}

/// A locator failure on a rendered recipe is reported, not silently unbound.
#[test]
fn default_resolution_reports_a_locator_failure() {
    let result =
        resolve_default_interpreter_with(&command_graph(), RecipeShell::PowerShell, true, || {
            Err(WindowsPowerShellLocationError::SystemDirectoryUnavailable)
        });
    let error = result.expect_err("a failed locator must not be swallowed");
    assert!(matches!(
        error,
        NinjaGenError::InterpreterResolutionFailed { .. }
    ));
    assert!(
        error.to_string().contains("system directory"),
        "the diagnostic should name the locator failure: {error}"
    );
}

/// A dependency-only graph renders without consulting the locator.
#[test]
fn default_resolution_skips_dependency_only_graphs() {
    let result = resolve_default_interpreter_with(
        &dependency_only_graph(),
        RecipeShell::PowerShell,
        true,
        || panic!("a dependency-only graph must not consult the locator"),
    );
    assert!(matches!(result, Ok(None)));
}

/// A non-Windows host never consults the Windows locator.
#[test]
fn default_resolution_skips_non_windows_hosts() {
    let result =
        resolve_default_interpreter_with(&command_graph(), RecipeShell::PowerShell, false, || {
            panic!("a non-Windows host must not consult the locator")
        });
    assert!(matches!(result, Ok(None)));
}

/// Bash keeps its runner-owned resolution policy and binds nothing here.
#[test]
fn default_resolution_skips_bash() {
    let result =
        resolve_default_interpreter_with(&command_graph(), RecipeShell::Bash, true, || {
            panic!("Bash must not consult the Windows PowerShell locator")
        });
    assert!(matches!(result, Ok(None)));
}

/// A successful lookup binds the located executable for rendering.
#[test]
fn default_resolution_binds_the_located_executable() {
    let expected = power_shell_interpreter();
    let result =
        resolve_default_interpreter_with(&command_graph(), RecipeShell::PowerShell, true, || {
            Ok(power_shell_interpreter())
        });
    let resolved = result
        .expect("a successful lookup should resolve")
        .expect("a rendered recipe should carry a binding");
    assert_eq!(resolved.as_path(), expected.as_path());
}

/// A non-default shell entry point refuses to render without a binding.
#[test]
fn generate_with_shell_requires_a_binding_for_bash() {
    let error = generate_with_shell(&command_graph(), RecipeShell::Bash)
        .expect_err("Bash rendering needs an explicit binding");
    assert!(matches!(
        error,
        NinjaGenError::InterpreterBindingRequired {
            shell: RecipeShell::Bash
        }
    ));
}

/// Off Windows, PowerShell rendering also needs an explicit binding.
#[cfg(not(windows))]
#[test]
fn generate_with_shell_requires_a_binding_for_power_shell_off_windows() {
    let error = generate_with_shell(&command_graph(), RecipeShell::PowerShell)
        .expect_err("PowerShell rendering needs an explicit binding off Windows");
    assert!(matches!(
        error,
        NinjaGenError::InterpreterBindingRequired {
            shell: RecipeShell::PowerShell
        }
    ));
}

/// A dependency-only graph renders through the default entry point unbound.
#[rstest]
#[case::bash(RecipeShell::Bash)]
#[case::power_shell(RecipeShell::PowerShell)]
fn generate_with_shell_renders_dependency_only_graphs(#[case] shell: RecipeShell) {
    let ninja = generate_with_shell(&dependency_only_graph(), shell)
        .expect("a dependency-only graph needs no interpreter binding");
    assert!(
        !ninja.contains("powershell.exe"),
        "a dependency-only graph must not name an interpreter: {ninja}"
    );
}

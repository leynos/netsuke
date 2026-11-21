//! Prepare the output directories an executable `phony` recipe writes into.
//!
//! Ninja creates the parent directories of an edge's outputs immediately before
//! it runs that edge. An executable `phony` edge's statement names only its
//! hidden output, so Ninja prepares `.netsuke/phony` and never the directories
//! the recipe writes into under its declared names. Each distinct parent
//! therefore gets a generated edge that creates it, and the recipe statement
//! lists that edge as an order-only dependency.
//!
//! The preparation edge depends on the recipe's own prerequisites, so it runs
//! after them: a prerequisite that creates the same directory with a
//! non-idempotent `mkdir` still succeeds. The edge is reached only through the
//! recipe statement, so an unselected edge creates no directory at all.
//!
//! The node the preparation edge declares is never created. Ninja reads that
//! absence as permanently out of date, so the idempotent command reruns on
//! every build that reaches it and a directory deleted since the last build is
//! recreated. The node lives under `.netsuke/phony`, the namespace generation
//! reserves, and its identity is content-addressed, so no user path and no
//! other edge can collide with it.

use super::RecipeShell;
use super::dyndep::content_digest;
use super::edge_metadata;
use super::error::NinjaGenError;
use super::escape::ShellText;
use super::force_run;
use super::join;
use super::ordered_edges;
use super::recipe_shell::InterpreterBinding;
use super::reserved_paths::PHONY_NAMESPACE;
use crate::ir::{BuildEdge, BuildGraph};
use crate::shell_word::{ShellDialect, quote_word};
use camino::{Utf8Path, Utf8PathBuf};
use std::fmt::Write;

/// Prefix naming a prepared directory's node inside the reserved namespace.
const NODE_PREFIX: &str = "dir-";
/// Prefix of the rule that creates one prepared directory.
const RULE_PREFIX: &str = "netsuke_mkdir_";
/// Schema tag incorporated into every identity this module derives.
const DIRECTORY_SCHEMA: &str = "netsuke-phony-dir-v1";

/// One output directory an executable `phony` recipe writes into.
struct OutputDir {
    /// Directory that must exist before the recipe runs.
    directory: Utf8PathBuf,
    /// Generated node whose edge creates that directory.
    node: Utf8PathBuf,
}

/// Return the generated preparation nodes one edge's recipe statement depends on.
///
/// Returns nothing for an edge that is not lowered to a hidden executable
/// statement, so ordinary statements keep their previous dependency lists.
pub(super) fn nodes(graph: &BuildGraph, edge: &BuildEdge) -> Vec<Utf8PathBuf> {
    for_edge(graph, edge)
        .into_iter()
        .map(|prepared| prepared.node)
        .collect()
}

/// Write the rules and edges that prepare executable `phony` output directories.
///
/// Statements follow graph order, so the emitted text is a function of the
/// graph rather than of any map iteration. A graph whose executable `phony`
/// edges declare only bare output names keeps its previous byte output.
///
/// # Errors
///
/// Returns an error when a rule's command cannot be rendered for the selected
/// shell or when the writer fails.
pub(super) fn write_statements<W: Write>(
    graph: &BuildGraph,
    out: &mut W,
    shell: RecipeShell,
    binding: InterpreterBinding<'_>,
) -> Result<(), NinjaGenError> {
    let prepared: Vec<(&BuildEdge, Vec<OutputDir>)> = ordered_edges(graph.edges())
        .into_iter()
        .map(|edge| (edge, for_edge(graph, edge)))
        .filter(|(_, directories)| !directories.is_empty())
        .collect();
    let directories = distinct_directories(&prepared);
    for directory in &directories {
        write_rule(out, shell, binding, directory)?;
    }
    for (edge, prepared_dirs) in &prepared {
        for prepared_dir in prepared_dirs {
            write_edge(out, edge, prepared_dir)?;
        }
    }
    Ok(())
}

/// Return every prepared directory once, in the order the graph first names it.
fn distinct_directories(prepared: &[(&BuildEdge, Vec<OutputDir>)]) -> Vec<Utf8PathBuf> {
    let mut directories: Vec<Utf8PathBuf> = Vec::new();
    for (_, prepared_dirs) in prepared {
        for prepared_dir in prepared_dirs {
            if !directories
                .iter()
                .any(|directory| directory == &prepared_dir.directory)
            {
                directories.push(prepared_dir.directory.clone());
            }
        }
    }
    directories
}

/// Return every output directory one executable `phony` edge writes into.
///
/// Explicit and implicit outputs both count, because the recipe writes both
/// under their declared names. A directory two outputs share is reported once,
/// in the order the edge first names it.
fn for_edge(graph: &BuildGraph, edge: &BuildEdge) -> Vec<OutputDir> {
    if !edge_metadata::is_executable_phony(graph, edge) {
        return Vec::new();
    }
    let mut prepared: Vec<OutputDir> = Vec::new();
    for output in edge.explicit_outputs.iter().chain(&edge.implicit_outputs) {
        let Some(directory) = preparable_parent(output) else {
            continue;
        };
        if prepared
            .iter()
            .any(|existing| existing.directory.as_path() == directory)
        {
            continue;
        }
        prepared.push(OutputDir {
            directory: directory.to_path_buf(),
            node: node_path(edge, directory),
        });
    }
    prepared
}

/// Return the parent directory a declared output needs, when it needs one.
///
/// A bare name's parent is the working directory itself, and a root has no
/// parent to create; both are skipped so the common case, an output in the
/// working directory, emits no statement and forks no shell.
fn preparable_parent(output: &Utf8Path) -> Option<&Utf8Path> {
    output
        .parent()
        .filter(|parent| !parent.as_str().is_empty())
        .filter(|parent| *parent != Utf8Path::new("."))
        .filter(|parent| *parent != Utf8Path::new("/"))
}

/// Derive the node that owns preparation of one directory for one edge.
///
/// The hidden output already identifies the edge uniquely, and the directory
/// completes the identity so a recipe with outputs in two directories gets one
/// node per directory.
fn node_path(edge: &BuildEdge, directory: &Utf8Path) -> Utf8PathBuf {
    let identity = force_run::hidden_output(edge);
    let digest = content_digest(
        DIRECTORY_SCHEMA,
        &format!("{}\0{}", identity.as_str(), directory.as_str()),
    );
    Utf8PathBuf::from(format!("{PHONY_NAMESPACE}/{NODE_PREFIX}{digest}"))
}

/// Name the rule that creates `directory`.
///
/// The rendered command embeds the quoted directory, so two directories cannot
/// share a rule. No manifest action can collide with the name, because a
/// manifest action's rule is named by its content hash.
fn rule_name(directory: &Utf8Path) -> String {
    let digest = content_digest(DIRECTORY_SCHEMA, directory.as_str());
    format!("{RULE_PREFIX}{digest}")
}

/// Write the rule that creates one prepared directory.
///
/// Rendering goes through the same interpreter transport as a manifest recipe,
/// so quoting and escaping are the responsibility of the code that owns that
/// shell rather than a second, weaker encoder here.
fn write_rule<W: Write>(
    out: &mut W,
    shell: RecipeShell,
    binding: InterpreterBinding<'_>,
    directory: &Utf8Path,
) -> Result<(), NinjaGenError> {
    let name = rule_name(directory);
    let command = shell.command_value_with_interpreter(&mkdir_script(shell, directory), binding)?;
    writeln!(out, "rule {name}")?;
    writeln!(out, "  command = {}", command.command())?;
    if let Some(content) = command.response_file_content() {
        // Only a directory near the Windows command-line limit reaches this
        // form, and it still needs the response-file bindings to be runnable.
        writeln!(out, "  rspfile = $out.netsuke-{name}.ps1")?;
        writeln!(out, "  rspfile_content = {content}")?;
    }
    writeln!(out)?;
    Ok(())
}

/// Write one preparation edge for `edge`.
///
/// The recipe's prerequisites are this edge's own inputs, so the directory is
/// created after them. They are not order-only here: a prerequisite that
/// writes into the directory must see it as that prerequisite left it.
fn write_edge<W: Write>(
    out: &mut W,
    edge: &BuildEdge,
    prepared: &OutputDir,
) -> Result<(), NinjaGenError> {
    write!(
        out,
        "build {}: {}",
        join(std::slice::from_ref(&prepared.node)),
        rule_name(&prepared.directory)
    )?;
    for prerequisites in [
        edge.inputs.as_slice(),
        edge.implicit_deps.as_slice(),
        edge.order_only_deps.as_slice(),
    ] {
        if !prerequisites.is_empty() {
            write!(out, " {}", join(prerequisites))?;
        }
    }
    writeln!(out)?;
    writeln!(out)?;
    Ok(())
}

/// Render the command that creates one directory in the shell's own dialect.
///
/// `mkdir -p` and `New-Item -Force` are both idempotent, which the permanent
/// dirtiness of the node depends on: the command may run on every build.
fn mkdir_script(shell: RecipeShell, directory: &Utf8Path) -> ShellText {
    let quoted = quote_word(shell.dialect(), directory.as_str());
    match shell.dialect() {
        ShellDialect::Sh => ShellText::new(format!("mkdir -p -- {quoted}")),
        ShellDialect::PowerShell => ShellText::new(format!(
            "New-Item -ItemType Directory -Force -Path {quoted} | Out-Null"
        )),
    }
}

#[cfg(test)]
mod tests;

//! Render one build edge as the statement or statements Ninja consumes.
//!
//! The display layer sits between the graph and the Ninja text and owns the
//! two shapes an edge can take. An ordinary edge renders as one `build`
//! statement naming its declared outputs. An executable `phony` edge renders
//! as two: a hidden statement whose output is a content-addressed path under
//! `.netsuke/phony`, and a native `phony` alias that publishes the declared
//! names so dependants and `netsuke clean` still see them. The hidden shape
//! exists because a user file at a declared name must never look like the
//! recipe's product.
//!
//! Several inputs come from other generated-state owners. `edge_metadata`
//! decides the edge class and selects the hidden output; `force_run` owns the
//! shared force node, which [`FORCE_PATH`] names as a render-time implicit
//! dependency when [`DisplayEdge::needs_force`] is set; and the dyndep render
//! path supplies lowered serial gates in place of the raw implicit list. The
//! hidden statement is also the only one that carries the directory-preparation
//! nodes, as order-only dependencies, because Ninja prepares a real output's
//! parents itself. A rule-level `restat` is cleared on the hidden statement,
//! because a hidden output that is never created cannot prune dependants.

use super::CONSOLE_POOL;
use super::escape::NinjaValue;
use super::force_run::FORCE_PATH;
use super::join;
use crate::ir::BuildEdge;
use camino::Utf8PathBuf;
use std::fmt::{self, Display, Formatter};

/// Render one build edge as the one or two statements Ninja needs.
///
/// An ordinary edge is a single statement. A `phony` edge is two: the hidden
/// statement that runs the recipe, and the alias that publishes the declared
/// names to the graph's dependants and to `netsuke clean`.
pub(crate) struct DisplayEdge<'a> {
    /// The build edge whose inputs and outputs are rendered.
    pub(crate) edge: &'a BuildEdge,
    /// The Ninja rule selected for the edge, including the built-in `phony` rule.
    pub(crate) action_name: &'a str,
    /// Whether the action sets `restat`, suppressing the edge-level override.
    pub(crate) action_restat: bool,
    /// Pool selected by the shared action rule, if any.
    pub(crate) action_pool: Option<&'a str>,
    /// Dependencies rendered after `|`, either the edge's implicit deps or lowered serial gates.
    pub(crate) implicit_deps: &'a [Utf8PathBuf],
    /// Optional localized inline-recipe progress description, escaped for Ninja.
    pub(super) description: Option<&'a NinjaValue>,
    /// Hidden executable output for a `phony` edge, whose public names become
    /// a native `phony` alias.
    pub(crate) hidden_output: Option<&'a Utf8PathBuf>,
    /// Whether the executable statement depends on the shared no-input force
    /// node, so the recipe runs on every invocation.
    pub(crate) needs_force: bool,
    /// Generated nodes that create the directories the recipe writes into.
    ///
    /// Only a hidden executable statement carries them; every other statement
    /// leaves the list empty, because Ninja prepares a real output's parents
    /// itself.
    pub(crate) directory_nodes: &'a [Utf8PathBuf],
}

impl Display for DisplayEdge<'_> {
    fn fmt(&self, output: &mut Formatter<'_>) -> fmt::Result {
        match self.hidden_output {
            Some(hidden) => {
                self.write_hidden(output, hidden)?;
                self.write_alias(output, hidden)?;
            }
            None => self.write_public(output)?,
        }
        Ok(())
    }
}

impl DisplayEdge<'_> {
    /// Write the statement that carries the rule, inputs and its bindings.
    ///
    /// A `phony` edge names its hidden output here; every other edge names its
    /// declared outputs. Progress output therefore still reports the public
    /// name because the description binding stays on this statement.
    fn write_public(&self, output: &mut Formatter<'_>) -> fmt::Result {
        self.write_build_line(output, &self.edge.explicit_outputs, true)?;
        self.write_flags(output)
    }

    /// Write the hidden executable statement for a `phony` edge.
    ///
    /// The hidden output exists so Ninja never mistakes a user file for the
    /// recipe's product; the public names are declared by the alias instead.
    /// `restat` is suppressed here because the hidden output is never
    /// materialized, so re-statting it cannot prune any downstream edge.
    fn write_hidden(&self, output: &mut Formatter<'_>, hidden: &Utf8PathBuf) -> fmt::Result {
        self.write_build_line(output, std::slice::from_ref(hidden), false)?;
        if self.action_restat {
            // The action's rule declares `restat = 1`, and Ninja lets an edge
            // inherit it. Leaving it in force here would let a never-created
            // hidden output prune the next run's dependants, so clear it the
            // same way `write_pool` clears an inherited console pool.
            writeln!(output, "  restat =")?;
        }
        self.write_description(output)?;
        self.write_pool(output)?;
        writeln!(output)
    }

    /// Write the native `phony` alias that gives a hidden edge its public names.
    ///
    /// Ninja's `clean` tool skips phony outputs, so a same-named file or
    /// directory the build did not create survives `netsuke clean`.
    fn write_alias(&self, output: &mut Formatter<'_>, hidden: &Utf8PathBuf) -> fmt::Result {
        write!(output, "build {}", join(&self.edge.explicit_outputs))?;
        if !self.edge.implicit_outputs.is_empty() {
            write!(output, " | {}", join(&self.edge.implicit_outputs))?;
        }
        writeln!(output, ": phony {}", join(std::slice::from_ref(hidden)))?;
        writeln!(output)
    }

    /// Write one `build` statement up to and including its dependencies.
    ///
    /// `is_public` distinguishes the declared-name statement from the hidden
    /// one. The hidden statement is the only one that takes the edge's
    /// directory nodes, and it leaves the real implicit outputs to the public
    /// alias. Those nodes join the order-only group, where a node that must run
    /// first and must never make the statement stale belongs.
    fn write_build_line(
        &self,
        output: &mut Formatter<'_>,
        outputs: &[Utf8PathBuf],
        is_public: bool,
    ) -> fmt::Result {
        let directory_nodes = if is_public {
            &[][..]
        } else {
            self.directory_nodes
        };
        write!(output, "build {}", join(outputs))?;
        if is_public && !self.edge.implicit_outputs.is_empty() {
            write!(output, " | {}", join(&self.edge.implicit_outputs))?;
        }
        write!(output, ": {}", self.action_name)?;
        if !self.edge.inputs.is_empty() {
            write!(output, " {}", join(&self.edge.inputs))?;
        }
        self.write_implicit_deps(output)?;
        match (
            self.edge.order_only_deps.is_empty(),
            directory_nodes.is_empty(),
        ) {
            (true, true) => {}
            (false, true) => write!(output, " || {}", join(&self.edge.order_only_deps))?,
            (true, false) => write!(output, " || {}", join(directory_nodes))?,
            (false, false) => write!(
                output,
                " || {} {}",
                join(&self.edge.order_only_deps),
                join(directory_nodes)
            )?,
        }
        writeln!(output)
    }

    /// Write the implicit dependency list, appending the force node when used.
    ///
    /// The force node is a render-time dependency rather than an IR one, so the
    /// graph keeps no knowledge of the backend's freshness trick.
    fn write_implicit_deps(&self, output: &mut Formatter<'_>) -> fmt::Result {
        match (self.implicit_deps.is_empty(), self.needs_force) {
            (true, false) => Ok(()),
            (false, false) => write!(output, " | {}", join(self.implicit_deps)),
            (true, true) => write!(output, " | {FORCE_PATH}"),
            (false, true) => write!(output, " | {} {FORCE_PATH}", join(self.implicit_deps)),
        }
    }

    /// Write the metadata bindings shared by public and hidden statements.
    fn write_flags(&self, output: &mut Formatter<'_>) -> fmt::Result {
        write_flag!(output, "restat", self.edge.always && !self.action_restat);
        self.write_description(output)?;
        self.write_pool(output)?;
        writeln!(output)
    }

    /// Write the optional inline progress description binding.
    fn write_description(&self, output: &mut Formatter<'_>) -> fmt::Result {
        write_kv!(output, "description", self.description);
        Ok(())
    }

    /// Write the pool binding, including the console opt-in and its clearing.
    fn write_pool(&self, output: &mut Formatter<'_>) -> fmt::Result {
        if self.edge.output_visibility == crate::ir::EdgeOutputVisibility::Immediate {
            writeln!(output, "  pool = {CONSOLE_POOL}")?;
        } else if self.action_pool == Some(CONSOLE_POOL) {
            // Rule bindings are inherited by edges, so clear a rule-level
            // console pool on edges that did not opt in themselves.
            writeln!(output, "  pool =")?;
        }
        Ok(())
    }
}

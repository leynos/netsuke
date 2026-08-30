//! Ninja build-edge display rendering.

use super::join;
use crate::ir::BuildEdge;
use camino::Utf8PathBuf;
use std::fmt::{self, Display, Formatter};

/// Render one build edge with its selected Ninja rule.
pub(crate) struct DisplayEdge<'a> {
    /// The build edge whose inputs and outputs are rendered.
    pub(crate) edge: &'a BuildEdge,
    /// The Ninja rule selected for the edge, including the built-in `phony` rule.
    pub(crate) action_name: &'a str,
    /// Whether the action sets `restat`, suppressing the edge-level override.
    pub(crate) action_restat: bool,
    /// Dependencies rendered after `|`, either the edge's implicit deps or lowered serial gates.
    pub(crate) implicit_deps: &'a [Utf8PathBuf],
}

impl Display for DisplayEdge<'_> {
    fn fmt(&self, output: &mut Formatter<'_>) -> fmt::Result {
        write!(output, "build {}", join(&self.edge.explicit_outputs))?;
        if !self.edge.implicit_outputs.is_empty() {
            write!(output, " | {}", join(&self.edge.implicit_outputs))?;
        }
        write!(output, ": {}", self.action_name)?;
        if !self.edge.inputs.is_empty() {
            write!(output, " {}", join(&self.edge.inputs))?;
        }
        if !self.implicit_deps.is_empty() {
            write!(output, " | {}", join(self.implicit_deps))?;
        }
        if !self.edge.order_only_deps.is_empty() {
            write!(output, " || {}", join(&self.edge.order_only_deps))?;
        }
        writeln!(output)?;
        write_flag!(output, "restat", self.edge.always && !self.action_restat);
        writeln!(output)
    }
}

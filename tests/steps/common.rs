//! Shared helpers for step modules.
//!
//! Centralises repeated accessors so individual step files stay concise and
//! reuse consistent error messages.

use crate::CliWorld;
use anyhow::{Context, Result};
use netsuke::ir::BuildGraph;

pub(super) fn build_graph_available(world: &CliWorld) -> Result<&BuildGraph> {
    build_graph(world, "build graph should be available")
}

pub(super) fn build_graph_generated(world: &CliWorld) -> Result<&BuildGraph> {
    build_graph(world, "build graph should have been generated")
}

pub(super) fn build_graph_generated_mut(world: &mut CliWorld) -> Result<&mut BuildGraph> {
    build_graph_mut(world, "build graph should have been generated")
}

fn build_graph<'a>(world: &'a CliWorld, message: &'static str) -> Result<&'a BuildGraph> {
    world.build_graph.as_ref().context(message)
}

fn build_graph_mut<'a>(
    world: &'a mut CliWorld,
    message: &'static str,
) -> Result<&'a mut BuildGraph> {
    world.build_graph.as_mut().context(message)
}

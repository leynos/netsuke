//! Intermediate Representation structures.
//!
//! This module defines the backend-agnostic build graph used by Netsuke after
//! validation. The IR mirrors the conceptual model of Ninja without embedding
//! any Ninja-specific syntax.
//!
//! # Examples
//!
//! ```
//! use netsuke::ir::{Action, BuildGraph, BuildEdge};
//! use netsuke::ast::{CommandIsolation, Recipe};
//! use camino::Utf8PathBuf;
//!
//! let action = Action {
//!     recipe: Recipe::Command {
//!         command: "echo hi".into(),
//!         isolation: CommandIsolation::Shared,
//!     },
//!     description: None,
//!     depfile: None,
//!     deps_format: None,
//!     pool: None,
//!     restat: false,
//! };
//! let mut graph = BuildGraph::default();
//! graph.actions.insert("a".into(), action);
//! graph.default_targets.push(Utf8PathBuf::from("hello"));
//! ```

#[cfg(test)]
mod action_hash_property_tests;
mod cmd_interpolate;
mod cycle;
mod from_manifest;
mod graph;

pub use cmd_interpolate::{INS_TOKEN, OUTS_TOKEN};
pub(crate) use graph::manifest_recipe_validation_message;
pub use graph::{Action, BuildEdge, BuildGraph, DependencyOrder, EdgeArena, EdgeId, IrGenError};

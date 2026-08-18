//! Manifest syntax for a single build target.

use serde::{Deserialize, Serialize};

use super::{DependencyOrder, Recipe, StringOrList, Vars};

/// A single build target.
///
/// Targets describe the files produced by a rule and their dependencies.
/// `phony` targets are always considered out of date, while `always` targets are
/// regenerated even if their inputs are unchanged.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    /// Output file or files.
    pub name: StringOrList,
    /// How the target should be built.
    #[serde(flatten)]
    pub recipe: Recipe,

    /// Input files consumed by the recipe.
    #[serde(default)]
    pub sources: StringOrList,

    /// Normal prerequisites that must be built first.
    #[serde(default)]
    pub deps: StringOrList,

    /// Ordering policy applied to the deps list.
    #[serde(default)]
    pub dependency_order: DependencyOrder,

    /// Dependencies that do not cause a rebuild when changed.
    #[serde(default)]
    pub order_only_deps: StringOrList,

    /// Target-scoped variables available during command execution.
    #[serde(default)]
    pub vars: Vars,

    /// Declares that the target does not correspond to a real file.
    #[serde(default)]
    pub phony: bool,

    /// Force the recipe to run even if the outputs are up to date.
    #[serde(default)]
    pub always: bool,

    /// Optional human-friendly summary of the public operation this target
    /// performs.
    ///
    /// Unlike [`crate::ast::Rule::description`], which explains work while Ninja executes
    /// a recipe, a target description is discovery metadata for humans: it is
    /// surfaced by `netsuke help targets` and never replaces a referenced rule
    /// description in Ninja progress output.
    #[serde(default)]
    pub description: Option<String>,
}

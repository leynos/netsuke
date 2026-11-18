//! Manifest syntax for a single build target.

use serde::{Deserialize, Serialize};

use super::{DependencyOrder, Recipe, StringOrList, Vars};

/// A single build target.
///
/// Targets describe the files produced by a rule and their dependencies. A
/// `phony` target names a logical operation rather than a file: the backend
/// publishes the name as a Ninja `phony` alias over a hidden output, so an
/// existing file or directory of that name neither suppresses the recipe nor is
/// removed by `clean`. An `always` target keeps its declared output but gains a
/// permanently dirty prerequisite, so the recipe runs even when its inputs are
/// unchanged. `console` asks Ninja to run an executable edge in its serial
/// console pool, which can make long-running output visible sooner at the cost
/// of serializing other console edges.
#[expect(
    clippy::struct_excessive_bools,
    reason = "manifest controls are independent and may be combined"
)]
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

    /// Declares that the target is a logical operation rather than a file.
    #[serde(default)]
    pub phony: bool,

    /// Force the recipe to run even if the outputs are up to date.
    #[serde(default)]
    pub always: bool,

    /// Run an executable action or target in Ninja's serial console pool.
    ///
    /// Defaults to `false`. Dependency-only entries cannot use this setting.
    #[serde(default)]
    pub console: bool,

    /// Internal discovery metadata marking a target with an unevaluated `when`.
    #[serde(default)]
    pub conditional: bool,

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

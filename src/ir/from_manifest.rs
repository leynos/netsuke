//! Manifest-to-IR conversion helpers.
//!
//! Implements [`BuildGraph::from_manifest`], which lowers a parsed
//! [`crate::ast::NetsukeManifest`] into a [`BuildGraph`].  Delegates
//! template rendering to [`crate::manifest::render`], command interpolation
//! to [`super::cmd_interpolate`], and cycle/missing-dependency detection to
//! [`super::cycle`].

use std::sync::Arc;

use camino::Utf8PathBuf;

use crate::ast::{MISSING_RECIPE_ERROR, NetsukeManifest, Recipe, Rule};
use crate::localization::{self, keys};

use super::{
    cycle::{self, CycleDetectionReport},
    graph::{Action, BuildEdge, BuildGraph, DependencyOrder, IrGenError, IrHashMap},
};

#[path = "from_manifest_support.rs"]
mod support;

#[cfg(kani)]
use support::find_duplicates;
use support::{
    ActionBindings, duplicate_output_error, get_target_display_name, insert_edge_for_outputs,
    register_action, resolve_rule, to_paths,
};

impl BuildGraph {
    /// Transform a manifest into a [`BuildGraph`].
    ///
    /// # Errors
    ///
    /// Returns [`IrGenError`] when a referenced rule is missing, multiple rules
    /// are specified for a single target, no rule is provided, or a directly
    /// deserialized manifest violates the recipe contract.
    pub fn from_manifest(manifest: &NetsukeManifest) -> Result<Self, IrGenError> {
        manifest
            .validate_recipes()
            .map_err(|_| IrGenError::InvalidManifest {
                message: MISSING_RECIPE_ERROR,
            })?;
        let mut graph = Self::default();
        let mut rule_map = IrHashMap::<String, Arc<Rule>>::default();

        Self::process_rules(manifest, &mut rule_map);
        Self::process_targets(manifest, &mut graph.actions, &mut graph.targets, &rule_map)?;
        Self::process_defaults(manifest, &mut graph.default_targets);

        graph.detect_cycles()?;

        Ok(graph)
    }

    /// Collect rule templates without deduplicating them.
    ///
    /// Rules are stored verbatim and expanded later when targets reference
    /// them. This allows each target's input and output paths to be embedded in
    /// the resulting command, meaning identical rule definitions may yield
    /// distinct actions once interpolated. Target descriptions remain discovery
    /// metadata and never take part in recipe resolution or Ninja progress.
    fn process_rules(manifest: &NetsukeManifest, rule_map: &mut IrHashMap<String, Arc<Rule>>) {
        for rule in &manifest.rules {
            rule_map.insert(rule.name.clone(), Arc::new(rule.clone()));
        }
    }

    /// Build an action and edge for every manifest target.
    ///
    /// Rule-backed targets resolve a single template first; command and script
    /// targets interpolate their command directly. Each resolved action is
    /// registered once under its content hash, and the resulting edge is
    /// attached to every explicit output.
    ///
    /// # Errors
    ///
    /// Returns [`IrGenError`] when a target writes an output already claimed by
    /// an earlier target or by a sibling output in the same target, when a
    /// rule selector does not resolve to exactly one rule, or when command
    /// interpolation fails.
    fn process_targets(
        manifest: &NetsukeManifest,
        actions: &mut IrHashMap<String, Action>,
        targets: &mut IrHashMap<Utf8PathBuf, BuildEdge>,
        rule_map: &IrHashMap<String, Arc<Rule>>,
    ) -> Result<(), IrGenError> {
        for target in manifest.actions.iter().chain(&manifest.targets) {
            let outputs = to_paths(&target.name);
            let inputs = to_paths(&target.sources);
            let implicit_deps = to_paths(&target.deps);
            tracing::debug!(
                target = ?target.name,
                implicit_deps_count = implicit_deps.len(),
                "populating implicit dependencies for target",
            );
            if let Some(error) = duplicate_output_error(&outputs, targets) {
                return Err(error);
            }

            let action_id = match &target.recipe {
                Recipe::Rule { rule } => {
                    let target_name = get_target_display_name(&outputs);
                    let tmpl = resolve_rule(rule, rule_map, &target_name)?;
                    // Target descriptions are deliberately omitted: rule
                    // descriptions remain the sole source of Ninja progress
                    // text.
                    register_action(
                        actions,
                        tmpl.recipe.clone(),
                        tmpl.description.as_deref(),
                        ActionBindings {
                            inputs: &inputs,
                            outputs: &outputs,
                        },
                    )?
                }
                Recipe::Command { .. } | Recipe::Script { .. } => register_action(
                    actions,
                    target.recipe.clone(),
                    None,
                    ActionBindings {
                        inputs: &inputs,
                        outputs: &outputs,
                    },
                )?,
            };

            let edge = BuildEdge {
                action_id,
                inputs,
                implicit_deps,
                dependency_order: target.dependency_order.into(),
                explicit_outputs: outputs,
                implicit_outputs: Vec::new(),
                order_only_deps: to_paths(&target.order_only_deps),
                phony: target.phony,
                always: target.always,
            };

            insert_edge_for_outputs(targets, edge);
        }
        Ok(())
    }

    /// Append the manifest's default targets to `defaults`.
    fn process_defaults(manifest: &NetsukeManifest, defaults: &mut Vec<Utf8PathBuf>) {
        defaults.extend(manifest.defaults.iter().map(Utf8PathBuf::from));
    }

    /// Detect dependency cycles and unresolved dependencies in the built graph.
    ///
    /// Unresolved dependencies are logged and treated as external files; a
    /// detected cycle fails the whole lowering.
    ///
    /// # Errors
    ///
    /// Returns [`IrGenError::CircularDependency`] when the target graph
    /// contains a dependency cycle.
    fn detect_cycles(&self) -> Result<(), IrGenError> {
        let CycleDetectionReport {
            cycle,
            missing_dependencies,
        } = cycle::analyse(&self.targets);

        for (dependent, missing) in &missing_dependencies {
            tracing::info!(
                dependent = %dependent,
                missing = %missing,
                "unresolved dependency: not a build target; assuming it is an external file",
            );
        }

        if let Some(detected_cycle) = cycle {
            let message = localization::message(keys::IR_CIRCULAR_DEPENDENCY)
                .with_arg("cycle", format!("{detected_cycle:?}"));
            return Err(IrGenError::CircularDependency {
                cycle: detected_cycle,
                missing_dependencies,
                message,
            });
        }

        tracing::info!(
            count = missing_dependencies.len(),
            "cycle detection complete; unresolved dependencies treated as external files",
        );
        Ok(())
    }
}

#[cfg(kani)]
#[path = "from_manifest_verification.rs"]
mod verification;

impl From<crate::ast::DependencyOrder> for DependencyOrder {
    fn from(order: crate::ast::DependencyOrder) -> Self {
        match order {
            crate::ast::DependencyOrder::Parallel => Self::Parallel,
            crate::ast::DependencyOrder::Serial => Self::Serial,
        }
    }
}

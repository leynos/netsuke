//! Turning drawn values into a well-formed specification.
//!
//! This is the resolution half of the generator: [`Blueprint`] holds the raw
//! strategy output, and [`Blueprint::build`] allocates the output pool, derives
//! the action identifiers, and resolves every edge's slots against the pool.
//! The split from the parent module is the file-size cap rather than a
//! boundary in the reasoning; the well-formedness argument lives here because
//! path allocation is what carries it.

use super::{
    EXTERNAL_PREFIX, EdgePlan, GraphSpec, MAX_OUTPUTS, OUTPUT_PREFIX, rotate,
};
use crate::ast::{Recipe, StringOrList};
use crate::ir::{Action, BuildEdge, DependencyOrder};
use camino::Utf8PathBuf;

/// The drawn values, before any path is allocated.
#[derive(Debug, Clone)]
pub(super) struct Blueprint {
    /// Number of actions to declare.
    pub(super) action_count: usize,
    /// Number of edges to declare.
    pub(super) edge_count: usize,
    /// Per-edge shape decisions.
    pub(super) plans: Vec<EdgePlan>,
    /// Per-action `restat` flags.
    pub(super) restat_flags: Vec<bool>,
    /// Requested default-target pool slots, repeats included.
    pub(super) default_picks: Vec<usize>,
    /// How many of the leading actions are dependency-only.
    pub(super) phony_count: usize,
    /// A shuffled permutation of `0..edge_count`.
    pub(super) shuffled: Vec<usize>,
    /// How far to rotate [`Self::shuffled`] to obtain the second order.
    pub(super) rotation: usize,
}

impl Blueprint {
    /// Resolve the drawn values into a well-formed specification.
    ///
    /// Path allocation carries the whole of the well-formedness argument, and
    /// it rests on one invariant: before edge `i` is served, `cursor` holds at
    /// most `MAX_OUTPUTS - (edge_count - i)`, so the headroom left after
    /// reserving one slot for each remaining edge is at least one. Every edge
    /// therefore receives at least one output, and the pool never exceeds its
    /// budget.
    pub(super) fn build(self) -> GraphSpec {
        let shares = self.edge_shares();
        let pool = allocate_pool(&shares);
        let actions = self.build_actions();
        let edges = self.build_edges(&shares, &pool, &actions);
        let default_targets = resolve(&pool, pool.len(), &self.default_picks);
        GraphSpec {
            actions,
            edges,
            default_targets,
            first_order: self.shuffled.clone(),
            second_order: rotate(&self.shuffled, self.rotation),
        }
    }

    /// Give every edge a pool share of at least one slot, within the budget.
    fn edge_shares(&self) -> Vec<usize> {
        let mut shares = Vec::with_capacity(self.plans.len());
        let mut cursor = 0usize;
        for (index, plan) in self.plans.iter().enumerate() {
            let remaining_after = self.edge_count.saturating_sub(index + 1);
            let headroom = MAX_OUTPUTS
                .saturating_sub(cursor)
                .saturating_sub(remaining_after);
            let wanted = 1 + plan.extra_outputs + plan.implicit_outputs;
            let take = wanted.min(headroom).max(1);
            shares.push(take);
            cursor += take;
        }
        shares
    }

    /// Declare the actions, marking the leading slice dependency-only.
    fn build_actions(&self) -> Vec<(String, Action)> {
        (0..self.action_count)
            .map(|index| {
                let restat = self.restat_flags.get(index).copied().unwrap_or(false);
                let recipe = if index < self.phony_count {
                    Recipe::Command {
                        command: StringOrList::Empty,
                    }
                } else {
                    Recipe::Command {
                        command: StringOrList::String(format!("echo {index:02}")),
                    }
                };
                (
                    format!("act-{index:04}"),
                    Action {
                        recipe,
                        description: None,
                        depfile: None,
                        deps_format: None,
                        pool: None,
                        restat,
                    },
                )
            })
            .collect()
    }

    /// Build every edge, resolving its slots against the pool.
    fn build_edges(
        &self,
        shares: &[usize],
        pool: &[Utf8PathBuf],
        actions: &[(String, Action)],
    ) -> Vec<BuildEdge> {
        let mut edges = Vec::with_capacity(self.plans.len());
        let mut cursor = 0usize;
        for (index, plan) in self.plans.iter().enumerate() {
            let share = shares.get(index).copied().unwrap_or(1);
            let start = cursor;
            cursor += share;
            let explicit_count = (1 + plan.extra_outputs).min(share);
            let explicit_outputs = pool
                .get(start..start + explicit_count)
                .unwrap_or_default()
                .to_vec();
            let implicit_outputs = pool
                .get(start + explicit_count..start + share)
                .unwrap_or_default()
                .to_vec();
            // Only slots allocated to earlier edges are visible, so every
            // dependency points backwards and no cycle can be generated.
            let available = start;
            edges.push(BuildEdge {
                action_id: Self::action_id_for(plan, actions),
                inputs: with_externals(
                    resolve(pool, available, &plan.input_positions),
                    index,
                    plan.external_inputs,
                ),
                implicit_deps: resolve(pool, available, &plan.implicit_positions),
                dependency_order: if plan.is_serial {
                    DependencyOrder::Serial
                } else {
                    DependencyOrder::Parallel
                },
                explicit_outputs,
                implicit_outputs,
                order_only_deps: resolve(pool, available, &plan.order_only_positions),
                phony: plan.is_phony,
                always: plan.is_always,
            });
        }
        edges
    }

    /// Choose an action identifier for one edge.
    ///
    /// Every identifier is drawn from the declared list, which makes the
    /// generator's missing-action path unreachable. The choice is by slot
    /// rather than by a flag, so an edge's action is always a real one and the
    /// phony class follows from the recipe the action carries.
    fn action_id_for(plan: &EdgePlan, actions: &[(String, Action)]) -> String {
        actions
            .get(plan.action_position)
            .map(|(id, _)| id.clone())
            .unwrap_or_default()
    }
}

/// Name every pool slot, in allocation order.
fn allocate_pool(shares: &[usize]) -> Vec<Utf8PathBuf> {
    let total: usize = shares.iter().sum();
    (0..total)
        .map(|slot| Utf8PathBuf::from(format!("{OUTPUT_PREFIX}/{slot:04}")))
        .collect()
}

/// Resolve requested pool slots into distinct paths from the leading window.
///
/// The window is the slot range allocated to earlier edges, so a request the
/// window does not cover is pulled down to its last slot rather than dropped:
/// dropping would silently thin the graph, and a fixed position may be the only
/// one a shrunk case has left. Duplicates are removed, because one edge listing
/// the same dependency twice is not a shape any obligation names.
fn resolve(pool: &[Utf8PathBuf], available: usize, positions: &[usize]) -> Vec<Utf8PathBuf> {
    let Some(window) = pool.get(..available) else {
        return Vec::new();
    };
    let Some(last) = window.len().checked_sub(1) else {
        return Vec::new();
    };
    let mut chosen: Vec<Utf8PathBuf> = Vec::new();
    for position in positions {
        let Some(path) = window.get((*position).min(last)) else {
            continue;
        };
        if !chosen.contains(path) {
            chosen.push(path.clone());
        }
    }
    chosen
}

/// Append generated external dependencies, which no edge in the pool produces.
fn with_externals(mut paths: Vec<Utf8PathBuf>, index: usize, count: usize) -> Vec<Utf8PathBuf> {
    paths.extend((0..count).map(|offset| {
        Utf8PathBuf::from(format!("{EXTERNAL_PREFIX}/{index:04}-{offset}"))
    }));
    paths
}

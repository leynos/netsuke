//! Hand-rolled topological-depth layered layout for the HTML renderer.
//!
//! The layout places sources at depth 0 and assigns each downstream node a
//! depth one greater than the maximum depth of any predecessor. Cycles are
//! rejected upstream by [`BuildGraph::from_manifest`](crate::ir::BuildGraph),
//! so the depth computation does not need a cycle-breaking pass beyond a
//! defensive cache insert.

use std::collections::BTreeMap;

use camino::Utf8Path;

use crate::graph_view::{EdgeView, GraphView};

pub(super) const COL_WIDTH: i32 = 240;
pub(super) const ROW_HEIGHT: i32 = 70;
pub(super) const NODE_WIDTH: i32 = 200;
pub(super) const NODE_HEIGHT: i32 = 44;
pub(super) const NODE_HEIGHT_HALF: i32 = NODE_HEIGHT >> 1;
pub(super) const MARGIN: i32 = 24;

#[derive(Debug, Clone, Copy)]
pub(super) struct Position {
    pub x: i32,
    pub y: i32,
}

pub(super) fn layout_positions(view: &GraphView) -> BTreeMap<&Utf8Path, Position> {
    let predecessors: BTreeMap<&Utf8Path, Vec<&Utf8Path>> = collect_predecessors(&view.edges);
    let mut depths: BTreeMap<&Utf8Path, i32> = BTreeMap::new();
    for node in &view.nodes {
        compute_depth(node.path.as_path(), &predecessors, &mut depths);
    }
    let mut by_depth: BTreeMap<i32, Vec<&Utf8Path>> = BTreeMap::new();
    for node in &view.nodes {
        let depth = *depths.get(node.path.as_path()).unwrap_or(&0);
        by_depth.entry(depth).or_default().push(node.path.as_path());
    }
    let mut positions = BTreeMap::new();
    for (depth, paths) in &by_depth {
        for (row, path) in paths.iter().enumerate() {
            positions.insert(
                *path,
                Position {
                    x: MARGIN + depth * COL_WIDTH,
                    // `row * ROW_HEIGHT` fits in i32 for any realistic graph.
                    y: MARGIN + i32::try_from(row).unwrap_or(i32::MAX) * ROW_HEIGHT,
                },
            );
        }
    }
    positions
}

pub(super) fn collect_predecessors(edges: &[EdgeView]) -> BTreeMap<&Utf8Path, Vec<&Utf8Path>> {
    let mut preds: BTreeMap<&Utf8Path, Vec<&Utf8Path>> = BTreeMap::new();
    for edge in edges {
        preds
            .entry(edge.to.as_path())
            .or_default()
            .push(edge.from.as_path());
    }
    preds
}

fn compute_depth<'a>(
    path: &'a Utf8Path,
    predecessors: &BTreeMap<&'a Utf8Path, Vec<&'a Utf8Path>>,
    cache: &mut BTreeMap<&'a Utf8Path, i32>,
) -> i32 {
    if let Some(depth) = cache.get(path) {
        return *depth;
    }
    // Insert 0 first to break any unexpected cycle defensively.
    cache.insert(path, 0);
    let depth = predecessors.get(path).map_or(0, |preds| {
        preds
            .iter()
            .map(|pred| compute_depth(pred, predecessors, cache))
            .max()
            .map_or(0, |m| m.saturating_add(1))
    });
    cache.insert(path, depth);
    depth
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;

    #[test]
    fn compute_depth_terminates_for_synthetic_cycle() {
        let a = Utf8PathBuf::from("a");
        let b = Utf8PathBuf::from("b");
        let mut predecessors: BTreeMap<&Utf8Path, Vec<&Utf8Path>> = BTreeMap::new();
        predecessors.insert(a.as_path(), vec![b.as_path()]);
        predecessors.insert(b.as_path(), vec![a.as_path()]);
        let mut cache = BTreeMap::new();

        let depth = compute_depth(a.as_path(), &predecessors, &mut cache);

        assert_eq!(depth, 2);
        assert_eq!(cache.get(a.as_path()), Some(&2));
        assert_eq!(cache.get(b.as_path()), Some(&1));
    }
}

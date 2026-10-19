//! `OBL-CYCLE`: cycles are rejected at larger N, and missing dependencies do
//! not create false cycles.
//!
//! The claim is again twofold: a manifest whose dependency relation contains a
//! cycle must lower to [`IrGenError::CircularDependency`], and an acyclic
//! manifest must lower even when some declared dependencies name paths no
//! target produces. The second half is not decoration — a detector that
//! treated every unresolved dependency as a cycle would pass the first half
//! and break ordinary manifests.
//!
//! **The cycle is constructed, not drawn.** A generator that adds one back
//! edge with arbitrary endpoints produces acyclic graphs in most cases: with
//! dependencies drawn only from lower indices, the edge `u -> v` closes a path
//! only when a path `v -> … -> u` already exists. The obligation would then
//! pass while testing almost nothing. [`Shape::Chain`] therefore builds a
//! chain first and closes it with one edge, and [`Shape::SelfEdge`] injects a
//! self-edge, which is a cycle unconditionally.
//!
//! **The long-cycle class is the point.** Kani's cycle harnesses are bounded
//! by unwinding height, so a cycle longer than that bound is a class only a
//! property reaches. [`Shape::Long`] builds a chain of at least
//! [`LONG_CYCLE_MIN`] nodes, and the tally in
//! [`the_run_reaches_the_required_cycle_classes`] fails the run if that class
//! is never reached — an obligation whose long-cycle count is zero is not
//! discharged, however green the run looks.
//!
//! This suite asserts rejection only. The canonical content of the reported
//! cycle — preserved length, closed cycle, interior multiset, stable start
//! node — belongs to roadmap item 4.2.2 and is proved by Kani over
//! `canonicalize_cycle_by`; restating it here would duplicate that proof.

use crate::ast::{DependencyOrder, NetsukeManifest, Recipe, StringOrList, Target, Vars};
use crate::ir::{BuildGraph, IrGenError};
use crate::ninja_gen::RecipeShell;
use proptest::prelude::*;
use semver::Version;

/// The shell the generated commands are written for.
const SHELL: RecipeShell = RecipeShell::Posix;

/// Fewest targets a generated manifest declares.
const MIN_TARGETS: usize = 2;
/// Greatest number of targets a generated manifest declares.
const MAX_TARGETS: usize = 30;
/// Fewest nodes in the long-cycle arm.
///
/// Kani's cycle harnesses unwind to a small bound; eight is comfortably past
/// it and still inside [`MAX_TARGETS`], so the class is reachable on every
/// run that draws a long shape at all.
pub(crate) const LONG_CYCLE_MIN: usize = 8;
/// Cases per property.
const CASES: u32 = 256;

/// The dependency shape a case was built from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Shape {
    /// Every dependency points at a lower-indexed target.
    Acyclic,
    /// As [`Shape::Acyclic`], plus dependencies on paths no target produces.
    AcyclicMissing,
    /// The first target depends on itself.
    SelfEdge,
    /// A chain closed by one edge, of length below [`LONG_CYCLE_MIN`].
    ShortChain,
    /// A chain closed by one edge, of length at least [`LONG_CYCLE_MIN`].
    Long,
}

impl Shape {
    /// Generate any shape with equal probability.
    fn arb() -> impl Strategy<Value = Self> {
        prop_oneof![
            Just(Self::Acyclic),
            Just(Self::AcyclicMissing),
            Just(Self::SelfEdge),
            Just(Self::ShortChain),
            Just(Self::Long),
        ]
    }

    /// Whether this shape contains a cycle by construction.
    pub(crate) const fn is_cyclic(self) -> bool {
        matches!(self, Self::SelfEdge | Self::ShortChain | Self::Long)
    }

    /// Whether this shape reaches the long-cycle class.
    pub(crate) const fn is_long(self) -> bool {
        matches!(self, Self::Long)
    }

    /// Fewest targets that can carry this shape's cycle at full length.
    ///
    /// Drawing the target count independently would let a [`Shape::Long`] case
    /// land on a two-target manifest, where the chain is truncated and the case
    /// is an ordinary short cycle. The tally counts shapes, not measured
    /// lengths, so such a case would over-report the class Kani cannot reach.
    /// Flooring the count here keeps the tally's claim true by construction.
    const fn min_targets(self) -> usize {
        match self {
            Self::Long => LONG_CYCLE_MIN,
            Self::Acyclic | Self::AcyclicMissing | Self::SelfEdge | Self::ShortChain => MIN_TARGETS,
        }
    }
}

/// One generated manifest and the shape its dependency relation has.
struct CycleCase {
    /// The manifest to lower.
    manifest: NetsukeManifest,
    /// The shape the dependency relation was built to have.
    shape: Shape,
}

impl std::fmt::Debug for CycleCase {
    /// Print the shape and target count, never the whole manifest.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "CycleCase {{ shape: {:?}, targets: {} }}",
            self.shape,
            self.manifest.targets.len(),
        )
    }
}

/// Generate a manifest with a dependency relation of a drawn shape.
fn arb_cycle_case() -> impl Strategy<Value = CycleCase> {
    (MIN_TARGETS..=MAX_TARGETS, Shape::arb())
        .prop_flat_map(|(drawn, shape)| {
            let count = drawn.max(shape.min_targets());
            (Just(shape), lower_picks(count))
        })
        .prop_map(|(shape, picks)| build_case(shape, &picks))
}

/// Draw a handful of dependency picks per target.
///
/// The pick is a candidate slot; whether it actually names a lower slot is
/// decided in [`build_case`], which knows each target's index. Drawing a
/// `(slot, pick)` pair here would need the slot before the vector exists,
/// which is why the pairing is deferred to the `enumerate` there.
fn lower_picks(targets: usize) -> impl Strategy<Value = Vec<Vec<usize>>> {
    let per_target = prop::collection::vec(0..MAX_TARGETS, 0..=2);
    (Just(targets), prop::collection::vec(per_target, targets))
        .prop_map(|(count, picks)| picks.into_iter().take(count).collect())
}

/// Build one manifest from drawn picks and rewrite it into `shape`.
fn build_case(shape: Shape, picks: &[Vec<usize>]) -> CycleCase {
    let mut deps: Vec<Vec<usize>> = picks
        .iter()
        .enumerate()
        .map(|(slot, drawn)| {
            let mut lower: Vec<usize> = drawn
                .iter()
                .copied()
                // The bound is the target's own index, not a drawn value: a
                // dependency may only name a strictly lower slot, so the
                // acyclic shapes cannot contain a cycle they never declared.
                .filter(|pick| *pick < slot)
                .collect();
            lower.sort_unstable();
            lower.dedup();
            lower
        })
        .collect();
    apply_shape(&mut deps, shape);
    let targets = deps
        .iter()
        .enumerate()
        .map(|(slot, dependencies)| target(slot, dependencies))
        .collect();
    CycleCase {
        manifest: manifest(targets),
        shape,
    }
}

/// Rewrite the acyclic dependency lists into `shape`.
///
/// The first two shapes keep the acyclic relation as drawn. The cyclic shapes
/// start from an empty relation and build the cycle explicitly, because a
/// drawn back edge would have to be checked rather than known.
fn apply_shape(deps: &mut [Vec<usize>], shape: Shape) {
    match shape {
        Shape::Acyclic => {}
        Shape::AcyclicMissing => add_missing(deps),
        Shape::SelfEdge => {
            if let Some(first) = deps.first_mut() {
                first.clear();
                first.push(0);
            }
        }
        Shape::ShortChain => close_chain(deps, 3.min(deps.len())),
        Shape::Long => close_chain(deps, LONG_CYCLE_MIN),
    }
}

/// Marker slot no generated target occupies.
///
/// Dependency lists are rendered from slots, so an unresolved dependency needs
/// a slot the target loop never visits. `MAX_TARGETS` is one past the highest
/// generated slot and `output` renders it as `out/0030`, which no target
/// declares. Keeping the marker in the slot type means [`build_case`] needs no
/// second representation for external dependencies.
const EXTERNAL_SLOT: usize = MAX_TARGETS;

/// Add a dependency on a path no target declares.
///
/// The name is outside every generated output, so the lowering has no edge for
/// it and must treat it as an external file rather than as a cycle. Attached to
/// the last target so it cannot be the node whose own dependency the detector
/// starts from, which keeps the shape otherwise acyclic.
fn add_missing(deps: &mut [Vec<usize>]) {
    if let Some(last) = deps.last_mut() {
        last.push(EXTERNAL_SLOT);
    }
}

/// Build a chain of `length` nodes and close it.
///
/// Node `i` depends on node `i - 1`, so the path `0 -> 1 -> … -> length - 1`
/// exists, and node `0` then gains a dependency on the last node, closing the
/// cycle. Length is the number of *nodes in the cycle*, so the chain has
/// `length` entries and the closing edge spans the whole chain.
fn close_chain(deps: &mut [Vec<usize>], length: usize) {
    // Clamp to the available targets so a caller may ask for a longer cycle
    // than the manifest can hold without indexing past the end.
    let nodes = length.min(deps.len());
    if nodes < 2 {
        return;
    }
    for slot in 0..nodes {
        if let Some(list) = deps.get_mut(slot) {
            list.clear();
        }
    }
    for slot in 0..nodes {
        if slot > 0
            && let Some(list) = deps.get_mut(slot)
        {
            list.push(slot - 1);
        }
    }
    if let Some(first) = deps.first_mut() {
        first.push(nodes - 1);
    }
}

/// Wrap targets in a manifest that declares no rules or defaults.
fn manifest(targets: Vec<Target>) -> NetsukeManifest {
    NetsukeManifest {
        netsuke_version: Version::new(1, 0, 0),
        vars: Vars::default(),
        macros: Vec::new(),
        rules: Vec::new(),
        actions: Vec::new(),
        targets,
        defaults: Vec::new(),
    }
}

/// Build one target depending on the named slots.
fn target(slot: usize, deps: &[usize]) -> Target {
    Target {
        name: StringOrList::String(output(slot)),
        recipe: Recipe::Command {
            command: StringOrList::String(format!("echo target-{slot:02}")),
        },
        sources: StringOrList::Empty,
        deps: if deps.is_empty() {
            StringOrList::Empty
        } else {
            StringOrList::List(deps.iter().map(|dep| output(*dep)).collect())
        },
        dependency_order: DependencyOrder::Parallel,
        order_only_deps: StringOrList::Empty,
        vars: Vars::default(),
        phony: false,
        always: false,
        conditional: false,
        description: None,
    }
}

/// The output path naming target slot `slot`.
fn output(slot: usize) -> String {
    format!("out/{slot:04}")
}

proptest! {
    #![proptest_config(ProptestConfig { cases: CASES, ..ProptestConfig::default() })]

    /// A constructed cycle is rejected; an acyclic manifest lowers.
    #[test]
    fn cycles_are_rejected_at_larger_n(case in arb_cycle_case()) {
        let result = BuildGraph::from_manifest_for_shell(&case.manifest, SHELL);
        if case.shape.is_cyclic() {
            let error = result.expect_err("a constructed cycle must be rejected");
            prop_assert!(
                matches!(error, IrGenError::CircularDependency { .. }),
                "{:?}: expected CircularDependency, got {error:?}",
                case.shape,
            );
        } else {
            prop_assert!(
                result.is_ok(),
                "{:?}: an acyclic manifest must lower even with unresolved dependencies, \
                 but got {:?}",
                case.shape,
                result.as_ref().err(),
            );
        }
    }
}

/// The non-vacuity classes a run must reach.
#[derive(Clone, Copy, Debug, Default)]
struct Reached {
    /// Cases drawing a self-edge.
    self_edges: usize,
    /// Cases drawing a short chain.
    short_chains: usize,
    /// Cases drawing a long chain, the class Kani cannot reach.
    long_chains: usize,
    /// Acyclic cases carrying an unresolved dependency.
    missing_dependencies: usize,
}

impl Reached {
    /// Fold one case into this tally.
    fn extended(self, shape: Shape) -> Self {
        Self {
            self_edges: self.self_edges + usize::from(shape == Shape::SelfEdge),
            short_chains: self.short_chains + usize::from(shape == Shape::ShortChain),
            long_chains: self.long_chains + usize::from(shape.is_long()),
            missing_dependencies: self.missing_dependencies
                + usize::from(shape == Shape::AcyclicMissing),
        }
    }
}

#[cfg(test)]
#[path = "cycles/tests.rs"]
mod tests;

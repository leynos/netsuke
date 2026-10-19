//! `OBL-DUP`: duplicate outputs are rejected at larger N.
//!
//! The claim is twofold, and both halves matter. A manifest that claims an
//! output twice, across targets or within one target, must lower to
//! [`IrGenError::DuplicateOutput`]; a manifest whose outputs are pairwise
//! distinct must lower successfully. Testing only the first half would be
//! satisfied by a lowering that rejected everything, which is why the control
//! arm asserts a *successful* lowering rather than merely the absence of a
//! `DuplicateOutput` error.
//!
//! The collision is injected, never discovered. [`DuplicateCase`] records the
//! exact path expected to collide at construction time, so the oracle is known
//! by construction and never consults `find_duplicates` or
//! `duplicate_output_error`. An oracle that called the production detector
//! would agree with it by definition.
//!
//! Three modes, because they reach different production branches: [`Mode::Across`]
//! puts one output in two different targets' `name` lists and is caught by the
//! graph's output index; [`Mode::Within`] puts it twice in one target's list
//! and is caught by the seen-list; [`Mode::None`] injects nothing and is the
//! control. A mutation that disables only one half leaves the other mode
//! passing, which is the separation `MUT-DUPWITHIN` exists to demonstrate and
//! the older `||`-to-`&&` patch cannot show.

use crate::ast::{DependencyOrder, NetsukeManifest, Recipe, StringOrList, Target, Vars};
use crate::ir::{BuildGraph, IrGenError};
use crate::ninja_gen::RecipeShell;
use proptest::prelude::*;
use semver::Version;

/// The shell the generated commands are written for.
///
/// The recipes are plain `echo` calls carrying no substitution tokens, so the
/// shell choice cannot affect lowering; naming it keeps the value out of the
/// properties' own logic.
const SHELL: RecipeShell = RecipeShell::Posix;

/// Fewest targets a generated manifest declares.
const MIN_TARGETS: usize = 1;
/// Greatest number of targets a generated manifest declares.
const MAX_TARGETS: usize = 30;
/// Size of the name namespace each target allocates from.
///
/// Every target owns its own window, keyed by target slot, so a name drawn
/// for one target can never collide with a name drawn for another. That
/// distinctness is a guarantee, not an accident: the control arm asserts a
/// *successful* lowering, and a generator that produced unplanted collisions
/// would make the control fail for reasons the obligation never claimed.
const NAMESPACE: usize = 64;
/// The path [`plant_across`] writes into two targets.
///
/// It sits outside every numeric namespace window, so it cannot be confused
/// with a name the generator drew.
const ACROSS_CONFLICT: &str = "out/conflict";
/// Greatest number of outputs one target declares.
const MAX_OUTPUTS: usize = 4;
/// Cases per property.
const CASES: u32 = 256;

/// Where a duplicate output was planted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    /// One path appears in two different targets.
    Across,
    /// One path appears twice in one target's list.
    Within,
    /// Every output is distinct.
    None,
}

impl Mode {
    /// Generate any mode with equal probability.
    fn arb() -> impl Strategy<Value = Self> {
        prop_oneof![Just(Self::Across), Just(Self::Within), Just(Self::None)]
    }
}

/// One generated manifest and the collision planted in it.
struct DuplicateCase {
    /// The manifest to lower.
    manifest: NetsukeManifest,
    /// The path planted twice, absent in [`Mode::None`].
    collided: Option<String>,
    /// How the path was planted.
    mode: Mode,
}

impl std::fmt::Debug for DuplicateCase {
    /// Print the shape and the planted path, never the whole manifest.
    ///
    /// Proptest renders a counter-example whatever the assertion's own message
    /// says, and a thirty-target manifest is long enough to bury the one line
    /// a reader needs.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "DuplicateCase {{ mode: {:?}, targets: {}, collided: {:?} }}",
            self.mode,
            self.manifest.targets.len(),
            self.collided,
        )
    }
}

/// Generate a manifest with a collision injected in a drawn mode.
fn arb_duplicate_case() -> impl Strategy<Value = DuplicateCase> {
    (MIN_TARGETS..=MAX_TARGETS, Mode::arb()).prop_flat_map(|(targets, mode)| {
        (
            Just(mode),
            prop::collection::vec(
                prop::collection::vec(0..NAMESPACE, 0..=MAX_OUTPUTS),
                targets,
            ),
        )
            .prop_map(|(chosen, picks)| build_case(chosen, &picks))
    })
}

/// Build one manifest from drawn output indices and plant `mode`'s collision.
fn build_case(mode: Mode, picks: &[Vec<usize>]) -> DuplicateCase {
    let mut names: Vec<Vec<String>> = picks
        .iter()
        .enumerate()
        .map(|(slot, target)| distinct_names(slot, target))
        .collect();
    let (effective_mode, collided) = plant(&mut names, mode);
    let targets = names
        .into_iter()
        .enumerate()
        .map(|(slot, outputs)| target(slot, outputs))
        .collect();
    DuplicateCase {
        manifest: manifest(targets),
        collided,
        mode: effective_mode,
    }
}

/// Render one target's drawn picks as distinct output names.
///
/// Picks are drawn with replacement, so a target can draw the same slot more
/// than once. Deduplicating here keeps the strategy a plain
/// `collection::vec` while making each target's outputs pairwise distinct —
/// the property the control arm relies on.
fn distinct_names(slot: usize, picks: &[usize]) -> Vec<String> {
    let mut slots = picks.to_vec();
    slots.sort_unstable();
    slots.dedup();
    slots.into_iter().map(|pick| name(slot, pick)).collect()
}

/// The output path naming target `slot`'s namespace slot `pick`.
fn name(slot: usize, pick: usize) -> String {
    format!("out/{slot:02}/{pick:02}")
}

/// Plant `mode`'s collision and report the mode actually planted.
///
/// A mode that cannot be planted — [`Mode::Across`] in a single-target
/// manifest — is reported as [`Mode::None`]. Returning the drawn mode would
/// label the case as a collision while carrying no collided path, and the
/// property would then assert a rejection that cannot happen.
fn plant(names: &mut [Vec<String>], mode: Mode) -> (Mode, Option<String>) {
    let planted = match mode {
        Mode::None => None,
        Mode::Across => plant_across(names),
        Mode::Within => plant_within(names),
    };
    planted.map_or((Mode::None, None), |path| (mode, Some(path)))
}

/// Append [`ACROSS_CONFLICT`] to the first and last targets.
///
/// The planted path is written to both lists rather than copied from one:
/// copying a drawn name could duplicate a path already inside the first
/// target, making the collision a *within*-target one. The mutation this mode
/// exists to separate, `MUT-DUPWITHIN`, disables the seen-list branch and
/// would then fail the across arm too.
///
/// Returns `None` below two targets, where there is no second list to
/// collide with.
fn plant_across(names: &mut [Vec<String>]) -> Option<String> {
    if names.len() < 2 {
        return None;
    }
    let planted = ACROSS_CONFLICT.to_owned();
    names.first_mut()?.push(planted.clone());
    names.last_mut()?.push(planted.clone());
    Some(planted)
}

/// Put one path twice in the last target's list.
///
/// A target may draw an empty list, so the path is defaulted rather than
/// skipped: the mode must be reachable at every target count, and skipping
/// would quietly remove `Within` from single-target runs. The default comes
/// from the last target's own namespace window, so it cannot collide with a
/// path another target already claims.
///
/// An empty list needs the default inserted twice. Pushing it once would leave
/// the target with a single output — a perfectly valid manifest — and the
/// property would then demand a rejection that the manifest never warranted.
fn plant_within(names: &mut [Vec<String>]) -> Option<String> {
    let last_slot = names.len().checked_sub(1)?;
    let outputs = names.last_mut()?;
    let path = outputs
        .first()
        .cloned()
        .unwrap_or_else(|| name(last_slot, 0));
    if outputs.is_empty() {
        outputs.push(path.clone());
    }
    outputs.push(path.clone());
    Some(path)
}

/// Wrap targets in a manifest that declares no rules or defaults.
///
/// Every target carries a literal command, so no rule resolution or default
/// expansion takes part and the only thing under test is output allocation.
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

/// Build one target emitting `outputs` through a literal command.
fn target(slot: usize, outputs: Vec<String>) -> Target {
    Target {
        name: list_or_empty(outputs),
        recipe: Recipe::Command {
            command: StringOrList::String(format!("echo target-{slot:02}")),
        },
        sources: StringOrList::Empty,
        deps: StringOrList::Empty,
        dependency_order: DependencyOrder::Parallel,
        order_only_deps: StringOrList::Empty,
        vars: Vars::default(),
        phony: false,
        always: false,
        conditional: false,
        description: None,
    }
}

/// Render an output list, using [`StringOrList::Empty`] for no outputs.
fn list_or_empty(items: Vec<String>) -> StringOrList {
    if items.is_empty() {
        StringOrList::Empty
    } else {
        StringOrList::List(items)
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: CASES, ..ProptestConfig::default() })]

    /// Every injected collision is rejected, and a clean manifest lowers.
    #[test]
    fn duplicate_outputs_are_rejected_at_larger_n(case in arb_duplicate_case()) {
        let expected = case.collided.clone();
        let result = BuildGraph::from_manifest_for_shell(&case.manifest, SHELL);
        match case.mode {
            Mode::None => {
                prop_assert!(
                    result.is_ok(),
                    "a manifest with pairwise-distinct outputs must lower, but got {:?}",
                    result.as_ref().err(),
                );
            }
            mode => {
                let error = result.expect_err("an injected duplicate must be rejected");
                let IrGenError::DuplicateOutput { outputs, .. } = error else {
                    prop_assert!(false, "{mode:?}: expected DuplicateOutput, got {error:?}");
                    return Ok(());
                };
                let collided = expected.expect("a duplicate mode always records its path");
                prop_assert!(
                    outputs.iter().any(|path| path == &collided),
                    "{mode:?}: expected the collided path {collided} to be reported, got {outputs:?}",
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the duplicate-output property suite.
    //!
    //! These pin the *generator*: each mode must plant its collision where the
    //! production check can see it, an unplantable mode must downgrade to the
    //! control rather than assert an impossible rejection, and drawn picks must
    //! still yield pairwise-distinct outputs. The obligation itself is
    //! `duplicate_outputs_are_rejected_at_larger_n` in `super`.

    use super::*;

    /// Each mode plants its collision where the production check can see it.
    #[test]
    fn every_mode_plants_the_shape_it_names() {
        let names = vec![vec!["out/00/01".to_owned()], vec!["out/01/02".to_owned()]];
        let mut across = names.clone();
        assert_eq!(plant_across(&mut across), Some(ACROSS_CONFLICT.to_owned()));
        assert_eq!(
            across,
            vec![
                vec!["out/00/01".to_owned(), ACROSS_CONFLICT.to_owned()],
                vec!["out/01/02".to_owned(), ACROSS_CONFLICT.to_owned()],
            ],
        );

        let mut within = names;
        assert_eq!(plant_within(&mut within), Some("out/01/02".to_owned()));
        assert_eq!(
            within,
            vec![
                vec!["out/00/01".to_owned()],
                vec!["out/01/02".to_owned(), "out/01/02".to_owned()],
            ],
        );
    }

    /// Repeated picks within a target still yield distinct output names.
    #[test]
    fn drawn_picks_are_deduplicated() {
        assert_eq!(
            distinct_names(3, &[4, 2, 4, 0]),
            ["out/03/00", "out/03/02", "out/03/04"]
        );
    }

    /// A within-target duplicate is reported against its own path.
    #[test]
    fn a_within_target_duplicate_is_rejected() {
        let case = build_case(Mode::Within, &[vec![7], vec![8]]);
        let result = BuildGraph::from_manifest_for_shell(&case.manifest, SHELL);
        let IrGenError::DuplicateOutput { outputs, .. } =
            result.expect_err("a within-target duplicate must be rejected")
        else {
            panic!("expected DuplicateOutput");
        };
        assert_eq!(outputs, vec!["out/01/08".to_owned()]);
    }

    /// A distinct-output manifest lowers.
    #[test]
    fn a_clean_manifest_lowers() {
        let case = build_case(Mode::None, &[vec![1], vec![2], vec![3]]);
        BuildGraph::from_manifest_for_shell(&case.manifest, SHELL)
            .expect("a manifest with distinct outputs must lower");
    }

    /// The single-target across mode is unreachable, not silently mislabelled.
    #[test]
    fn across_needs_two_targets() {
        let mut one = vec![vec!["out/01".to_owned()]];
        assert_eq!(plant_across(&mut one), None);
    }

    /// A mode the generator cannot plant is reported as the control.
    #[test]
    fn an_unplantable_mode_downgrades_to_the_control() {
        let case = build_case(Mode::Across, &[vec![1]]);
        assert_eq!(case.mode, Mode::None);
        assert_eq!(case.collided, None);
    }

    /// An empty target still receives two copies of the planted path.
    #[test]
    fn a_within_plant_into_an_empty_list_is_still_a_duplicate() {
        let case = build_case(Mode::Within, &[Vec::new()]);
        let result = BuildGraph::from_manifest_for_shell(&case.manifest, SHELL);
        assert!(
            matches!(result, Err(IrGenError::DuplicateOutput { ref outputs, .. })
                if outputs == &["out/00/00".to_owned()]),
            "expected a DuplicateOutput for out/00/00, got {result:?}"
        );
    }
}

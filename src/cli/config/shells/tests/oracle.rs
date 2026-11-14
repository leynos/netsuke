//! An independent quadratic oracle for registry acceptance.
//!
//! The production validator walks the merged sequence once and remembers which
//! names it has accepted. This module states the same rule directly instead:
//! every name is unreserved, and every pair of positions carries distinct
//! names. It is quadratic on purpose, because a different shape is what makes
//! it independent evidence rather than a second copy of the walk.
//!
//! The Verus proof in `verus/configured_shells/main.rs` establishes that the
//! model walk and this pairwise statement agree at every sequence length. The
//! properties here supply the bounded, tested correspondence between that model
//! and the production function: they sample generated sequences of validated
//! names and must agree with the oracle on every sample. They do not prove
//! agreement for all production inputs, and they say nothing about discovery,
//! merging, or the configuration-loading path around the validator.

use super::super::validate_composition;
use super::definitions;
use proptest::prelude::*;

/// Restate the reserved built-in spellings without reusing the lookup path.
const BUILT_IN_NAMES: [&str; 4] = ["sh", "bash", "pwsh", "powershell"];

/// Unreserved names, which can reach acceptance on their own.
const UNRESERVED_NAMES: [&str; 3] = ["dash", "fish", "nu"];

/// Names a generated sequence may draw from, covering both outcomes.
///
/// The pool pairs every unreserved name with every reserved spelling, so a
/// generated sequence can reach acceptance and each rejection reason.
const NAME_POOL: &[&str] = &["dash", "fish", "nu", "sh", "bash", "pwsh", "powershell"];

/// State the acceptance rule pairwise, carrying no traversal state.
fn oracle_accepts(names: &[&str]) -> bool {
    let unreserved = names.iter().all(|name| !BUILT_IN_NAMES.contains(name));
    let distinct = names
        .iter()
        .enumerate()
        .all(|(index, name)| names.iter().skip(index + 1).all(|later| later != name));
    unreserved && distinct
}

proptest! {
    /// Check the production walk against the pairwise oracle.
    #[test]
    fn production_composition_agrees_with_the_oracle(names in generated_names()) {
        let accepted = validate_composition(&definitions(&names)).is_ok();
        prop_assert_eq!(accepted, oracle_accepts(&names), "names: {:?}", &names);
    }

    /// Check the walk against the oracle across a generated set of layers.
    ///
    /// Merging concatenates every layer's definitions in merge order and never
    /// replaces, keys, or deduplicates them, so the flattened sequence is the
    /// whole registry. Generating layers rather than one sequence is what lets
    /// the samples reach an empty layer, a duplicate inside one layer, and a
    /// duplicate split across two layers.
    #[test]
    fn layered_registries_agree_with_the_oracle(layers in generated_layers()) {
        let names = merged(&layers);
        prop_assert_eq!(
            validate_composition(&definitions(&names)).is_ok(),
            oracle_accepts(&names),
            "layers: {:?}",
            &layers
        );
    }

    /// Check that regrouping unchanged ordered layers cannot change acceptance.
    ///
    /// The same definitions are split at different boundaries, which is the
    /// only difference between two operator files that declare them
    /// differently. Acceptance must follow the ordered sequence alone.
    #[test]
    fn regrouping_layers_preserves_acceptance(
        layers in generated_layers(),
        cuts in proptest::collection::vec(0usize..=40, 0..=8),
    ) {
        let names = merged(&layers);
        let regrouped = regroup(&names, &cuts);
        prop_assert_eq!(merged(&regrouped), names.clone());
        prop_assert_eq!(
            validate_composition(&definitions(&names)).is_ok(),
            validate_composition(&definitions(&merged(&regrouped))).is_ok(),
            "layers: {:?}, cuts: {:?}",
            &layers,
            &cuts
        );
    }
}

/// Generate sequences of zero to five names drawn from the pool.
fn generated_names() -> impl Strategy<Value = Vec<&'static str>> {
    proptest::collection::vec(prop::sample::select(NAME_POOL), 0..=5)
}

/// Generate zero to eight layers of zero to sixteen definitions each.
///
/// The bounds are the ones the plan states. A drawn layer may be empty, repeat
/// one name, or repeat a name an earlier layer used, so every class the
/// composition rules distinguish is reachable without filtering.
fn generated_layers() -> impl Strategy<Value = Vec<Vec<&'static str>>> {
    proptest::collection::vec(proptest::collection::vec(generated_name(), 0..=16), 0..=8)
}

/// Draw one name, favouring an unreserved spelling three to one.
///
/// A uniform draw from [`NAME_POOL`] puts a reserved spelling in almost every
/// registry of up to 128 names. Both the walk and the oracle then reject the
/// sample for the reserved rule, which masks the duplicate rule: a mutation
/// that stopped rejecting duplicates would still agree with the oracle on
/// nearly every sample. Weighting unreserved names keeps duplicate-only
/// registries common, so the duplicate rule is exercised rather than masked.
fn generated_name() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        3 => prop::sample::select(UNRESERVED_NAMES.as_slice()),
        1 => prop::sample::select(BUILT_IN_NAMES.as_slice()),
    ]
}

/// Flatten layers in merge order, which is the registry composition validates.
fn merged(layers: &[Vec<&'static str>]) -> Vec<&'static str> {
    layers.iter().flatten().copied().collect()
}

/// Split `names` into consecutive layers whose lengths are drawn from `cuts`.
///
/// This mirrors an operator moving definitions between files without changing
/// their order. A cut past the end contributes an empty layer, which is the
/// case that must not narrow the registry.
fn regroup<'a>(names: &[&'a str], cuts: &[usize]) -> Vec<Vec<&'a str>> {
    let mut remaining = names;
    let mut layers = Vec::with_capacity(cuts.len() + 1);
    for cut in cuts {
        let take = (*cut).min(remaining.len());
        let (layer, rest) = remaining.split_at(take);
        layers.push(layer.to_vec());
        remaining = rest;
    }
    layers.push(remaining.to_vec());
    layers
}

/// Show the oracle reaches both outcomes, so agreement is not vacuous.
#[test]
fn the_oracle_distinguishes_accepted_from_rejected() {
    assert!(oracle_accepts(&[]));
    assert!(oracle_accepts(&["dash", "fish"]));
    assert!(!oracle_accepts(&["dash", "dash"]));
    assert!(!oracle_accepts(&["sh"]));
}

/// Show the pool can produce every outcome the oracle distinguishes.
#[test]
fn the_generator_pool_reaches_every_outcome() {
    let reserved = NAME_POOL
        .iter()
        .copied()
        .filter(|name| BUILT_IN_NAMES.contains(name))
        .count();
    assert_eq!(reserved, BUILT_IN_NAMES.len());
    assert_eq!(NAME_POOL.len() - reserved, UNRESERVED_NAMES.len());
    let unreserved_reachable = UNRESERVED_NAMES
        .iter()
        .filter(|name| NAME_POOL.contains(name) && !BUILT_IN_NAMES.contains(name))
        .count();
    assert_eq!(
        unreserved_reachable,
        UNRESERVED_NAMES.len(),
        "the weighted strategy must draw names the pool really contains"
    );
}

/// Record one witness for every layer class the composition rules distinguish.
///
/// The generated properties sample these classes but cannot promise that each
/// one appears in a run, so this test states the witness set explicitly: it is
/// the non-vacuity record for the layered property. Every witness is drawn from
/// [`NAME_POOL`], so none of them depends on an input the generator cannot
/// produce.
#[test]
fn every_layer_class_reaches_a_verdict() {
    let cases: [(&str, Vec<Vec<&str>>, bool); 6] = [
        ("empty layers only", vec![vec![], vec![], vec![]], true),
        (
            "unique names across layers",
            vec![vec!["dash"], vec!["fish", "nu"]],
            true,
        ),
        ("reserved spelling", vec![vec!["dash"], vec!["sh"]], false),
        ("identical duplicate", vec![vec!["dash", "dash"]], false),
        (
            "duplicate across layers",
            vec![vec!["dash"], vec!["dash"]],
            false,
        ),
        (
            "empty layer cannot clear a duplicate",
            vec![vec!["dash"], vec![], vec!["dash"]],
            false,
        ),
    ];
    for (class, layers, accepted) in cases {
        let names = merged(&layers);
        assert_eq!(
            validate_composition(&definitions(&names)).is_ok(),
            accepted,
            "class {class:?} with layers {layers:?}"
        );
        assert_eq!(
            oracle_accepts(&names),
            accepted,
            "oracle disagrees for class {class:?}"
        );
    }
}

//! An independent quadratic oracle for registry acceptance.
//!
//! The production validator walks the merged sequence once and remembers which
//! names it has accepted. This module states the same rule directly instead:
//! every name is unreserved, and every pair of positions carries distinct
//! names. It is quadratic on purpose, because a different shape is what makes
//! it independent evidence rather than a second copy of the walk.
//!
//! The Verus proof in `verus/configured_shells/main.rs` establishes that the
//! walk and this pairwise statement agree at every sequence length. This suite
//! closes the remaining gap by checking that the production function is an
//! instance of that walk, over generated sequences of validated names.

use super::super::validate_composition;
use super::definitions;
use proptest::prelude::*;

/// Restate the reserved built-in spellings without reusing the lookup path.
const BUILT_IN_NAMES: [&str; 4] = ["sh", "bash", "pwsh", "powershell"];

/// Names a generated sequence may draw from, covering both outcomes.
///
/// The pool pairs three unreserved names with every reserved spelling, so a
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
}

/// Generate sequences of zero to five names drawn from the pool.
fn generated_names() -> impl Strategy<Value = Vec<&'static str>> {
    proptest::collection::vec(prop::sample::select(NAME_POOL), 0..=5)
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
    assert_eq!(NAME_POOL.len() - reserved, 3);
}

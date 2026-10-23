//! `OBL-ACTION`: equal actions intern to equal identifiers.
//!
//! Interning is what makes the emitter's action sort a total order over a set
//! of distinct rules, so it rests on one claim: structural equality implies
//! digest equality. Two `Action`s that compare equal must hash equally, or the
//! same rule would be interned twice under different keys and
//! `ordered_actions` would have two indistinguishable entries to order.
//!
//! The converse is deliberately *not* claimed. "Unequal implies unequal
//! digest" is collision-freedom of SHA-256, which `AXIOM-HASHER` puts outside
//! this plan's scope and which no amount of sampling could establish. The
//! observed-collision property below is recorded as a *non-vacuity* check on
//! the generated domain rather than as a theorem: it says the domain is wide
//! enough that a hasher ignoring a field would be caught, not that the hash is
//! injective.
//!
//! # Reachability of the field domain
//!
//! `register_action` (`src/ir/from_manifest/support/mod.rs`) hard-codes
//! `depfile: None`, `deps_format: None`, `pool: None`, and `restat: false`, so
//! no manifest can currently produce an `Action` that varies those four
//! fields. The evidence here therefore covers the hasher's *input* domain, not
//! the shipped lowering pipeline: it protects the emitter against a future
//! construction site that does set them, and against a hasher change that
//! starts ignoring one. Stating that plainly is what stops the result being
//! read as stronger than it is.
//!
//! # Layout
//!
//! [`generator`] holds the strategies and the arm taxonomy; this file holds the
//! properties and their evidence. The split is forced by `module_max_lines`:
//! the 400-line ceiling applies to each module's own file, and the combined
//! content in one file was over it. Generation and assertion also change for
//! different reasons — a new field widens one, a new law joins the other.

mod generator;

use crate::ast::{CommandIsolation, Recipe, StringOrList};
use crate::hasher::ActionHasher;
use crate::ir::Action;
use generator::{Arm, Field, action_case};
use proptest::prelude::*;
use proptest::test_runner::{RngSeed, TestCaseError, TestRunner};
use std::cell::Cell;

/// The seed the coverage tally runs under.
///
/// A coverage assertion is a claim about the generator rather than about one
/// draw from it, so it must not be able to fail because an unlucky seed
/// under-sampled a class. The law properties keep Proptest's random seed, for
/// the reason `determinism::property_tests::support::COVERAGE_SEED` records: a
/// law is worth checking against inputs no earlier run has tried.
const COVERAGE_SEED: u64 = 0x4143_5449_4f4e; // "ACTION" in ASCII.

proptest! {
    // Hashing is a serialization and a digest, so a case costs far more than a
    // path comparison. The budget buys breadth over the field domain rather
    // than over sequence length: every case is one pair of `Action` values.
    #![proptest_config(ProptestConfig { cases: 512, .. ProptestConfig::default() })]

    /// Structurally equal actions hash identically, over every generated arm.
    ///
    /// The statement is the implication `a == b` implies `hash(a) == hash(b)`,
    /// so a case whose two values are unequal is *not* evidence against it —
    /// and the independent arm is designed to produce exactly such cases, to
    /// catch a digest that varies per call. Asserting equality unconditionally
    /// would instead demand that two unequal actions collide, which is the
    /// opposite of what the obligation says and is what an earlier revision of
    /// this property did: it failed immediately on the independent arm, where
    /// `restat` happened to differ between the two draws.
    ///
    /// The clone arm catches a hasher that depends on a value's address or on
    /// per-instance state. The independent arm is a breadth check: its two
    /// draws are usually unequal, so it contributes no equal pair to assert
    /// and mostly exercises the generator rather than the hasher. Note that
    /// this property **cannot** detect `MUT-HASHMETA` even through the
    /// single-field arm: a mutation skipping `pool` changes both hashes of a
    /// pair that agrees on `pool` in the same way, so the two sides stay
    /// equal. Detecting a hasher that ignores a field is the job of
    /// [`unequal_generated_actions_are_observed_to_differ`], whose single
    /// field arm is what makes the pair unequal in exactly that field.
    #[test]
    fn equal_actions_hash_equally(case in action_case()) {
        let (left, right) = case.pair();
        if left != right {
            // No antecedent, so no claim to check. Returning is not a silent
            // pass: the coverage tally separately proves the arms that do
            // produce equal pairs fire, and the companion property below is
            // what covers the unequal pairs this arm exists to generate.
            return Ok(());
        }
        prop_assert_eq!(
            hash(&left)?,
            hash(&right)?,
            "equal actions must intern to one identifier; arm {:?}, left {:?}, right {:?}",
            case.arm,
            left,
            right,
        );
    }

    /// The generated domain separates actions a hasher could conflate.
    ///
    /// This is the non-vacuity check for the property above, not a collision
    /// claim: it asserts that any two generated values are either genuinely
    /// equal or genuinely separated, so the domain cannot consist of one
    /// repeated value that would satisfy the main property trivially.
    ///
    /// It draws from `action_case()` rather than twice from
    /// `action_strategy()`, and that is load-bearing. Two independent draws
    /// differ in many fields at once, so a hasher that skipped one field would
    /// almost certainly still separate them: measured against
    /// `MUT-HASHMETA`, an independent-draw form of this property *passed*
    /// while the hasher was dropping `pool` on every call. The single-field
    /// arm produces exactly the pair such a hasher merges, which is what makes
    /// this property the one `MUT-HASHMETA` fails. The `left == right` escape
    /// is still needed for the clone arm, whose pair is equal by construction.
    #[test]
    fn unequal_generated_actions_are_observed_to_differ(case in action_case()) {
        let (left, right) = case.pair();
        prop_assert!(
            left == right || hash(&left)? != hash(&right)?,
            "two unequal generated actions shared a digest, which would merge \
             distinct rules under one identifier; arm {:?}, left {:?}, right {:?}",
            case.arm,
            left,
            right,
        );
    }
}

/// Hash one action, reporting a hasher failure as a shrinkable test failure.
///
/// # Errors
///
/// Returns a test failure when the hasher cannot serialize the action.
fn hash(action: &Action) -> Result<String, TestCaseError> {
    ActionHasher::hash(action)
        .map_err(|error| TestCaseError::fail(format!("a generated action must serialize: {error}")))
}

/// What a coverage run observed, tallied per case.
///
/// The field counters are asserted by [`the_run_reaches_every_action_field`],
/// because a field at zero means the single-field arm never exercised it and
/// the obligation's *Covers* claim is false. The arm counters are asserted too:
/// every arm must fire at least once, or the property's domain is narrower
/// than its statement.
#[derive(Clone, Copy, Debug, Default)]
struct Reach {
    /// Cases whose second value was a clone.
    cloned: usize,
    /// Cases whose second value was an independent draw.
    independent: usize,
    /// Cases whose second value differed in exactly one field.
    mutated: usize,
    /// Per-field counts for the mutated arm, in [`Field::ALL`] order.
    fields: [usize; 6],
}

impl Reach {
    /// Fold one case's arm into the tally.
    ///
    /// The per-field counters advance by zipping rather than by indexing, so
    /// the workspace `indexing_slicing` deny never has to be reasoned about
    /// here.
    fn extended(self, arm: Arm) -> Self {
        let mut next = Self {
            cloned: self.cloned + usize::from(arm == Arm::Clone),
            independent: self.independent + usize::from(arm == Arm::Independent),
            mutated: self.mutated + usize::from(matches!(arm, Arm::Mutated(_))),
            fields: self.fields,
        };
        for (slot, field) in next.fields.iter_mut().zip(Field::ALL) {
            *slot += usize::from(arm == Arm::Mutated(field));
        }
        next
    }
}

/// The generated domain reaches every arm and every one of the six fields.
///
/// `MUT-HASHMETA` makes the hasher skip `pool`, and it can only fail a run
/// that mutated `pool` and compared the digests. The same is true, field by
/// field, for the other five, so each is asserted separately rather than
/// collapsed into "the mutated arm fired". The tally lives in a `Cell` because
/// `TestRunner::run` takes an `Fn`.
#[test]
fn the_run_reaches_every_action_field() {
    let tallies: Cell<Reach> = Cell::new(Reach::default());
    let mut runner = TestRunner::new(ProptestConfig {
        cases: 512,
        rng_seed: RngSeed::Fixed(COVERAGE_SEED),
        ..ProptestConfig::default()
    });
    runner
        .run(&action_case(), |case| {
            tallies.set(tallies.get().extended(case.arm));
            Ok(())
        })
        .expect("the action strategy should be generatable");

    let observed = tallies.into_inner();
    macro_rules! covered {
        ($field:ident, $why:expr) => {
            assert!(observed.$field > 0, "{}, reached: {observed:?}", $why);
        };
    }
    covered!(
        cloned,
        "no case compared a value with its clone, so a hasher depending on \
         address or per-instance state was never exercised"
    );
    covered!(
        independent,
        "no case compared two independent draws, so per-call variability was \
         never exercised"
    );
    covered!(
        mutated,
        "no case mutated a single field, so the arm MUT-HASHMETA relies on was \
         never exercised"
    );
    for (slot, field) in observed.fields.iter().zip(Field::ALL) {
        assert!(
            *slot > 0,
            "no case mutated {field:?}, so a hasher ignoring it would be \
             invisible; reached: {observed:?}",
        );
    }
}

/// Two actions differing only in `pool` must not intern together.
///
/// This is the directed witness for `MUT-HASHMETA`, and it is stated beside
/// the property rather than left to the mutation run because it is also the
/// shortest statement of what the mutation breaks. `pool` is the field no
/// manifest can currently set, which is exactly why the hasher must still
/// honour it: a future construction site that sets it must not silently merge
/// two throttled rules into one.
#[test]
fn a_pool_difference_is_not_interned_away() {
    let base = Action {
        recipe: Recipe::Command {
            command: StringOrList::String("cc -c".to_owned()),
            isolation: CommandIsolation::Shared,
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let pooled = Action {
        pool: Some("console".to_owned()),
        ..base.clone()
    };

    assert_ne!(
        base, pooled,
        "the witness must differ, or it proves nothing"
    );
    assert_ne!(
        ActionHasher::hash(&base).expect("the base action should hash"),
        ActionHasher::hash(&pooled).expect("the pooled action should hash"),
        "an action's pool must take part in its identity, or two rules \
         differing only in throttling intern to one identifier",
    );
}

/// A `restat` difference must not be interned away either.
///
/// The same argument as `pool`, for the other field no manifest can set.
/// `restat` changes when Ninja re-runs a rule, so two actions differing only
/// in it are different operations and cannot share an identifier.
#[test]
fn a_restat_difference_is_not_interned_away() {
    let base = Action {
        recipe: Recipe::Command {
            command: StringOrList::String("cc -c".to_owned()),
            isolation: CommandIsolation::Shared,
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let restatting = Action {
        restat: true,
        ..base.clone()
    };

    assert_ne!(
        base, restatting,
        "the witness must differ, or it proves nothing"
    );
    assert_ne!(
        ActionHasher::hash(&base).expect("the base action should hash"),
        ActionHasher::hash(&restatting).expect("the restat action should hash"),
        "an action's restat flag must take part in its identity",
    );
}

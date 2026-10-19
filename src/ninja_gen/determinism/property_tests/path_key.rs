//! `OBL-PATHKEY`: `path_key` is a canonical, injective key for a path list.
//!
//! `path_key` sorts a path list and joins it with NUL, which is what lets the
//! emitter order edges by *output set* rather than by the order a caller
//! happened to hand them over. Two claims carry that:
//!
//! 1. Permutation invariance — the key depends on the list's contents, not its
//!    order, so reordering a caller's inputs cannot reorder the emitted file.
//! 2. Injectivity on the keyable domain — two lists that are not permutations
//!    of one another cannot share a key, so the key is safe as a dedup key.
//!
//! Sorting alone does not give the second. Joining with NUL cannot tell a
//! separator from a NUL *inside* an element, so `["a", "b"]` and `["a\0b"]`
//! are different lists with one key. The obligation therefore restricts
//! injectivity to lists of non-empty, NUL-free paths, and the directed tests
//! near the foot of this module record why each half of that restriction is
//! load-bearing rather than decorative.

use super::support::{MAX_PATHS, coverage_runner, keyable_paths};
use crate::ninja_gen::path_key;
use camino::Utf8PathBuf;
use proptest::prelude::*;
use std::cell::Cell;

proptest! {
    // Keying a list is a sort and a join, so this suite is far cheaper per case
    // than the emission suites. The budget goes where it can find something:
    // an injectivity violation needs two draws to collide, not a long one.
    #![proptest_config(ProptestConfig { cases: 256, .. ProptestConfig::default() })]

    /// A permutation of a list keys identically to the list.
    ///
    /// This is the claim the emitter's edge ordering rests on. The permutation
    /// is drawn as part of the case rather than applied inside the body, so
    /// Proptest shrinks the shuffle along with the paths. Nothing is asserted
    /// about *whether* the shuffle moved anything: it legitimately may not.
    #[test]
    fn path_key_is_permutation_invariant((paths, permuted) in permuted_pairs()) {
        prop_assert_eq!(
            path_key(&paths),
            path_key(&permuted),
            "a permutation of a path list must key identically",
        );
    }

    /// Two keyable lists key equally exactly when they are permutations.
    ///
    /// The oracle is sorted-multiset equality computed here, which is
    /// structurally unlike join-with-NUL: it compares the lists themselves
    /// rather than a rendering of them. Both directions are asserted, which
    /// is what stops a trivially constant key from passing — an implication
    /// alone would be satisfied by a `path_key` that always returned `""`.
    #[test]
    fn path_key_is_injective_on_keyable_paths((left, right) in shared_pool_pairs()) {
        prop_assert_eq!(
            path_key(&left) == path_key(&right),
            sorted_multiset(&left) == sorted_multiset(&right),
            "two keyable lists share a key exactly when they are permutations \
             of one another; left {:?}, right {:?}",
            left,
            right,
        );
    }
}

/// One list of 0 to [`MAX_PATHS`] paths drawn from a shared pool.
///
/// Both properties' domains are stated as "lists from a shared pool", and the
/// pool is what makes the interesting shapes reachable rather than merely
/// possible. Drawing each path independently would leave equal multisets and
/// repeated elements essentially unreachable, and a class that never occurs is
/// a claim the suite does not actually test. Selections over a pool of at most
/// eight slots reach them by construction.
fn pooled_picks() -> impl Strategy<Value = Vec<Utf8PathBuf>> {
    keyable_paths().prop_flat_map(|pool| {
        let slots = pool.len().max(1);
        (
            Just(pool),
            prop::collection::vec(0usize..slots, 0..=MAX_PATHS),
        )
            .prop_map(|(shared, picks)| resolve(&shared, &picks))
    })
}

/// Path lists paired with a permutation of themselves.
///
/// The shuffle is drawn by Proptest rather than applied in the body, so the
/// permutation shrinks with the case. A list shorter than two elements cannot
/// be reordered at all and is still a valid case — permutation invariance
/// holds there, trivially — so the per-case fact is measured by
/// [`the_run_reaches_lists_of_every_shape`] rather than asserted here.
fn permuted_pairs() -> impl Strategy<Value = (Vec<Utf8PathBuf>, Vec<Utf8PathBuf>)> {
    pooled_picks().prop_flat_map(|paths| {
        let shuffled = paths.clone();
        (Just(paths), Just(shuffled).prop_shuffle())
    })
}

/// Two lists drawn from one shared pool, so key collisions stay reachable.
///
/// Independently drawn paths essentially never form permutations of one
/// another, which would leave the `true` arm of the injectivity equivalence
/// unreachable and the property vacuous in the one direction that can detect a
/// broken key. Drawing both lists as index selections over a small common pool
/// makes equal multisets — and therefore equal keys — genuinely reachable,
/// while unequal pairs remain the common case.
fn shared_pool_pairs() -> impl Strategy<Value = (Vec<Utf8PathBuf>, Vec<Utf8PathBuf>)> {
    keyable_paths().prop_flat_map(|pool| {
        let slots = pool.len().max(1);
        (
            Just(pool),
            prop::collection::vec(0usize..slots, 0..=MAX_PATHS),
            prop::collection::vec(0usize..slots, 0..=MAX_PATHS),
        )
            .prop_map(|(shared, left, right)| (resolve(&shared, &left), resolve(&shared, &right)))
    })
}

/// Map pool-index selections onto the paths they name.
///
/// Out-of-range picks are skipped rather than clamped. Shrinking can shorten a
/// pool while leaving an index behind it, and a clamped index would invent a
/// path the draw did not ask for, which would make the shrunk case describe a
/// different list from the one that failed.
fn resolve(pool: &[Utf8PathBuf], picks: &[usize]) -> Vec<Utf8PathBuf> {
    picks
        .iter()
        .filter_map(|pick| pool.get(*pick).cloned())
        .collect()
}

/// A list's contents as a sorted multiset, the oracle for injectivity.
fn sorted_multiset(paths: &[Utf8PathBuf]) -> Vec<String> {
    let mut sorted: Vec<String> = paths.iter().map(|path| path.as_str().to_owned()).collect();
    sorted.sort_unstable();
    sorted
}

/// What a run's lists looked like and what its shuffles did, tallied per case.
///
/// The shape classes are asserted by [`the_run_reaches_lists_of_every_shape`],
/// because a class at zero means the domain is narrower than the obligation
/// claims. The two shuffle-outcome classes are recorded but deliberately *not*
/// asserted: `prop_shuffle` may return the order it was given, so whether any
/// particular run reordered is a sampling fact, not a guarantee. The
/// guaranteed perturbation is the constructed pair in
/// [`the_constructed_permutation_is_genuinely_perturbed`].
///
/// The four order-sensitive classes require *distinct* paths as well as two or
/// more of them. A list whose elements are all equal is trivially both sorted
/// and reverse-sorted and can never be reordered, so counting it would let a
/// run report that the sort's identity arm and its worst case were both
/// exercised when neither had seen anything the other had not. Repeats and
/// emptiness are unaffected: they are shape facts and count at every length.
#[derive(Clone, Copy, Debug, Default)]
struct ListShapes {
    /// Cases with no paths at all.
    empty: usize,
    /// Cases with exactly one path, where no reordering is possible.
    singleton: usize,
    /// Cases of two or more distinct paths already in sorted order.
    already_sorted: usize,
    /// Cases of two or more distinct paths in the exact reverse of sorted order.
    reverse_sorted: usize,
    /// Cases where one path appears more than once.
    with_repeats: usize,
    /// Cases of two or more distinct paths the shuffle genuinely reordered.
    reordered: usize,
    /// Cases of two or more distinct paths the shuffle left as it found them.
    unchanged: usize,
}

impl ListShapes {
    /// Fold one case's paths and their permutation into the tally.
    fn extended(self, paths: &[Utf8PathBuf], permuted: &[Utf8PathBuf]) -> Self {
        let mut sorted = paths.to_vec();
        sorted.sort_unstable();
        let mut descending = sorted.clone();
        descending.reverse();
        let mut distinct: Vec<&str> = paths.iter().map(|path| path.as_str()).collect();
        distinct.sort_unstable();
        distinct.dedup();

        // Ordering classes are only meaningful once an order exists *and* has
        // something to disagree about, so a list of fewer than two paths, or
        // one whose paths are all equal, is excluded from all four rather than
        // counted as vacuously sorted.
        let ordered = paths.len() >= 2 && paths.len() == distinct.len();
        Self {
            empty: self.empty + usize::from(paths.is_empty()),
            singleton: self.singleton + usize::from(paths.len() == 1),
            already_sorted: self.already_sorted + usize::from(ordered && sorted == paths),
            reverse_sorted: self.reverse_sorted + usize::from(ordered && descending == paths),
            with_repeats: self.with_repeats + usize::from(paths.len() != distinct.len()),
            reordered: self.reordered + usize::from(ordered && permuted != paths),
            unchanged: self.unchanged + usize::from(ordered && permuted == paths),
        }
    }
}

/// The generated domain reaches every list shape the obligation names.
///
/// Permutation invariance is a claim about *reordering*, and a run drawn from a
/// domain that never offered a repeated path, an already-sorted list, or a
/// reverse-sorted one would report success while leaving that half of the
/// obligation untested. The tally lives in a `Cell` because `TestRunner::run`
/// takes an `Fn`.
#[test]
fn the_run_reaches_lists_of_every_shape() {
    let tallies: Cell<ListShapes> = Cell::new(ListShapes::default());
    coverage_runner(256)
        .run(&permuted_pairs(), |(paths, permuted)| {
            tallies.set(tallies.get().extended(&paths, &permuted));
            Ok(())
        })
        .expect("the path strategy should be generatable");

    let observed = tallies.into_inner();
    macro_rules! covered {
        ($field:ident, $why:expr) => {
            assert!(observed.$field > 0, "{}, reached: {observed:?}", $why);
        };
    }
    covered!(empty, "no case was the empty list");
    covered!(singleton, "no case was a one-path list");
    covered!(
        with_repeats,
        "no case carried a repeated path, so the sort's tie handling was never \
         exercised and a stable-sort defect would be invisible"
    );
    covered!(
        already_sorted,
        "no case of two or more paths was already sorted, so the arm where the \
         sort is the identity was never exercised"
    );
    covered!(
        reverse_sorted,
        "no case of two or more paths was reverse-sorted, so the worst-case \
         reordering for the sort was never exercised"
    );
}

/// A duplicated path inside one list must not disturb the key.
///
/// Repeats are where a sort's tie handling shows: two entries comparing equal
/// must both survive the join, or a list would lose an element on the way to
/// its key. The two orders here are not a sampled shuffle but a constructed
/// pair, so the perturbation is guaranteed rather than hoped for — which is
/// what the sampled properties above deliberately do not claim.
#[test]
fn the_constructed_permutation_is_genuinely_perturbed() {
    let ascending = [
        Utf8PathBuf::from("out/a"),
        Utf8PathBuf::from("out/a"),
        Utf8PathBuf::from("out/b"),
    ];
    let descending = [
        Utf8PathBuf::from("out/b"),
        Utf8PathBuf::from("out/a"),
        Utf8PathBuf::from("out/a"),
    ];
    assert_ne!(
        ascending, descending,
        "the constructed pair must actually perturb the order, or it \
         demonstrates nothing that the sampled cases do not",
    );
    assert_eq!(
        path_key(&ascending),
        path_key(&descending),
        "reversing a list must not change its key, repeated paths included",
    );
}

/// A NUL inside an element is indistinguishable from the separator.
///
/// `["a", "b"]` and `["a\0b"]` are different path lists and they share one
/// key: joining with NUL cannot tell a separator from a NUL within an element.
/// Neither list is emittable — the whole-graph validator rejects control
/// characters — but the collision is exactly why the obligation restricts
/// injectivity to keyable paths instead of stating it over every string list.
///
/// The assertion is a disjunction over the two states the obligation admits,
/// and it is not a tautology: the second arm is stronger than "the collision
/// is gone", because a future encoding could separate these two lists while
/// still conflating `[]` with `[""]`. Such an encoding would satisfy neither
/// arm, and this test would correctly fail — the precondition would then no
/// longer describe what the key actually needs.
#[test]
fn the_nul_separator_is_load_bearing() {
    let separated = [Utf8PathBuf::from("a"), Utf8PathBuf::from("b")];
    let embedded = [Utf8PathBuf::from("a\0b")];
    assert_ne!(
        separated.as_slice(),
        embedded.as_slice(),
        "the witness must be two different lists, or it demonstrates nothing",
    );

    let empty_list: [Utf8PathBuf; 0] = [];
    let one_empty = [Utf8PathBuf::from("")];
    let emptiness_still_conflates = path_key(&empty_list) == path_key(&one_empty);
    let nul_collides = path_key(&separated) == path_key(&embedded);

    assert!(
        nul_collides || !emptiness_still_conflates,
        "either the NUL separator is load-bearing, or the encoding has become \
         injective without it — in which case OBL-PATHKEY's keyable-path \
         precondition should be revisited rather than this test relaxed",
    );
}

/// An empty path is indistinguishable from its absence.
///
/// `path_key([])` and `path_key([""])` are both the empty string. The empty
/// path passes `unsupported_character` because it contains no rejected
/// character, so non-emptiness is not implied by path validation — it is a
/// separate precondition, and this records why it is stated separately.
///
/// Like the NUL witness, the assertion is a disjunction: if a future encoding
/// separates the empty list from the one-element empty path, the obligation's
/// precondition is no longer load-bearing and this test should fail so that
/// the precondition is revisited, not so that the test is relaxed.
#[test]
fn the_non_emptiness_precondition_is_load_bearing() {
    let empty_list: [Utf8PathBuf; 0] = [];
    let one_empty = [Utf8PathBuf::from("")];
    assert_ne!(
        empty_list.len(),
        one_empty.len(),
        "a zero-element list is not a one-element list, however they key",
    );
    let separated = [Utf8PathBuf::from("a"), Utf8PathBuf::from("b")];
    let embedded = [Utf8PathBuf::from("a\0b")];
    let emptiness_conflates = path_key(&empty_list) == path_key(&one_empty);
    let nul_collides = path_key(&separated) == path_key(&embedded);
    assert!(
        emptiness_conflates || !nul_collides,
        "either the non-emptiness precondition is load-bearing, or the \
         encoding has become injective without it — in which case \
         OBL-PATHKEY's keyable-path precondition should be revisited rather \
         than this test relaxed",
    );
}

/// Pin the obligation's documented domain against the shared strategy.
///
/// The two are one fact written in two places, and a drift between them would
/// leave the property silently testing a narrower domain than it claims.
const _: () = assert!(MAX_PATHS == 8, "OBL-PATHKEY's domain is 0 to 8 paths");

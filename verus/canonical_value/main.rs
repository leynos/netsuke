//! A refinement proof for first-index canonical deduplication.
//!
//! `stable_deduplicate` admits each value in input order and retains it exactly
//! when its key has not been seen at an earlier position. This file proves that
//! algorithm against an **independent** specification: an index is retained
//! when no earlier index carries an equal key — a quadratic condition stated
//! over the input itself, not derived from any membership structure.
//!
//! `least_indices` states the specification, whose five clauses
//! `least_indices_position`, `least_indices_ordered_distinct`, and
//! `least_indices_covers` derive and `contract_ok` collects. Collecting a
//! contract is not the same as carrying one through a walk: the invariant is
//! `state_ok`, and `contract_ok` is only the end-of-walk consequence
//! `state_ok_contract` reads off it.
//! `model_seen`/`model_retained` restate the algorithm as a left-to-right
//! recursion whose `_contains` and `_is_least` lemmas carry the induction
//! obligation: at processed index `i` the seen set is exactly the prefix's
//! keys, and the retained sequence exactly that prefix's least occurrences in
//! increasing order. `runtime_run` is the walk carrying `state_ok` as its
//! invariant, so each append-or-skip branch is a proved transition, and
//! `runtime_retention_matches_spec` ties the runtime's membership test to the
//! specification's retention predicate.
//!
//! `runtime_run` is a `proof fn`: it returns no retained sequence and builds no
//! collection. Its `retained` argument is the abstract sequence the invariant
//! relates, and the function's only product is the proof that the clauses hold.
//! The runtime's own output is what `stable_deduplicate` returns at run time,
//! and the connection between the two is the structural correspondence the
//! oracle checks, not anything Verus decides.
//!
//! The model's keys are an abstract sort, the runtime's `CanonicalKey` byte
//! strings: nothing here proves the template-to-key conversion, `HashSet`'s
//! internals, or the production adapter. `Seq<u64>` is a stand-in whose only
//! property this file uses is decidable equality, and it is not a genuinely
//! unrestricted abstract key type: a theorem over arbitrary byte-key sequences
//! would take an arbitrary key type, or supply a formal abstraction argument.
//! What is unbounded here is the sequence length, not the key domain — the
//! induction runs over every `keys` length, while `u64` caps the key domain at
//! 2^64 values. The independent quadratic oracle in
//! `src/stdlib/canonical_value/tests/oracle.rs` checks the implementation
//! against this model on real values. No `assume`, `admit`, external body, or
//! axiom beyond `vstd`'s own appears below.

use vstd::prelude::*;

verus! {

/// Report whether some index strictly before `index` holds an equal key.
spec fn appears_before(keys: Seq<u64>, index: int, key: u64) -> bool {
    exists |j: int| 0 <= j < index && keys[j] == key
}

/// Report whether `index` is the least occurrence of its own key: the
/// specification's retention predicate.
spec fn is_least_occurrence(keys: Seq<u64>, index: int) -> bool {
    0 <= index < keys.len() && !appears_before(keys, index, keys[index])
}

/// List the least occurrences among the first `processed` indices, ascending.
spec fn least_indices(keys: Seq<u64>, processed: int) -> Seq<int>
    decreases processed,
{
    if processed <= 0 {
        Seq::empty()
    } else if is_least_occurrence(keys, processed - 1) {
        least_indices(keys, processed - 1).push(processed - 1)
    } else {
        least_indices(keys, processed - 1)
    }
}

/// The keys the kernel has seen after processing the first `processed` indices.
///
/// Membership of the result is characterised by `model_seen_contains`, not by
/// this recursion's shape.
spec fn model_seen(keys: Seq<u64>, processed: int) -> Set<u64>
    decreases processed,
{
    if processed <= 0 {
        Set::empty()
    } else if model_seen(keys, processed - 1).contains(keys[processed - 1]) {
        model_seen(keys, processed - 1)
    } else {
        model_seen(keys, processed - 1).insert(keys[processed - 1])
    }
}

/// The source indices the kernel retains after processing `processed` indices.
///
/// An index is appended exactly when the membership test reports its key as
/// new: the algorithm's append-or-skip branch and nothing else.
spec fn model_retained(keys: Seq<u64>, processed: int) -> Seq<int>
    decreases processed,
{
    if processed <= 0 {
        Seq::empty()
    } else if model_seen(keys, processed - 1).contains(keys[processed - 1]) {
        model_retained(keys, processed - 1)
    } else {
        model_retained(keys, processed - 1).push(processed - 1)
    }
}

/// Prove the kernel's seen-keys are exactly the keys of the processed prefix.
///
/// The first half of the induction obligation: at processed index `i` the
/// accumulated set has a member exactly when some `K[j]` with `0 <= j < i`
/// equals that key.
proof fn model_seen_contains(keys: Seq<u64>, processed: int, key: u64)
    requires 0 <= processed,
    ensures model_seen(keys, processed).contains(key) <==> appears_before(keys, processed, key),
    decreases processed,
{
    if processed == 0 {
        assert(!appears_before(keys, 0, key));
    } else {
        model_seen_contains(keys, processed - 1, key);
        let earlier = model_seen(keys, processed - 1);
        let current = keys[processed - 1];
        assert(appears_before(keys, processed, key)
            <==> (appears_before(keys, processed - 1, key) || current == key));
        if earlier.contains(current) {
            assert(model_seen(keys, processed) == earlier);
        } else {
            assert(model_seen(keys, processed) == earlier.insert(current));
            assert(model_seen(keys, processed).contains(key)
                <==> (earlier.contains(key) || current == key));
        }
    }
}

/// Prove the kernel retains exactly the least occurrences of the prefix.
///
/// The second half of the induction obligation: the retained sequence is
/// `least_indices`, that prefix's least occurrences in increasing order.
proof fn model_retained_is_least(keys: Seq<u64>, processed: int)
    requires 0 <= processed <= keys.len(),
    ensures model_retained(keys, processed) == least_indices(keys, processed),
    decreases processed,
{
    if processed > 0 {
        model_retained_is_least(keys, processed - 1);
        model_seen_contains(keys, processed - 1, keys[processed - 1]);
        let current = keys[processed - 1];
        if model_seen(keys, processed - 1).contains(current) {
            assert(appears_before(keys, processed - 1, current));
            assert(!is_least_occurrence(keys, processed - 1));
            assert(model_retained(keys, processed) == model_retained(keys, processed - 1));
            assert(least_indices(keys, processed) == least_indices(keys, processed - 1));
        } else {
            assert(!appears_before(keys, processed - 1, current));
            assert(is_least_occurrence(keys, processed - 1));
            assert(model_retained(keys, processed)
                == model_retained(keys, processed - 1).push(processed - 1));
            assert(least_indices(keys, processed)
                == least_indices(keys, processed - 1).push(processed - 1));
        }
    }
}

/// State the five clauses the specification promises a retained sequence.
spec fn contract_ok(keys: Seq<u64>, retained: Seq<int>) -> bool {
    &&& forall |t: int| 0 <= t < retained.len() ==> 0 <= #[trigger] retained[t] < keys.len()
    &&& (forall |t: int| 0 <= t < retained.len()
            ==> !appears_before(keys, #[trigger] retained[t], keys[retained[t]]))
    &&& (forall |t: int, u: int| 0 <= t < u < retained.len()
            ==> #[trigger] retained[t] < #[trigger] retained[u])
    &&& (forall |t: int, u: int| 0 <= t < u < retained.len()
            ==> keys[#[trigger] retained[t]] != keys[#[trigger] retained[u]])
    &&& (forall |i: int| 0 <= i < keys.len() && is_least_occurrence(keys, i)
            ==> retained.contains(i))
}

/// Prove retained positions are in range and are least occurrences.
///
/// The input-range and minimality clauses: a retained index is strictly below
/// the prefix's length, and its key appears nowhere before it, so no later
/// equal value could have been retained in its place.
proof fn least_indices_position(keys: Seq<u64>, processed: int, t: int)
    requires 0 <= t < least_indices(keys, processed).len(), 0 <= processed <= keys.len(),
    ensures
        0 <= least_indices(keys, processed)[t] < processed,
        !appears_before(keys, least_indices(keys, processed)[t],
            keys[least_indices(keys, processed)[t]]),
    decreases processed,
{
    if processed > 0 {
        let earlier = least_indices(keys, processed - 1);
        if is_least_occurrence(keys, processed - 1) {
            assert(least_indices(keys, processed) == earlier.push(processed - 1));
            if t < earlier.len() {
                least_indices_position(keys, processed - 1, t);
            } else {
                assert(t == earlier.len());
                assert(least_indices(keys, processed)[t] == processed - 1);
            }
        } else {
            assert(least_indices(keys, processed) == earlier);
            least_indices_position(keys, processed - 1, t);
        }
    }
}

/// Prove distinct retained positions hold increasing indices, hence distinct
/// keys.
///
/// The strict index-ordering and key-uniqueness clauses: the ordering makes
/// "first representatives in input order" a statement about the output's
/// order, and the uniqueness rules out a later equal value standing in.
proof fn least_indices_ordered_distinct(keys: Seq<u64>, processed: int, t: int, u: int)
    requires 0 <= t < u < least_indices(keys, processed).len(), 0 <= processed <= keys.len(),
    ensures
        least_indices(keys, processed)[t] < least_indices(keys, processed)[u],
        keys[least_indices(keys, processed)[t]] != keys[least_indices(keys, processed)[u]],
    decreases processed,
{
    if processed > 0 {
        let earlier = least_indices(keys, processed - 1);
        if is_least_occurrence(keys, processed - 1) {
            assert(least_indices(keys, processed) == earlier.push(processed - 1));
            if u < earlier.len() {
                least_indices_ordered_distinct(keys, processed - 1, t, u);
            } else {
                assert(u == earlier.len());
                least_indices_position(keys, processed - 1, t);
                assert(keys[least_indices(keys, processed)[t]] != keys[processed - 1]);
            }
        } else {
            assert(least_indices(keys, processed) == earlier);
            least_indices_ordered_distinct(keys, processed - 1, t, u);
        }
    }
}

/// Prove every least occurrence of the prefix is retained somewhere.
///
/// The coverage clause, and the only lemma needing a two-parameter induction:
/// `processed` grows while `i` stays fixed, so the measure is the distance
/// between them rather than `processed` alone.
proof fn least_indices_covers(keys: Seq<u64>, processed: int, i: int)
    requires 0 <= i < processed <= keys.len(), is_least_occurrence(keys, i),
    ensures least_indices(keys, processed).contains(i),
    decreases processed - i,
{
    if processed == i + 1 {
        let earlier = least_indices(keys, i);
        assert(least_indices(keys, processed) == earlier.push(i));
        assert(least_indices(keys, processed)[earlier.len() as int] == i);
        assert(least_indices(keys, processed).contains(i));
    } else {
        assert(i < processed - 1);
        least_indices_covers(keys, processed - 1, i);
        let earlier = least_indices(keys, processed - 1);
        let current = least_indices(keys, processed);
        let j = choose |j: int| 0 <= j < earlier.len() && earlier[j] == i;
        if is_least_occurrence(keys, processed - 1) {
            assert(current == earlier.push(processed - 1));
            assert(current[j] == earlier[j]);
            assert(current.contains(i));
        } else {
            assert(current == earlier);
        }
    }
}

/// State the relation the walk maintains at every processed index.
///
/// The walk's bookkeeping, the kernel's seen-keys law as an invariant, and the
/// retained sequence having reached the specification. A state satisfying all
/// three refines the specification at that index.
spec fn state_ok(keys: Seq<u64>, seen: Set<u64>, retained: Seq<int>, processed: int) -> bool {
    &&& 0 <= processed <= keys.len()
    &&& forall |key: u64| #[trigger] seen.contains(key) <==> appears_before(keys, processed, key)
    &&& retained == least_indices(keys, processed)
}

/// State the relation the walk satisfies before it has processed anything.
proof fn state_ok_initial(keys: Seq<u64>)
    ensures state_ok(keys, Set::empty(), Seq::empty(), 0),
{
    assert(least_indices(keys, 0) == Seq::<int>::empty());
}

/// Report whether the runtime retains the value at `processed`.
spec fn runtime_retains(keys: Seq<u64>, seen: Set<u64>, processed: int) -> bool {
    !seen.contains(keys[processed])
}

/// Prove the runtime's retention test is the specification's predicate.
///
/// Given the invariant's seen-keys law at `processed`, the runtime retains the
/// index exactly when it is a least occurrence: an equality of predicates, not
/// a resemblance between two code paths.
proof fn runtime_retention_matches_spec(keys: Seq<u64>, seen: Set<u64>, processed: int)
    requires 0 <= processed < keys.len(),
        forall |key: u64| #[trigger] seen.contains(key) <==> appears_before(keys, processed, key),
    ensures runtime_retains(keys, seen, processed) <==> is_least_occurrence(keys, processed),
{
    if seen.contains(keys[processed]) {
        assert(appears_before(keys, processed, keys[processed]));
    } else {
        assert(!appears_before(keys, processed, keys[processed]));
    }
}

/// Prove the skip branch — the key was already seen — preserves the invariant.
proof fn state_ok_skip(keys: Seq<u64>, seen: Set<u64>, retained: Seq<int>, processed: int)
    requires
        state_ok(keys, seen, retained, processed),
        processed < keys.len(),
        seen.contains(keys[processed]),
    ensures state_ok(keys, seen, retained, processed + 1),
{
    runtime_retention_matches_spec(keys, seen, processed);
    assert(!is_least_occurrence(keys, processed));
    assert(least_indices(keys, processed + 1) == least_indices(keys, processed));
}

/// Prove the append branch — the key is new — preserves the invariant.
///
/// The branch inserts the key and appends the index, the two transitions
/// `least_indices` makes for a least occurrence.
proof fn state_ok_take(keys: Seq<u64>, seen: Set<u64>, retained: Seq<int>, processed: int)
    requires
        state_ok(keys, seen, retained, processed),
        processed < keys.len(),
        !seen.contains(keys[processed]),
    ensures state_ok(keys, seen.insert(keys[processed]), retained.push(processed), processed + 1),
{
    let current = keys[processed];
    runtime_retention_matches_spec(keys, seen, processed);
    assert(is_least_occurrence(keys, processed));
    assert(least_indices(keys, processed + 1)
        == least_indices(keys, processed).push(processed));
    assert forall |key: u64|
        seen.insert(current).contains(key) <==> appears_before(keys, processed + 1, key) by {
        assert(seen.insert(current).contains(key) <==> (seen.contains(key) || current == key));
        assert(appears_before(keys, processed + 1, key)
            <==> (appears_before(keys, processed, key) || current == key));
    }
}

/// Derive the contract's clauses from the invariant at the end of the walk.
///
/// Each clause is the corresponding `least_indices_*` lemma, with the invariant
/// supplying `retained == least_indices(keys, keys.len())`.
proof fn state_ok_contract(keys: Seq<u64>, seen: Set<u64>, retained: Seq<int>)
    requires state_ok(keys, seen, retained, keys.len() as int),
    ensures contract_ok(keys, retained),
{
    let total = keys.len() as int;
    assert forall |t: int| 0 <= t < retained.len() implies
        0 <= #[trigger] retained[t] < keys.len() by {
        least_indices_position(keys, total, t);
    }
    assert forall |t: int| 0 <= t < retained.len() implies
        !appears_before(keys, #[trigger] retained[t], keys[retained[t]]) by {
        least_indices_position(keys, total, t);
    }
    assert forall |t: int, u: int| 0 <= t < u < retained.len() implies
        #[trigger] retained[t] < #[trigger] retained[u] by {
        least_indices_ordered_distinct(keys, total, t, u);
    }
    assert forall |t: int, u: int| 0 <= t < u < retained.len() implies
        keys[#[trigger] retained[t]] != keys[#[trigger] retained[u]] by {
        least_indices_ordered_distinct(keys, total, t, u);
    }
    assert forall |i: int| 0 <= i < keys.len() && is_least_occurrence(keys, i) implies
        retained.contains(i) by {
        least_indices_covers(keys, total, i);
    }
}

/// Prove the runtime walk establishes the specification's contract.
///
/// The recursion is the algorithm's decision at each index — skip, leaving the
/// state alone, or take, inserting the key and appending the index — each
/// branch carrying the state through its step lemma, so that
/// `state_ok_contract` reads the clauses off the invariant at the end.
proof fn runtime_run(keys: Seq<u64>, seen: Set<u64>, retained: Seq<int>, index: int)
    requires state_ok(keys, seen, retained, index), index <= keys.len(),
    ensures contract_ok(keys, least_indices(keys, keys.len() as int)),
    decreases keys.len() - index,
{
    if index == keys.len() {
        state_ok_contract(keys, seen, retained);
    } else if seen.contains(keys[index]) {
        state_ok_skip(keys, seen, retained, index);
        runtime_run(keys, seen, retained, index + 1);
    } else {
        state_ok_take(keys, seen, retained, index);
        runtime_run(keys, seen.insert(keys[index]), retained.push(index), index + 1);
    }
}

/// Prove the runtime's walk refines the independent least-index specification.
///
/// This is the walk started where `stable_deduplicate` starts — the empty seen
/// set and the empty retained sequence — pinning the kernel's accumulation to
/// the specification so all three descriptions agree.
proof fn runtime_refines_least_indices(keys: Seq<u64>)
    ensures
        contract_ok(keys, least_indices(keys, keys.len() as int))
            && model_retained(keys, keys.len() as int) == least_indices(keys, keys.len() as int),
{
    model_retained_is_least(keys, keys.len() as int);
    state_ok_initial(keys);
    runtime_run(keys, Set::empty(), Seq::empty(), 0);
}

/// Provide the entry point Verus requires; it carries no proof of its own.
fn main() {}

} // verus!

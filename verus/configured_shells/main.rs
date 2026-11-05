//! A refinement proof for trusted configured-shell composition.
//!
//! Configuration composition accepts the concatenated definition sequence `D`
//! exactly when no name is a built-in and no two positions carry equal names.
//! That pairwise statement is the specification, [`accept`], and it is stated
//! over `D` itself rather than derived from any collection. The production
//! validator walks `D` from left to right, rejecting a reserved name or one
//! already seen, and [`walk`] restates that traversal. This file proves the two
//! agree at every length of `D`.
//!
//! A name is modelled as a byte sequence. That keeps name equality decidable
//! without a hash or any surrogate key, and it lets "already seen" be modelled
//! as membership in the accepted prefix rather than as a set: the
//! representation relationship the walk needs is `seen` ranges exactly over
//! `D[0..processed)`, which [`walk_prefix`] states directly. The production
//! `HashMap` used as a seen-set is not modelled here, so nothing below proves
//! its internals; the independent quadratic Rust oracle in
//! `src/cli/config/shells/tests/oracle.rs` checks the production function
//! against [`accept`] on real validated names.
//!
//! What is unbounded is sequence length: the induction runs over every length
//! of `D`, while the name domain is the byte strings the parser admits. No
//! `assume`, `admit`, external body, or axiom beyond `vstd`'s own appears
//! below.

use vstd::prelude::*;

verus! {

/// A shell name, modelled as its UTF-8 bytes.
type Name = Seq<u8>;

/// Report whether `name` is one of the four reserved built-in spellings.
///
/// The production code reads this set from `BuiltInShell`; the model restates
/// it so the specification does not depend on the implementation it constrains.
spec fn reserved(name: Name) -> bool {
    ||| name == seq![b's', b'h']
    ||| name == seq![b'b', b'a', b's', b'h']
    ||| name == seq![b'p', b'w', b's', b'h']
    ||| name == seq![b'p', b'o', b'w', b'e', b'r', b's', b'h', b'e', b'l', b'l']
}

/// Report whether an index strictly before `index` carries an equal name.
spec fn appears_before(names: Seq<Name>, index: int) -> bool {
    exists |j: int| 0 <= j < index && names[j] == names[index]
}

/// State that the first `processed` names are unreserved and pairwise distinct.
///
/// [`accept`] restricted to a prefix length. The walk's carried invariant is
/// stated over a prefix length rather than over a sliced sequence, so no lemma
/// about `subrange` composition is needed to relate one step to the next.
spec fn accept_prefix(names: Seq<Name>, processed: int) -> bool {
    &&& forall |i: int| 0 <= i < processed ==> !reserved(#[trigger] names[i])
    &&& forall |i: int, j: int| 0 <= i < j < processed
            ==> #[trigger] names[i] != #[trigger] names[j]
}

/// State the specification: no reserved name, and no two positions equal.
///
/// This is the pairwise condition the contract states. It mentions no
/// collection, no traversal order, and no partial state.
spec fn accept(names: Seq<Name>) -> bool {
    &&& forall |i: int| 0 <= i < names.len() ==> !reserved(#[trigger] names[i])
    &&& forall |i: int, j: int| 0 <= i < j < names.len()
            ==> #[trigger] names[i] != #[trigger] names[j]
}

/// Run the validator's recursion over the first `processed` names.
///
/// This is the production loop: test the current name against the reserved set
/// and against the already accepted prefix, then continue. The accepted prefix
/// is exactly `names[0..processed)`, which is the representation relationship
/// `seen == prefix` in concrete form.
spec fn walk_prefix(names: Seq<Name>, processed: int) -> bool
    decreases processed,
{
    if processed <= 0 {
        true
    } else {
        walk_prefix(names, processed - 1)
            && !reserved(names[processed - 1])
            && !appears_before(names, processed - 1)
    }
}

/// Run the validator over every name.
spec fn walk(names: Seq<Name>) -> bool {
    walk_prefix(names, names.len() as int)
}

/// Prove the specification at full length is its own prefix restriction.
proof fn accept_is_accept_prefix(names: Seq<Name>)
    ensures accept(names) <==> accept_prefix(names, names.len() as int),
{
}

/// Prove an accepted prefix restricts to an accepted shorter prefix.
proof fn accept_prefix_weaken(names: Seq<Name>, processed: int)
    requires 0 < processed <= names.len(),
    ensures accept_prefix(names, processed) ==> accept_prefix(names, processed - 1),
{
}

/// Prove the last entry of an accepted prefix is unreserved and fresh.
///
/// The forward half of the induction step: extending the prefix by one name
/// only adds the obligations this name must meet.
proof fn accept_prefix_forward(names: Seq<Name>, processed: int)
    requires
        0 < processed <= names.len(),
        accept_prefix(names, processed),
    ensures
        accept_prefix(names, processed - 1),
        !reserved(names[processed - 1]),
        !appears_before(names, processed - 1),
{
    let last = processed - 1;
    accept_prefix_weaken(names, processed);
    assert(!reserved(names[last]));
    if appears_before(names, last) {
        let j = choose |j: int| 0 <= j < last && names[j] == names[last];
        assert(names[j] == names[last]);
        assert(names[j] != names[last]);
    }
}

/// Prove a fresh, unreserved name extends an accepted prefix.
///
/// The backward half of the induction step: the new name meets its own two
/// obligations, and it cannot collide with an earlier name because
/// `appears_before` reports no such collision.
proof fn accept_prefix_backward(names: Seq<Name>, processed: int)
    requires
        0 < processed <= names.len(),
        accept_prefix(names, processed - 1),
        !reserved(names[processed - 1]),
        !appears_before(names, processed - 1),
    ensures accept_prefix(names, processed),
{
    let last = processed - 1;
    assert forall |i: int| 0 <= i < processed implies !reserved(#[trigger] names[i]) by {
        if i == last {
        } else {
            assert(0 <= i < last);
        }
    }
    assert forall |i: int, j: int| 0 <= i < j < processed implies
        #[trigger] names[i] != #[trigger] names[j] by {
        if j < last {
        } else {
            if names[i] == names[last] {
                assert(0 <= i < last);
                assert(appears_before(names, last));
            }
        }
    }
}

/// Prove the walk accepts exactly the specification's prefixes.
///
/// The induction carries the equivalence from one prefix length to the next,
/// using the two halves of the step above, so it holds at every length.
proof fn walk_prefix_is_accept_prefix(names: Seq<Name>, processed: int)
    requires 0 <= processed <= names.len(),
    ensures walk_prefix(names, processed) <==> accept_prefix(names, processed),
    decreases processed,
{
    if processed > 0 {
        walk_prefix_is_accept_prefix(names, processed - 1);
        if accept_prefix(names, processed) {
            accept_prefix_forward(names, processed);
            assert(walk_prefix(names, processed - 1));
            assert(walk_prefix(names, processed));
        }
        if walk_prefix(names, processed) {
            assert(accept_prefix(names, processed - 1));
            assert(!reserved(names[processed - 1]));
            assert(!appears_before(names, processed - 1));
            accept_prefix_backward(names, processed);
        }
    }
}

/// Prove the complete walk accepts exactly the specification's sequences.
///
/// The refinement statement: the traversal and the pairwise condition agree on
/// every sequence, at every length.
proof fn walk_is_accept(names: Seq<Name>)
    ensures walk(names) <==> accept(names),
{
    walk_prefix_is_accept_prefix(names, names.len() as int);
    accept_is_accept_prefix(names);
}

/// Exhibit accepted and rejected inputs, so the theorem is not vacuous.
///
/// The witnesses cover a successful registry, including the empty one and two
/// distinct names, a reserved name, and a duplicate introduced only in the last
/// position — each an input the walk must classify as the specification does.
proof fn non_vacuity_witnesses() {
    let empty: Seq<Name> = Seq::empty();
    let two: Seq<Name> = seq![
        seq![b'd', b'a', b's', b'h'],
        seq![b'c', b'u', b's', b't', b'o', b'm'],
    ];
    let built_in: Seq<Name> = seq![seq![b's', b'h']];
    let duplicate_last: Seq<Name> = seq![
        seq![b'd', b'a', b's', b'h'],
        seq![b'd', b'a', b's', b'h'],
    ];

    walk_is_accept(empty);
    walk_is_accept(two);
    walk_is_accept(built_in);
    walk_is_accept(duplicate_last);

    assert(walk(empty));
    assert(walk(two));
    assert(!walk(built_in));
    assert(!walk(duplicate_last));
}

/// Provide the entry point Verus requires; it carries no proof of its own.
fn main() {}

} // verus!

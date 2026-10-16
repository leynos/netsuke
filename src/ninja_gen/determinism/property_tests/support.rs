//! Strategies and helpers shared by the obligation suites.
//!
//! Each obligation in this plan states its own domain, but a few pieces are
//! the same wherever they appear: the path domain `OBL-PATHKEY` reasons
//! about, and the one place a graph is built from a specification. Keeping
//! them here rather than in each suite is what stops the two suites from
//! drifting into *slightly* different domains, which is a defect no gate
//! would catch — both would pass, and neither would be evidence about the
//! other.

use crate::ir::BuildGraph;
use crate::ninja_gen::determinism::strategy::graph::GraphSpec;
use camino::Utf8PathBuf;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

/// Greatest number of paths one generated list carries.
///
/// `OBL-PATHKEY` states its domain as 0 to 8 paths. The bound is small on
/// purpose: keying is a sort and a join, so a longer list adds cost without
/// reaching a shape the short lists cannot, and an injectivity violation needs
/// two draws to collide rather than a long one.
pub(crate) const MAX_PATHS: usize = 8;

/// Path lists over the keyable domain: non-empty, and free of NUL.
///
/// Both restrictions are preconditions of the injectivity claim rather than
/// conveniences. Joining with NUL cannot tell a separator from a NUL inside an
/// element, so `["a", "b"]` and `["a\0b"]` share one key; and an empty path is
/// indistinguishable from its absence, so `[]` and `[""]` do too. The
/// directed witnesses in `path_key.rs` record why each half is load-bearing.
///
/// The strategy constructs valid values rather than filtering for them.
/// Rejection sampling would spend the case budget discarding its own output,
/// and — worse — would make shrinking incoherent, since a rejected shrunk
/// candidate is indistinguishable from an uninteresting one.
pub(crate) fn keyable_paths() -> impl Strategy<Value = Vec<Utf8PathBuf>> {
    prop::collection::vec(keyable_path(), 0..=MAX_PATHS)
}

/// One non-empty path built from NUL-free characters.
fn keyable_path() -> impl Strategy<Value = Utf8PathBuf> {
    prop::collection::vec(non_nul_char(), 1..=8)
        .prop_map(|characters| Utf8PathBuf::from(characters.into_iter().collect::<String>()))
}

/// Any character except NUL, drawn without rejection.
///
/// The replacement is a `map` rather than a `prop_filter`, which keeps the
/// draw total. `\u{1}` is consequently over-represented; it is only ever a
/// path character, so the bias costs nothing.
fn non_nul_char() -> impl Strategy<Value = char> {
    any::<char>().prop_map(|character| {
        if character == '\0' {
            '\u{1}'
        } else {
            character
        }
    })
}

/// Build one materialization, reporting a generator defect as a test failure.
///
/// Returned rather than asserted so that a colliding pool reaches Proptest as
/// a shrinkable failure carrying the specification's shape, instead of as a
/// panic shrinking cannot reduce. The pool's slices are disjoint by
/// construction, so this is unreachable in a correct generator — which is
/// exactly why it must be reported rather than swallowed.
///
/// # Errors
///
/// Returns a test failure when the specification's pool slices collide.
pub(crate) fn materialize(spec: &GraphSpec, order: &[usize]) -> Result<BuildGraph, TestCaseError> {
    spec.materialize(order).map_err(|error| {
        TestCaseError::fail(format!("the pool's slices must not collide: {error}"))
    })
}

//! Bounded admission: the cost of tracking where admission currently is.
//!
//! The node and byte ceilings bound the value admission materializes, not the
//! bookkeeping it performs to report a refusal. This battery pins that
//! distinction: a wide collection beneath very long member names stays inside
//! every ceiling, so it must be admitted, and admitting it must cost work
//! proportional to its occurrences rather than to the ancestor bytes each
//! descendant could copy.
//!
//! The case drives the public surface, so it measures the traversal the
//! canonicalizer really runs rather than a stand-in for it.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Result, ensure};
use minijinja::value::Value;
use rstest::rstest;

use super::*;

use super::budget::{counted_sequence, pair_map};

/// Bytes of each member name in the wide-under-long-names fixture.
///
/// Four names this size charge exactly half the production byte ceiling, so the
/// fixture is admitted while every descendant's path carries megabytes of
/// ancestor names.
const PATH_NAME_BYTES: usize = 1 << 20;

/// Mappings stacked above the leaf sequence in the wide-under-long-names
/// fixture.
const PATH_LEVELS: usize = 4;

/// Items in the leaf sequence of the wide-under-long-names fixture.
///
/// Sized so the fixture clears the production node ceiling with a wide margin —
/// one root, four mapped occurrences, and this many items — while staying cheap
/// to build.
const PATH_LEAF_WIDTH: usize = 90_000;

/// Build a distinct member name of [`PATH_NAME_BYTES`] bytes for `level`.
///
/// The one-byte prefix keeps the four names distinct while the padding keeps
/// each level's byte charge identical, so no level is singled out by the
/// fixture.
fn long_member_name(level: usize) -> String {
    format!("{level}{}", "x".repeat(PATH_NAME_BYTES - 1))
}

/// Build a wide leaf sequence beneath [`PATH_LEVELS`] long-named mappings.
///
/// The counter records how many items admission pulled from the leaf, so a case
/// can show that the work tracks occurrences rather than the name bytes above
/// them.
fn wide_leaf_under_long_names() -> (Value, Arc<AtomicUsize>) {
    let (leaf, pulls) = counted_sequence(PATH_LEAF_WIDTH);
    let mut current = leaf;
    for level in 0..PATH_LEVELS {
        current = pair_map(
            vec![(Value::from(long_member_name(level)), current)],
            Some(1),
        );
    }
    (current, pulls)
}

/// Admission's path bookkeeping does not grow with ancestor-name bytes.
///
/// Traversal carries a borrowed chain, so extending a path for a child costs a
/// reference and an index or name reference however many bytes the ancestors'
/// names hold; the owned path is materialized only to render a refusal.
/// Extending an owned path per child instead would copy every ancestor name
/// once per descendant: four 1 MiB names above a 90,000-item sequence would
/// move roughly 360 GB during an admission that both ceilings allow, so the
/// accepted key this case asserts is precisely what such a copy could not
/// produce.
///
/// The pull count is the second half of the claim. Work follows occurrences,
/// not name bytes, so the leaf is pulled once per item and never re-read.
#[rstest]
fn path_bookkeeping_does_not_grow_with_ancestor_name_bytes() -> Result<()> {
    let (value, pulls) = wide_leaf_under_long_names();
    let key = canonical_key(&value).expect("the fixture is inside every default ceiling");
    ensure!(
        key.as_bytes().len() > PATH_NAME_BYTES,
        "the key must carry the four member names, got {} bytes",
        key.as_bytes().len(),
    );
    let pulled = pulls.load(Ordering::SeqCst);
    ensure!(
        pulled == PATH_LEAF_WIDTH,
        "the leaf must be pulled once per item, got {pulled} pulls",
    );
    Ok(())
}

//! The match collectors and the boundaries they stop at.
//!
//! The cases here count pulls rather than retained items, because the promise
//! is that a collector stops as soon as the ceiling is passed. That only holds
//! if the collector checks while it grows, so the count is the observable that
//! distinguishes the two implementations. The same count separates the two
//! failure paths a fallible collector has: a per-item error must end the
//! collection immediately, while the ceiling refusal must arrive one item past
//! the ceiling, from the collector's own check.

use googletest::prelude::*;

use super::*;

/// A caller error that receives a ceiling refusal through `From`.
///
/// `try_collect_matches` returns a per-item failure unchanged and converts
/// only its own ceiling refusal, so the caller's type has to differ from
/// [`StreamLimitExceeded`] for the conversion to be observable: with the
/// refusal type as `E` both paths would produce the same value.
#[derive(Clone, Debug, Eq, PartialEq)]
struct CeilingRefusal(StreamLimitExceeded);

impl From<StreamLimitExceeded> for CeilingRefusal {
    fn from(refusal: StreamLimitExceeded) -> Self {
        Self(refusal)
    }
}

/// An unbounded match stream is refused one item past the ceiling.
#[test]
fn match_collection_stops_one_item_past_the_ceiling() {
    let mut stream = CountingStream::new(std::iter::repeat(1_u64));
    let refused =
        collect_matches(stream.by_ref()).expect_err("an unbounded stream exceeds the ceiling");
    assert_that!(
        refused,
        eq(StreamLimitExceeded {
            kind: StreamKind::MatchCount,
            ceiling: MATCH_COUNT_CEILING,
        })
    );
    assert_eq!(
        stream.pulls(),
        100_001,
        "the collector must stop after one item past the ceiling"
    );
}

/// A stream holding exactly the ceiling is collected whole.
#[test]
fn match_collection_accepts_exactly_the_ceiling() {
    let mut stream = CountingStream::new(0..MATCH_COUNT_CEILING);
    let collected = collect_matches(stream.by_ref()).expect("exactly the ceiling is admitted");
    assert_eq!(
        collected.len(),
        usize::try_from(MATCH_COUNT_CEILING).expect("the ceiling fits a usize"),
        "every item up to the ceiling must be retained"
    );
    assert_eq!(
        collected.last().copied(),
        Some(MATCH_COUNT_CEILING - 1),
        "the last item must be the one before the ceiling"
    );
}

/// A per-item failure is returned unchanged, and nothing after it is pulled.
#[test]
fn try_collect_matches_stops_at_the_first_item_error() {
    let mut stream = CountingStream::new((0..10).map(|position| {
        if position == 4 {
            Err(StreamLimitExceeded {
                kind: StreamKind::MatchCount,
                ceiling: MATCH_COUNT_CEILING,
            })
        } else {
            Ok(position)
        }
    }));
    let refused = try_collect_matches(stream.by_ref()).expect_err("the fifth item fails");
    assert_that!(
        refused,
        eq(StreamLimitExceeded {
            kind: StreamKind::MatchCount,
            ceiling: MATCH_COUNT_CEILING,
        })
    );
    assert_eq!(
        stream.pulls(),
        5,
        "nothing after the failing item may be pulled"
    );
}

/// An unbounded fallible stream is refused one item past the ceiling.
///
/// The fallible collector shares the ceiling check with [`collect_matches`],
/// but nothing else proves it: the per-item failure case above stops at the
/// fifth item, so an implementation that dropped its own ceiling check would
/// stay green there and pull the whole stream here.
#[test]
fn try_collect_matches_stops_one_item_past_the_ceiling() {
    let mut stream = CountingStream::new(std::iter::repeat(Ok::<u64, StreamLimitExceeded>(1)));
    let refused =
        try_collect_matches(stream.by_ref()).expect_err("an unbounded stream exceeds the ceiling");
    assert_that!(
        refused,
        eq(StreamLimitExceeded {
            kind: StreamKind::MatchCount,
            ceiling: MATCH_COUNT_CEILING,
        })
    );
    assert_eq!(
        stream.pulls(),
        100_001,
        "the collector must stop after one item past the ceiling"
    );
}

/// A fallible stream holding exactly the ceiling is collected whole.
#[test]
fn try_collect_matches_accepts_exactly_the_ceiling() {
    let mut stream =
        CountingStream::new((0..MATCH_COUNT_CEILING).map(Ok::<u64, StreamLimitExceeded>));
    let collected = try_collect_matches(stream.by_ref()).expect("exactly the ceiling is admitted");
    assert_eq!(
        collected.len(),
        usize::try_from(MATCH_COUNT_CEILING).expect("the ceiling fits a usize"),
        "every item up to the ceiling must be retained"
    );
    assert_eq!(
        collected.last().copied(),
        Some(MATCH_COUNT_CEILING - 1),
        "the last item must be the one before the ceiling"
    );
}

/// The ceiling refusal reaches the caller as its own error type.
///
/// Every item is an `Ok`, so the only error this collection can produce is the
/// refusal `try_collect_matches` converts with `From`, which is how the
/// mapping path is exercised rather than assumed.
#[test]
fn try_collect_matches_converts_the_ceiling_refusal_for_the_caller() {
    let mut stream = CountingStream::new(std::iter::repeat(Ok::<u64, CeilingRefusal>(1)));
    let refused = try_collect_matches(stream.by_ref()).expect_err("the stream passes the ceiling");
    assert_eq!(
        refused,
        CeilingRefusal(StreamLimitExceeded {
            kind: StreamKind::MatchCount,
            ceiling: MATCH_COUNT_CEILING,
        }),
        "the ceiling refusal must arrive as the caller's error type"
    );
    assert_eq!(
        stream.pulls(),
        100_001,
        "the refusal must arrive after one item past the ceiling"
    );
}

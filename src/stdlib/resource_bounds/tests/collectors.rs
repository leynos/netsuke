//! The match collectors and the boundaries they stop at.
//!
//! The cases here count pulls rather than retained items, because the promise
//! is that a collector stops as soon as the ceiling is passed. That only holds
//! if the collector checks while it grows, so the count is the observable that
//! distinguishes the two implementations. The same count separates the two
//! failure paths a fallible collector has: a per-item error must end the
//! collection immediately, while the ceiling refusal must arrive one item past
//! the ceiling, from the collector's own check.
//!
//! The two boundary cases are written once and parameterised by [`Collector`],
//! because the infallible and fallible collectors promise the same ceiling
//! behaviour. Only the item type differs, so a case that does not depend on
//! failure is the same case for both.

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

/// Which collector a shared boundary case drives.
///
/// The fallible variant uses [`StreamLimitExceeded`] as its error type so both
/// variants answer `Result<Vec<u64>, StreamLimitExceeded>`, which is what lets
/// one set of assertions cover the two ceilings.
#[derive(Clone, Copy, Debug)]
enum Collector {
    /// `collect_matches`, over items that cannot fail.
    Total,
    /// `try_collect_matches`, over items that may be an error.
    Fallible,
}

impl Collector {
    /// Pull from an endless stream of ones until the collector stops.
    ///
    /// The answer is paired with the pull count, so a case can assert on where
    /// the collector stopped rather than inferring it from what it kept.
    fn stop_on_endless_ones(self) -> (Result<Vec<u64>, StreamLimitExceeded>, usize) {
        match self {
            Self::Total => {
                let mut stream = CountingStream::new(std::iter::repeat(1_u64));
                (collect_matches(stream.by_ref()), stream.pulls())
            }
            Self::Fallible => {
                let mut stream =
                    CountingStream::new(std::iter::repeat(Ok::<u64, StreamLimitExceeded>(1)));
                (try_collect_matches(stream.by_ref()), stream.pulls())
            }
        }
    }

    /// Pull exactly the ceiling's worth of items, returning what was kept.
    fn collect_exactly_the_ceiling(self) -> Vec<u64> {
        match self {
            Self::Total => {
                let mut stream = CountingStream::new(0..MATCH_COUNT_CEILING);
                collect_matches(stream.by_ref()).expect("exactly the ceiling is admitted")
            }
            Self::Fallible => {
                let mut stream = CountingStream::new(
                    (0..MATCH_COUNT_CEILING).map(Ok::<u64, StreamLimitExceeded>),
                );
                try_collect_matches(stream.by_ref()).expect("exactly the ceiling is admitted")
            }
        }
    }
}

/// Assert that an endless stream is refused after one pull past the ceiling.
///
/// The refusal is matched as a value rather than by variant, so a collector
/// that reported the wrong stream kind or ceiling would fail here too.
fn assert_stops_one_item_past_the_ceiling(collector: Collector) {
    let (outcome, pulls) = collector.stop_on_endless_ones();
    let refused = outcome.expect_err("an unbounded stream exceeds the ceiling");
    assert_that!(
        refused,
        eq(StreamLimitExceeded {
            kind: StreamKind::MatchCount,
            ceiling: MATCH_COUNT_CEILING,
        })
    );
    assert_eq!(
        pulls, 100_001,
        "the collector must stop after one item past the ceiling"
    );
}

/// Assert that a stream holding exactly the ceiling is collected whole.
fn assert_accepts_exactly_the_ceiling(collector: Collector) {
    let collected = collector.collect_exactly_the_ceiling();
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

/// An unbounded match stream is refused one item past the ceiling.
#[test]
fn match_collection_stops_one_item_past_the_ceiling() {
    assert_stops_one_item_past_the_ceiling(Collector::Total);
}

/// A stream holding exactly the ceiling is collected whole.
#[test]
fn match_collection_accepts_exactly_the_ceiling() {
    assert_accepts_exactly_the_ceiling(Collector::Total);
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
    assert_stops_one_item_past_the_ceiling(Collector::Fallible);
}

/// A fallible stream holding exactly the ceiling is collected whole.
#[test]
fn try_collect_matches_accepts_exactly_the_ceiling() {
    assert_accepts_exactly_the_ceiling(Collector::Fallible);
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

//! Streaming ceilings for match counts and output bytes.
//!
//! A stream cannot be measured before it is walked, so its bound is applied as
//! it grows: [`RunningTotal`] adds one amount at a time and refuses the addition
//! that would pass the ceiling for its [`StreamKind`]. The two collectors use
//! that counter, which means the "check as it grows" rule has one
//! implementation rather than one per consumer.
//!
//! # Why the counter is neither `Copy` nor `Clone`
//!
//! A stream-wide total must be shared by reference across everything it counts
//! — every document of `from_yaml_all`, every entry of a directory walk. A
//! counter passed by value would restart at zero per document and never reach
//! its ceiling, which is exactly the defect this module exists to prevent.

use std::fmt;

use super::{MATCH_COUNT_CEILING, OUTPUT_BYTES_CEILING, kernel};

/// A quantity counted as it grows; each kind fixes its own ceiling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamKind {
    /// A number of matches collected from one traversal.
    MatchCount,
    /// A number of output bytes accumulated across a stream.
    OutputBytes,
}

impl StreamKind {
    /// Return the ceiling this kind is checked against.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::stdlib::{MATCH_COUNT_CEILING, OUTPUT_BYTES_CEILING, StreamKind};
    ///
    /// assert_eq!(StreamKind::MatchCount.ceiling(), MATCH_COUNT_CEILING);
    /// assert_eq!(StreamKind::OutputBytes.ceiling(), OUTPUT_BYTES_CEILING);
    /// ```
    #[must_use]
    pub const fn ceiling(self) -> u64 {
        match self {
            Self::MatchCount => MATCH_COUNT_CEILING,
            Self::OutputBytes => OUTPUT_BYTES_CEILING,
        }
    }
}

/// A checked running total for one stream.
///
/// Start one with [`RunningTotal::new`] and pass it by mutable reference to
/// every step of the stream. A refused addition leaves the total unchanged, so
/// the caller can report the refusal and abandon the stream without having to
/// reason about a partially applied step.
#[derive(Debug)]
pub struct RunningTotal {
    /// The quantity this total counts.
    kind: StreamKind,
    /// The sum admitted so far.
    admitted: u64,
    /// The ceiling applied to that sum.
    ceiling: u64,
}

impl RunningTotal {
    /// Start a total at zero for `kind`.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::stdlib::{RunningTotal, StreamKind};
    ///
    /// let total = RunningTotal::new(StreamKind::MatchCount);
    /// assert_eq!(total.admitted(), 0);
    /// ```
    #[must_use]
    pub const fn new(kind: StreamKind) -> Self {
        Self {
            kind,
            admitted: 0,
            ceiling: kind.ceiling(),
        }
    }

    /// Start a total at zero against an explicit ceiling.
    ///
    /// Available to tests and Kani only. The public constructor takes its
    /// ceiling from the stream kind, so a production caller cannot pair an
    /// amount with a bound that is not the table 3 bound for it.
    #[cfg(any(test, kani))]
    #[must_use]
    pub(super) const fn with_ceiling(kind: StreamKind, ceiling: u64) -> Self {
        Self {
            kind,
            admitted: 0,
            ceiling,
        }
    }

    /// Add `amount` to the total, or refuse it.
    ///
    /// # Errors
    ///
    /// Returns [`StreamLimitExceeded`] when the checked sum would exceed this
    /// total's ceiling, including when the sum itself overflows `u64`. The
    /// total is left unchanged on refusal.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::stdlib::{RunningTotal, StreamKind};
    ///
    /// let mut total = RunningTotal::new(StreamKind::OutputBytes);
    /// total.admit(1_024).expect("one kibibyte is within the ceiling");
    /// total.admit_one().expect("one more byte is within the ceiling");
    /// assert_eq!(total.admitted(), 1_025);
    /// ```
    pub fn admit(&mut self, amount: u64) -> Result<(), StreamLimitExceeded> {
        let total = kernel::admit_total(self.ceiling, self.admitted, amount).ok_or(
            StreamLimitExceeded {
                kind: self.kind,
                ceiling: self.ceiling,
            },
        )?;
        self.admitted = total;
        Ok(())
    }

    /// Add one to the total, or refuse it.
    ///
    /// # Errors
    ///
    /// Returns [`StreamLimitExceeded`] exactly as [`Self::admit`] does.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::stdlib::{RunningTotal, StreamKind};
    ///
    /// let mut total = RunningTotal::new(StreamKind::MatchCount);
    /// total.admit_one().expect("the first match is within the ceiling");
    /// assert_eq!(total.admitted(), 1);
    /// ```
    pub fn admit_one(&mut self) -> Result<(), StreamLimitExceeded> {
        self.admit(1)
    }

    /// Return the total admitted so far.
    #[must_use]
    pub const fn admitted(&self) -> u64 {
        self.admitted
    }
}

/// Refusal raised when a running total would pass its stream's ceiling.
///
/// The refusal names the kind and the ceiling but not a total, because a
/// stream stops at the first excess item and its true total is then unknown.
/// Reporting the running value would present an estimate as a count (D-08).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamLimitExceeded {
    /// The quantity that passed its ceiling.
    pub kind: StreamKind,
    /// The ceiling that quantity passed.
    pub ceiling: u64,
}

/// `StreamLimitExceeded` crosses `E: From<StreamLimitExceeded>` bounds in
/// [`try_collect_matches`], whose callers report it through `anyhow::Error`,
/// which requires the `std::error::Error` trait.
impl std::error::Error for StreamLimitExceeded {}

impl fmt::Display for StreamLimitExceeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            StreamKind::MatchCount => {
                write!(
                    f,
                    "the match stream exceeds the ceiling of {} matches",
                    self.ceiling
                )
            }
            StreamKind::OutputBytes => {
                write!(
                    f,
                    "the output exceeds the ceiling of {} bytes",
                    self.ceiling
                )
            }
        }
    }
}

/// Collect at most [`MATCH_COUNT_CEILING`] items, pulling at most one more.
///
/// The count is checked as the collection grows, so a stream longer than the
/// ceiling is refused after one item past it rather than after the whole
/// stream has been materialized.
///
/// # Errors
///
/// Returns [`StreamLimitExceeded`] with [`StreamKind::MatchCount`] when the
/// stream holds more items than the ceiling.
///
/// # Examples
///
/// ```
/// use netsuke::stdlib::collect_matches;
///
/// let matches = collect_matches(["alpha", "beta"]).expect("two matches fit");
/// assert_eq!(matches, ["alpha", "beta"]);
/// ```
pub fn collect_matches<I: IntoIterator>(items: I) -> Result<Vec<I::Item>, StreamLimitExceeded> {
    let mut total = RunningTotal::new(StreamKind::MatchCount);
    let mut collected = Vec::new();
    for item in items {
        total.admit_one()?;
        collected.push(item);
    }
    Ok(collected)
}

/// Collect items that may fail, stopping at the first error or at the ceiling.
///
/// A per-match failure — a missing capture group, a directory entry that
/// cannot be read — is returned unchanged rather than wrapped, and nothing
/// after it is pulled.
///
/// # Errors
///
/// Returns the first item error, or [`StreamLimitExceeded`] converted through
/// `E`'s `From` implementation when the stream holds more items than
/// [`MATCH_COUNT_CEILING`].
///
/// # Examples
///
/// ```
/// use netsuke::stdlib::{StreamLimitExceeded, try_collect_matches};
///
/// let items = [Ok::<_, StreamLimitExceeded>(1_u8), Ok(2)];
/// assert_eq!(try_collect_matches(items).expect("two matches fit"), [1, 2]);
/// ```
pub fn try_collect_matches<T, E, I>(items: I) -> Result<Vec<T>, E>
where
    I: IntoIterator<Item = Result<T, E>>,
    E: From<StreamLimitExceeded>,
{
    let mut total = RunningTotal::new(StreamKind::MatchCount);
    let mut collected = Vec::new();
    for item in items {
        let value = item?;
        total.admit_one().map_err(E::from)?;
        collected.push(value);
    }
    Ok(collected)
}

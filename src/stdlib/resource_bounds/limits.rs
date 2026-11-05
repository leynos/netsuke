//! Input-length and nesting-depth ceilings.
//!
//! Both bounds are single comparisons, so they need no arithmetic kernel and no
//! proof obligation; what they do need is a shape that cannot be used with the
//! wrong quantity. [`admit_input`] takes bytes rather than a length, so a
//! character count cannot be passed where a byte count is meant, and
//! [`NestingDepth`] is the only type that can report a depth, so a caller
//! cannot invent an observed depth it did not measure.
//!
//! # Depth semantics
//!
//! The root value sits at depth zero and entering a container moves to the
//! next level, so 128 levels are admitted and the attempt to enter the 129th
//! is refused with observed depth 129. `serde_json` admits one level fewer
//! (127) and `serde-saphyr` recurses while parsing, so this bound only guards
//! recursion Netsuke owns and must be checked before the recursive step. The
//! reconciliation with parser-internal limits belongs to roadmap 6.2.1.

use thiserror::Error;

use super::{INPUT_LENGTH_CEILING, NESTING_DEPTH_CEILING};

/// Refusal raised when an input exceeds [`INPUT_LENGTH_CEILING`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
#[error("input length {length} bytes exceeds the ceiling of {ceiling} bytes")]
pub struct InputLengthExceeded {
    /// The measured byte length of the input.
    pub length: u64,
    /// The ceiling that length exceeded.
    pub ceiling: u64,
}

/// Admit an input whose byte length is within [`INPUT_LENGTH_CEILING`].
///
/// The check runs before the input is parsed, and measures bytes rather than
/// characters: a multibyte string is judged by what the parser must scan.
///
/// # Errors
///
/// Returns [`InputLengthExceeded`] carrying the exact byte length and the
/// ceiling when the input is longer than the ceiling.
///
/// # Examples
///
/// ```
/// use netsuke::stdlib::admit_input;
///
/// admit_input(b"a small manifest").expect("sixteen bytes are within the ceiling");
///
/// // Two characters, three bytes: the ceiling counts bytes.
/// admit_input("é!").expect("three bytes are within the ceiling");
/// ```
pub fn admit_input(input: impl AsRef<[u8]>) -> Result<(), InputLengthExceeded> {
    admit_length(measured_length(input.as_ref()))
}

/// Admit a byte length against [`INPUT_LENGTH_CEILING`].
///
/// Kept separate from [`admit_input`] so a boundary case can be stated as a
/// number, without allocating an eight-megabyte buffer to measure it.
pub(super) const fn admit_length(length: u64) -> Result<(), InputLengthExceeded> {
    if length <= INPUT_LENGTH_CEILING {
        Ok(())
    } else {
        Err(InputLengthExceeded {
            length,
            ceiling: INPUT_LENGTH_CEILING,
        })
    }
}

/// Measure a slice's byte length as a `u64`.
///
/// Every supported target has a `usize` no wider than 64 bits, so the
/// conversion cannot narrow. The fallback keeps the function total without a
/// panic and still refuses an impossible length rather than admitting it.
fn measured_length(bytes: &[u8]) -> u64 {
    u64::try_from(bytes.len()).unwrap_or(u64::MAX)
}

/// The number of containers enclosing the value currently being visited.
///
/// Each descent is a checked step, so a recursive helper advances the depth as
/// it recurses and refuses before entering the container that would exceed the
/// ceiling. A depth value is obtained from [`Self::root`] and advanced only by
/// [`Self::descend`], so it always reports a depth the caller measured.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NestingDepth(u32);

impl NestingDepth {
    /// Return the depth of a root value.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::stdlib::NestingDepth;
    ///
    /// assert_eq!(NestingDepth::root().level(), 0);
    /// ```
    #[must_use]
    pub const fn root() -> Self {
        Self(0)
    }

    /// Return how many containers enclose the value at this depth.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::stdlib::NestingDepth;
    ///
    /// let first = NestingDepth::root().descend().expect("one level is admitted");
    /// assert_eq!(first.level(), 1);
    /// ```
    #[must_use]
    pub const fn level(self) -> u32 {
        self.0
    }

    /// Descend into one container, or refuse the level beyond the ceiling.
    ///
    /// # Errors
    ///
    /// Returns [`DepthExceeded`] carrying the attempted depth and
    /// [`NESTING_DEPTH_CEILING`] when this descent would enter a level deeper
    /// than the ceiling. The receiver is unchanged, so a refused descent can
    /// be reported without losing the depth that produced it.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::stdlib::{DepthExceeded, NestingDepth};
    ///
    /// let at_ceiling = (0..128).try_fold(NestingDepth::root(), |depth, _| depth.descend())
    ///     .expect("128 levels are admitted");
    /// assert_eq!(at_ceiling.level(), 128);
    ///
    /// let refused = at_ceiling.descend().expect_err("the 129th level is refused");
    /// assert_eq!(refused, DepthExceeded { depth: 129, ceiling: 128 });
    /// ```
    pub const fn descend(self) -> Result<Self, DepthExceeded> {
        let depth = self.0.saturating_add(1);
        if depth <= NESTING_DEPTH_CEILING {
            Ok(Self(depth))
        } else {
            Err(DepthExceeded {
                depth,
                ceiling: NESTING_DEPTH_CEILING,
            })
        }
    }
}

/// Refusal raised when a value is nested beyond [`NESTING_DEPTH_CEILING`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
#[error("nesting depth {depth} exceeds the ceiling of {ceiling} levels")]
pub struct DepthExceeded {
    /// The depth the caller attempted to enter.
    pub depth: u32,
    /// The ceiling that depth exceeded.
    pub ceiling: u32,
}

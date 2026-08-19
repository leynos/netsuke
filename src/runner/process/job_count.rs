//! Validated Ninja parallel-job counts.
//!
//! The CLI accepts an absent job count (Ninja's own default) or a value
//! between 1 and [`MAX_JOBS`]. Callers without CLI state reach the process
//! layer directly, so the field in [`super::NinjaProcessOptions`] carries this
//! sealed type: construction is fallible, and an out-of-range count cannot be
//! fed to a `-j` flag from any call site.

use std::{fmt, io};

/// Maximum number of parallel Ninja jobs accepted by the process layer.
///
/// Mirrors the CLI layer's constants (`cli::MAX_JOBS` and the build-script
/// variant); the process layer cannot import them without depending on CLI
/// internals, so they must be kept in step as a single documented bound.
const MAX_JOBS: usize = 64;

/// A validated count of parallel Ninja jobs.
///
/// The supported semantics match the CLI's job-count validation: the value is
/// always in `1..=MAX_JOBS`. The field is private, so every instance must pass
/// through [`NinjaJobCount::try_new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NinjaJobCount(usize);

impl NinjaJobCount {
    /// Validate a requested job count.
    ///
    /// # Errors
    ///
    /// Returns [`io::ErrorKind::InvalidInput`] when `value` lies outside
    /// `1..=MAX_JOBS`.
    pub fn try_new(value: usize) -> io::Result<Self> {
        if (1..=MAX_JOBS).contains(&value) {
            Ok(Self(value))
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Ninja job count must be between 1 and {MAX_JOBS}, got {value}"),
            ))
        }
    }
}

impl fmt::Display for NinjaJobCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    //! Boundary coverage for the job-count invariant.

    use super::*;

    #[test]
    fn accepts_the_supported_range_boundaries() {
        for value in [1, MAX_JOBS] {
            let count = NinjaJobCount::try_new(value)
                .unwrap_or_else(|_| panic!("{value} should be a supported job count"));
            assert_eq!(
                count.to_string(),
                value.to_string(),
                "the accepted count should keep its value"
            );
        }
    }

    #[test]
    fn rejects_counts_outside_the_supported_range() {
        for value in [0, MAX_JOBS + 1] {
            let Err(error) = NinjaJobCount::try_new(value) else {
                panic!("{value} should be rejected as a job count");
            };
            assert_eq!(
                error.kind(),
                io::ErrorKind::InvalidInput,
                "an out-of-range count should be an invalid-input error"
            );
        }
    }
}

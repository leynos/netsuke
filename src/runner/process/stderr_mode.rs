//! Policy for routing a Ninja subprocess's standard streams.
//!
//! [`StderrMode`] carries the child-output routing decision explicitly instead
//! of burying it in a boolean re-derived inside the process layer. The runner
//! chooses the policy at the runner boundary when it builds a request: in JSON
//! diagnostics mode ([`crate::cli::Cli`]`::json`) the child's output must not
//! pollute the machine-readable streams, so the mode maps to [`Suppress`];
//! otherwise it maps to [`Forward`]. The policy travels on
//! [`crate::runner::NinjaBuildRequest`] and [`crate::runner::NinjaToolRequest`]
//! as the `stderr_mode` field, and the process layer only consumes that field —
//! it never re-derives the policy from CLI state itself.
//!
//! [`Forward`] releases the child's stdout and stderr to the parent's
//! corresponding streams, preserving ordering for builds whose output the user
//! watches live. [`Suppress`] drains both streams to `io::sink()`, keeping JSON
//! diagnostics machine-readable: stdout carries only the versioned result
//! document and stderr only the diagnostic document, with no child output mixed
//! in.

/// Policy for routing a Ninja subprocess's standard streams.
///
/// Governs both child stdout and child stderr routing: [`StderrMode::Suppress`]
/// keeps JSON diagnostics machine-readable by draining both streams to
/// `io::sink()`, while [`StderrMode::Forward`] releases them to the parent's
/// corresponding streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StderrMode {
    /// Forward child stdout and stderr to the parent's streams.
    Forward,
    /// Drain both child streams, discarding their output.
    Suppress,
}

impl StderrMode {
    /// Derive the policy from whether JSON diagnostics are enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::runner::StderrMode;
    ///
    /// assert_eq!(StderrMode::from_json_enabled(true), StderrMode::Suppress);
    /// assert_eq!(StderrMode::from_json_enabled(false), StderrMode::Forward);
    /// ```
    #[must_use]
    pub const fn from_json_enabled(json: bool) -> Self {
        if json { Self::Suppress } else { Self::Forward }
    }

    /// Return `true` when the policy drains child streams to `io::sink()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::runner::StderrMode;
    ///
    /// assert!(StderrMode::Suppress.is_suppress());
    /// assert!(!StderrMode::Forward.is_suppress());
    /// ```
    #[must_use]
    pub const fn is_suppress(self) -> bool {
        matches!(self, Self::Suppress)
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `StderrMode` policy derivation.

    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(true, StderrMode::Suppress)]
    #[case(false, StderrMode::Forward)]
    fn from_json_enabled_maps_boolean(#[case] json: bool, #[case] expected: StderrMode) {
        assert_eq!(StderrMode::from_json_enabled(json), expected);
    }

    #[rstest]
    #[case(StderrMode::Suppress, true)]
    #[case(StderrMode::Forward, false)]
    fn is_suppress_reflects_variant(#[case] mode: StderrMode, #[case] expected: bool) {
        assert_eq!(mode.is_suppress(), expected);
    }
}

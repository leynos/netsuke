//! Child standard-stream routing policy for Ninja subprocesses.

use crate::cli::Cli;

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
    #[must_use]
    pub const fn from_json_enabled(json: bool) -> Self {
        if json { Self::Suppress } else { Self::Forward }
    }

    /// Derive the policy from the resolved CLI diagnostics preference.
    #[must_use]
    pub const fn from_cli(cli: &Cli) -> Self {
        Self::from_json_enabled(cli.json)
    }

    /// Return `true` when the policy drains child streams to `io::sink()`.
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

    #[test]
    fn from_cli_delegates_to_json_setting() {
        let human = Cli::default();
        assert_eq!(StderrMode::from_cli(&human), StderrMode::Forward);

        let json = Cli {
            json: true,
            ..Cli::default()
        };
        assert_eq!(StderrMode::from_cli(&json), StderrMode::Suppress);
    }
}

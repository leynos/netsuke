//! Bounded rejection reasons for recipe-command interpolation.
//!
//! Interpolation validates text that may already contain values resolved from
//! the environment, because `env()` runs while the manifest is rendered. A
//! diagnostic therefore names the syntax that failed instead of quoting the
//! command: quoting it would copy a permitted secret into stderr in human and
//! JSON output alike. Each description is fixed English text inserted into a
//! localized sentence, matching how heredoc-delimiter rejections report
//! themselves.

use super::IrGenError;
use super::posix_lexical::{DelimiterRejection, HeredocActivationRejection};
use crate::localization::{self, keys};

/// Explain why a recipe command failed interpolation validation.
///
/// Variants are deliberately coarse: each names a class of rejection whose
/// remedy is to move or remove a marker, so the message stays useful without
/// echoing any part of the recipe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RejectionReason {
    /// A marker appeared where the shell context forbids substitution.
    MarkerInProtectedRegion,
    /// A marker appeared inside a PowerShell here-string body.
    MarkerInHereString,
    /// A marker appeared in raw native-command arguments.
    MarkerInRawArgument,
    /// An escape sequence would have altered the marker that follows it.
    EscapeBeforeMarker,
    /// The traversal lost the source position it was substituting at.
    UnsupportedMarkerPosition,
    /// The rendered command contains an odd number of backticks.
    UnmatchedBacktick,
    /// The rendered command is not parseable as a shell command.
    UnparseableCommand,
}

impl RejectionReason {
    /// Describe the rejection in bounded text that is safe to render.
    const fn description(self) -> &'static str {
        match self {
            Self::MarkerInProtectedRegion => "recipe marker inside a protected shell region",
            Self::MarkerInHereString => "recipe marker inside a PowerShell here-string",
            Self::MarkerInRawArgument => "recipe marker inside raw native command arguments",
            Self::EscapeBeforeMarker => "escape sequence before a recipe marker",
            Self::UnsupportedMarkerPosition => "recipe marker at an unsupported source position",
            Self::UnmatchedBacktick => "unmatched backtick",
            Self::UnparseableCommand => "unparseable shell syntax",
        }
    }
}

/// Build the diagnostic for a command rejected during placeholder expansion.
pub(super) fn invalid_command_error(command: String, reason: RejectionReason) -> IrGenError {
    invalid_command_message(command, reason.description())
}

/// Build the diagnostic for a heredoc delimiter the tracker cannot model.
///
/// The failure is discovered while copying a declaration, but the remedy is
/// the same as for any other interpolation rejection, so it travels on the
/// same localized message and names the delimiter syntax that failed.
pub(super) fn invalid_heredoc_delimiter_error(
    command: String,
    reason: DelimiterRejection,
) -> IrGenError {
    invalid_command_message(command, reason.description())
}

/// Build the diagnostic for a heredoc body the tracker cannot start safely.
///
/// The body boundary fails closed for the same reason the delimiter parser
/// does, so the rejection joins the same localized message and names the
/// boundary that could not be placed rather than quoting the recipe.
pub(super) fn invalid_heredoc_boundary_error(
    command: String,
    reason: HeredocActivationRejection,
) -> IrGenError {
    invalid_command_message(command, reason.description())
}

/// Assemble the shared rejection diagnostic from bounded text alone.
///
/// The reason is a `&'static str` rather than an owned string so the type
/// rules out quoting rendered recipe text: only fixed descriptions reach the
/// message that both human and JSON output render.
fn invalid_command_message(command: String, reason: &'static str) -> IrGenError {
    IrGenError::InvalidCommand {
        command,
        reason,
        message: localization::message(keys::IR_INVALID_COMMAND).with_arg("reason", reason),
    }
}

#[cfg(test)]
mod tests {
    //! Prove a rejection names its syntax without quoting the recipe.
    //!
    //! The tables bind every variant to the exact fixed text it renders, so a
    //! swapped or emptied description fails even though the type system already
    //! rules out quoting the command.

    use super::super::posix_lexical::DelimiterRejection;
    use super::{RejectionReason, invalid_command_error, invalid_heredoc_delimiter_error};
    use crate::ir::IrGenError;
    use proptest::prelude::*;
    use rstest::rstest;

    /// A token that must never reach the rendered message.
    const SECRET: &str = "s3cr3t-value";

    /// Build a rejected command that embeds [`SECRET`].
    fn secret_command() -> String {
        format!("echo {SECRET} `")
    }

    /// The message must carry the reason and never the rejected command.
    #[test]
    fn rejection_message_omits_the_command() {
        let command = secret_command();
        let error = invalid_command_error(command.clone(), RejectionReason::UnmatchedBacktick);
        let rendered = error.to_string();
        assert!(
            !rendered.contains(SECRET) && !rendered.contains(&command),
            "the rejection must name the syntax, not quote the command: {rendered}"
        );
        assert!(
            rendered.contains("unmatched backtick"),
            "the rejection must still name the failed syntax: {rendered}"
        );
        assert!(
            matches!(
                error,
                IrGenError::InvalidCommand { reason, .. } if reason == "unmatched backtick"
            ),
            "the reason field must carry the bounded description: {error:?}"
        );
    }

    /// Every variant must reach the message with its own fixed description.
    ///
    /// The description is also asserted exactly, so swapping two variants or
    /// returning a constant string fails at the table row that changed.
    #[rstest]
    #[case::protected_region(
        RejectionReason::MarkerInProtectedRegion,
        "recipe marker inside a protected shell region"
    )]
    #[case::here_string(
        RejectionReason::MarkerInHereString,
        "recipe marker inside a PowerShell here-string"
    )]
    #[case::raw_argument(
        RejectionReason::MarkerInRawArgument,
        "recipe marker inside raw native command arguments"
    )]
    #[case::escape_before_marker(
        RejectionReason::EscapeBeforeMarker,
        "escape sequence before a recipe marker"
    )]
    #[case::unsupported_position(
        RejectionReason::UnsupportedMarkerPosition,
        "recipe marker at an unsupported source position"
    )]
    #[case::unmatched_backtick(RejectionReason::UnmatchedBacktick, "unmatched backtick")]
    #[case::unparseable(RejectionReason::UnparseableCommand, "unparseable shell syntax")]
    fn every_rejection_reason_renders_its_fixed_text(
        #[case] reason: RejectionReason,
        #[case] expected: &str,
    ) {
        let command = secret_command();
        let error = invalid_command_error(command.clone(), reason);
        let rendered = error.to_string();
        assert!(
            matches!(
                &error,
                IrGenError::InvalidCommand { reason: actual, .. } if actual == &expected
            ),
            "the reason field must carry {expected:?}: {error:?}"
        );
        assert!(
            rendered.contains(expected),
            "the rendered message must name {expected:?}: {rendered}"
        );
        assert!(
            !rendered.contains(&command) && !rendered.contains(SECRET),
            "the rendered message must not quote the command: {rendered}"
        );
    }

    /// Every delimiter rejection must keep its own fixed description.
    #[rstest]
    #[case::missing(
        DelimiterRejection::MissingDelimiter,
        "unsupported heredoc delimiter syntax: missing delimiter"
    )]
    #[case::unclosed_quote(
        DelimiterRejection::UnclosedQuote,
        "unsupported heredoc delimiter syntax: unclosed quote"
    )]
    #[case::trailing_backslash(
        DelimiterRejection::TrailingBackslash,
        "unsupported heredoc delimiter syntax: trailing backslash"
    )]
    #[case::line_continuation(
        DelimiterRejection::LineContinuation,
        "unsupported heredoc delimiter syntax: line continuation"
    )]
    #[case::expansion(
        DelimiterRejection::UnsupportedExpansion,
        "unsupported heredoc delimiter syntax: expansion"
    )]
    #[case::command_substitution(
        DelimiterRejection::UnsupportedCommandSubstitution,
        "unsupported heredoc delimiter syntax: command substitution"
    )]
    #[case::rendered_delimiter(
        DelimiterRejection::RenderedDelimiterInvalid,
        "unsupported heredoc delimiter syntax: rendered delimiter"
    )]
    fn every_delimiter_rejection_renders_its_fixed_text(
        #[case] reason: DelimiterRejection,
        #[case] expected: &str,
    ) {
        let command = format!("cat <<{SECRET}\nbody\n");
        let error = invalid_heredoc_delimiter_error(command.clone(), reason);
        let rendered = error.to_string();
        assert!(
            matches!(
                &error,
                IrGenError::InvalidCommand { reason: actual, .. } if actual == &expected
            ),
            "the reason field must carry {expected:?}: {error:?}"
        );
        assert!(
            rendered.contains(expected),
            "the rendered message must name {expected:?}: {rendered}"
        );
        assert!(
            !rendered.contains(&command) && !rendered.contains(SECRET),
            "the rendered message must not quote the command: {rendered}"
        );
    }

    /// A token that also occurs inside the fixed reason is not a disclosure.
    ///
    /// `"back"` occurs in `"unmatched backtick"`, so a test cannot assert its
    /// absence from the message. It can still assert that the command is
    /// absent and that the fixed reason supplies the only occurrence.
    #[test]
    fn a_token_shared_with_the_reason_is_not_disclosed() {
        let token = "back";
        let command = format!("echo {token} `");
        let error = invalid_command_error(command.clone(), RejectionReason::UnmatchedBacktick);
        let rendered = error.to_string();
        assert!(
            !rendered.contains(&command),
            "the rejection must not quote the command: {rendered}"
        );
        assert!(
            matches!(
                error,
                IrGenError::InvalidCommand { reason, .. } if reason == "unmatched backtick"
            ),
            "the reason field must carry the bounded description: {error:?}"
        );
        assert_eq!(
            rendered.matches(token).count(),
            "unmatched backtick".matches(token).count(),
            "the token must not add an occurrence beyond the fixed reason: {rendered}"
        );
    }

    proptest! {
        /// Never quote a token embedded in a rejected command.
        ///
        /// The named tests are the readable examples; this property covers
        /// arbitrary token text, including characters that a naive formatter
        /// might treat as markup. A fixed sentinel keeps a generated token from
        /// colliding with fixed words in the reason, which would make an
        /// absence assertion fail without any disclosure.
        #[test]
        fn rejection_never_quotes_a_token_from_the_command(token in "[a-z0-9]{4,12}") {
            let secret = format!("netsuke-secret-{token}");
            let command = format!("echo {secret} `");
            let error = invalid_command_error(command.clone(), RejectionReason::UnmatchedBacktick);
            let rendered = error.to_string();
            prop_assert!(!rendered.contains(&secret), "the token leaked: {rendered}");
            prop_assert!(!rendered.contains(&command), "the command leaked: {rendered}");
            prop_assert!(rendered.contains("unmatched backtick"));
        }
    }
}

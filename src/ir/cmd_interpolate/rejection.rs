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
use super::posix_lexical::DelimiterRejection;
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

    use super::{RejectionReason, invalid_command_error};
    use crate::ir::IrGenError;

    /// The message must carry the reason and never the rejected command.
    #[test]
    fn rejection_message_omits_the_command() {
        const SECRET: &str = "s3cr3t-value";
        let command = format!("echo {SECRET} `");
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
}

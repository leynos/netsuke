//! Describe the lexical boundary one PowerShell entry leaves for its successor.
//!
//! A shared PowerShell command list is assembled into one script, so every
//! entry is parsed where the previous entry stopped. Carrying the boundary
//! explicitly lets lowering refuse an entry that would reclassify its
//! successor's marker, and lets the sticky fail-closed classification follow
//! the list once any entry has needed it.

use super::super::super::{QuoteContext, SubstitutionTraversal};
use super::{PowerShellCommentState, PowerShellLexicalState};

/// Name the lexical region an entry leaves open for the next entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ir::cmd_interpolate) enum PowerShellUnfinishedRegion {
    /// An ASCII single-quoted string has no closing quote.
    SingleQuotedString,
    /// An ASCII double-quoted string has no closing quote.
    DoubleQuotedString,
    /// A here-string body has no line-start terminator.
    HereString,
    /// A block comment has no matching terminator.
    BlockComment,
    /// A `$()` expression has no closing parenthesis.
    CommandSubstitution,
    /// The final character is a backtick that would escape the next entry.
    TrailingEscape,
}

impl PowerShellUnfinishedRegion {
    /// Name the region for diagnostics and programmatic classification.
    ///
    /// The value is a stable code rather than prose: the catalogue message is
    /// localized while the code stays language-neutral, so translators never
    /// inherit an English phrase they cannot translate in place.
    pub(in crate::ir::cmd_interpolate) const fn code(self) -> &'static str {
        match self {
            Self::SingleQuotedString => "single_quoted_string",
            Self::DoubleQuotedString => "double_quoted_string",
            Self::HereString => "here_string",
            Self::BlockComment => "block_comment",
            Self::CommandSubstitution => "command_substitution",
            Self::TrailingEscape => "trailing_backtick_escape",
        }
    }
}

/// Summarise the lexical state one shared PowerShell entry leaves behind.
#[derive(Clone, Copy, Debug)]
pub(in crate::ir::cmd_interpolate) struct InterpolationBoundary {
    /// Record the region still open when the entry ended.
    unfinished: Option<PowerShellUnfinishedRegion>,
    /// Record whether this entry or an earlier one needed fail-closed rejection.
    is_fail_closed: bool,
}

impl InterpolationBoundary {
    /// Build a boundary summary for one finished traversal.
    const fn new(unfinished: Option<PowerShellUnfinishedRegion>, is_fail_closed: bool) -> Self {
        Self {
            unfinished,
            is_fail_closed,
        }
    }

    /// Report the region left open at the end of the entry, if any.
    pub(in crate::ir::cmd_interpolate) const fn unfinished_region(
        self,
    ) -> Option<PowerShellUnfinishedRegion> {
        self.unfinished
    }

    /// Report whether the shared script must stay fail-closed from here on.
    pub(in crate::ir::cmd_interpolate) const fn is_fail_closed(self) -> bool {
        self.is_fail_closed
    }
}

impl PowerShellLexicalState {
    /// Initialise lexical tracking that inherits an earlier entry's classification.
    ///
    /// Line-comment text such as an apostrophe makes the partial scanner
    /// fail closed. That classification must survive into later entries even
    /// though each entry starts with fresh quote and comment state, because the
    /// shared interpreter has already seen the text.
    pub(in crate::ir::cmd_interpolate::substitution) const fn with_inherited_fail_closed(
        is_fail_closed: bool,
    ) -> Self {
        Self {
            here_string_quote: None,
            comment_state: PowerShellCommentState::None,
            is_fail_closed,
            has_variable_assignment_since_boundary: false,
            block_comment_depth: 0,
            has_trailing_escape: false,
        }
    }

    /// Report whether the scanner must reject markers encountered from here on.
    pub(in crate::ir::cmd_interpolate::substitution) const fn is_fail_closed(&self) -> bool {
        self.is_fail_closed
    }

    /// Record a final backtick that would escape the character after the entry.
    pub(in crate::ir::cmd_interpolate::substitution) const fn note_trailing_escape(&mut self) {
        self.has_trailing_escape = true;
    }

    /// Report the here-string, block comment, or escape region left open.
    ///
    /// Quote state and `$()` nesting live on the traversal rather than here, so
    /// those regions are completed by [`SubstitutionTraversal`].
    pub(in crate::ir::cmd_interpolate::substitution) fn unfinished_region(
        &self,
    ) -> Option<PowerShellUnfinishedRegion> {
        if self.here_string_quote.is_some() {
            return Some(PowerShellUnfinishedRegion::HereString);
        }
        if self.block_comment_depth > 0 {
            return Some(PowerShellUnfinishedRegion::BlockComment);
        }
        self.has_trailing_escape
            .then_some(PowerShellUnfinishedRegion::TrailingEscape)
    }
}

impl SubstitutionTraversal<'_, '_> {
    /// Report the region the current entry leaves open for its successor.
    pub(super) fn unfinished_power_shell_region(&self) -> Option<PowerShellUnfinishedRegion> {
        if let Some(region) = self.power_shell_lexical.unfinished_region() {
            return Some(region);
        }
        if !self.command_substitutions.is_empty() {
            return Some(PowerShellUnfinishedRegion::CommandSubstitution);
        }
        match self.active_quote_context() {
            QuoteContext::Single(_) => Some(PowerShellUnfinishedRegion::SingleQuotedString),
            QuoteContext::Double(_) => Some(PowerShellUnfinishedRegion::DoubleQuotedString),
            QuoteContext::Unquoted => None,
        }
    }

    /// Finish the traversal and describe the boundary it leaves behind.
    pub(in crate::ir::cmd_interpolate::substitution) fn finish_with_boundary(
        self,
    ) -> (String, InterpolationBoundary) {
        let boundary = InterpolationBoundary::new(
            self.unfinished_power_shell_region(),
            self.power_shell_lexical.is_fail_closed(),
        );
        (self.output, boundary)
    }
}

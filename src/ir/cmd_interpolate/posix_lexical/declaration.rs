//! Parse and queue the delimiter of a POSIX heredoc declaration.
//!
//! A declaration is copied with its delimiter, so quote removal and any
//! rejection are decided once, before marker lowering resumes. Delimiter forms
//! the tracker cannot model are rejected rather than guessed at, because a
//! mis-tracked terminator would let marker lowering resume inside text the
//! shell still holds as heredoc data.

use super::super::CommandBindings;
use super::delimiter::{parse_delimiter, render_delimiter, rendered_terminator};
use super::{
    EscapeBeforeMarker, HeredocDeclaration, HeredocDelimiter, PosixCharacter, PosixLexicalState,
    SourceChar,
};

impl PosixLexicalState {
    /// Preserve one heredoc declaration and queue its body delimiter.
    ///
    /// A here-string (`<<<`) copies its operator without queueing a body.
    /// Delimiter forms the tracker cannot model are rejected rather than
    /// copied, because a mis-tracked terminator would let marker lowering
    /// resume inside the shell's real heredoc body.
    ///
    /// # Errors
    ///
    /// Returns [`EscapeBeforeMarker`] when the delimiter word would lower a
    /// marker directly after an active escape.
    pub(in crate::ir::cmd_interpolate) fn append_heredoc_declaration(
        &mut self,
        character: &mut PosixCharacter<'_, '_>,
        bindings: &CommandBindings,
    ) -> Result<HeredocDeclaration, EscapeBeforeMarker> {
        let Some(operator) = heredoc_operator(character) else {
            return Ok(HeredocDeclaration::NotADeclaration);
        };
        match operator {
            HeredocOperator::HereString => Ok(copy_here_string_operator(character)),
            HeredocOperator::Heredoc(word) => {
                self.copy_heredoc_declaration(character, word, bindings)
            }
        }
    }

    /// Copy one heredoc declaration and queue the delimiter of its body.
    fn copy_heredoc_declaration(
        &mut self,
        character: &mut PosixCharacter<'_, '_>,
        word: HeredocWord,
        bindings: &CommandBindings,
    ) -> Result<HeredocDeclaration, EscapeBeforeMarker> {
        let parsed = match parse_delimiter(character.chars, word.start) {
            Ok(parsed) => parsed,
            Err(reason) => return Ok(HeredocDeclaration::Rejected(reason)),
        };
        let rendered = render_delimiter(character.chars, word.start, parsed.end, bindings)?;
        let terminator = match rendered_terminator(&rendered) {
            Ok(terminator) => terminator,
            Err(reason) => return Ok(HeredocDeclaration::Rejected(reason)),
        };
        // `heredoc_operator` only advances `word.start` over characters it has
        // already read, so the prefix length is always non-negative and in
        // range; skipping and taking keeps that path branch-free.
        character.output.extend(
            character
                .chars
                .iter()
                .skip(character.pos)
                .take(word.start.saturating_sub(character.pos)),
        );
        character.output.push_str(&rendered);
        self.pending_heredocs.push_back(HeredocDelimiter {
            text: terminator.text,
            is_quoted: terminator.is_quoted,
            strips_leading_tabs: word.strips_leading_tabs,
        });
        Ok(HeredocDeclaration::Copied(parsed.end))
    }
}

/// Describe how the `<<` operator at the current position continues.
enum HeredocOperator {
    /// A here-string (`<<<`) takes its input from the following word.
    HereString,
    /// A heredoc whose delimiter word follows the operator.
    Heredoc(HeredocWord),
}

/// Locate the delimiter word that follows a heredoc operator.
#[derive(Clone, Copy)]
struct HeredocWord {
    /// Record the source index where the delimiter word begins.
    start: usize,
    /// Record whether `<<-` strips leading tabs before delimiter matching.
    strips_leading_tabs: bool,
}

/// Classify the heredoc or here-string operator at the current character.
///
/// Returns `None` when the characters do not begin an operator, so the caller
/// can continue ordinary lexical scanning.
fn heredoc_operator(character: &PosixCharacter<'_, '_>) -> Option<HeredocOperator> {
    if character.ch != '<' || character.chars.get(character.pos + 1) != Some(&'<') {
        return None;
    }
    if character.chars.get(character.pos + 2) == Some(&'<') {
        return Some(HeredocOperator::HereString);
    }
    let mut start = character.pos + 2;
    let strips_leading_tabs = character.chars.get(start) == Some(&'-');
    if strips_leading_tabs {
        start += 1;
    }
    while character
        .chars
        .get(start)
        .is_some_and(|ch| SourceChar::new(*ch).is_blank())
    {
        start += 1;
    }
    Some(HeredocOperator::Heredoc(HeredocWord {
        start,
        strips_leading_tabs,
    }))
}

/// Copy a `<<<` here-string operator without queueing a heredoc body.
///
/// A here-string takes its input from the word that follows it on the same
/// command line, so no later line terminates it.
fn copy_here_string_operator(character: &mut PosixCharacter<'_, '_>) -> HeredocDeclaration {
    let operator_end = character.pos + 3;
    character.output.extend(
        character
            .chars
            .get(character.pos..operator_end)
            .into_iter()
            .flatten(),
    );
    HeredocDeclaration::Copied(operator_end)
}

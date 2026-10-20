//! Recognize comment characters that can change PowerShell scanner state.

use super::{PowerShellCommandSubstitutionContext, PowerShellSourceCharacter};

/// Report whether comment text can alter the partial lexical scanner.
pub(in crate::ir::cmd_interpolate::substitution::powershell) fn comment_character_changes_scanner_state(
    character: PowerShellSourceCharacter<'_>,
    context: PowerShellCommandSubstitutionContext,
) -> bool {
    matches!(character.value(), '\'' | '"' | '`')
        || character.is_typographic_quote()
        || (character.value() == '$' && character.next().is_some_and(|next| next.value() == '('))
        || (matches!(context, PowerShellCommandSubstitutionContext::Inside)
            && matches!(character.value(), '(' | ')'))
        // Deliberately over-reject here-string-looking text inside comments.
        || character.here_string_opener().is_some()
        || character.starts_stop_parsing_token()
}

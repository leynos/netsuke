//! Track PowerShell syntax needed for safe recipe placeholder substitution.

mod lexical;
mod stop_parsing;
mod tokens;

pub(super) use lexical::PowerShellLexicalState;
pub(super) use stop_parsing::PowerShellStopParsingState;
pub(super) use tokens::PowerShellSourceCharacter;

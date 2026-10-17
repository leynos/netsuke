//! Track PowerShell syntax needed for safe recipe placeholder substitution.

mod lexical;
mod stop_parsing;
mod tokens;

pub(super) use lexical::{PowerShellLexicalState, PowerShellSourceCharacter};
pub(super) use stop_parsing::PowerShellStopParsingState;

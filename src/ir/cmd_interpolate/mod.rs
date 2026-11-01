//! Command interpolation utilities for IR actions.
//!
//! Substitute the internal tokens emitted for `{{ ins }}` and `{{ outs }}`
//! markers in recipe commands and scripts. Shell variables such as `$in` and
//! `$out` remain unchanged. POSIX-compatible routes track quoting so paths are
//! encoded for their insertion context; PowerShell accepts markers only at
//! unquoted sites. Called by [`super::from_manifest`] during IR lowering.

use crate::localization::{self, keys};
use crate::shell_word;
use camino::Utf8PathBuf;

#[cfg(test)]
use std::cell::Cell;

use super::IrGenError;
use crate::recipe_shell::RecipeShell;

mod command_substitution;
mod posix_lexical;
mod script_substitution;
mod substitution;

use script_substitution::ScriptSubstitutionTraversal;
use substitution::SubstitutionTraversal;

/// Contextual path substitutions prepared for one recipe.
///
/// A rule command list shares its input/output bindings, so lowering creates
/// this once and reuses it for every entry rather than re-quoting paths for
/// each command.
#[derive(Debug, Clone)]
pub(crate) struct CommandBindings {
    /// Selects the interpreter whose command syntax is valid after lowering.
    shell: RecipeShell,
    /// Input paths encoded for unquoted, single-quoted, and double-quoted sites.
    ins: PathSubstitutions,
    /// Output paths encoded for unquoted, single-quoted, and double-quoted sites.
    outs: PathSubstitutions,
}

/// Retain path text for each shell quote context used during marker lowering.
///
/// POSIX and Bash traversals use their contextual encodings. PowerShell's
/// quoted-context fields reuse its standalone single-quoted path literal. That
/// representation is not safe inside an existing PowerShell string, so the
/// PowerShell traversal must reject markers at those sites.
#[derive(Debug, Clone)]
struct PathSubstitutions {
    /// Use when the marker is outside shell quotes.
    unquoted: String,
    /// Use between existing POSIX single quotes; unsafe for PowerShell strings.
    single_quoted: String,
    /// Use between existing POSIX double quotes; unsafe for PowerShell strings.
    double_quoted: String,
}

impl CommandBindings {
    /// Quote the paths once for every command in one recipe.
    #[must_use]
    pub(crate) fn new(inputs: &[Utf8PathBuf], outputs: &[Utf8PathBuf], shell: RecipeShell) -> Self {
        record_binding_preparation();
        Self {
            shell,
            ins: PathSubstitutions::new(inputs, shell),
            outs: PathSubstitutions::new(outputs, shell),
        }
    }

    /// Select a binding encoded for the Netsuke marker's shell quote context.
    fn substitution(&self, placeholder: Placeholder, context: QuoteContext) -> &str {
        let paths = match placeholder {
            Placeholder::Inputs => &self.ins,
            Placeholder::Outputs => &self.outs,
        };
        match context {
            QuoteContext::Unquoted => &paths.unquoted,
            QuoteContext::Single(_) => &paths.single_quoted,
            QuoteContext::Double(_) => &paths.double_quoted,
        }
    }
}

impl PathSubstitutions {
    /// Prepare path encodings used during Netsuke marker lowering.
    fn new(paths: &[Utf8PathBuf], shell: RecipeShell) -> Self {
        let unquoted = quote_paths(paths, shell).join(" ");
        if shell == RecipeShell::PowerShell {
            return Self {
                single_quoted: unquoted.clone(),
                double_quoted: unquoted.clone(),
                unquoted,
            };
        }
        Self {
            unquoted,
            single_quoted: paths
                .iter()
                .map(|path| path.as_str().replace('\'', "'\"'\"'"))
                .collect::<Vec<_>>()
                .join("' '"),
            double_quoted: paths
                .iter()
                .map(|path| quote_double_quoted_path(path.as_str()))
                .collect::<Vec<_>>()
                .join("\" \""),
        }
    }
}

#[cfg(test)]
thread_local! {
    static BINDING_PREPARATIONS: Cell<usize> = const { Cell::new(0) };
}

/// Count one binding preparation in test builds.
#[cfg(test)]
fn record_binding_preparation() {
    BINDING_PREPARATIONS.with(|count| count.set(count.get() + 1));
}

/// Skip binding-preparation counting outside test builds.
#[cfg(not(test))]
const fn record_binding_preparation() {}

#[cfg(test)]
pub(crate) fn reset_binding_preparations() {
    BINDING_PREPARATIONS.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn binding_preparations() -> usize {
    BINDING_PREPARATIONS.with(Cell::get)
}

/// Quote each path for the selected legacy recipe interpreter.
fn quote_paths(paths: &[Utf8PathBuf], shell: RecipeShell) -> Vec<String> {
    paths.iter().map(|path| quote_path(path, shell)).collect()
}

/// Quote one path for the selected legacy recipe interpreter.
fn quote_path(path: &Utf8PathBuf, shell: RecipeShell) -> String {
    shell_word::quote_word(shell.dialect(), path.as_str())
}

/// Escape one path for insertion between existing POSIX double quotes.
fn quote_double_quoted_path(path: &str) -> String {
    path.chars()
        .flat_map(|ch| {
            matches!(ch, '\\' | '"' | '$' | '`')
                .then_some('\\')
                .into_iter()
                .chain([ch])
        })
        .collect()
}

/// Returns `true` when the command contains an odd number of backticks.
///
/// # Examples
/// ```rust,ignore
/// assert!(has_unmatched_backticks("echo`"));
/// assert!(!has_unmatched_backticks("`echo`"));
/// ```
fn has_unmatched_backticks(s: &str) -> bool {
    s.chars().filter(|&c| c == '`').count().rem_euclid(2) != 0
}

/// Interpolate `template` with an explicit legacy recipe shell for structural tests.
#[cfg(test)]
pub(crate) fn interpolate_command_with_shell(
    template: &str,
    inputs: &[Utf8PathBuf],
    outputs: &[Utf8PathBuf],
    shell: RecipeShell,
) -> Result<String, IrGenError> {
    let bindings = CommandBindings::new(inputs, outputs, shell);
    interpolate_command_with_bindings(template, &bindings)
}

/// Interpolate `template` with bindings prepared for its enclosing recipe.
pub(crate) fn interpolate_command_with_bindings(
    template: &str,
    bindings: &CommandBindings,
) -> Result<String, IrGenError> {
    let interpolated = substitute(template, bindings)?;
    if !is_valid_command_for_shell(&interpolated, bindings.shell) {
        return Err(invalid_command_error(interpolated));
    }
    Ok(interpolated)
}

/// Interpolate a script without requiring command-shaped shell syntax.
///
/// POSIX and Bash scripts may contain heredocs, comments, and other valid
/// shell text that `shlex` cannot parse as a command; their traversal preserves
/// the script-specific lexical rules. PowerShell scripts use the same
/// insertion-site rules as PowerShell commands: markers are valid only when
/// unquoted, and are rejected in quoted, escaped, or command-substitution
/// contexts or here-strings. PowerShell comment text isn't treated as an inert
/// region, so quote characters in a comment can make later markers fail closed.
/// This function does not apply the command-shaped `shlex` guard.
pub(crate) fn interpolate_script_with_bindings(
    template: &str,
    bindings: &CommandBindings,
) -> Result<String, IrGenError> {
    substitute_script(template, bindings)
}

/// Builds the diagnostic for a command rejected during placeholder expansion.
fn invalid_command_error(command: String) -> IrGenError {
    let snippet = command.chars().take(160).collect();
    let message = localization::message(keys::IR_INVALID_COMMAND).with_arg("snippet", &snippet);
    IrGenError::InvalidCommand {
        command,
        snippet,
        message,
    }
}

/// Report whether `command` satisfies the selected shell's command syntax boundary.
fn is_valid_command_for_shell(command: &str, shell: RecipeShell) -> bool {
    if shell == RecipeShell::PowerShell {
        return true;
    }
    !has_unmatched_backticks(command) && shlex::split(command).is_some()
}

/// Identifies the private token emitted for a Netsuke recipe marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Placeholder {
    /// Select the input-path binding.
    Inputs,
    /// Select the output-path binding.
    Outputs,
}

/// Records the active shell quote context surrounding a recipe marker.
#[derive(Debug, Clone, Copy)]
pub(super) enum QuoteContext {
    /// The marker is outside shell quotes.
    Unquoted,
    /// The marker is inside single quotes.
    Single(char),
    /// The marker is inside double quotes.
    Double(char),
}

/// Find an internal recipe token at `pos`.
///
/// # Examples
/// ```rust,ignore
/// let chars: Vec<char> = INS_TOKEN.chars().collect();
/// let res = find_substitution(&chars, 0);
/// assert!(matches!(res, Some((Placeholder::Inputs, _))));
/// ```
pub(super) fn find_substitution(chars: &[char], pos: usize) -> Option<(Placeholder, usize)> {
    (chars.get(pos) == Some(&'_')).then_some(()).and_then(|()| {
        try_match_token(chars, pos, INS_TOKEN, Placeholder::Inputs)
            .or_else(|| try_match_token(chars, pos, OUTS_TOKEN, Placeholder::Outputs))
    })
}

/// Find an internal token at `pos` while traversing a script.
pub(super) fn find_script_substitution(chars: &[char], pos: usize) -> Option<(Placeholder, usize)> {
    find_substitution(chars, pos)
}

/// Return the replacement and matched length when `token` starts at `pos`.
fn try_match_token(
    chars: &[char],
    pos: usize,
    token: &str,
    placeholder: Placeholder,
) -> Option<(Placeholder, usize)> {
    let token_len = token.chars().count();
    if pos + token_len > chars.len() {
        return None;
    }

    let mut matched_len = 0;
    for (i, token_ch) in token.chars().enumerate() {
        if chars.get(pos + i) != Some(&token_ch) {
            return None;
        }
        matched_len += 1;
    }

    Some((placeholder, matched_len))
}

/// Replace input and output tokens using the selected shell's backtick semantics.
///
/// A placeholder inside backticks cannot be safely lowered because command
/// substitution shields it from the normal replacement path. Reject it during
/// the same traversal so malformed commands never reach the Ninja backend.
fn substitute(template: &str, bindings: &CommandBindings) -> Result<String, IrGenError> {
    let chars: Vec<char> = template.chars().collect();
    let mut traversal = SubstitutionTraversal::new(template, &chars, bindings);
    let mut pos = 0;
    while pos < chars.len() {
        pos = traversal.append_substitution_at_position(pos)?;
    }
    Ok(traversal.finish())
}

/// Replace script placeholders using the traversal for their selected shell.
///
/// PowerShell reuses command traversal because its path bindings are safe only
/// at unquoted insertion sites. POSIX and Bash retain script-aware handling for
/// comments, heredocs, escapes, and quoted marker insertion.
fn substitute_script(template: &str, bindings: &CommandBindings) -> Result<String, IrGenError> {
    match bindings.shell {
        RecipeShell::PowerShell => substitute(template, bindings),
        RecipeShell::Posix | RecipeShell::Bash => substitute_posix_script(template, bindings),
    }
}

/// Substitute POSIX script markers while preserving comments and heredocs.
fn substitute_posix_script(
    template: &str,
    bindings: &CommandBindings,
) -> Result<String, IrGenError> {
    let chars: Vec<char> = template.chars().collect();
    let mut traversal = ScriptSubstitutionTraversal::new(template, &chars, bindings);
    let mut pos = 0;
    while pos < chars.len() {
        pos = traversal.append_substitution_at_position(pos)?;
    }
    Ok(traversal.finish())
}
/// Internal token emitted for the `{{ ins }}` marker during manifest rendering and
/// consumed during command interpolation; it is not general template syntax.
pub const INS_TOKEN: &str = "__NETSUKE_INS_PLACEHOLDER__";

/// Internal token emitted for the `{{ outs }}` marker during manifest rendering and
/// consumed during command interpolation; it is not general template syntax.
pub const OUTS_TOKEN: &str = "__NETSUKE_OUTS_PLACEHOLDER__";

const _: () = assert!(
    matches!(INS_TOKEN.as_bytes().first(), Some(b'_'))
        && matches!(OUTS_TOKEN.as_bytes().first(), Some(b'_')),
    "the token matcher in find_substitution only runs at underscore positions",
);

#[cfg(test)]
mod property_tests;

#[cfg(kani)]
mod verification;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod power_shell_tests;

#[cfg(test)]
mod powershell_script_tests;

#[cfg(test)]
mod lexical_boundary_tests;

#[cfg(test)]
mod backslash_marker_tests;

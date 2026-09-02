//! Command interpolation utilities for IR actions.
//!
//! Provides [`interpolate_command`], which substitutes the internal markers
//! emitted for `{{ ins }}` and `{{ outs }}` in recipe command strings. Literal
//! `$ins` and `$outs` remain shell variables. POSIX-compatible routes track
//! shell quoting so path text is encoded for its insertion context. Called by
//! [`super::from_manifest`] during IR lowering.

use crate::localization::{self, keys};
use camino::Utf8PathBuf;
use shell_quote::{QuoteRefExt, Sh};

#[cfg(test)]
use std::cell::Cell;

use super::IrGenError;
use crate::recipe_shell::RecipeShell;

mod command_substitution;
mod posix_lexical;
mod substitution;

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

/// Retain path text encoded for each POSIX shell quote context.
#[derive(Debug, Clone)]
struct PathSubstitutions {
    /// Use when the marker is not enclosed by shell quotes.
    unquoted: String,
    /// Use between an existing pair of POSIX single quotes.
    single_quoted: String,
    /// Use between an existing pair of POSIX double quotes.
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

    /// Select a binding encoded for the marker's shell quote context.
    fn substitution(&self, placeholder: Placeholder, context: QuoteContext) -> &str {
        let paths = match placeholder {
            Placeholder::Inputs => &self.ins,
            Placeholder::Outputs => &self.outs,
        };
        match context {
            QuoteContext::Unquoted => &paths.unquoted,
            QuoteContext::Single => &paths.single_quoted,
            QuoteContext::Double => &paths.double_quoted,
        }
    }
}

impl PathSubstitutions {
    /// Encode paths for each POSIX quote context used during marker lowering.
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
    if shell == RecipeShell::PowerShell {
        return format!("'{}'", path.as_str().replace('\'', "''"));
    }
    // Utf8PathBuf guarantees UTF-8, and shell quoting should preserve it.
    let bytes: Vec<u8> = path.as_str().quoted(Sh);
    match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(err) => {
            debug_assert!(false, "shell quoting produced non UTF-8 bytes: {err}");
            String::from_utf8_lossy(err.as_bytes()).into_owned()
        }
    }
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
/// Script recipes may contain heredocs, comments, and other valid shell text
/// that `shlex` cannot parse as a command. POSIX-compatible routes retain the
/// backtick exclusion: placeholders within them would otherwise evade lowering
/// and become silently empty shell variables after the Ninja backend escapes `$`.
pub(crate) fn interpolate_script_with_bindings(
    template: &str,
    bindings: &CommandBindings,
) -> Result<String, IrGenError> {
    substitute(template, bindings)
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

/// Identifies the private marker emitted for a Netsuke recipe placeholder.
#[derive(Debug, Clone, Copy)]
pub(super) enum Placeholder {
    /// Select the input-path binding.
    Inputs,
    /// Select the output-path binding.
    Outputs,
}

/// Records the POSIX quote context surrounding a recipe marker.
#[derive(Debug, Clone, Copy)]
pub(super) enum QuoteContext {
    /// The marker is outside shell quotes.
    Unquoted,
    /// The marker is inside POSIX single quotes.
    Single,
    /// The marker is inside POSIX double quotes.
    Double,
}

/// Finds the appropriate substitution marker at `pos`.
///
/// # Examples
/// ```rust,ignore
/// let chars: Vec<char> = INS_TOKEN.chars().collect();
/// let res = find_substitution(&chars, 0);
/// assert!(matches!(res, Some((Placeholder::Inputs, _))));
/// ```
pub(super) fn find_substitution(chars: &[char], pos: usize) -> Option<(Placeholder, usize)> {
    try_match_token(chars, pos, INS_TOKEN, Placeholder::Inputs)
        .or_else(|| try_match_token(chars, pos, OUTS_TOKEN, Placeholder::Outputs))
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

/// Internal marker emitted for `{{ ins }}` during manifest rendering and
/// consumed during command interpolation; it is not general template syntax.
pub const INS_TOKEN: &str = "__NETSUKE_INS_PLACEHOLDER__";

/// Internal marker emitted for `{{ outs }}` during manifest rendering and
/// consumed during command interpolation; it is not general template syntax.
pub const OUTS_TOKEN: &str = "__NETSUKE_OUTS_PLACEHOLDER__";

#[cfg(test)]
#[path = "posix_lexical_tests.rs"]
mod posix_lexical_tests;
#[cfg(test)]
#[path = "../cmd_interpolate_property_tests.rs"]
mod property_tests;
#[cfg(test)]
#[path = "../cmd_interpolate_tests.rs"]
mod tests;

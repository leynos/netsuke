//! Command interpolation utilities for IR actions.
//!
//! Provides [`interpolate_command`], which substitutes `$in`, `$out`,
//! `__NETSUKE_INS_PLACEHOLDER__`, and `__NETSUKE_OUTS_PLACEHOLDER__` tokens
//! in recipe command strings. POSIX-compatible routes preserve
//! backtick-delimited regions from interpolation; PowerShell routes interpret
//! backticks as their native escape character. Called by [`super::from_manifest`]
//! during IR lowering.

use crate::localization::{self, keys};
use camino::Utf8PathBuf;
use shell_quote::{QuoteRefExt, Sh};

#[cfg(test)]
use std::cell::Cell;

use super::IrGenError;
use crate::recipe_shell::RecipeShell;

mod substitution;

use substitution::SubstitutionTraversal;

/// Quoted `$in` and `$out` substitutions prepared for one recipe.
///
/// A rule command list shares its input/output bindings, so lowering creates
/// this once and reuses it for every entry rather than re-quoting paths for
/// each command.
#[derive(Debug, Clone)]
pub(crate) struct CommandBindings {
    /// Selects the interpreter whose command syntax is valid after lowering.
    shell: RecipeShell,
    /// Quoted and joined input paths for `$in` substitution.
    ins: String,
    /// Quoted and joined output paths for `$out` substitution.
    outs: String,
}

impl CommandBindings {
    /// Quote the paths once for every command in one recipe.
    #[must_use]
    pub(crate) fn new(inputs: &[Utf8PathBuf], outputs: &[Utf8PathBuf], shell: RecipeShell) -> Self {
        record_binding_preparation();
        Self {
            shell,
            ins: quote_paths(inputs, shell).join(" "),
            outs: quote_paths(outputs, shell).join(" "),
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

/// Returns whether `ch` is a valid identifier character (ASCII letter, digit, or underscore).
///
/// # Examples
/// ```rust,ignore
/// assert!(is_identifier_char('a'));
/// assert!(!is_identifier_char('-'));
/// ```
const fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

/// Checks if `pattern` matches `chars` starting at `pos`.
///
/// # Examples
/// ```rust,ignore
/// let chars: Vec<char> = "in-out".chars().collect();
/// assert!(matches_pattern_at_position(&chars, 0, &['i', 'n']));
/// assert!(!matches_pattern_at_position(&chars, 3, &['i', 'n']));
/// ```
fn matches_pattern_at_position(chars: &[char], pos: usize, pattern: &[char]) -> bool {
    pattern
        .iter()
        .enumerate()
        .all(|(off, ch)| matches!(chars.get(pos + off), Some(c) if c == ch))
}

/// Ensures characters around the token are not identifier characters.
///
/// # Examples
/// ```rust,ignore
/// let chars: Vec<char> = "$in".chars().collect();
/// assert!(has_valid_word_boundaries(&chars, 0, 2));
/// let chars: Vec<char> = "$input".chars().collect();
/// assert!(!has_valid_word_boundaries(&chars, 0, 2));
/// ```
fn has_valid_word_boundaries(chars: &[char], pos: usize, len: usize) -> bool {
    let prev_ok = chars
        .get(pos.wrapping_sub(1))
        .is_none_or(|c| !is_identifier_char(*c));
    let next_ok = chars
        .get(pos + len + 1)
        .is_none_or(|c| !is_identifier_char(*c));
    prev_ok && next_ok
}

/// Returns the skip length when `pattern` matches at `pos`.
///
/// # Examples
/// ```rust,ignore
/// let chars: Vec<char> = "$in".chars().collect();
/// let res = try_match_placeholder(&chars, 0, &['i', 'n']);
/// assert_eq!(res, Some(3));
/// ```
fn try_match_placeholder(chars: &[char], pos: usize, pattern: &[char]) -> Option<usize> {
    if matches_pattern_at_position(chars, pos + 1, pattern)
        && has_valid_word_boundaries(chars, pos, pattern.len())
    {
        Some(pattern.len() + 1)
    } else {
        None
    }
}

/// Finds the appropriate substitution for `$in` or `$out` at `pos`.
///
/// # Examples
/// ```rust,ignore
/// let chars: Vec<char> = "$in".chars().collect();
/// let res = find_substitution(&chars, 0, "a", "");
/// assert_eq!(res, Some(("a", 3)));
/// ```
fn find_substitution<'a>(
    chars: &[char],
    pos: usize,
    ins: &'a str,
    outs: &'a str,
) -> Option<(&'a str, usize)> {
    (chars
        .get(pos)
        .is_some_and(|ch| *ch == '$')
        .then_some(())
        .and_then(|()| {
            try_match_placeholder(chars, pos, &['i', 'n'])
                .map(|skip| (ins, skip))
                .or_else(|| {
                    try_match_placeholder(chars, pos, &['o', 'u', 't']).map(|skip| (outs, skip))
                })
        }))
    .or_else(|| {
        try_match_token(chars, pos, INS_TOKEN, ins)
            .or_else(|| try_match_token(chars, pos, OUTS_TOKEN, outs))
    })
}

/// Return the replacement and matched length when `token` starts at `pos`.
fn try_match_token<'a>(
    chars: &[char],
    pos: usize,
    token: &str,
    replacement: &'a str,
) -> Option<(&'a str, usize)> {
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

    Some((replacement, matched_len))
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
pub(crate) const INS_TOKEN: &str = "__NETSUKE_INS_PLACEHOLDER__";

/// Internal marker emitted for `{{ outs }}` during manifest rendering and
/// consumed during command interpolation; it is not general template syntax.
pub(crate) const OUTS_TOKEN: &str = "__NETSUKE_OUTS_PLACEHOLDER__";

#[cfg(test)]
#[path = "../cmd_interpolate_property_tests.rs"]
mod property_tests;
#[cfg(test)]
#[path = "../cmd_interpolate_tests.rs"]
mod tests;

//! Command interpolation utilities for IR actions.

use camino::Utf8PathBuf;
use shell_quote::{QuoteRefExt, Sh};

use super::IrGenError;

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

pub(crate) fn interpolate_command(
    template: &str,
    inputs: &[Utf8PathBuf],
    outputs: &[Utf8PathBuf],
) -> Result<String, IrGenError> {
    fn quote_paths(paths: &[Utf8PathBuf]) -> Vec<String> {
        paths
            .iter()
            .map(|p| {
                // Utf8PathBuf guarantees UTF-8, and shell quoting should preserve it.
                let bytes: Vec<u8> = p.as_str().quoted(Sh);
                match String::from_utf8(bytes) {
                    Ok(text) => text,
                    Err(err) => {
                        debug_assert!(false, "shell quoting produced non UTF-8 bytes: {err}");
                        String::from_utf8_lossy(&err.into_bytes()).into_owned()
                    }
                }
            })
            .collect()
    }

    let ins = quote_paths(inputs);
    let outs = quote_paths(outputs);
    let interpolated = substitute(template, &ins, &outs);
    if has_unmatched_backticks(&interpolated) || shlex::split(&interpolated).is_none() {
        let snippet = interpolated.chars().take(160).collect();
        return Err(IrGenError::InvalidCommand {
            command: interpolated,
            snippet,
        });
    }
    Ok(interpolated)
}

/// Returns whether `ch` is a valid identifier character (ASCII letter, digit, or underscore).
///
/// # Examples
/// ```rust,ignore
/// assert!(is_identifier_char('a'));
/// assert!(!is_identifier_char('-'));
/// ```
fn is_identifier_char(ch: char) -> bool {
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
    try_match_placeholder(chars, pos, &['i', 'n'])
        .map(|skip| (ins, skip))
        .or_else(|| try_match_placeholder(chars, pos, &['o', 'u', 't']).map(|skip| (outs, skip)))
}

fn substitute(template: &str, ins: &[String], outs: &[String]) -> String {
    let chars: Vec<char> = template.chars().collect();
    let ins_joined = ins.join(" ");
    let outs_joined = outs.join(" ");
    let mut out = String::with_capacity(template.len());
    let mut in_backticks = false;
    let mut i = 0;
    while let Some(&ch) = chars.get(i) {
        if ch == '`' {
            in_backticks ^= true;
            out.push(ch);
            i += 1;
            continue;
        }

        if in_backticks {
            out.push(ch);
            i += 1;
            continue;
        }

        if ch == '$'
            && let Some((replacement, skip)) =
                find_substitution(&chars, i, &ins_joined, &outs_joined)
        {
            out.push_str(replacement);
            i += skip;
        } else {
            out.push(ch);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    use camino::Utf8PathBuf;

    #[test]
    fn interpolate_command_rejects_unbalanced_backticks() {
        let path = Utf8PathBuf::from("a");
        let err = interpolate_command(
            "echo `",
            std::slice::from_ref(&path),
            std::slice::from_ref(&path),
        )
        .expect_err("command should be rejected");
        match err {
            IrGenError::InvalidCommand { command, .. } => {
                assert_eq!(command, "echo `");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn interpolate_command_replaces_placeholders() {
        let ins = vec![Utf8PathBuf::from("in"), Utf8PathBuf::from("aux")];
        let outs = vec![Utf8PathBuf::from("out")];
        let command = interpolate_command("cp $in $out", &ins, &outs).expect("command");
        assert_eq!(command, "cp in aux out");
    }

    #[test]
    fn interpolate_command_preserves_backtick_tokens() {
        let ins = vec![Utf8PathBuf::from("src")];
        let outs = vec![Utf8PathBuf::from("out")];
        let command =
            interpolate_command("echo `cat $in` && echo $out", &ins, &outs).expect("command");
        assert_eq!(command, "echo `cat $in` && echo out");
    }
}

//! Independent `CommandLineToArgvW` oracle for rendered command lines.
//!
//! Recipe rendering quotes executables and arguments with Windows rules, and
//! Ninja expands its own escapes over the result. Checking the rendered text
//! with a substring search cannot show what a launched process would receive,
//! so these tests decode the expanded command line with a separate
//! implementation of the documented [`CommandLineToArgvW`][ms-argv] backslash
//! rules and compare argument vectors.
//!
//! The oracle is deliberately written from the published algorithm rather than
//! from the renderer, so a mistake in the renderer cannot hide behind a matching
//! mistake here.
//!
//! [ms-argv]: https://learn.microsoft.com/windows/win32/api/shellapi/nf-shellapi-commandlinetoargvw

use std::iter::Peekable;
use std::str::Chars;

/// Split one Windows command line into the argument vector a program receives.
///
/// Whitespace separates arguments outside double quotes. A run of `2n`
/// backslashes before a quote yields `n` backslashes and toggles quoting, while
/// `2n + 1` backslashes yield `n` backslashes and one literal quote. Backslashes
/// not followed by a quote are literal. A doubled quote inside a quoted argument
/// is one literal quote, matching the documented behaviour for arguments after
/// the program name.
#[must_use]
pub fn split(command_line: &str) -> Vec<String> {
    let mut chars = command_line.chars().peekable();
    let mut arguments = Vec::new();
    while skip_separators(&mut chars) {
        arguments.push(parse_argument(&mut chars));
    }
    arguments
}

/// Skip argument separators and report whether another argument follows.
fn skip_separators(chars: &mut Peekable<Chars<'_>>) -> bool {
    while matches!(chars.peek(), Some(' ' | '\t')) {
        chars.next();
    }
    chars.peek().is_some()
}

/// Parse one argument, stopping at the next unquoted separator.
fn parse_argument(chars: &mut Peekable<Chars<'_>>) -> String {
    let mut argument = String::new();
    let mut quoted = false;
    while let Some(&next) = chars.peek() {
        if !quoted && matches!(next, ' ' | '\t') {
            break;
        }
        push_escaped(&mut argument, chars, &mut quoted);
    }
    argument
}

/// Consume one backslash run and the character that follows it.
fn push_escaped(argument: &mut String, chars: &mut Peekable<Chars<'_>>, quoted: &mut bool) {
    let backslashes = take_backslashes(chars);
    if chars.peek() != Some(&'"') {
        argument.push_str(&"\\".repeat(backslashes));
        if let Some(ch) = chars.next() {
            argument.push(ch);
        }
        return;
    }
    chars.next();
    argument.push_str(&"\\".repeat(backslashes.div_euclid(2)));
    if backslashes.rem_euclid(2) == 1 {
        argument.push('"');
    } else if *quoted && chars.peek() == Some(&'"') {
        argument.push('"');
        chars.next();
    } else {
        *quoted = !*quoted;
    }
}

/// Consume a run of backslashes and return its length.
fn take_backslashes(chars: &mut Peekable<Chars<'_>>) -> usize {
    let mut count = 0;
    while chars.peek() == Some(&'\\') {
        chars.next();
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    //! Checks the oracle against worked examples of the published rules.

    use super::split;

    #[test]
    fn splits_quoted_paths_and_flags() {
        assert_eq!(
            split(
                r#""C:\Program Files\PowerShell\powershell.exe" -NoLogo -EncodedCommand SQBFAFgA"#
            ),
            vec![
                r"C:\Program Files\PowerShell\powershell.exe",
                "-NoLogo",
                "-EncodedCommand",
                "SQBFAFgA",
            ]
        );
    }

    #[test]
    fn keeps_backslash_runs_before_a_quote() {
        assert_eq!(split(r#"exe "a\\b" "c\"d""#), vec!["exe", r"a\\b", "c\"d"]);
    }

    #[test]
    fn preserves_literal_dollars_and_unicode() {
        assert_eq!(
            split(r#""/tmp/$dollar/λambda" -e -c "echo hi""#),
            vec!["/tmp/$dollar/λambda", "-e", "-c", "echo hi"]
        );
    }
}

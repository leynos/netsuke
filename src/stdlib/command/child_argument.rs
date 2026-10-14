//! Platform-aware quoting helpers for shell arguments.

use std::fmt;

#[cfg(not(windows))]
use shell_quote::{QuoteRefExt, Sh};

use crate::localization::{self, keys};

/// Failure modes for shell argument quoting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum QuoteError {
    /// The argument contains a line break, which cannot be quoted safely.
    ContainsLineBreak,
}

impl fmt::Display for QuoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContainsLineBreak => {
                write!(
                    f,
                    "{}",
                    localization::message(keys::COMMAND_QUOTE_LINE_BREAK)
                )
            }
        }
    }
}

/// `QuoteError` crosses into `anyhow::Result` in the Windows quoting tests,
/// which requires the `std::error::Error` trait.
impl std::error::Error for QuoteError {}

/// Quote an argument for the platform shell, rejecting line breaks.
///
/// # Errors
///
/// Returns [`QuoteError::ContainsLineBreak`] when `arg` contains a newline or
/// carriage return.
#[cfg(windows)]
pub(super) fn quote_child_argument(arg: &str) -> Result<String, QuoteError> {
    if arg.chars().any(|ch| matches!(ch, '\n' | '\r')) {
        return Err(QuoteError::ContainsLineBreak);
    }

    if arg.is_empty() {
        return Ok("\"\"".to_owned());
    }

    let needs_quotes = arg.chars().any(|ch| {
        matches!(
            ch,
            ' ' | '\t' | '"' | '^' | '&' | '|' | '<' | '>' | '%' | '!'
        )
    });
    if !needs_quotes {
        return Ok(arg.to_owned());
    }

    let mut buf = String::with_capacity(arg.len() + 2);
    buf.push('"');
    for ch in arg.chars() {
        match ch {
            '"' => {
                buf.push('^');
                buf.push('"');
            }
            '^' | '&' | '|' | '<' | '>' => {
                buf.push('^');
                buf.push(ch);
            }
            '%' => {
                buf.push('%');
                buf.push('%');
            }
            '!' => {
                buf.push('^');
                buf.push('!');
            }
            _ => buf.push(ch),
        }
    }
    buf.push('"');
    Ok(buf)
}

/// Quote an argument for the platform shell, rejecting line breaks.
///
/// This is the *second* sanctioned caller of `shell_quote`'s quoter, and its
/// divergence from [`crate::shell_word::quote_word`] is deliberate: this
/// function encodes one `cmd.exe`/`exec` child argument rather than recipe
/// text, and the two boundaries do not share an admissibility rule. It rejects
/// only `\n` and `\r` — not NUL, which `is_recipe_admissible` also rejects —
/// because a child argument reaches `Command::arg`, which tolerates a NUL-free
/// argument of any shape, while recipe text must survive a Ninja binding. See
/// ADR-014 for the Ninja side of that split.
///
/// # Errors
///
/// Returns [`QuoteError::ContainsLineBreak`] when `arg` contains a newline or
/// carriage return.
#[cfg(not(windows))]
#[expect(
    clippy::disallowed_methods,
    reason = "Sanctioned second call site: quoting one child argument, not \
              recipe text; see the admissibility divergence above."
)]
pub(super) fn quote_child_argument(arg: &str) -> Result<String, QuoteError> {
    if arg.chars().any(|ch| matches!(ch, '\n' | '\r')) {
        return Err(QuoteError::ContainsLineBreak);
    }

    let bytes = arg.quoted(Sh);
    match String::from_utf8(bytes) {
        Ok(text) => Ok(text),
        Err(err) => {
            debug_assert!(false, "quoted args must be valid UTF-8: {err}");
            Ok(String::from_utf8_lossy(err.as_bytes()).into_owned())
        }
    }
}

#[cfg(all(windows, test))]
mod tests {
    //! Unit tests for the Windows `cmd.exe` quoting rules implemented by
    //! `quote_child_argument` in the parent module. Gated on `windows` because
    //! it exercises the `cfg(windows)` branch of `quote_child_argument`, so it
    //! does not run on other platforms; see `non_windows_tests` below for the
    //! Unix counterpart.
    use super::*;
    use anyhow::{Result, ensure};

    #[test]
    fn quote_escapes_cmd_metacharacters() -> Result<()> {
        let success_cases = [
            ("simple", "simple"),
            ("", "\"\""),
            ("needs space", "\"needs space\""),
            ("pipe|test", "\"pipe^|test\""),
            ("redir<test", "\"redir^<test\""),
            ("redir>test", "\"redir^>test\""),
            ("caret^test", "\"caret^^test\""),
            ("tab\ttab", "\"tab\ttab\""),
            ("report&del *.txt", "\"report^&del *.txt\""),
            ("%TEMP%", "\"%%TEMP%%\""),
            ("echo!boom", "\"echo^!boom\""),
            ("say \"hi\"", "\"say ^\"hi^\"\""),
            ("\"", "\"^\"\""),
            ("foo\"bar\"baz", "\"foo^\"bar^\"baz\""),
            ("!DELAYED!", "\"^!DELAYED^!\""),
            ("\"!VAR!\"", "\"^\"^!VAR^!^\"\""),
            (r#"C:\\path\\\"ending"#, r#""C:\\path\\\^"ending""#),
        ];

        for (input, expected) in success_cases {
            let actual = quote_child_argument(input)?;
            ensure!(
                actual == expected,
                "quote_child_argument({input:?}) -> {actual:?}, expected {expected:?}"
            );
        }

        let error_cases = [
            ("line\nbreak", QuoteError::ContainsLineBreak),
            ("carriage\rreturn", QuoteError::ContainsLineBreak),
        ];

        for (input, expected) in error_cases {
            let err = quote_child_argument(input).expect_err(&format!(
                "quote_child_argument({input:?}) succeeded but expected error {expected:?}"
            ));
            ensure!(
                err == expected,
                "quote_child_argument({input:?}) returned error {err:?}, expected {expected:?}"
            );
        }
        Ok(())
    }
}

#[cfg(all(test, not(windows)))]
mod non_windows_tests {
    //! Unit tests for Unix-specific shell quoting behaviour.
    use super::*;

    #[test]
    fn quote_child_argument_rejects_line_breaks_on_unix() {
        let err = quote_child_argument("line\nbreak").expect_err("line feeds should be rejected");
        assert_eq!(err, QuoteError::ContainsLineBreak);
    }

    #[test]
    fn quote_child_argument_wraps_arguments_with_spaces() {
        let quoted = quote_child_argument("needs space").expect("quote should succeed");
        assert_ne!(quoted, "needs space", "quote should escape spaces");
        assert!(
            quoted.contains('\'') || quoted.contains('"'),
            "quote should include quoting characters: {quoted}"
        );
    }
}

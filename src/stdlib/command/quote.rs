//! Platform-aware quoting helpers for shell arguments.

use std::fmt;

#[cfg(not(windows))]
use shell_quote::{QuoteRefExt, Sh};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum QuoteError {
    ContainsLineBreak,
}

impl fmt::Display for QuoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContainsLineBreak => f.write_str(
                "arguments containing carriage returns or line feeds cannot be safely quoted",
            ),
        }
    }
}

#[cfg(windows)]
pub(super) fn quote(arg: &str) -> Result<String, QuoteError> {
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

#[cfg(not(windows))]
pub(super) fn quote(arg: &str) -> Result<String, QuoteError> {
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
            (r#"C:\\path\\\"ending"#, r#""C:\\path\^"ending""#),
        ];

        for (input, expected) in success_cases {
            let actual = quote(input)?;
            ensure!(
                actual == expected,
                "quote({input:?}) -> {actual:?}, expected {expected:?}"
            );
        }

        let error_cases = [
            ("line\nbreak", QuoteError::ContainsLineBreak),
            ("carriage\rreturn", QuoteError::ContainsLineBreak),
        ];

        for (input, expected) in error_cases {
            let err = quote(input).expect_err(&format!(
                "quote({input:?}) succeeded but expected error {expected:?}"
            ));
            ensure!(
                err == expected,
                "quote({input:?}) returned error {err:?}, expected {expected:?}"
            );
        }
        Ok(())
    }
}

#[cfg(all(test, not(windows)))]
mod non_windows_tests {
    use super::*;

    #[test]
    fn quote_rejects_line_breaks_on_unix() {
        let err = quote("line\nbreak").expect_err("line feeds should be rejected");
        assert_eq!(err, QuoteError::ContainsLineBreak);
    }

    #[test]
    fn quote_wraps_arguments_with_spaces() {
        let quoted = quote("needs space").expect("quote should succeed");
        assert_ne!(quoted, "needs space", "quote should escape spaces");
        assert!(
            quoted.contains('\'') || quoted.contains('"'),
            "quote should include quoting characters: {quoted}"
        );
    }
}

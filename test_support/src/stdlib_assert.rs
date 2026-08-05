//! Helpers for assertions around stdlib rendering outputs.
use anyhow::{Result, bail};

/// Extract the stdlib output when present, otherwise surface an informative
/// error. This mirrors the behaviour of the Cucumber step assertions so unit
/// tests can guard the branching logic.
///
/// # Returns
///
/// `Ok(output)` when `output` is `Some`; `output` takes precedence when both
/// `output` and `error` are present.
///
/// # Errors
///
/// Returns an error when `output` is absent. Any `error` text supplied is
/// included in that error.
pub fn stdlib_output_or_error<'a>(output: Option<&'a str>, error: Option<&str>) -> Result<&'a str> {
    match (output, error) {
        (Some(out), _) => Ok(out),
        (None, Some(err)) => bail!("expected stdlib output; stdlib error present: {err}"),
        (None, None) => bail!("expected stdlib output"),
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the standard library assertion helpers.

    use super::*;

    #[test]
    fn returns_output_when_present() {
        let result = stdlib_output_or_error(Some("value"), None);
        assert_eq!(result.ok(), Some("value"));
    }

    #[test]
    fn surfaces_stdlib_error_when_output_missing() {
        let result = stdlib_output_or_error(None, Some("boom"));
        let msg = result
            .err()
            .map_or_else(String::new, |error| error.to_string());
        assert!(
            msg.contains("expected stdlib output; stdlib error present: boom"),
            "message was {msg}"
        );
    }

    #[test]
    fn reports_missing_output_when_both_absent() {
        let result = stdlib_output_or_error(None, None);
        assert_eq!(
            result.err().map(|error| error.to_string()).as_deref(),
            Some("expected stdlib output")
        );
    }
}

//! Helpers for assertions around stdlib rendering outputs.
use anyhow::{Result, bail};

/// Extract the stdlib output when present, otherwise surface an informative
/// error. This mirrors the behaviour of the Cucumber step assertions so unit
/// tests can guard the branching logic.
pub fn stdlib_output_or_error<'a>(output: Option<&'a str>, error: Option<&str>) -> Result<&'a str> {
    if let Some(out) = output {
        return Ok(out);
    }
    if let Some(err) = error {
        bail!("expected stdlib output; stdlib error present: {err}");
    }
    bail!("expected stdlib output");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_output_when_present() {
        let result = stdlib_output_or_error(Some("value"), None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "value");
    }

    #[test]
    fn surfaces_stdlib_error_when_output_missing() {
        let err =
            stdlib_output_or_error(None, Some("boom")).expect_err("should propagate stdlib error");
        let msg = err.to_string();
        assert!(
            msg.contains("expected stdlib output; stdlib error present: boom"),
            "message was {msg}"
        );
    }

    #[test]
    fn reports_missing_output_when_both_absent() {
        let err = stdlib_output_or_error(None, None)
            .expect_err("should fail when neither output nor error present");
        assert_eq!(err.to_string(), "expected stdlib output");
    }
}

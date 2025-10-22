//! Argument redaction helpers for the Ninja runner.
//! Provides the `CommandArg` wrapper used by doctests and logging.

#[derive(Debug, Clone)]
pub struct CommandArg(String);
impl CommandArg {
    #[must_use]
    pub fn new(arg: String) -> Self {
        Self(arg)
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_sensitive_key(key: &str) -> bool {
    const SENSITIVE_KEYS: [&str; 7] = [
        "password",
        "token",
        "secret",
        "api_key",
        "apikey",
        "auth",
        "authorization",
    ];
    SENSITIVE_KEYS
        .iter()
        .any(|candidate| key.eq_ignore_ascii_case(candidate))
}

/// Check if `arg` contains a sensitive keyword.
///
/// # Examples
/// ```
/// # #[cfg(doctest)]
/// # use netsuke::runner::doc::{CommandArg, contains_sensitive_keyword};
/// assert!(contains_sensitive_keyword(&CommandArg::new("token=abc".into())));
/// assert!(!contains_sensitive_keyword(&CommandArg::new("path=/tmp".into())));
/// ```
#[must_use]
pub fn contains_sensitive_keyword(arg: &CommandArg) -> bool {
    arg.as_str()
        .split_once('=')
        .is_some_and(|(key, _)| is_sensitive_key(key.trim()))
}

/// Determine whether the argument should be redacted.
///
/// # Examples
/// ```
/// # #[cfg(doctest)]
/// # use netsuke::runner::doc::{CommandArg, is_sensitive_arg};
/// assert!(is_sensitive_arg(&CommandArg::new("password=123".into())));
/// assert!(!is_sensitive_arg(&CommandArg::new("file=readme".into())));
/// ```
#[must_use]
pub fn is_sensitive_arg(arg: &CommandArg) -> bool {
    contains_sensitive_keyword(arg)
}

/// Redact sensitive information in a single argument.
///
/// Sensitive values are replaced with `***REDACTED***`, preserving keys.
///
/// # Examples
/// ```
/// # #[cfg(doctest)]
/// # use netsuke::runner::doc::{CommandArg, redact_argument};
/// let arg = CommandArg::new("token=abc".into());
/// assert_eq!(redact_argument(&arg).as_str(), "token=***REDACTED***");
/// let arg = CommandArg::new("path=/tmp".into());
/// assert_eq!(redact_argument(&arg).as_str(), "path=/tmp");
/// ```
#[must_use]
pub fn redact_argument(arg: &CommandArg) -> CommandArg {
    if is_sensitive_arg(arg) {
        if let Some((key, _)) = arg.as_str().split_once('=') {
            let trimmed = key.trim();
            return CommandArg::new(format!("{trimmed}=***REDACTED***"));
        }
        return CommandArg::new(String::from("***REDACTED***"));
    }
    arg.clone()
}

/// Redact sensitive information from all `args`.
///
/// # Examples
/// ```
/// # #[cfg(doctest)]
/// # use netsuke::runner::doc::{CommandArg, redact_sensitive_args};
/// let args = vec![
///     CommandArg::new("ninja".into()),
///     CommandArg::new("token=abc".into()),
/// ];
/// let redacted = redact_sensitive_args(&args);
/// assert_eq!(redacted[1].as_str(), "token=***REDACTED***");
/// ```
#[must_use]
pub fn redact_sensitive_args(args: &[CommandArg]) -> Vec<CommandArg> {
    args.iter().map(redact_argument).collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, reason = "tests validate redaction logic")]
    use super::*;

    #[test]
    fn contains_sensitive_keyword_only_flags_known_keys() {
        let token = CommandArg::new(String::from("token=abc"));
        assert!(contains_sensitive_keyword(&token));

        let positional = CommandArg::new(String::from("secrets.yml"));
        assert!(!contains_sensitive_keyword(&positional));

        let path_arg = CommandArg::new(String::from("path=/tmp/secrets.yml"));
        assert!(!contains_sensitive_keyword(&path_arg));

        let spaced = CommandArg::new(String::from("  PASSWORD = value "));
        assert!(contains_sensitive_keyword(&spaced));
    }

    #[test]
    fn redact_argument_preserves_non_sensitive_pairs() {
        let redacted = redact_argument(&CommandArg::new(String::from("auth = token123")));
        assert_eq!(redacted.as_str(), "auth=***REDACTED***");

        let untouched = redact_argument(&CommandArg::new(String::from("path=/var/secrets")));
        assert_eq!(untouched.as_str(), "path=/var/secrets");

        let positional = redact_argument(&CommandArg::new(String::from("secret")));
        assert_eq!(positional.as_str(), "secret");
    }
}

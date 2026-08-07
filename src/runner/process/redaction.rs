//! Argument redaction helpers for the Ninja runner.
//! Provides the `CommandArg` wrapper used by doctests and logging.

/// A single command-line argument passed to a spawned process.
///
/// Wrapping the raw `String` gives the redaction helpers a dedicated type to
/// operate on. The type does not record whether the text is raw or redacted,
/// so it offers no guarantee on its own: logging call sites must consume only
/// the values returned by [`redact_argument`] or [`redact_sensitive_args`].
///
/// The example is `ignore`d because `CommandArg` is crate-private; the
/// `cfg(doctest)` re-export in `runner::process::doc` is compiled out of the
/// library that doctests link against, so no doctest can import it. Behaviour
/// is covered by the unit tests in this module instead.
///
/// # Examples
/// ```ignore
/// use netsuke::runner::process::redaction::CommandArg;
/// let arg = CommandArg::new("token=abc".into());
/// assert_eq!(arg.as_str(), "token=abc");
/// ```
#[derive(Debug, Clone)]
pub struct CommandArg(String);
impl CommandArg {
    /// Wrap a raw argument string.
    #[must_use]
    pub const fn new(arg: String) -> Self {
        Self(arg)
    }
    /// Borrow the underlying argument text.
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

/// Determine whether the argument should be redacted.
///
/// An argument is sensitive when it is a `key=value` pair whose key matches a
/// known sensitive keyword such as `password`, `token`, or `secret`.
///
/// # Examples
/// ```ignore
/// use netsuke::runner::process::redaction::{CommandArg, is_sensitive_arg};
/// assert!(is_sensitive_arg(&CommandArg::new("password=123".into())));
/// assert!(!is_sensitive_arg(&CommandArg::new("file=readme".into())));
/// ```
#[must_use]
pub fn is_sensitive_arg(arg: &CommandArg) -> bool {
    arg.as_str()
        .split_once('=')
        .is_some_and(|(key, _)| is_sensitive_key(key.trim()))
}

/// Redact sensitive information in a single argument.
///
/// Sensitive values are replaced with `***REDACTED***`, preserving keys.
///
/// # Examples
/// ```ignore
/// use netsuke::runner::process::redaction::{CommandArg, redact_argument};
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
/// ```ignore
/// use netsuke::runner::process::redaction::{CommandArg, redact_sensitive_args};
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
    //! Unit tests for sensitive environment variable redaction.

    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("token=abc")]
    #[case("")]
    #[case("  spaced  ")]
    fn command_arg_borrows_the_wrapped_text(#[case] raw: &str) {
        let arg = CommandArg::new(String::from(raw));
        assert_eq!(arg.as_str(), raw);
    }

    #[test]
    fn is_sensitive_arg_only_flags_known_keys() {
        let token = CommandArg::new(String::from("token=abc"));
        assert!(is_sensitive_arg(&token));

        let positional = CommandArg::new(String::from("secrets.yml"));
        assert!(!is_sensitive_arg(&positional));

        let path_arg = CommandArg::new(String::from("path=/tmp/secrets.yml"));
        assert!(!is_sensitive_arg(&path_arg));

        let spaced = CommandArg::new(String::from("  PASSWORD = value "));
        assert!(is_sensitive_arg(&spaced));
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

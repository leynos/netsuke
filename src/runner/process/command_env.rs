//! Environment overrides applied to spawned Ninja commands.
//!
//! Ninja resolves its own executable and any tool it shells out to from the
//! environment it is given. Tests need to control that — chiefly `PATH`, so a
//! fake Ninja can be placed ahead of the real one — without mutating the
//! parent process, which would race every other test in the same binary.
//!
//! [`CommandEnv`] carries those overrides as data. Production supplies
//! [`CommandEnv::inherit`], which changes nothing and leaves the child with the
//! parent's environment exactly as before.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::process::Command;

/// Whether two environment variable names denote the same variable.
///
/// The answer is a property of the target platform, not a stylistic choice.
/// Unix environment names are case-sensitive, so `Path` and `PATH` are two
/// different variables and must not be conflated. Windows resolves names
/// case-insensitively, so treating them as distinct would let a `CommandEnv`
/// hold two entries the child process would collapse into one — with the
/// surviving value chosen by `std`, not by the last `with_var` call.
///
/// Windows case folding is approximated with ASCII folding. Environment names
/// outside ASCII are vanishingly rare and the full `CompareStringOrdinal`
/// mapping is not reachable from safe `std`; the approximation is exact for
/// every name Netsuke composes or documents.
#[cfg(windows)]
pub(super) fn env_names_eq(left: &OsStr, right: &OsStr) -> bool {
    left.eq_ignore_ascii_case(right)
}

/// Whether two environment variable names denote the same variable.
///
/// See the Windows counterpart for why this is target-specific: Unix
/// environment names are case-sensitive, so the comparison is exact.
#[cfg(not(windows))]
pub(super) fn env_names_eq(left: &OsStr, right: &OsStr) -> bool {
    left == right
}

/// Environment values applied to a spawned command.
///
/// An empty set means "inherit the parent environment", which is production
/// behaviour. Each entry is applied with [`Command::env`], so it overrides the
/// inherited value for that variable and leaves every other variable alone.
///
/// # Examples
///
/// ```rust
/// use netsuke::runner::CommandEnv;
///
/// let env = CommandEnv::inherit();
/// assert!(env.is_empty());
///
/// let env = CommandEnv::inherit().with_var("PATH", "/fake/bin");
/// assert!(!env.is_empty());
/// ```
#[derive(Default, Clone, PartialEq, Eq)]
pub struct CommandEnv {
    vars: Vec<(OsString, OsString)>,
}

/// Redacted `Debug` output: a shape, never a payload.
///
/// Overrides are arbitrary environment variables, so both names and values may
/// carry secrets. The same contract the `ninja_subprocess` span honours applies
/// here — a bounded count plus a `PATH` flag is the most that can be disclosed
/// — because a `CommandEnv` reaches logs by any route that formats a struct
/// containing one with `{:?}`, not only through the runner's own logging.
impl fmt::Debug for CommandEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandEnv")
            .field("override_count", &self.vars.len())
            .field("path_overridden", &self.is_path_overridden())
            .finish_non_exhaustive()
    }
}

impl CommandEnv {
    /// Apply no overrides, so the child inherits the parent environment.
    #[must_use]
    pub fn inherit() -> Self {
        Self::default()
    }

    /// Override `key` with `value` for the spawned command.
    ///
    /// A later call for the same key replaces the earlier one, so a composed
    /// environment cannot end up carrying two values for one variable. "The
    /// same key" follows the target's own rule — exact on Unix, ASCII
    /// case-insensitive on Windows — so the set of overrides here always
    /// matches the set the child process will see. Replacement keeps the
    /// casing first recorded, as the platform's own environment block does.
    #[must_use]
    pub fn with_var(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        let name = key.as_ref().to_os_string();
        let setting = value.as_ref().to_os_string();
        if let Some(existing) = self
            .vars
            .iter_mut()
            .find(|(existing, _)| env_names_eq(existing, &name))
        {
            existing.1 = setting;
        } else {
            self.vars.push((name, setting));
        }
        self
    }

    /// Override `PATH` for the spawned command.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use netsuke::runner::CommandEnv;
    ///
    /// let composed = std::env::join_paths(["/opt/bin", "/usr/bin"])
    ///     .expect("separator-free entries always join");
    /// let env = CommandEnv::inherit().with_path(&composed);
    /// assert_eq!(env.get("PATH"), Some(composed.as_os_str()));
    /// // The parent is untouched: only the spawned command sees this.
    /// assert!(!env.is_empty());
    /// ```
    #[must_use]
    pub fn with_path(self, path: impl AsRef<OsStr>) -> Self {
        self.with_var("PATH", path)
    }

    /// Whether any override is set.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }

    /// Look up a configured override, for assertions and composition.
    ///
    /// Reports only what this `CommandEnv` overrides — never the parent's
    /// value — so `None` means "inherited", not "unset". Keys are matched by
    /// the target's own rule, so a lookup answers with the value the child
    /// would actually receive.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use netsuke::runner::CommandEnv;
    /// use std::ffi::OsStr;
    ///
    /// let env = CommandEnv::inherit().with_var("NINJA_STATUS", "[%f/%t] ");
    /// assert_eq!(env.get("NINJA_STATUS"), Some(OsStr::new("[%f/%t] ")));
    /// // Not overridden here, so the child inherits whatever the parent has.
    /// assert_eq!(env.get("PATH"), None);
    /// ```
    #[must_use]
    pub fn get(&self, key: impl AsRef<OsStr>) -> Option<&OsStr> {
        let wanted = key.as_ref();
        self.vars
            .iter()
            .find(|(name, _)| env_names_eq(name, wanted))
            .map(|(_, value)| value.as_os_str())
    }

    /// Whether `PATH` is among the overrides, for the redacted `Debug` shape.
    ///
    /// Kept private: callers wanting this fact for diagnostics read the
    /// `path_overridden` span field, which is derived from the prepared
    /// `Command` and therefore reflects what will actually be spawned.
    fn is_path_overridden(&self) -> bool {
        self.get("PATH").is_some()
    }

    /// Apply the overrides to `cmd`.
    ///
    /// Deliberately additive rather than using `env_clear`: Ninja needs the
    /// ambient environment to function, and clearing it would make a test
    /// environment diverge from production in ways unrelated to what the test
    /// is pinning.
    pub(crate) fn apply(&self, cmd: &mut Command) {
        for (key, value) in &self.vars {
            cmd.env(key, value);
        }
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for override replacement and redacted diagnostics.

    use super::*;

    /// On Unix, `Path` and `PATH` are separate variables and stay separate.
    ///
    /// Folding them together here would silently rewrite one variable when the
    /// caller named the other, and `get` would then report a value the child
    /// never receives.
    #[cfg(unix)]
    #[test]
    fn differently_cased_keys_are_distinct_variables() {
        let env = CommandEnv::inherit()
            .with_var("Path", "/mixed")
            .with_path("/upper");

        assert_eq!(env.get("Path"), Some(OsStr::new("/mixed")));
        assert_eq!(env.get("PATH"), Some(OsStr::new("/upper")));
    }

    /// On Windows, `Path` and `PATH` are one variable, so the later call wins.
    ///
    /// Storing both would leave `get` answering with a value `std` discards
    /// when it builds the child's environment block.
    #[cfg(windows)]
    #[test]
    fn differently_cased_keys_denote_one_variable() {
        let env = CommandEnv::inherit()
            .with_var("Path", "old")
            .with_path("new");

        assert_eq!(env.get("Path"), Some(OsStr::new("new")));
        assert_eq!(env.get("PATH"), Some(OsStr::new("new")));
    }

    /// `Debug` discloses the shape of the overrides and nothing of their
    /// contents, so formatting a struct that holds a `CommandEnv` cannot leak
    /// a secret into a log.
    #[test]
    fn debug_redacts_override_names_and_values() {
        let env = CommandEnv::inherit()
            .with_var("NETSUKE_API_TOKEN", "s3cr3t-value")
            .with_path("/opt/toolchain/bin");

        let rendered = format!("{env:?}");

        assert!(
            !rendered.contains("NETSUKE_API_TOKEN"),
            "an override name leaked into Debug output: {rendered}"
        );
        assert!(
            !rendered.contains("s3cr3t-value"),
            "an override value leaked into Debug output: {rendered}"
        );
        assert!(
            !rendered.contains("/opt/toolchain/bin"),
            "a PATH value leaked into Debug output: {rendered}"
        );
        assert!(
            rendered.contains("override_count: 2") && rendered.contains("path_overridden: true"),
            "Debug output should still summarize the overrides: {rendered}"
        );
    }

    /// An inherited environment reports the empty shape production always has.
    #[test]
    fn debug_reports_the_inherited_shape() {
        let rendered = format!("{:?}", CommandEnv::inherit());

        assert!(
            rendered.contains("override_count: 0") && rendered.contains("path_overridden: false"),
            "unexpected Debug output: {rendered}"
        );
    }
}

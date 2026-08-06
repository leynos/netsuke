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
use std::process::Command;

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
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CommandEnv {
    vars: Vec<(OsString, OsString)>,
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
    /// environment cannot end up carrying two values for one variable.
    #[must_use]
    pub fn with_var(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        let name = key.as_ref().to_os_string();
        let setting = value.as_ref().to_os_string();
        if let Some(existing) = self.vars.iter_mut().find(|(existing, _)| *existing == name) {
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
    /// value — so `None` means "inherited", not "unset".
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
            .find(|(name, _)| name == wanted)
            .map(|(_, value)| value.as_os_str())
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

//! Recipe-shell dialect configuration on [`StdlibConfig`].
//!
//! The recipe-text filters quote paths and arguments for one shell dialect, so
//! the configuration has to carry that choice. It lives here rather than in
//! `config/mod.rs` for the reason `which.rs` gives for its own clustering:
//! `config/mod.rs` holds the shared configuration surface, and a feature's
//! builders and accessors belong together.
//!
//! Only the **dialect** is stored, not the interpreter. `RecipeShell::Posix`
//! and `RecipeShell::Bash` both quote as `sh`, so keeping the wider type would
//! leave a `recipe_shell()` accessor inviting a question the configuration
//! cannot answer honestly.

use super::StdlibConfig;
use crate::recipe_shell::RecipeShell;
use crate::shell_word::ShellDialect;

impl StdlibConfig {
    /// Select the recipe interpreter whose quoting rules the filters follow.
    ///
    /// The interpreter is collapsed to its dialect on the way in, so `Posix`
    /// and `Bash` are stored identically.
    ///
    /// The builder can safely land ahead of its callers: being `pub`, it is not
    /// dead code before the runner threads a resolved shell through it, and it
    /// already accepts the value that call site needs to pass.
    ///
    /// # Examples
    ///
    /// ```
    /// # use cap_std::{ambient_authority, fs_utf8::Dir};
    /// # use netsuke::recipe_shell::RecipeShell;
    /// # use netsuke::stdlib::StdlibConfig;
    /// let dir = Dir::open_ambient_dir(".", ambient_authority())
    ///     .expect("open ambient workspace");
    /// let _config = StdlibConfig::new(dir)
    ///     .expect("construct stdlib config")
    ///     .with_recipe_shell(RecipeShell::Bash);
    /// // Bash and Posix both quote as `sh`; only the transport differs.
    /// ```
    #[must_use]
    pub const fn with_recipe_shell(mut self, shell: RecipeShell) -> Self {
        self.dialect = shell.dialect();
        self
    }

    /// Return the dialect the recipe-text filters quote for.
    ///
    /// Read by `register_read_only_helpers`, which resolves the default once
    /// per environment rather than per call.
    pub(crate) const fn dialect(&self) -> ShellDialect {
        self.dialect
    }
}

#[cfg(test)]
mod tests {
    //! The dialect follows the selected interpreter, and collapses `Posix` and
    //! `Bash` together.
    use super::StdlibConfig;
    use crate::recipe_shell::RecipeShell;
    use crate::shell_word::ShellDialect;
    use anyhow::Result;

    /// Build a configuration at the process cwd for dialect tests.
    ///
    /// Returns the fallible constructor's result rather than unwrapping it, so
    /// the `expect` sits in the `#[test]` bodies where Whitaker's
    /// `no_expect_outside_tests` recognises it. That lint does not treat a
    /// `#[cfg(test)]` helper as test code, and `StdlibConfig::from_current_dir`
    /// is the same constructor `config_tests.rs` exercises for the same reason.
    fn config() -> Result<StdlibConfig> {
        StdlibConfig::from_current_dir()
    }

    /// `with_recipe_shell` stores the dialect, not the interpreter, so `Bash`
    /// and `Posix` are indistinguishable afterwards — which is the point: this
    /// is the three-to-two surjection `RecipeShell::dialect` documents.
    #[test]
    fn dialect_follows_recipe_shell() {
        let base = config().expect("open workspace for dialect test");
        assert_eq!(
            base.clone().with_recipe_shell(RecipeShell::Posix).dialect(),
            ShellDialect::Sh
        );
        assert_eq!(
            base.clone().with_recipe_shell(RecipeShell::Bash).dialect(),
            ShellDialect::Sh
        );
        assert_eq!(
            base.with_recipe_shell(RecipeShell::PowerShell).dialect(),
            ShellDialect::PowerShell
        );
    }

    /// The default is the host interpreter's dialect, so a caller that never
    /// touches the builder still gets correct quoting for the host.
    #[test]
    fn default_dialect_matches_the_host_interpreter() {
        let base = config().expect("open workspace for dialect test");
        assert_eq!(base.dialect(), RecipeShell::host_default().dialect());
    }
}

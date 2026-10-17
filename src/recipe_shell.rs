//! Define the shared legacy-recipe interpreter contract.
//!
//! This data-only module is intentionally below both IR lowering and Ninja
//! rendering. Lowering needs the selected interpreter to quote placeholders,
//! while the Ninja adapter owns the interpreter-specific command transport.
//!
//! It stays data-only: the encoder it maps into lives in the private
//! `shell_word` module, so this module carries no `shell_quote` dependency.
//! That module is deliberately not linked here — it is private, and a public
//! module's docs may not name it with an intra-doc link.

use crate::shell_word::ShellDialect;
use camino::{Utf8Path, Utf8PathBuf};

/// Identify the exact executable bound into a Windows Bash build plan.
///
/// `RecipeShell::Bash` selects POSIX-compatible quoting. This value separately
/// records which executable the runner resolved and preflighted, so generated
/// build commands invoke that same runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BashExecutable(Utf8PathBuf);

impl BashExecutable {
    /// Construct a Bash executable binding from an absolute UTF-8 path.
    ///
    /// # Errors
    ///
    /// Returns an error when `path` is relative or contains a NUL character.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let executable = BashExecutable::from_absolute_path(
    ///     camino::Utf8PathBuf::from("C:/Program Files/Git/bin/bash.exe"),
    /// )?;
    /// assert!(executable.as_path().is_absolute());
    /// ```
    pub(crate) fn from_absolute_path(path: Utf8PathBuf) -> anyhow::Result<Self> {
        anyhow::ensure!(path.is_absolute(), "Bash executable path must be absolute");
        anyhow::ensure!(
            !path.as_str().contains('\0'),
            "Bash executable path cannot contain a NUL character"
        );
        Ok(Self(path))
    }

    /// Borrow the executable path used for probing and generated commands.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let executable = resolve_bash_runtime()?;
    /// assert!(executable.as_path().is_absolute());
    /// ```
    pub(crate) fn as_path(&self) -> &Utf8Path {
        &self.0
    }
}

/// Select the interpreter that receives completed legacy recipe text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecipeShell {
    /// Use the host POSIX shell through Ninja's ordinary Unix execution path.
    Posix,
    /// Use Windows PowerShell with an encoded script argument.
    PowerShell,
    /// Use an explicitly selected Bash compatibility runtime on Windows.
    Bash,
}

impl RecipeShell {
    /// Return the interpreter Netsuke selects when no Windows override exists.
    ///
    /// This is the value to pass to the manifest loaders when no override has
    /// been resolved. They require a [`RecipeShell`] rather than deriving one
    /// so that a caller which *has* resolved `NETSUKE_WINDOWS_SHELL` can make
    /// the template filters agree with the interpreter it will execute under;
    /// this is the answer for a caller that has not.
    ///
    /// It reads no environment, so it cannot fail. On Windows it returns
    /// [`RecipeShell::PowerShell`]; elsewhere, [`RecipeShell::Posix`].
    #[must_use]
    pub const fn host_default() -> Self {
        if cfg!(windows) {
            Self::PowerShell
        } else {
            Self::Posix
        }
    }

    /// Return the dialect whose quoting rules this interpreter follows.
    ///
    /// `Posix` and `Bash` share `Sh`: they differ in transport, not in lexis.
    /// There is deliberately no inverse — this is a three-to-two surjection,
    /// so a `ShellDialect::recipe_shell` would have to pick one of
    /// `Posix`/`Bash` arbitrarily and its doc comment could not be truthful.
    pub(crate) const fn dialect(self) -> ShellDialect {
        match self {
            Self::Posix | Self::Bash => ShellDialect::Sh,
            Self::PowerShell => ShellDialect::PowerShell,
        }
    }
}

#[cfg(test)]
mod tests {
    //! Verifies host-default legacy-recipe interpreter selection.

    /// Select Windows PowerShell when Windows has no explicit compatibility override.
    #[cfg(windows)]
    #[test]
    fn host_default_selects_windows_power_shell() {
        assert_eq!(
            super::RecipeShell::host_default(),
            super::RecipeShell::PowerShell
        );
    }
}

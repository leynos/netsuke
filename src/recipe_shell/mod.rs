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

mod power_shell_location;

use crate::shell_word::ShellDialect;
use camino::{Utf8Path, Utf8PathBuf};
pub(crate) use power_shell_location::{
    KnownFolderSystemDirectory, SystemDirectory, WindowsPowerShellLocationError,
    locate_windows_power_shell,
};
use thiserror::Error;

/// Identify the exact interpreter executable bound into a generated plan.
///
/// A [`RecipeShell`] selects quoting and transport rules; this value separately
/// records which executable receives the rendered recipe. Binding the absolute
/// path keeps a generated plan from resolving an unqualified name through
/// Windows' executable search order, where a workspace file can shadow the
/// intended interpreter (CWE-427).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterpreterExecutable(Utf8PathBuf);

/// Rejections applied while constructing an [`InterpreterExecutable`].
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum InterpreterExecutableError {
    /// The supplied path cannot be executed without a directory search.
    #[error("interpreter executable path must be absolute: {path}")]
    RelativePath {
        /// Path that was supplied to the constructor.
        path: Utf8PathBuf,
    },
    /// A NUL character cannot appear in a process argument.
    #[error("interpreter executable path cannot contain a NUL character")]
    NulCharacter,
}

impl InterpreterExecutable {
    /// Construct an interpreter binding from an absolute UTF-8 path.
    ///
    /// # Errors
    ///
    /// Returns [`InterpreterExecutableError::RelativePath`] when `path` is not
    /// absolute, or [`InterpreterExecutableError::NulCharacter`] when it
    /// contains a NUL character.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::ninja_gen::{InterpreterExecutable, InterpreterExecutableError};
    ///
    /// assert!(matches!(
    ///     InterpreterExecutable::from_absolute_path("bash.exe".into()),
    ///     Err(InterpreterExecutableError::RelativePath { .. })
    /// ));
    /// ```
    pub fn from_absolute_path(path: Utf8PathBuf) -> Result<Self, InterpreterExecutableError> {
        if !path.is_absolute() {
            return Err(InterpreterExecutableError::RelativePath { path });
        }
        if path.as_str().contains('\0') {
            return Err(InterpreterExecutableError::NulCharacter);
        }
        Ok(Self(path))
    }

    /// Borrow the executable path used for probing and generated commands.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::ninja_gen::InterpreterExecutable;
    ///
    /// let path = if cfg!(windows) {
    ///     "C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe"
    /// } else {
    ///     "/bin/sh"
    /// };
    /// let executable = InterpreterExecutable::from_absolute_path(path.into())
    ///     .expect("fixture path should be absolute");
    /// assert!(executable.as_path().is_absolute());
    /// ```
    #[must_use]
    pub fn as_path(&self) -> &Utf8Path {
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

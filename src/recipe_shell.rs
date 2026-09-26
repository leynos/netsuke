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
    pub(crate) const fn host_default() -> Self {
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

//! Define the shared legacy-recipe interpreter contract.
//!
//! This data-only module is intentionally below both IR lowering and Ninja
//! rendering. Lowering needs the selected interpreter to quote placeholders,
//! while the Ninja adapter owns the interpreter-specific command transport.

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

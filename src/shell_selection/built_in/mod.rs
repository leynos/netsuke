//! Describe ADR-019's finite registry and explicit host-dependent defaults.

use super::name::ShellName;
use std::fmt;

/// Classify the target family for structured-command shell selection.
///
/// # Examples
///
/// ```
/// use netsuke::shell_selection::{BuiltInShell, HostFamily};
/// assert_eq!(BuiltInShell::host_default(HostFamily::Windows), BuiltInShell::PowerShell);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HostFamily {
    /// Apply the Unix-like invocation table.
    UnixLike,
    /// Apply the Windows invocation table.
    Windows,
}

impl HostFamily {
    /// Enumerate both host families independently of the compiling host.
    pub const ALL: [Self; 2] = [Self::UnixLike, Self::Windows];

    /// Return the compile-time target family without reading the environment.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::{recipe_shell::RecipeShell, shell_selection::HostFamily};
    /// assert_eq!(HostFamily::current() == HostFamily::Windows,
    ///     RecipeShell::host_default() == RecipeShell::PowerShell);
    /// ```
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::UnixLike
        }
    }
}

/// Describe executable lookup without performing it.
///
/// # Examples
///
/// ```
/// use netsuke::shell_selection::{BuiltInExecutable, BuiltInShell};
/// assert_eq!(BuiltInShell::Sh.executable(), BuiltInExecutable::AbsolutePath("/bin/sh"));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuiltInExecutable {
    /// Use a fixed absolute path without searching the host environment.
    AbsolutePath(
        /// Retain the fixed path.
        &'static str,
    ),
    /// Search a bare executable name through the future trusted resolver.
    BareName(
        /// Retain the executable name.
        &'static str,
    ),
}

/// Enumerate the four structured-command shells in ADR-019 Table 1.
///
/// # Examples
///
/// ```
/// use netsuke::shell_selection::{BuiltInShell, ShellName};
/// assert_eq!(BuiltInShell::lookup(&ShellName::parse("bash")?), Some(BuiltInShell::Bash));
/// # Ok::<(), netsuke::shell_selection::ShellNameError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuiltInShell {
    /// Select the Unix-like POSIX shell.
    Sh,
    /// Select Bash on either host family.
    Bash,
    /// Select PowerShell 7 on either host family.
    Pwsh,
    /// Select Windows PowerShell.
    PowerShell,
}

impl BuiltInShell {
    /// Enumerate each built-in once in ADR-019 table order.
    pub const ALL: [Self; 4] = [Self::Sh, Self::Bash, Self::Pwsh, Self::PowerShell];

    /// Return the reserved registry spelling.
    ///
    /// ```
    /// assert_eq!(netsuke::shell_selection::BuiltInShell::Pwsh.name(), "pwsh");
    /// ```
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Sh => "sh",
            Self::Bash => "bash",
            Self::Pwsh => "pwsh",
            Self::PowerShell => "powershell",
        }
    }

    /// Find a built-in by an already validated name.
    ///
    /// ```
    /// use netsuke::shell_selection::{BuiltInShell, ShellName};
    /// assert_eq!(BuiltInShell::lookup(&ShellName::parse("dash")?), None);
    /// # Ok::<(), netsuke::shell_selection::ShellNameError>(())
    /// ```
    #[must_use]
    pub fn lookup(name: &ShellName) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|shell| shell.name() == name.as_str())
    }

    /// Report support using the explicit host family.
    ///
    /// ```
    /// use netsuke::shell_selection::{BuiltInShell, HostFamily};
    /// assert!(!BuiltInShell::Sh.is_supported_on(HostFamily::Windows));
    /// ```
    #[must_use]
    pub const fn is_supported_on(self, host: HostFamily) -> bool {
        !matches!(
            (self, host),
            (Self::Sh, HostFamily::Windows) | (Self::PowerShell, HostFamily::UnixLike)
        )
    }

    /// Return the fixed executable form without resolving it.
    ///
    /// ```
    /// use netsuke::shell_selection::{BuiltInExecutable, BuiltInShell};
    /// assert_eq!(BuiltInShell::PowerShell.executable(), BuiltInExecutable::BareName("powershell.exe"));
    /// ```
    #[must_use]
    pub const fn executable(self) -> BuiltInExecutable {
        match self {
            Self::Sh => BuiltInExecutable::AbsolutePath("/bin/sh"),
            Self::Bash => BuiltInExecutable::BareName("bash"),
            Self::Pwsh => BuiltInExecutable::BareName("pwsh"),
            Self::PowerShell => BuiltInExecutable::BareName("powershell.exe"),
        }
    }

    /// Return arguments that precede the single rendered source argument.
    ///
    /// ```
    /// assert_eq!(netsuke::shell_selection::BuiltInShell::Bash.fixed_args(), ["--noprofile", "--norc", "-c"]);
    /// ```
    #[must_use]
    pub const fn fixed_args(self) -> &'static [&'static str] {
        match self {
            Self::Sh => &["-c"],
            Self::Bash => &["--noprofile", "--norc", "-c"],
            Self::Pwsh | Self::PowerShell => {
                &["-NoLogo", "-NoProfile", "-NonInteractive", "-Command"]
            }
        }
    }

    /// Map Boolean `true` to ADR-019's host default.
    ///
    /// ```
    /// use netsuke::shell_selection::{BuiltInShell, HostFamily};
    /// assert_eq!(BuiltInShell::host_default(HostFamily::UnixLike), BuiltInShell::Sh);
    /// ```
    #[must_use]
    pub const fn host_default(host: HostFamily) -> Self {
        match host {
            HostFamily::UnixLike => Self::Sh,
            HostFamily::Windows => Self::PowerShell,
        }
    }
}

impl fmt::Display for BuiltInShell {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

impl From<BuiltInShell> for ShellName {
    fn from(shell: BuiltInShell) -> Self {
        // O2 exhaustively validates this sole unchecked construction bridge.
        Self(shell.name().to_owned())
    }
}

#[cfg(test)]
mod tests;
